//! Graph, cluster and preset files, with their own version ladder.
//!
//! The same shape as the design file's: a `format_version` beside the
//! document, one migration step per version so a step can never be
//! skipped, and a newer file refused with a clear line. Clusters are graphs
//! with exposed inputs and outputs; presets are named values for a
//! cluster's exposed inputs. Bundled graphs live in the repository; the
//! user's live under the data root beside designs, profiles and outlines.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ringdesign_core::library;
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, Node};
use crate::registry::Registry;
use crate::value::Literal;

pub const GRAPH_EXT: &str = "graph.json";
pub const CLUSTER_EXT: &str = "cluster.json";
pub const PRESET_EXT: &str = "preset.json";
/// The newest version this build reads; version 2 fences an in-plane revolution, and a cut on a ring of parts alone, off from older readers.
pub const GRAPH_FORMAT_VERSION: u32 = 2;
/// The version a file without an in-plane revolution or a cut on a ring of parts alone is written at.
pub const PLAIN_GRAPH_FORMAT_VERSION: u32 = 1;
/// The oldest version whose nodes this build reads as they stand: a node's own migration runs only on files older than it.
pub const NODE_SHAPE_VERSION: u32 = 1;
const VERSION_KEY: &str = "format_version";

/// One step per version, index `v` taking a version-`v` document to `v + 1`.
static MIGRATIONS: &[fn(&mut serde_json::Value)] = &[migrate_v0_to_v1, migrate_v1_to_v2];

/// Version 0 is a bare `Graph` with no version key at all.
fn migrate_v0_to_v1(_doc: &mut serde_json::Value) {}

/// Version 2 only fences an in-plane revolution and a cut on a ring of parts alone off from older readers; a version-1 document has the same shape.
fn migrate_v1_to_v2(_doc: &mut serde_json::Value) {}

/// Whether a literal holds what an older reader must be fenced from: a revolution read in its sketch's plane, or a cut on a ring of parts alone.
fn literal_fenced(l: &Literal) -> bool {
    match l {
        Literal::Json(v) => ringdesign_core::cad::turns_in_plane_json(v) || ringdesign_core::parts::cuts_apart_json(v),
        Literal::List(items) => items.iter().any(literal_fenced),
        _ => false,
    }
}

/// The version `g` is written at: the newest when a node carries a revolution read in its sketch's plane, or its `cad.feature` nodes, or a cluster's, make a ring of parts alone carrying a cut.
pub fn graph_version_for(g: &Graph) -> u32 {
    let features = g.nodes.iter().filter(|n| n.kind == "cad.feature").map(|n| &n.params);
    let nested = |n: &Node| ringdesign_core::cad::turns_in_plane_json(&n.params) || ringdesign_core::parts::cuts_apart_json(&n.params) || n.inputs.values().any(literal_fenced);
    if ringdesign_core::parts::features_cut_apart(features) || g.nodes.iter().any(nested) { GRAPH_FORMAT_VERSION } else { PLAIN_GRAPH_FORMAT_VERSION }
}

/// The version `p` is written at: the newest when a value carries a revolution read in its sketch's plane or a ring of parts alone carrying a cut.
pub fn preset_version_for(p: &Preset) -> u32 {
    if p.values.values().any(literal_fenced) { GRAPH_FORMAT_VERSION } else { PLAIN_GRAPH_FORMAT_VERSION }
}

/// The version of a graph or preset document, refused by `kind` when it is newer than `reads_up_to`.
fn version_of(doc: &serde_json::Value, kind: &str, reads_up_to: u32) -> anyhow::Result<u32> {
    let version = doc.get(VERSION_KEY).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version > reads_up_to {
        anyhow::bail!("{kind} file is format version {version}, but this build reads up to {reads_up_to} — it was saved by a newer RingDesigner");
    }
    Ok(version)
}

#[derive(Serialize)]
struct Versioned<'a, T: Serialize> {
    format_version: u32,
    #[serde(flatten)]
    doc: &'a T,
}

pub fn graph_dir() -> PathBuf {
    library::data_root().join("graphs")
}

pub fn cluster_dir() -> PathBuf {
    library::data_root().join("clusters")
}

pub fn preset_dir() -> PathBuf {
    library::data_root().join("presets")
}

pub fn graph_to_string(g: &Graph) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&Versioned { format_version: graph_version_for(g), doc: g })?)
}

pub fn save_graph(path: impl AsRef<Path>, g: &Graph) -> anyhow::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, graph_to_string(g)?)?;
    Ok(())
}

/// Read a graph file, walking it up the ladder, then giving every node
/// its kind's own migration when a registry is at hand.
pub fn load_graph_str(text: &str, reg: Option<&Registry>) -> anyhow::Result<Graph> {
    read_graph(text, reg, GRAPH_FORMAT_VERSION)
}

/// [`load_graph_str`] as a build reading up to `reads_up_to` would.
fn read_graph(text: &str, reg: Option<&Registry>, reads_up_to: u32) -> anyhow::Result<Graph> {
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    let version = version_of(&doc, "graph", reads_up_to)?;
    for step in &MIGRATIONS[version as usize..] {
        step(&mut doc);
    }
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(VERSION_KEY);
    }
    let mut g: Graph = serde_json::from_value(doc)?;
    if version < NODE_SHAPE_VERSION {
        if let Some(reg) = reg {
            for node in &mut g.nodes {
                if let Some(f) = reg.get(&node.kind).and_then(|s| s.migrate) {
                    f(node, version);
                }
            }
        }
    }
    Ok(g)
}

pub fn load_graph(path: impl AsRef<Path>, reg: Option<&Registry>) -> anyhow::Result<Graph> {
    load_graph_str(&std::fs::read_to_string(path)?, reg)
}

fn list_in(dir: &Path, ext: &str, reg: Option<&Registry>) -> Vec<Graph> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let suffix = format!(".{ext}");
    let mut out: Vec<Graph> = rd
        .flatten()
        .filter(|e| e.path().to_string_lossy().ends_with(&suffix))
        .filter_map(|e| load_graph(e.path(), reg).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// A file name from a graph's name: lowercase, dashes for what is not
/// alphanumeric.
pub fn slug(name: &str) -> String {
    let s: String = name.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "untitled".into() } else { s }
}

pub fn save_cluster_in(dir: &Path, g: &Graph) -> anyhow::Result<PathBuf> {
    let path = dir.join(format!("{}.{CLUSTER_EXT}", slug(&g.name)));
    save_graph(&path, g)?;
    Ok(path)
}

pub fn save_cluster(g: &Graph) -> anyhow::Result<PathBuf> {
    save_cluster_in(&cluster_dir(), g)
}

pub fn list_clusters_in(dir: &Path, reg: Option<&Registry>) -> Vec<Graph> {
    list_in(dir, CLUSTER_EXT, reg)
}

/// The user's clusters, then the bundled ones a user file does not shadow.
pub fn list_clusters(reg: Option<&Registry>) -> Vec<Graph> {
    let mut out = list_clusters_in(&cluster_dir(), reg);
    for c in crate::templates::bundled_clusters() {
        if !out.iter().any(|u| u.name == c.name) {
            out.push(c);
        }
    }
    out
}

pub fn load_cluster_in(dir: &Path, name: &str, reg: Option<&Registry>) -> Option<Graph> {
    let path = dir.join(format!("{}.{CLUSTER_EXT}", slug(name)));
    if path.exists() {
        return load_graph(path, reg).ok();
    }
    list_clusters_in(dir, reg).into_iter().find(|g| g.name == name)
}

pub fn load_cluster(name: &str, reg: Option<&Registry>) -> Option<Graph> {
    load_cluster_in(&cluster_dir(), name, reg).or_else(|| crate::templates::bundled_clusters().into_iter().find(|c| c.name == name))
}

pub fn save_graph_in(dir: &Path, g: &Graph) -> anyhow::Result<PathBuf> {
    let path = dir.join(format!("{}.{GRAPH_EXT}", slug(&g.name)));
    save_graph(&path, g)?;
    Ok(path)
}

pub fn list_graphs_in(dir: &Path, reg: Option<&Registry>) -> Vec<Graph> {
    list_in(dir, GRAPH_EXT, reg)
}

pub fn list_graphs(reg: Option<&Registry>) -> Vec<Graph> {
    list_graphs_in(&graph_dir(), reg)
}

/// Named values for a cluster's exposed inputs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub cluster: String,
    #[serde(default)]
    pub values: BTreeMap<String, Literal>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

impl Preset {
    /// Set this preset's values on a cluster node's inputs; other inputs
    /// are left alone. Returns the names the node had no pin for.
    pub fn apply(&self, node: &mut Node, reg: &Registry) -> Vec<String> {
        let pins: Vec<String> = reg.node_pins(node).map(|(ins, _)| ins.into_iter().map(|p| p.name).collect()).unwrap_or_default();
        let mut unknown = Vec::new();
        for (k, v) in &self.values {
            if pins.contains(k) {
                node.inputs.insert(k.clone(), v.clone());
            } else {
                unknown.push(k.clone());
            }
        }
        unknown
    }
}

pub fn preset_to_string(p: &Preset) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&Versioned { format_version: preset_version_for(p), doc: p })?)
}

pub fn load_preset_str(text: &str) -> anyhow::Result<Preset> {
    read_preset(text, GRAPH_FORMAT_VERSION)
}

/// [`load_preset_str`] as a build reading up to `reads_up_to` would.
fn read_preset(text: &str, reads_up_to: u32) -> anyhow::Result<Preset> {
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    version_of(&doc, "preset", reads_up_to)?;
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(VERSION_KEY);
    }
    Ok(serde_json::from_value(doc)?)
}

pub fn save_preset_in(dir: &Path, p: &Preset) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.{PRESET_EXT}", slug(&p.name)));
    std::fs::write(&path, preset_to_string(p)?)?;
    Ok(path)
}

pub fn save_preset(p: &Preset) -> anyhow::Result<PathBuf> {
    save_preset_in(&preset_dir(), p)
}

pub fn list_presets_in(dir: &Path) -> Vec<Preset> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let suffix = format!(".{PRESET_EXT}");
    let mut out: Vec<Preset> = rd
        .flatten()
        .filter(|e| e.path().to_string_lossy().ends_with(&suffix))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| load_preset_str(&t).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// The user's presets, then the bundled ones a user file does not shadow.
pub fn list_presets() -> Vec<Preset> {
    let mut out = list_presets_in(&preset_dir());
    for p in crate::templates::bundled_presets() {
        if !out.iter().any(|u| u.name == p.name) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Mode;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringdesign-graph-files-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn every_version_has_a_migration_step() {
        assert_eq!(MIGRATIONS.len(), GRAPH_FORMAT_VERSION as usize, "one step per version, none skipped");
    }

    /// A revolution about the finger's axis, its line read in its sketch's plane or in the world.
    fn turn(in_plane: bool) -> serde_json::Value {
        serde_json::json!({ "Revolve": { "sketch": { "feature": 1 }, "pivot": [-10.0, 0.0, 0.0], "axis": [0.0, 1.0, 0.0], "degrees": 360.0, "in_plane": in_plane } })
    }

    #[test]
    fn an_in_plane_revolution_fences_its_graph_cluster_and_preset_at_two_and_the_rest_stay_at_one() {
        let reg = Registry::builtin();
        let mut plain = Graph::new("Turned", Mode::Free);
        let n = plain.add("cad.feature").unwrap();
        plain.node_mut(n).unwrap().params = serde_json::json!({ "id": 2, "name": "Shank", "enabled": true, "operation": turn(false) });
        // Without one the file is what version 1 always wrote, byte for byte, and a build reading up to 1 opens it.
        let text = graph_to_string(&plain).unwrap();
        assert_eq!(text, serde_json::to_string_pretty(&Versioned { format_version: 1, doc: &plain }).unwrap());
        assert_eq!(read_graph(&text, Some(&reg), PLAIN_GRAPH_FORMAT_VERSION).unwrap(), plain);
        // In a node's params, on its operation pin, or inside a cluster, it is written at 2, reads back whole, and a build reading up to 1 refuses it by name.
        let mut in_params = plain.clone();
        in_params.nodes[0].params["operation"] = turn(true);
        let mut on_pin = plain.clone();
        on_pin.nodes[0].inputs.insert("operation".into(), Literal::Json(turn(true)));
        let mut in_cluster = Graph::new("Turned cluster", Mode::Free);
        let c = in_cluster.add("cluster").unwrap();
        in_cluster.node_mut(c).unwrap().params = serde_json::json!({ "graph": serde_json::to_value(&in_params).unwrap() });
        for (name, g) in [("params", &in_params), ("pin", &on_pin), ("cluster", &in_cluster)] {
            assert_eq!(graph_version_for(g), GRAPH_FORMAT_VERSION, "{name}");
            let text = graph_to_string(g).unwrap();
            assert!(text.contains("\"format_version\": 2"), "{name}");
            assert_eq!(&load_graph_str(&text, Some(&reg)).unwrap(), g, "{name}");
            let older = read_graph(&text, None, PLAIN_GRAPH_FORMAT_VERSION).unwrap_err().to_string();
            assert_eq!(older, "graph file is format version 2, but this build reads up to 1 — it was saved by a newer RingDesigner", "{name}");
        }
        // A preset carrying one on a value is fenced the same way; one without is written at 1.
        let preset = |op: serde_json::Value| Preset { name: "Turned".into(), cluster: "Turned cluster".into(), values: [("Operation".to_string(), Literal::Json(op))].into_iter().collect(), doc: String::new() };
        let text = preset_to_string(&preset(turn(false))).unwrap();
        assert!(text.contains("\"format_version\": 1"));
        assert_eq!(read_preset(&text, PLAIN_GRAPH_FORMAT_VERSION).unwrap(), preset(turn(false)));
        let text = preset_to_string(&preset(turn(true))).unwrap();
        assert!(text.contains("\"format_version\": 2"));
        assert_eq!(load_preset_str(&text).unwrap(), preset(turn(true)));
        let older = read_preset(&text, PLAIN_GRAPH_FORMAT_VERSION).unwrap_err().to_string();
        assert_eq!(older, "preset file is format version 2, but this build reads up to 1 — it was saved by a newer RingDesigner");
    }

    #[test]
    fn a_cut_on_a_ring_of_parts_alone_fences_its_graph_cluster_and_preset_at_two_and_a_banded_one_stays_at_one() {
        use ringdesign_core::cad::{Attach, Component, Document, Feature, Operation};
        let reg = Registry::builtin();
        let feature = |id: u64, name: &str, operation: Operation, attach: Attach| Feature { id, name: name.into(), enabled: true, operation, component: Component { attach, ..Component::default() } };
        let parts = |band: bool| {
            let mut doc = Document::default();
            let shank = band.then(|| feature(1, "Procedural shank", Operation::Band, Attach::Separate));
            for f in shank.into_iter().chain([feature(2, "Block", Operation::Box { size: [4.0, 3.0, 2.0] }, Attach::Separate), feature(3, "Pocket", Operation::Box { size: [2.0, 1.5, 1.6] }, Attach::Cut)]) {
                doc.append(f).unwrap();
            }
            ringdesign_core::RingDesign { cad: Some(doc), ..ringdesign_core::RingDesign::default() }
        };
        assert!(ringdesign_core::parts::cuts_apart(&parts(false)) && !ringdesign_core::parts::cuts_apart(&parts(true)));
        let alone = crate::nodes::cad::from_document(&parts(false)).unwrap();
        let banded = crate::nodes::cad::from_document(&parts(true)).unwrap();
        // Beside a band the cut carves the band, which every reader knows: written at 1, as version 1 always wrote it.
        let text = graph_to_string(&banded).unwrap();
        assert_eq!(text, serde_json::to_string_pretty(&Versioned { format_version: 1, doc: &banded }).unwrap());
        assert_eq!(read_graph(&text, Some(&reg), PLAIN_GRAPH_FORMAT_VERSION).unwrap(), banded);
        // Alone, in the graph's feature nodes or inside a cluster, it is written at 2, round-trips byte for byte, and a build reading up to 1 refuses it.
        let mut in_cluster = Graph::new("Parts cluster", Mode::Free);
        let c = in_cluster.add("cluster").unwrap();
        in_cluster.node_mut(c).unwrap().params = serde_json::json!({ "graph": serde_json::to_value(&alone).unwrap() });
        for (name, g) in [("nodes", &alone), ("cluster", &in_cluster)] {
            assert_eq!(graph_version_for(g), GRAPH_FORMAT_VERSION, "{name}");
            let text = graph_to_string(g).unwrap();
            assert!(text.contains("\"format_version\": 2"), "{name}");
            let back = load_graph_str(&text, Some(&reg)).unwrap();
            assert_eq!(&back, g, "{name}");
            assert_eq!(graph_to_string(&back).unwrap(), text, "{name}");
            let older = read_graph(&text, None, PLAIN_GRAPH_FORMAT_VERSION).unwrap_err().to_string();
            assert_eq!(older, "graph file is format version 2, but this build reads up to 1 — it was saved by a newer RingDesigner", "{name}");
        }
        // A design carrying the graph alone is fenced at the design's own newest version.
        let driven = ringdesign_core::RingDesign { graph: Some(serde_json::to_value(&alone).unwrap()), ..ringdesign_core::RingDesign::default() };
        assert_eq!(library::format_version_for(&driven), library::FORMAT_VERSION);
        // A preset carrying the document on a value is fenced the same way; the banded one is written at 1.
        let preset = |band: bool| Preset { name: "Parts".into(), cluster: "Parts cluster".into(), values: [("Document".to_string(), Literal::Json(serde_json::to_value(parts(band).cad).unwrap()))].into_iter().collect(), doc: String::new() };
        assert!(preset_to_string(&preset(true)).unwrap().contains("\"format_version\": 1"));
        let text = preset_to_string(&preset(false)).unwrap();
        assert!(text.contains("\"format_version\": 2"));
        assert_eq!(load_preset_str(&text).unwrap(), preset(false));
        let older = read_preset(&text, PLAIN_GRAPH_FORMAT_VERSION).unwrap_err().to_string();
        assert_eq!(older, "preset file is format version 2, but this build reads up to 1 — it was saved by a newer RingDesigner");
    }

    /// Marks the node it migrates with the version the file had.
    fn mark(node: &mut Node, from: u32) {
        node.params = serde_json::json!({ "migrated_from": from });
    }

    #[test]
    fn a_nodes_migration_runs_on_an_older_file_and_never_on_a_current_one() {
        let mut reg = Registry::builtin();
        reg.register(crate::registry::NodeSpec::new("test.shaped", "Shaped", crate::registry::Category::Util).migrate(mark)).unwrap();
        let mut g = Graph::new("Shaped", Mode::Free);
        let n = g.add("test.shaped").unwrap();
        g.node_mut(n).unwrap().params = serde_json::json!({ "kept": true });
        // A plain file and one fenced at 2 read back as written, and write back byte for byte.
        let plain = graph_to_string(&g).unwrap();
        assert!(plain.contains("\"format_version\": 1"));
        let back = load_graph_str(&plain, Some(&reg)).unwrap();
        assert_eq!(back, g);
        assert_eq!(graph_to_string(&back).unwrap(), plain);
        let mut turned = g.clone();
        let t = turned.add("cad.feature").unwrap();
        turned.node_mut(t).unwrap().params = serde_json::json!({ "id": 2, "name": "Shank", "enabled": true, "operation": turn(true) });
        let fenced = graph_to_string(&turned).unwrap();
        assert!(fenced.contains("\"format_version\": 2"));
        let back = load_graph_str(&fenced, Some(&reg)).unwrap();
        assert_eq!(back, turned);
        assert_eq!(graph_to_string(&back).unwrap(), fenced);
        // A bare version-0 file is older than the shapes this build writes, and its node is migrated.
        let bare = serde_json::to_string(&g).unwrap();
        let migrated = load_graph_str(&bare, Some(&reg)).unwrap();
        assert_eq!(migrated.nodes[0].params, serde_json::json!({ "migrated_from": 0 }));
    }

    #[test]
    fn graphs_round_trip_with_their_version_and_refuse_a_newer_one() {
        let reg = Registry::builtin();
        let mut g = Graph::new("Court band", Mode::SandRing);
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "width_mm", Literal::Number(6.0)).unwrap();
        g.expose(p, "width_mm", "Width").unwrap();
        g.expose_output(p, "profile", "profile").unwrap();
        let text = graph_to_string(&g).unwrap();
        assert!(text.contains("\"format_version\": 1"));
        let back = load_graph_str(&text, Some(&reg)).unwrap();
        assert_eq!(back, g);
        // A bare graph (version 0) reads, walking the ladder.
        let bare = serde_json::to_string(&g).unwrap();
        assert_eq!(load_graph_str(&bare, Some(&reg)).unwrap(), g);
        let newer = text.replace("\"format_version\": 1", "\"format_version\": 99");
        let err = load_graph_str(&newer, None).unwrap_err();
        assert!(err.to_string().contains("newer RingDesigner"), "{err}");
        let dir = tmp("graphs");
        let path = save_graph_in(&dir, &g).unwrap();
        assert!(path.to_string_lossy().ends_with("court-band.graph.json"));
        assert_eq!(list_graphs_in(&dir, Some(&reg)).len(), 1);
        let cpath = save_cluster_in(&dir, &g).unwrap();
        assert!(cpath.to_string_lossy().ends_with("court-band.cluster.json"));
        assert_eq!(load_cluster_in(&dir, "Court band", Some(&reg)).unwrap().outputs.len(), 1);
        assert!(load_cluster_in(&dir, "nope", None).is_none());
    }

    #[test]
    fn presets_name_values_for_a_cluster_and_apply_to_its_node() {
        let dir = tmp("presets");
        let p = Preset {
            name: "Wide court".into(),
            cluster: "Court band".into(),
            values: [("Width".to_string(), Literal::Number(8.0)), ("Nope".to_string(), Literal::Bool(true))].into_iter().collect(),
            doc: "an 8 mm court".into(),
        };
        let path = save_preset_in(&dir, &p).unwrap();
        assert!(path.to_string_lossy().ends_with("wide-court.preset.json"));
        let listed = list_presets_in(&dir);
        assert_eq!(listed, vec![p.clone()]);
        let err = load_preset_str(&preset_to_string(&p).unwrap().replace("\"format_version\": 1", "\"format_version\": 7")).unwrap_err();
        assert!(err.to_string().contains("newer"), "{err}");
        assert_eq!(slug("  Wide  Court / 2 "), "wide--court---2");
    }
}
