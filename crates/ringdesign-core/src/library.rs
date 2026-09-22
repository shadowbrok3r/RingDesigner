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
/// Version stamped into saved design files; files without one are version 0.
///
/// Additive fields with compatible defaults do not require a bump. Version 2
/// changes what defines the solid: a CAD assembly can replace the cached band.
/// Older apps must refuse it instead of opening that cache as the whole design.
/// Version 1 documents already have defaults for the new fields, so their
/// migration changes no values. Version 3 also protects imported base geometry
/// from being discarded by an older app. Every version has a migration step.
// Version 4 protects the sand-support surface and high-resolution embedded maps.
pub const FORMAT_VERSION: u32 = 5;

/// Version stamped into saved profile and outline files.
///
/// The design has had a ladder since the beginning; these two had nothing —
/// no version, and a reader that skipped anything it could not parse without
/// a word, so a file from a newer build simply stopped appearing in the
/// picker. Same envelope as the design's: a flattened key that an older build
/// ignores, absent meaning version 0.
pub const ASSET_FORMAT_VERSION: u32 = 1;

/// Serialization wrapper that puts the version key ahead of an asset's fields.
#[derive(serde::Serialize)]
struct VersionedAsset<'a, T: serde::Serialize> {
    format_version: u32,
    #[serde(flatten)]
    asset: &'a T,
}

/// An asset document as versioned JSON text.
fn asset_json<T: serde::Serialize>(asset: &T, pretty: bool) -> anyhow::Result<String> {
    let doc = VersionedAsset { format_version: ASSET_FORMAT_VERSION, asset };
    Ok(if pretty {
        serde_json::to_string_pretty(&doc)?
    } else {
        serde_json::to_string(&doc)?
    })
}

/// Parse an asset document, refusing one from a future build out loud.
///
/// The version key is flattened alongside the asset's own fields, so serde
/// reads the asset itself whether or not the key is there.
fn asset_from_str<T: serde::de::DeserializeOwned>(
    text: &str,
    path: &Path,
) -> Option<T> {
    let doc = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(doc) => doc,
        Err(e) => {
            log::warn!("{}: could not read ({e}) — skipping", path.display());
            return None;
        }
    };
    let version = match doc.get(VERSION_KEY) {
        None => 0,
        Some(value) => match value.as_u64() {
            Some(version) => version,
            None => {
                log::warn!("{}: invalid asset format version — skipping", path.display());
                return None;
            }
        },
    };
    if version > u64::from(ASSET_FORMAT_VERSION) {
        log::warn!(
            "{}: format version {version}, but this build reads up to {ASSET_FORMAT_VERSION} — skipping",
            path.display()
        );
        return None;
    }
    match serde_json::from_value::<T>(doc) {
        Ok(a) => Some(a),
        Err(e) => {
            log::warn!("{}: could not read ({e}) — skipping", path.display());
            None
        }
    }
}

const VERSION_KEY: &str = "format_version";

/// `MIGRATIONS[n]` rewrites a version-`n` document in place to version `n + 1`.
static MIGRATIONS: &[fn(&mut serde_json::Value)] = &[migrate_v0_to_v1, migrate_v1_to_v2, migrate_v2_to_v3, migrate_v3_to_v4, migrate_v4_to_v5];

/// Version 0 predates the version field; the document already has v1's shape.
fn migrate_v0_to_v1(_doc: &mut serde_json::Value) {}

/// CAD and manufacturing fields have serde defaults, so older designs need
/// no data rewrite. The version bump prevents an older app from ignoring a
/// CAD assembly and silently treating its cached band parameters as the ring.
fn migrate_v1_to_v2(_doc: &mut serde_json::Value) {}
// Older readers must not silently discard an imported solid.
fn migrate_v2_to_v3(_doc: &mut serde_json::Value) {}
// Older readers would drop sand support and reduce full-ring relief to 512 px.
fn migrate_v3_to_v4(_doc: &mut serde_json::Value) {}
/// A CAD component stands by a `placement`: the two anchor numbers become `Ring`. The same
/// component lives in every `cad.feature` node of the design's graph, so those are rewritten too.
fn migrate_v4_to_v5(doc: &mut serde_json::Value) {
    fn component(c: &mut serde_json::Value) {
        let Some(obj) = c.as_object_mut() else { return };
        let theta = obj.remove("ring_anchor_deg").and_then(|v| v.as_f64());
        let height = obj.remove("anchor_height_mm").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if obj.contains_key("placement") {
            return;
        }
        obj.insert(
            "placement".into(),
            match theta {
                Some(theta) => serde_json::json!({ "kind": "ring", "theta_deg": theta, "height_mm": height }),
                None => serde_json::json!({ "kind": "free" }),
            },
        );
    }
    if let Some(features) = doc.pointer_mut("/cad/features").and_then(|v| v.as_array_mut()) {
        for f in features {
            if let Some(c) = f.get_mut("component") {
                component(c);
            }
        }
    }
    if let Some(nodes) = doc.pointer_mut("/graph/nodes").and_then(|v| v.as_array_mut()) {
        for n in nodes.iter_mut().filter(|n| n.get("kind").and_then(|k| k.as_str()) == Some("cad.feature")) {
            if let Some(c) = n.pointer_mut("/params/component") {
                component(c);
            }
        }
    }
}

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
    write_atomic(path, design_json(design)?.as_bytes())
}

/// Write a file without a window in which it is half a file.
///
/// A design carrying embedded 16-bit PNGs and hand-drawn strokes is a large
/// single write, and `fs::write` truncates before it writes: interrupt it —
/// full disk, power, a panic in the middle — and the design is gone, not
/// stale. Write a sibling temp, fsync it, keep the previous file as `.bak`,
/// then rename. Rename is atomic on every filesystem this runs on, so the
/// path either names the old file or the new one and never a truncated one.
pub fn write_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let path = path.as_ref();
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("part")
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    // One generation back, so a bad save is recoverable by renaming a file.
    if path.exists() {
        let _ = std::fs::rename(path, path.with_extension(format!(
            "{}.bak",
            path.extension().and_then(|e| e.to_str()).unwrap_or("old")
        )));
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
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
    let version = match doc.get(VERSION_KEY) {
        Some(v) => v.as_u64().ok_or_else(|| anyhow::anyhow!("Invalid design format version"))?,
        None => 0,
    };
    if version > u64::from(FORMAT_VERSION) {
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
    let design: RingDesign = serde_json::from_value(doc)?;
    if let Some(base) = &design.imported_base { base.validate_design(&design)?; }
    Ok(design)
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
        let Ok(text) = std::fs::read_to_string(&path) else {
            log::warn!("{}: could not be read — skipping", path.display());
            continue;
        };
        let Some(o) = asset_from_str::<crate::CustomOutline>(&text, &path) else { continue };
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
    write_atomic(&path, asset_json(outline, false)?.as_bytes())?;
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
    write_atomic(&path, asset_json(profile, true)?.as_bytes())?;
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
        let Ok(text) = std::fs::read_to_string(&path) else {
            log::warn!("{}: could not be read — skipping", path.display());
            continue;
        };
        let Some(profile) = asset_from_str::<crate::BandProfile>(&text, &path) else { continue };
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
    fn a_format_4_anchor_becomes_a_ring_placement_in_the_document_and_in_its_graph() {
        let mut doc = serde_json::to_value(RingDesign::default()).unwrap();
        doc["format_version"] = 4.into();
        doc["cad"] = serde_json::json!({ "features": [ { "id": 1, "name": "Bezel", "enabled": true,
            "operation": { "Cylinder": { "radius_mm": 3.0, "height_mm": 2.0 } },
            "component": { "ring_anchor_deg": 90.0, "anchor_height_mm": 1.5 } } ], "outputs": [1] });
        doc["graph"] = serde_json::json!({ "name": "g", "mode": "Free", "nodes": [
            { "id": 7, "kind": "cad.feature", "params": { "id": 7, "name": "Head", "enabled": true,
              "operation": { "Sphere": { "radius_mm": 2.0 } },
              "component": { "ring_anchor_deg": 45.0, "anchor_height_mm": 0.25 } } },
            { "id": 8, "kind": "design.set", "params": { "component": { "ring_anchor_deg": 1.0 } } }
        ] });
        let d = load_design_str(&doc.to_string()).unwrap();
        let c = &d.cad.as_ref().unwrap().features[0].component;
        assert_eq!(c.placement, crate::cad::Placement::ring(90.0, 1.5));
        let graph = d.graph.unwrap();
        let head = &graph["nodes"][0]["params"]["component"];
        assert_eq!(head["placement"], serde_json::json!({ "kind": "ring", "theta_deg": 45.0, "height_mm": 0.25 }));
        assert!(head.get("ring_anchor_deg").is_none());
        // Only CAD feature nodes are rewritten; other nodes' params are their own business.
        assert_eq!(graph["nodes"][1]["params"]["component"]["ring_anchor_deg"], 1.0);
        // The same fold happens on any read that skips the ladder, such as a graph file.
        let legacy: crate::cad::Component = serde_json::from_str(r#"{"ring_anchor_deg": 30.0, "anchor_height_mm": 0.5}"#).unwrap();
        assert_eq!(legacy.placement, crate::cad::Placement::ring(30.0, 0.5));
        let free: crate::cad::Component = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(free.placement, crate::cad::Placement::Free);
        // Bare ordinals still read as references, and signed ones survive a round trip.
        let op: crate::cad::Operation = serde_json::from_str(r#"{"Fillet": {"source": 1, "edges": [0, 2], "radius_mm": 0.3}}"#).unwrap();
        let crate::cad::Operation::Fillet { edges, .. } = &op else { panic!() };
        assert_eq!(edges.iter().map(|e| e.ordinal).collect::<Vec<_>>(), vec![0, 2]);
        assert!(edges.iter().all(|e| e.signature.is_none()));
        let text = serde_json::to_string(&op).unwrap();
        assert_eq!(serde_json::from_str::<crate::cad::Operation>(&text).unwrap().sources(), vec![1]);
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
        let mut v1: serde_json::Value = serde_json::from_str(&v0).unwrap();
        v1[VERSION_KEY] = 1.into();
        v1["name"] = "Legacy workshop ring".into();
        let loaded = load_design_str(&v1.to_string()).unwrap();
        assert_eq!(loaded.name, "Legacy workshop ring");
        assert!(loaded.cad.is_none() && loaded.manufacturing.is_none());
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
        for version in [u64::from(FORMAT_VERSION) + 1, 1u64 << 32] {
            doc["format_version"] = version.into();
            let err = load_design_str(&doc.to_string()).unwrap_err();
            assert!(err.to_string().contains("newer RingDesigner"), "{err}");
        }
        for version in [serde_json::json!(-1), serde_json::json!(1.5), serde_json::json!("2")] {
            doc["format_version"] = version;
            assert!(load_design_str(&doc.to_string()).is_err());
        }
    }
}

#[cfg(test)]
mod asset_version_tests {
    use super::*;

    /// The design has had a version ladder since the beginning. Profiles and
    /// outlines had none, and a reader that skipped whatever it could not
    /// parse without a word — so a file from a newer build did not fail, it
    /// just stopped appearing in the picker.
    #[test]
    fn a_saved_profile_carries_its_version_and_still_reads_one_without() {
        let dir = std::env::temp_dir().join("ringdesign-asset-version");
        let _ = std::fs::remove_dir_all(&dir);

        let mut p = crate::BandProfile::default();
        p.width_mm = 5.5;
        let path = save_profile_in(&dir, "probe", &p).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(doc[VERSION_KEY], u64::from(ASSET_FORMAT_VERSION));

        let back = list_profiles_in(&dir);
        assert_eq!(back.len(), 1);
        assert!((back[0].1.width_mm - 5.5).abs() < 1e-9);

        // A file written before the key existed is version 0 and still loads.
        let mut bare = doc.clone();
        bare.as_object_mut().unwrap().remove(VERSION_KEY);
        std::fs::write(dir.join("bare.profile.json"), serde_json::to_string(&bare).unwrap())
            .unwrap();
        assert_eq!(list_profiles_in(&dir).len(), 2, "an unversioned file still reads");

        // One from a future build is refused rather than silently dropped as
        // unparseable — same rule the design ladder applies.
        let mut future = doc.clone();
        for version in [
            serde_json::json!(ASSET_FORMAT_VERSION + 1),
            serde_json::json!(1u64 << 32),
            serde_json::json!(-1),
            serde_json::json!(1.5),
            serde_json::json!("1"),
        ] {
            future[VERSION_KEY] = version;
            std::fs::write(dir.join("future.profile.json"), serde_json::to_string(&future).unwrap())
                .unwrap();
            assert_eq!(list_profiles_in(&dir).len(), 2, "a future or invalid version is skipped");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The same envelope on the other library, which is the one that carries a
    /// 720-entry polar table a user drew.
    #[test]
    fn a_saved_outline_carries_its_version() {
        let dir = std::env::temp_dir().join("ringdesign-outline-version");
        let _ = std::fs::remove_dir_all(&dir);

        let pts: Vec<[f64; 2]> = (0..64)
            .map(|i| {
                let t = i as f64 / 64.0 * std::f64::consts::TAU;
                [t.cos() * 1.2, t.sin()]
            })
            .collect();
        let o = crate::CustomOutline::from_points("probe", &pts).expect("a closed plan");
        let path = save_outline_in(&dir, &o).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc[VERSION_KEY], u64::from(ASSET_FORMAT_VERSION));
        assert_eq!(list_outlines_in(&dir).len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
