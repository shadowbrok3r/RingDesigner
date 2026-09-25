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
/// The newest version this build reads and stamps into saved design files; files without one are version 0.
/// A file is stamped with the oldest version that carries it, [`format_version_for`].
///
/// Additive fields with compatible defaults do not require a bump. Version 2
/// changes what defines the solid: a CAD assembly can replace the cached band.
/// Older apps must refuse it instead of opening that cache as the whole design.
/// Version 1 documents already have defaults for the new fields, so their
/// migration changes no values. Version 3 also protects imported base geometry
/// from being discarded by an older app. Every version has a migration step.
// Version 4 protects the sand-support surface and high-resolution embedded maps.
// Version 6 protects a stored mesh, which no earlier build can parse, and keeps each one once in the file's table;
// it also protects a revolution whose line is read in its sketch's plane, which an earlier build would turn about the world's line,
// a pattern of several parts, which an earlier build cannot parse, a cut carved from a ring of parts alone, which an earlier
// build pours as metal, and a stamp with a tier, a shaped top or an outline over 512 points, which an earlier build flattens
// or refuses.
// A design with none of these is still written at 5.
pub const FORMAT_VERSION: u32 = 6;

/// The version a design carrying none of the format-6 features is written at, so builds that read up to it still open the file.
pub const PLAIN_FORMAT_VERSION: u32 = 5;

/// The version `design` is written at: the newest when it carries a stored mesh, an in-plane revolution, a pattern of several parts, a cut on a ring of parts alone or a stamp a format-5 build cannot strike.
pub fn format_version_for(design: &RingDesign) -> u32 {
    if crate::cad::stored::carried_by(design)
        || crate::cad::turns_in_plane(design)
        || crate::cad::pattern::several_sources(design)
        || crate::parts::cuts_apart(design)
        || design.stamps.iter().any(|s| !s.is_plain())
        || design.imported_base.as_ref().is_some_and(|base| crate::imported_base::PresetSource::of(&base.source).is_some())
        || design.graph.as_ref().is_some_and(template_features_in_json)
    {
        FORMAT_VERSION
    } else {
        PLAIN_FORMAT_VERSION
    }
}

/// Source references and template controls whose geometry earlier readers cannot reproduce.
pub fn template_features_in_json(value: &serde_json::Value) -> bool {
    const PLACEMENT: &[&str] = &["placement", "blend_mm", "theta_deg", "height_mm", "across_mm", "spin_deg", "cant_deg", "tilt_deg"];
    let new_pin = |kind: &str, pin: &str| (kind == "cad.feature" && PLACEMENT.contains(&pin)) || (kind == "shank" && pin == "keys");
    if value.get("source").is_some_and(|source| source.get("preset").is_some()) { return true; }
    if let Some(kind) = value.get("kind").and_then(serde_json::Value::as_str) {
        if matches!(kind, "base.preset" | "shank.key" | "stamp" | "stamp.top" | "stamp.row" | "design.stamps")
            || kind.starts_with("stamp.outline.") || kind.starts_with("cad.op.") { return true; }
        if value.get("inputs").and_then(serde_json::Value::as_object).is_some_and(|inputs| inputs.iter().any(|(pin, v)| new_pin(kind, pin) && !v.is_null())) { return true; }
    }
    if let Some(nodes) = value.get("nodes").and_then(serde_json::Value::as_array) {
        for (list, target) in [("wires", "to"), ("exposed", "node")] {
            if value.get(list).and_then(serde_json::Value::as_array).is_some_and(|items| items.iter().any(|item| {
                nodes.iter().any(|node| item.get(target).is_some_and(|id| Some(id) == node.get("id"))
                    && node.get("kind").and_then(serde_json::Value::as_str).is_some_and(|kind|
                        item.get("input").and_then(serde_json::Value::as_str).is_some_and(|pin| new_pin(kind, pin))))
            })) { return true; }
        }
    }
    match value {
        serde_json::Value::Object(object) => object.values().any(template_features_in_json),
        serde_json::Value::Array(items) => items.iter().any(template_features_in_json),
        _ => false,
    }
}

#[cfg(test)]
mod template_source_tests {
    use super::*;
    use crate::imported_base::{ImportedBase, PRESETS, PresetSource, Source};
    use std::sync::Arc;

    #[test]
    fn bundled_stock_references_reopen_exactly_and_inline_or_modified_stock_keeps_its_source() {
        for preset in PRESETS {
            for sand_master in [false, true].into_iter().filter(|sand| !sand || preset.sand_safe()) {
                let reference = PresetSource { preset: preset.id.into(), sand_master };
                let source = reference.load().unwrap();
                let mut d = RingDesign::default();
                ImportedBase::attach(&mut d, source.clone()).unwrap();
                let text = design_json(&d).unwrap();
                let saved: serde_json::Value = serde_json::from_str(&text).unwrap();
                assert_eq!(saved["imported_base"]["source"], serde_json::to_value(&reference).unwrap());
                assert_eq!(saved["format_version"], FORMAT_VERSION);
                let loaded = load_design_str(&text).unwrap();
                assert_eq!(serde_json::to_vec(&*loaded.imported_base.unwrap().source).unwrap(), serde_json::to_vec(&*source).unwrap());
                assert!(read_design(&text, PLAIN_FORMAT_VERSION).unwrap_err().to_string().contains("format version 6"));
                let mut inline = saved;
                inline["format_version"] = serde_json::json!(PLAIN_FORMAT_VERSION);
                inline["imported_base"]["source"] = serde_json::to_value(&*source).unwrap();
                assert_eq!(serde_json::to_vec(&*load_design_str(&inline.to_string()).unwrap().imported_base.unwrap().source).unwrap(), serde_json::to_vec(&*source).unwrap());
                assert!(text.len() < 25_000, "{}: {} bytes", preset.id, text.len());
            }
        }
        let source = PRESETS[0].load().unwrap();
        for field in ["name", "version", "source_sha256", "vertices", "faces", "calibration"] {
            let mut value = serde_json::to_value(&*source).unwrap();
            match field {
                "name" => value["name"] = "Custom Signet 001".into(),
                "version" => value["version"] = 2.into(),
                "source_sha256" => value["source_sha256"] = "Different provenance".into(),
                "vertices" => value["vertices"][0][0] = (value["vertices"][0][0].as_f64().unwrap() + 0.0001).into(),
                "faces" => value["faces"][0].as_array_mut().unwrap().swap(1, 2),
                _ => value["calibration"]["shoulder_start_mm"] = (value["calibration"]["shoulder_start_mm"].as_f64().unwrap() + 0.0001).into(),
            }
            let changed: Source = serde_json::from_value(value.clone()).unwrap();
            assert!(PresetSource::of(&changed).is_none(), "{field}");
            let base = ImportedBase { source: Arc::new(changed), chart: None, bare: false, sand_envelope: false };
            let saved = serde_json::to_value(&base).unwrap();
            assert_eq!(saved["source"], value, "{field}");
            let reloaded: ImportedBase = serde_json::from_value(saved).unwrap();
            assert_eq!(serde_json::to_value(&*reloaded.source).unwrap(), value, "{field}");
        }
        for source in [serde_json::json!({"preset":"999"}), serde_json::json!({"preset":"001","vertices":[]})] {
            assert!(serde_json::from_value::<ImportedBase>(serde_json::json!({"source":source})).is_err());
        }
    }

    #[test]
    fn new_template_controls_are_fenced_even_when_only_wired_exposed_or_nested() {
        for (kind, pin) in [("shank", "keys"), ("cad.feature", "placement"), ("cad.feature", "theta_deg"), ("cad.feature", "blend_mm")] {
            let node = serde_json::json!({"id":7,"kind":kind,"inputs":{}});
            let plain = serde_json::json!({"nodes":[node],"wires":[],"exposed":[]});
            assert!(!template_features_in_json(&plain));
            for form in ["literal", "wire", "exposure"] {
                let mut graph = plain.clone();
                match form {
                    "literal" => graph["nodes"][0]["inputs"][pin] = serde_json::json!([]),
                    "wire" => graph["wires"] = serde_json::json!([{"from":8,"out":"value","to":7,"input":pin}]),
                    _ => graph["exposed"] = serde_json::json!([{"name":"Edit", "node":7, "input":pin}]),
                }
                for value in [graph.clone(), serde_json::json!({"nested":[{"params":{"graph":graph}}]})] {
                    let d = RingDesign { graph: Some(value), ..RingDesign::default() };
                    assert_eq!(format_version_for(&d), FORMAT_VERSION, "{kind}.{pin}/{form}");
                }
            }
        }
        for kind in ["base.preset", "shank.key", "stamp", "stamp.top", "stamp.row", "design.stamps", "stamp.outline.keel", "cad.op.head.claw"] {
            let d = RingDesign { graph: Some(serde_json::json!({"nodes":[{"kind":kind}]})), ..RingDesign::default() };
            assert_eq!(format_version_for(&d), FORMAT_VERSION, "{kind}");
        }
    }
}

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
static MIGRATIONS: &[fn(&mut serde_json::Value)] = &[migrate_v0_to_v1, migrate_v1_to_v2, migrate_v2_to_v3, migrate_v3_to_v4, migrate_v4_to_v5, migrate_v5_to_v6];

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
/// Version 6 only fences a stored mesh and an in-plane revolution off from older readers; a version-5 document has the same shape.
fn migrate_v5_to_v6(_doc: &mut serde_json::Value) {}

/// The key a version-6 file keeps each of its stored meshes under once, by content digest.
const STORED_MESHES: &str = "stored_meshes";

/// Serialization wrapper that puts the version key ahead of the design fields and the stored meshes after them.
#[derive(serde::Serialize)]
struct VersionedDesign<'a> {
    format_version: u32,
    #[serde(flatten)]
    design: &'a RingDesign,
    #[serde(skip_serializing_if = "Option::is_none")]
    stored_meshes: Option<Table<'a>>,
}

/// A file's stored meshes by digest, each written in full.
struct Table<'a>(&'a std::collections::BTreeMap<String, crate::cad::stored::Packed>);

impl serde::Serialize for Table<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (key, mesh) in self.0 {
            map.serialize_entry(key, &crate::cad::stored::Whole(mesh))?;
        }
        map.end()
    }
}

/// The design document as versioned JSON text, at the oldest version that carries it.
pub fn design_json(design: &RingDesign) -> anyhow::Result<String> {
    let format_version = format_version_for(design);
    let inline = || -> anyhow::Result<String> { Ok(serde_json::to_string_pretty(&VersionedDesign { format_version, design, stored_meshes: None })?) };
    // Only a stored mesh earns the table; a design fenced at 6 for anything else is written inline.
    if !crate::cad::stored::carried_by(design) {
        return inline();
    }
    // Every stored mesh once in the file's table, the document's and the graph's copies each a reference into it.
    let Some((table, graph)) = stored_table(design) else { return inline() };
    let mut shared = design.clone();
    shared.graph = graph;
    let text = {
        let _references = crate::cad::stored::ByReference::new();
        serde_json::to_string_pretty(&VersionedDesign { format_version, design: &shared, stored_meshes: Some(Table(&table)) })?
    };
    // The writer's own reload check: a file that would not reopen bit for bit is written with its meshes inline.
    let reopens = read_design(&text, FORMAT_VERSION).ok().and_then(|back| serde_json::to_string(&back).ok()) == serde_json::to_string(design).ok();
    if reopens { Ok(text) } else { inline() }
}

/// Every stored mesh `design` carries, by digest, and its graph with each mesh there a reference; `None` when two meshes share a digest.
fn stored_table(design: &RingDesign) -> Option<(std::collections::BTreeMap<String, crate::cad::stored::Packed>, Option<serde_json::Value>)> {
    let mut table = std::collections::BTreeMap::new();
    let mut keep = |mesh: crate::cad::stored::Packed| -> Option<String> {
        let key = mesh.digest();
        match table.get(&key) {
            Some(held) if *held != mesh => None,
            Some(_) => Some(key),
            None => {
                table.insert(key.clone(), mesh);
                Some(key)
            }
        }
    };
    for f in design.cad.iter().flat_map(|doc| &doc.features) {
        if let crate::cad::Operation::Stored { mesh, .. } = &f.operation {
            keep(mesh.clone())?;
        }
    }
    let mut graph = design.graph.clone();
    if let Some(g) = &mut graph {
        refer(g, &mut keep)?;
    }
    Some((table, graph))
}

/// The mesh object of a stored operation `v` holds directly: `{"Stored": {"recipe": …, "mesh": …}}`.
fn stored_mesh_of(v: &mut serde_json::Value) -> Option<&mut serde_json::Value> {
    let stored = v.as_object_mut()?.get_mut("Stored")?.as_object_mut()?;
    if !stored.contains_key("recipe") {
        return None;
    }
    stored.get_mut("mesh")
}

/// Every stored mesh written in full in `v` swapped for its reference, each handed to `keep`; `None` when `keep` refuses one.
fn refer(v: &mut serde_json::Value, keep: &mut impl FnMut(crate::cad::stored::Packed) -> Option<String>) -> Option<()> {
    if let Some(mesh) = stored_mesh_of(v)
        && let Ok(packed) = serde_json::from_value::<crate::cad::stored::Packed>(mesh.clone())
    {
        *mesh = serde_json::json!({ crate::cad::stored::REFERENCE_KEY: keep(packed)? });
    }
    match v {
        serde_json::Value::Object(map) => map.values_mut().try_for_each(|item| refer(item, keep)),
        serde_json::Value::Array(items) => items.iter_mut().try_for_each(|item| refer(item, keep)),
        _ => Some(()),
    }
}

/// Every reference in `v` to the file's table swapped for the mesh it names; refused by name when one is not there.
fn resolve(v: &mut serde_json::Value, table: &serde_json::Map<String, serde_json::Value>) -> anyhow::Result<()> {
    if let Some(mesh) = stored_mesh_of(v)
        && let Some(key) = mesh.get(crate::cad::stored::REFERENCE_KEY).and_then(serde_json::Value::as_str)
    {
        let held = table.get(key).ok_or_else(|| anyhow::anyhow!("Stored mesh {key} is not in the file's table"))?;
        *mesh = held.clone();
    }
    match v {
        serde_json::Value::Object(map) => map.values_mut().try_for_each(|item| resolve(item, table)),
        serde_json::Value::Array(items) => items.iter_mut().try_for_each(|item| resolve(item, table)),
        _ => Ok(()),
    }
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
    read_design(text, FORMAT_VERSION)
}

/// [`load_design_str`] as a build that reads up to version `newest` runs it.
fn read_design(text: &str, newest: u32) -> anyhow::Result<RingDesign> {
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    let version = match doc.get(VERSION_KEY) {
        Some(v) => v.as_u64().ok_or_else(|| anyhow::anyhow!("Invalid design format version"))?,
        None => 0,
    };
    if version > u64::from(newest) {
        anyhow::bail!(
            "design file is format version {version}, but this build reads up to {newest} \
             — it was saved by a newer RingDesigner"
        );
    }
    for step in &MIGRATIONS[version as usize..newest as usize] {
        step(&mut doc);
    }
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(VERSION_KEY);
    }
    // A version-6 file keeps each stored mesh once; every reference to it takes it back.
    if version >= 6 {
        let table = match doc.as_object_mut().and_then(|obj| obj.remove(STORED_MESHES)) {
            Some(serde_json::Value::Object(table)) => table,
            Some(_) => anyhow::bail!("The file's stored meshes are not a table"),
            None => serde_json::Map::new(),
        };
        resolve(&mut doc, &table)?;
    }
    let design: RingDesign = serde_json::from_value(doc)?;
    if let Some(base) = &design.imported_base { base.validate_design(&design)?; }
    Ok(design)
}

/// The user's own alpha library in the platform data directory. This is where
/// imports land and where a converted collection belongs.
pub fn user_alpha_dir() -> PathBuf {
    data_root().join("alphas")
}

/// Every directory scanned at startup.
///
/// The bundled alphas used to be a directory here too, found through the
/// source tree's path as of the build — which existed on one machine. They are
/// compiled in now ([`crate::AlphaLibrary::load_bundled`]), and what is left is
/// the user's own, loaded after them so a file of the same name wins.
pub fn alpha_dirs() -> Vec<PathBuf> {
    vec![user_alpha_dir()]
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

/// Every signet plan the program offers: the bundled factory ones, then the
/// user's own, which shadow a bundled plan of the same name. Sorted by name.
pub fn list_outlines() -> Vec<crate::CustomOutline> {
    let bundled = ringdesign_assets::OUTLINES
        .iter()
        .filter_map(|a| asset_from_str::<crate::CustomOutline>(&a.text(), Path::new(a.file)))
        .filter(|o| o.r.len() == 720 && o.r.iter().all(|v| v.is_finite() && *v > 0.0));
    overlay(bundled, list_outlines_in(&outline_dir()), |o| o.name.clone())
}

/// The user's own entries laid over the bundled ones, keyed by `key`, sorted
/// by that key. A jeweller's file of a bundled name replaces it rather than
/// appearing twice.
fn overlay<T>(
    bundled: impl Iterator<Item = T>,
    user: Vec<T>,
    key: impl Fn(&T) -> String,
) -> Vec<T> {
    let mut out: Vec<T> = bundled.collect();
    for item in user {
        let k = key(&item);
        match out.iter().position(|b| key(b) == k) {
            Some(i) => out[i] = item,
            None => out.push(item),
        }
    }
    out.sort_by_key(&key);
    out
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

/// Every profile the program offers: the bundled factory sections, then the
/// user's own, which shadow a bundled section of the same name. Sorted by
/// name. Unreadable files are skipped — one bad import must not hide the rest
/// of the library.
pub fn list_profiles() -> Vec<(String, crate::BandProfile)> {
    let bundled = ringdesign_assets::PROFILES.iter().filter_map(|a| {
        Some((a.name.to_string(), asset_from_str::<crate::BandProfile>(&a.text(), Path::new(a.file))?))
    });
    overlay(bundled, list_profiles_in(&profile_dir()), |(name, _)| name.clone())
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

    /// The factory library travels in the binary, so an install on a machine
    /// that has never run the harvest tools still has it. Before this it was
    /// written into the data root by `tools/harvest/`, and a copied binary
    /// came up with the procedural patterns and nothing else.
    #[test]
    fn the_factory_library_is_bundled_not_installed() {
        let mut lib = crate::AlphaLibrary::builtin();
        let builtins = lib.len();
        let added = lib.load_bundled();
        assert_eq!(added, ringdesign_assets::ALPHAS.len(), "every bundled alpha decoded");
        assert_eq!(lib.len(), builtins + added, "a bundled name collided with a builtin");
        for name in ["crack-01", "hatch-00", "pattern-01", "scale-01"] {
            assert!(lib.get(name).is_some(), "{name} is missing from the bundle");
        }

        let profiles = list_profiles();
        assert!(profiles.iter().any(|(n, _)| n == "CG Round"), "factory sections are bundled");
        assert!(list_outlines().iter().any(|o| o.name == "CG Clover"), "factory plans are bundled");

        // A user file of a bundled name shadows it rather than doubling it.
        let dir = std::env::temp_dir().join("ringdesign-overlay-test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut mine = crate::BandProfile::default();
        mine.apply_style(crate::ProfileStyle::KnifeEdge);
        save_profile_in(&dir, "CG Round", &mine).unwrap();
        let merged = overlay(
            ringdesign_assets::PROFILES.iter().filter_map(|a| {
                Some((a.name.to_string(), asset_from_str::<crate::BandProfile>(&a.text(), Path::new(a.file))?))
            }),
            list_profiles_in(&dir),
            |(name, _): &(String, crate::BandProfile)| name.clone(),
        );
        assert_eq!(merged.iter().filter(|(n, _)| n == "CG Round").count(), 1);
        assert_eq!(merged.iter().find(|(n, _)| n == "CG Round").unwrap().1.style, crate::ProfileStyle::KnifeEdge);
        assert_eq!(merged.len(), ringdesign_assets::PROFILES.len());
        let _ = std::fs::remove_dir_all(&dir);
    }

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
        assert_eq!(doc["format_version"], u64::from(PLAIN_FORMAT_VERSION), "a design without a stored mesh stays readable by a build that reads up to 5");
    }

    /// A closed tetrahedron packed as another kernel would hand it over.
    fn stored_feature(id: u64) -> crate::cad::Feature {
        use crate::cad::{Component, Feature, Operation, SurfaceKind, stored::{Packed, Recipe}};
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mesh = Packed::encode(&positions, &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], &[0, 0, 0, 0], &[SurfaceKind::Freeform]).unwrap();
        let recipe = Recipe { kernel: "occt".into(), op: "import".into(), ..Recipe::default() };
        Feature { id, name: recipe.label().into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component: Component::default() }
    }

    /// A design at every version before the last reads through each step to the one this build writes.
    #[test]
    fn every_older_version_climbs_the_ladder_to_the_last() {
        use crate::cad::{Component, Document, Feature, Operation, Placement};
        let mut design = RingDesign { name: "Ladder".into(), ..RingDesign::default() };
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let post = Component { placement: Placement::ring(90.0, 0.25), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: post }).unwrap();
        design.cad = Some(doc);
        design.graph = Some(serde_json::json!({ "name": "g", "mode": "Free", "nodes": [{ "id": 3, "kind": "cad.feature", "params": { "id": 3, "name": "Head", "enabled": true, "operation": { "Sphere": { "radius_mm": 2.0 } }, "component": { "placement": { "kind": "free" } } } }] }));
        let want = serde_json::to_value(&design).unwrap();
        for version in 0..FORMAT_VERSION {
            let mut doc = want.clone();
            doc[VERSION_KEY] = version.into();
            let read = load_design_str(&doc.to_string()).unwrap_or_else(|e| panic!("version {version}: {e}"));
            assert_eq!(serde_json::to_value(&read).unwrap(), want, "version {version} reads as the design it holds");
        }
        // The last step itself rewrites nothing: version 6 only fences a stored mesh off from older readers.
        let mut doc = want.clone();
        migrate_v5_to_v6(&mut doc);
        assert_eq!(doc, want);
        // A design that gained a stored mesh after a version-5 save climbs to 6 on its next save and reads back whole.
        design.cad.as_mut().unwrap().append(stored_feature(4)).unwrap();
        let text = design_json(&design).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap()[VERSION_KEY], u64::from(FORMAT_VERSION));
        assert_eq!(serde_json::to_value(load_design_str(&text).unwrap()).unwrap(), serde_json::to_value(&design).unwrap());
    }

    /// A build that reads up to 5 fails to parse a stored mesh; written at 6, the file is refused by name instead.
    #[test]
    fn a_stored_mesh_is_written_at_six_and_a_build_reading_up_to_five_refuses_it_by_name() {
        let plain = RingDesign::default();
        let mut in_document = RingDesign::default();
        let mut doc = crate::cad::Document::default();
        doc.append(stored_feature(1)).unwrap();
        in_document.cad = Some(doc);
        // A driven design carries the mesh in its graph's feature node too; a graph alone is enough to need 6.
        let node = serde_json::json!({ "id": 7, "kind": "cad.feature", "params": serde_json::to_value(stored_feature(7)).unwrap() });
        let in_graph = RingDesign { graph: Some(serde_json::json!({ "name": "g", "mode": "Free", "nodes": [node] })), ..RingDesign::default() };
        let in_cluster = RingDesign { graph: Some(serde_json::json!({ "name": "g", "nodes": [{ "id": 1, "kind": "cluster", "params": { "graph": { "nodes": [node] } } }] })), ..RingDesign::default() };
        assert_eq!(format_version_for(&plain), PLAIN_FORMAT_VERSION);
        for (name, design) in [("document", &in_document), ("graph", &in_graph), ("cluster", &in_cluster)] {
            assert_eq!(format_version_for(design), FORMAT_VERSION, "a stored mesh in the {name}");
            let text = design_json(design).unwrap();
            let older = read_design(&text, PLAIN_FORMAT_VERSION).unwrap_err().to_string();
            assert!(older.contains("format version 6") && older.contains("newer RingDesigner"), "{name}: {older}");
            let read = load_design_str(&text).unwrap();
            assert_eq!(serde_json::to_string(&read).unwrap(), serde_json::to_string(design).unwrap(), "{name}: this build reads it back bit for bit");
        }
        // The same older build still opens every design without one.
        let text = design_json(&plain).unwrap();
        assert!(read_design(&text, PLAIN_FORMAT_VERSION).is_ok());
    }

    /// The wrapper every file was written with before the table: the version first, the design's own fields after it.
    #[derive(serde::Serialize)]
    struct Inline<'a> {
        format_version: u32,
        #[serde(flatten)]
        design: &'a RingDesign,
    }

    /// The Court band as a closed mesh, packed as another kernel would hand it over.
    fn band_packed() -> crate::cad::stored::Packed {
        band_packed_at(96, 48)
    }

    /// [`band_packed`] swept `theta` by `profile`.
    fn band_packed_at(theta: usize, profile: usize) -> crate::cad::stored::Packed {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mesh = crate::mesh::build(&court, &crate::AlphaLibrary::builtin(), crate::BuildParams { theta_steps: theta, profile_steps: profile, refine: None, ..Default::default() }).mesh;
        let positions: Vec<[f64; 3]> = mesh.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
        crate::cad::stored::Packed::encode(&positions, &mesh.faces, &vec![0; mesh.faces.len()], &[crate::cad::SurfaceKind::Freeform]).unwrap()
    }

    /// A design driven by its graph that carries one stored mesh, as the lift writes one: in its document and in the graph's feature node.
    fn driven_with(mesh: crate::cad::stored::Packed) -> RingDesign {
        use crate::cad::{Component, Document, Feature, Operation, stored::Recipe};
        let recipe = Recipe { kernel: "occt".into(), op: "import".into(), params: serde_json::json!({ "file": "band.step" }), digest: String::new() };
        let stored = Feature { id: 7, name: recipe.label().into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component: Component::default() };
        let mut doc = Document::default();
        doc.append(stored.clone()).unwrap();
        let node = serde_json::json!({ "id": 7, "kind": "cad.feature", "params": serde_json::to_value(&stored).unwrap() });
        RingDesign { name: "Driven".into(), cad: Some(doc), graph: Some(serde_json::json!({ "name": "g", "mode": "Free", "nodes": [node] })), ..RingDesign::default() }
    }

    #[test]
    fn a_stored_mesh_is_written_once_and_reopens_bit_for_bit() {
        let mesh = band_packed();
        let design = driven_with(mesh.clone());
        let inline = serde_json::to_string_pretty(&Inline { format_version: FORMAT_VERSION, design: &design }).unwrap();
        let text = design_json(&design).unwrap();
        // One table entry, two references to it, the packed stream once in the file.
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(doc[VERSION_KEY], u64::from(FORMAT_VERSION));
        assert!(text.trim_start().starts_with("{\n  \"format_version\": 6"), "{}", &text[..60]);
        assert_eq!(doc[STORED_MESHES].as_object().unwrap().keys().collect::<Vec<_>>(), [&mesh.digest()]);
        assert_eq!(text.matches(&format!("\"{}\": \"{}\"", crate::cad::stored::REFERENCE_KEY, mesh.digest())).count(), 2);
        assert_eq!(text.matches(&mesh.data[..64]).count(), 1);
        assert_eq!(inline.matches(&mesh.data[..64]).count(), 2);
        // The file shrinks by the copy it no longer carries.
        let ratio = text.len() as f64 / inline.len() as f64;
        eprintln!("driven stored design: {} bytes inline, {} with the table ({:.3}), packed stream {} bytes", inline.len(), text.len(), ratio, mesh.data.len());
        assert!(ratio > 0.49 && ratio < 0.52, "{ratio}");
        // It reopens bit for bit, graph and document both, and a build reading up to 5 refuses it by name.
        let back = load_design_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), serde_json::to_string(&design).unwrap());
        let older = read_design(&text, PLAIN_FORMAT_VERSION).unwrap_err().to_string();
        assert!(older.contains("format version 6") && older.contains("newer RingDesigner"), "{older}");
        // A file written inline still reads, and a reference the table does not hold is refused by name.
        assert_eq!(serde_json::to_string(&load_design_str(&inline).unwrap()).unwrap(), serde_json::to_string(&design).unwrap());
        let mut broken = doc.clone();
        broken[STORED_MESHES] = serde_json::json!({});
        let refused = load_design_str(&broken.to_string()).unwrap_err().to_string();
        assert_eq!(refused, format!("Stored mesh {} is not in the file's table", mesh.digest()));
    }

    /// Timings for the report: `cargo test -p ringdesign-core measured_stored_saves -- --ignored --nocapture`.
    #[test]
    #[ignore = "timings only"]
    fn measured_stored_saves() {
        for (theta, profile) in [(96, 48), (512, 192), (1024, 384)] {
            let design = driven_with(band_packed_at(theta, profile));
            let started = std::time::Instant::now();
            let inline = serde_json::to_string_pretty(&Inline { format_version: FORMAT_VERSION, design: &design }).unwrap();
            let before = started.elapsed();
            let started = std::time::Instant::now();
            let text = design_json(&design).unwrap();
            let after = started.elapsed();
            let started = std::time::Instant::now();
            load_design_str(&text).unwrap();
            let load = started.elapsed();
            eprintln!(
                "{theta}x{profile}: {} triangles, {} bytes inline in {before:.1?}, {} with the table in {after:.1?} ({:.3}), reopened in {load:.1?}",
                theta * profile * 2,
                inline.len(),
                text.len(),
                text.len() as f64 / inline.len() as f64
            );
        }
    }

    #[test]
    fn meshes_the_same_share_one_entry_and_meshes_that_differ_keep_their_own() {
        use crate::cad::{Operation, SurfaceKind, stored::Packed};
        let mut design = driven_with(band_packed());
        let small = Packed::encode(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]], &[0, 0, 0, 0], &[SurfaceKind::Freeform]).unwrap();
        let doc = design.cad.as_mut().unwrap();
        for (id, mesh) in [(8, band_packed()), (9, small.clone())] {
            let mut f = doc.features[0].clone();
            f.id = id;
            if let Operation::Stored { mesh: m, .. } = &mut f.operation {
                *m = mesh;
            }
            doc.append(f).unwrap();
        }
        let text = design_json(&design).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let mut keys: Vec<String> = doc[STORED_MESHES].as_object().unwrap().keys().cloned().collect();
        keys.sort();
        let mut want = vec![band_packed().digest(), small.digest()];
        want.sort();
        assert_eq!(keys, want);
        assert_eq!(text.matches(&format!("\"{}\": ", crate::cad::stored::REFERENCE_KEY)).count(), 4);
        assert_eq!(serde_json::to_string(&load_design_str(&text).unwrap()).unwrap(), serde_json::to_string(&design).unwrap());
    }

    /// A shank turned about the finger's axis: read in the world, or in its XZ section's own plane.
    fn turned(in_plane: bool) -> crate::cad::Feature {
        use crate::cad::{Component, Feature, Operation};
        use crate::sketch::{Sketch, Workplane};
        let mut section = Sketch::rectangle(2.0, 2.0);
        section.plane = Workplane { origin: [10.0, 0.0, 0.0], ..Workplane::section() };
        let (pivot, axis) = if in_plane { ([-10.0, 0.0, 0.0], [0.0, 1.0, 0.0]) } else { ([0.0; 3], [0.0, 0.0, 1.0]) };
        Feature { id: 2, name: "Shank".into(), enabled: true, operation: Operation::Revolve { sketch: section.into(), pivot, axis, degrees: 360.0, in_plane }, component: Component::default() }
    }

    /// A build that reads up to 5 would turn an in-plane line about the world's; written at 6, it is refused by name, and no table is written.
    #[test]
    fn a_revolution_read_in_its_plane_is_written_at_six_without_a_stored_mesh_table() {
        let mut in_document = RingDesign { name: "Turned".into(), ..RingDesign::default() };
        let mut doc = crate::cad::Document::default();
        doc.append(turned(true)).unwrap();
        in_document.cad = Some(doc);
        let node = serde_json::json!({ "id": 2, "kind": "cad.feature", "params": serde_json::to_value(turned(true)).unwrap() });
        let in_graph = RingDesign { graph: Some(serde_json::json!({ "name": "g", "mode": "Free", "nodes": [node] })), ..RingDesign::default() };
        let in_cluster = RingDesign { graph: Some(serde_json::json!({ "name": "g", "nodes": [{ "id": 1, "kind": "cluster", "params": { "graph": { "nodes": [node] } } }] })), ..RingDesign::default() };
        for (name, design) in [("document", &in_document), ("graph", &in_graph), ("cluster", &in_cluster)] {
            assert!(crate::cad::turns_in_plane(design) && !crate::cad::stored::carried_by(design), "{name}");
            assert_eq!(format_version_for(design), FORMAT_VERSION, "an in-plane revolution in the {name}");
            let text = design_json(design).unwrap();
            assert_eq!(text, serde_json::to_string_pretty(&Inline { format_version: FORMAT_VERSION, design }).unwrap(), "{name}: written inline");
            assert!(!text.contains(STORED_MESHES), "{name}");
            let older = read_design(&text, PLAIN_FORMAT_VERSION).unwrap_err().to_string();
            assert!(older.contains("format version 6") && older.contains("newer RingDesigner"), "{name}: {older}");
            assert_eq!(serde_json::to_string(&load_design_str(&text).unwrap()).unwrap(), serde_json::to_string(design).unwrap(), "{name}: read back bit for bit");
        }
        // Both lines turn the same ring; the world's is written at 5 as it always was.
        let lib = crate::AlphaLibrary::builtin();
        let volume = |d: &RingDesign| crate::cad::evaluate(d, &lib, crate::BuildParams::default()).unwrap().components[0].mesh.volume_mm3();
        let mut world = in_document.clone();
        world.cad.as_mut().unwrap().features[0] = turned(false);
        assert_eq!(volume(&world), volume(&in_document));
        let v = volume(&world);
        assert!((v / (std::f64::consts::PI * (11.0f64.powi(2) - 9.0f64.powi(2)) * 2.0) - 1.0).abs() < 0.01, "{v}");
        assert!(!crate::cad::turns_in_plane(&world));
        assert_eq!(design_json(&world).unwrap(), serde_json::to_string_pretty(&Inline { format_version: PLAIN_FORMAT_VERSION, design: &world }).unwrap());
        // A stored mesh beside it still earns the table.
        in_document.cad.as_mut().unwrap().append(stored_feature(3)).unwrap();
        let text = design_json(&in_document).unwrap();
        assert!(text.contains(STORED_MESHES) && text.contains(r#""in_plane": true"#));
        assert_eq!(serde_json::to_string(&load_design_str(&text).unwrap()).unwrap(), serde_json::to_string(&in_document).unwrap());
    }

    #[test]
    fn a_design_without_a_stored_mesh_is_written_at_five_as_it_always_was() {
        use crate::cad::{Attach, Component, Document, Feature, Operation, Placement};
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut parted = court.clone();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.0 }, component: Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.9), ..Component::default() } }).unwrap();
        parted.cad = Some(doc);
        let mut driven = parted.clone();
        driven.graph = Some(serde_json::json!({ "name": "g", "mode": "Free", "nodes": [{ "id": 3, "kind": "cad.feature", "params": { "id": 3, "name": "Head", "enabled": true, "operation": { "Sphere": { "radius_mm": 2.0 } } } }] }));
        let mut turned_in_world = RingDesign::default();
        let mut doc = Document::default();
        doc.append(turned(false)).unwrap();
        turned_in_world.cad = Some(doc);
        for (name, d) in [("default", RingDesign::default()), ("court", court), ("parted", parted), ("driven", driven), ("turned in the world", turned_in_world)] {
            let text = design_json(&d).unwrap();
            assert_eq!(text, serde_json::to_string_pretty(&Inline { format_version: PLAIN_FORMAT_VERSION, design: &d }).unwrap(), "{name}");
            assert!(!text.contains(STORED_MESHES), "{name}");
        }
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
