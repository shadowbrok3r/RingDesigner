//! Persisting designs and locating the alpha library on disk.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::RingDesign;

/// Overrides where the user's own files live, for platforms with no `$HOME`.
static DATA_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Point the library and design directories at `root`, once, before anything reads them.
///
/// Android has neither `XDG_DATA_HOME` nor `HOME`, so [`user_alpha_dir`] would otherwise fall back
/// to `"."` — which is `/` for an app process, and unwritable. The host hands us
/// `getFilesDir()` instead.
pub fn set_data_root(root: impl Into<PathBuf>) {
    let _ = DATA_ROOT.set(root.into());
}

/// The configured root, or the XDG-derived one.
pub fn data_root() -> PathBuf {
    if let Some(root) = DATA_ROOT.get() {
        return root.clone();
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ringdesigner")
}

/// File extension for saved designs.
pub const DESIGN_EXT: &str = "ring.json";
/// `RingDesign::graph` joined the file without a version bump: an absent
/// field reads as `None`, and an older build ignores the key.

/// Version stamped into saved design files; files without one are version 0.
pub const FORMAT_VERSION: u32 = 1;

const VERSION_KEY: &str = "format_version";

/// `MIGRATIONS[n]` rewrites a version-`n` document in place to version `n + 1`.
static MIGRATIONS: &[fn(&mut serde_json::Value)] = &[migrate_v0_to_v1];

/// Version 0 predates the version field; the document already has v1's shape.
fn migrate_v0_to_v1(_doc: &mut serde_json::Value) {}

/// Serialization wrapper that puts the version key ahead of the design fields.
#[derive(serde::Serialize)]
struct VersionedDesign<'a> {
    format_version: u32,
    #[serde(flatten)]
    design: &'a RingDesign,
}

/// The design document as versioned JSON text.
pub fn design_json(design: &RingDesign) -> anyhow::Result<String> {
    let doc = VersionedDesign { format_version: FORMAT_VERSION, design };
    Ok(serde_json::to_string_pretty(&doc)?)
}

pub fn save_design(path: impl AsRef<Path>, design: &RingDesign) -> anyhow::Result<()> {
    std::fs::write(path, design_json(design)?)?;
    Ok(())
}

/// Save with every referenced, non-regenerable alpha embedded from `lib`.
pub fn save_design_embedded(
    path: impl AsRef<Path>,
    design: &RingDesign,
    lib: &crate::AlphaLibrary,
) -> anyhow::Result<()> {
    let mut design = design.clone();
    design.embed_alphas(lib);
    save_design(path, &design)
}

pub fn load_design(path: impl AsRef<Path>) -> anyhow::Result<RingDesign> {
    let text = std::fs::read_to_string(path)?;
    load_design_str(&text)
}

/// Parse a design document, migrating older versions up to [`FORMAT_VERSION`].
pub fn load_design_str(text: &str) -> anyhow::Result<RingDesign> {
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    let version = doc
        .get(VERSION_KEY)
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    if version > FORMAT_VERSION {
        anyhow::bail!(
            "design file is format version {version}, but this build reads up to {FORMAT_VERSION} \
             — it was saved by a newer RingDesigner"
        );
    }
    for step in &MIGRATIONS[version as usize..] {
        step(&mut doc);
    }
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(VERSION_KEY);
    }
    Ok(serde_json::from_value(doc)?)
}

/// Alphas bundled with the source tree: `<workspace>/assets/alphas`.
pub fn bundled_alpha_dir() -> Option<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("assets").join("alphas"))
        .filter(|p| p.is_dir())
}

/// The user's own alpha library in the platform data directory. This is where
/// imports land and where a converted collection belongs.
pub fn user_alpha_dir() -> PathBuf {
    data_root().join("alphas")
}

/// Every directory scanned at startup, bundled first so a user file of the same
/// name wins.
pub fn alpha_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.extend(bundled_alpha_dir());
    let user = user_alpha_dir();
    if !out.contains(&user) {
        out.push(user);
    }
    out
}

/// Where an import writes by default.
pub fn default_alpha_dir() -> PathBuf {
    user_alpha_dir()
}

/// Designs directory, created on demand.
pub fn default_design_dir() -> PathBuf {
    data_root().join("designs")
}

/// Where the user's true gem meshes live — one `<cut>.obj` per faceted
/// cut, consumed by the render-only previews in [`crate::gems`]. The app
/// ships none of its own: absent files fall back to procedural stones.
pub fn gem_mesh_dir() -> PathBuf {
    data_root().join("gems")
}

/// Where saved cross-section profiles live — the user's own profile
/// library, sibling to the designs. One `<name>.profile.json` per shape,
/// applied by [`crate::BandProfile::apply_shape`] so a profile is a
/// section, never a size.
pub fn profile_dir() -> PathBuf {
    data_root().join("profiles")
}

/// Imported signet plans live beside the profiles, one
/// `<name>.outline.json` each — a serialized [`crate::CustomOutline`].
/// Library entries are import stock: applying one copies it into the
/// design, so the file stays self-contained.
pub fn outline_dir() -> PathBuf {
    data_root().join("outlines")
}

/// Every saved signet plan, sorted by name.
pub fn list_outlines() -> Vec<crate::CustomOutline> {
    list_outlines_in(&outline_dir())
}

/// [`list_outlines`] from an explicit directory.
pub fn list_outlines_in(dir: &Path) -> Vec<crate::CustomOutline> {
    let mut out: Vec<crate::CustomOutline> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        if !path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".outline.json"))
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(o) = serde_json::from_str::<crate::CustomOutline>(&text) else { continue };
        if o.r.len() == 720 && o.r.iter().all(|v| v.is_finite() && *v > 0.0) {
            out.push(o);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Save a signet plan into the library, named by its own `name`.
pub fn save_outline_in(dir: &Path, outline: &crate::CustomOutline) -> anyhow::Result<PathBuf> {
    let stem: String = outline
        .name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect();
    if stem.is_empty() {
        anyhow::bail!("an outline needs a name");
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{stem}.outline.json"));
    std::fs::write(&path, serde_json::to_string(outline)?)?;
    Ok(path)
}

/// Save the profile's shape under a name. The name becomes the file stem;
/// anything path-hostile is flattened to `_`.
pub fn save_profile(name: &str, profile: &crate::BandProfile) -> anyhow::Result<PathBuf> {
    save_profile_in(&profile_dir(), name, profile)
}

/// [`save_profile`] into an explicit directory.
pub fn save_profile_in(
    dir: &Path,
    name: &str,
    profile: &crate::BandProfile,
) -> anyhow::Result<PathBuf> {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect();
    if stem.is_empty() {
        anyhow::bail!("a profile needs a name");
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{stem}.profile.json"));
    let text = serde_json::to_string_pretty(profile)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Every saved profile, by name, sorted. Unreadable files are skipped —
/// one bad import must not hide the rest of the library.
pub fn list_profiles() -> Vec<(String, crate::BandProfile)> {
    list_profiles_in(&profile_dir())
}

/// [`list_profiles`] from an explicit directory.
pub fn list_profiles_in(dir: &Path) -> Vec<(String, crate::BandProfile)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let path = e.path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".profile.json"))
        else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(profile) = serde_json::from_str::<crate::BandProfile>(&text) else { continue };
        out.push((name.to_string(), profile));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_profile_is_a_shape_never_a_size() {
        let dir = std::env::temp_dir().join("ringdesign-profile-lib-test");
        let _ = std::fs::remove_dir_all(&dir);

        let mut knife = crate::BandProfile::default();
        knife.apply_style(crate::ProfileStyle::KnifeEdge);
        knife.width_mm = 3.0;
        knife.thickness_mm = 1.5;
        save_profile_in(&dir, "My knife", &knife).unwrap();
        let mut dome = crate::BandProfile::default();
        dome.apply_style(crate::ProfileStyle::HalfRound);
        save_profile_in(&dir, "Big dome", &dome).unwrap();
        assert!(save_profile_in(&dir, "   ", &dome).is_err(), "a blank name refuses");

        let listed = list_profiles_in(&dir);
        assert_eq!(
            listed.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["Big dome", "My knife"],
            "sorted by name"
        );

        // Applying keeps the band's own size: the profile is a section.
        let mut band = crate::BandProfile::default();
        band.width_mm = 6.0;
        band.thickness_mm = 2.6;
        let saved = &listed.iter().find(|(n, _)| n == "My knife").unwrap().1;
        band.apply_shape(saved);
        assert_eq!(band.style, crate::ProfileStyle::KnifeEdge);
        assert_eq!(band.width_mm, 6.0);
        assert_eq!(band.thickness_mm, 2.6);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("ringdesign_library_test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn every_version_has_a_migration_step() {
        assert_eq!(MIGRATIONS.len(), FORMAT_VERSION as usize);
    }

    #[test]
    fn save_stamps_the_current_version_first() {
        let path = temp_file("stamped.ring.json");
        save_design(&path, &RingDesign::default()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.trim_start().starts_with("{\n  \"format_version\""),
            "version key should lead the file: {}",
            &text[..60.min(text.len())]
        );
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(doc["format_version"], u64::from(FORMAT_VERSION));
    }

    #[test]
    fn a_round_trip_preserves_the_design() {
        let mut design = RingDesign::default();
        design.name = "Round trip".into();
        design.size = crate::RingSize(9.25);
        let path = temp_file("roundtrip.ring.json");
        save_design(&path, &design).unwrap();
        let loaded = load_design(&path).unwrap();
        assert_eq!(
            serde_json::to_value(&design).unwrap(),
            serde_json::to_value(&loaded).unwrap()
        );
    }

    #[test]
    fn a_v0_file_without_a_version_still_loads() {
        // Files saved before the version field are exactly the bare struct.
        let v0 = serde_json::to_string_pretty(&RingDesign::default()).unwrap();
        assert!(!v0.contains(VERSION_KEY));
        let loaded = load_design_str(&v0).unwrap();
        assert_eq!(loaded.name, RingDesign::default().name);
    }

    #[test]
    fn an_imported_alpha_travels_inside_the_design_file() {
        use crate::field::{Layer, LayerEntry};
        use crate::tiling::TilingLayer;

        let mut design = RingDesign::default();
        let ctx = design.field_context();
        design.layers.layers.push(LayerEntry::new(
            "custom tile",
            Layer::Tiling(TilingLayer::default_for("my import", &ctx)),
        ));

        let mut lib = crate::AlphaLibrary::builtin();
        let data: Vec<f32> = (0..16 * 16).map(|i| (i % 16) as f32 / 15.0).collect();
        lib.insert(crate::Alpha::new("my import", 16, 16, data.clone()));

        let path = temp_file("embedded.ring.json");
        save_design_embedded(&path, &design, &lib).unwrap();

        // A fresh machine has only the builtins.
        let loaded = load_design(&path).unwrap();
        assert_eq!(loaded.embedded.len(), 1, "one non-builtin alpha referenced");
        let mut fresh = crate::AlphaLibrary::builtin();
        loaded.unpack_embedded(&mut fresh);
        let a = fresh.get("my import").expect("embedded alpha unpacked");
        assert_eq!((a.width, a.height), (16, 16));
        let worst = a
            .data
            .iter()
            .zip(&data)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1.0 / 65535.0 + 1e-6, "16-bit round trip, off by {worst}");
    }

    #[test]
    fn builtins_are_not_embedded() {
        use crate::field::{Layer, LayerEntry};
        use crate::tiling::TilingLayer;

        let mut design = RingDesign::default();
        let ctx = design.field_context();
        design.layers.layers.push(LayerEntry::new(
            "rope",
            Layer::Tiling(TilingLayer::default_for(
                crate::alpha::Procedural::Rope.label(),
                &ctx,
            )),
        ));
        design.embed_alphas(&crate::AlphaLibrary::builtin());
        assert!(design.embedded.is_empty());
    }

    #[test]
    fn a_newer_version_is_refused_with_a_clear_error() {
        let mut doc = serde_json::to_value(RingDesign::default()).unwrap();
        doc["format_version"] = (FORMAT_VERSION + 1).into();
        let err = load_design_str(&doc.to_string()).unwrap_err();
        assert!(err.to_string().contains("newer RingDesigner"), "{err}");
    }
}
