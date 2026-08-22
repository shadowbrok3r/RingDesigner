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
pub const GRAPH_FORMAT_VERSION: u32 = 1;
const VERSION_KEY: &str = "format_version";

/// One step per version, index `v` taking a version-`v` document to `v + 1`.
static MIGRATIONS: &[fn(&mut serde_json::Value)] = &[migrate_v0_to_v1];

/// Version 0 is a bare `Graph` with no version key at all.
fn migrate_v0_to_v1(_doc: &mut serde_json::Value) {}

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
    Ok(serde_json::to_string_pretty(&Versioned { format_version: GRAPH_FORMAT_VERSION, doc: g })?)
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
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    let version = doc.get(VERSION_KEY).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version > GRAPH_FORMAT_VERSION {
        anyhow::bail!(
            "graph file is format version {version}, but this build reads up to {GRAPH_FORMAT_VERSION} — it was saved by a newer RingDesigner"
        );
    }
    for step in &MIGRATIONS[version as usize..] {
        step(&mut doc);
    }
    if let Some(obj) = doc.as_object_mut() {
        obj.remove(VERSION_KEY);
    }
    let mut g: Graph = serde_json::from_value(doc)?;
    if version < GRAPH_FORMAT_VERSION {
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

pub fn list_clusters(reg: Option<&Registry>) -> Vec<Graph> {
    list_clusters_in(&cluster_dir(), reg)
}

pub fn load_cluster_in(dir: &Path, name: &str, reg: Option<&Registry>) -> Option<Graph> {
    let path = dir.join(format!("{}.{CLUSTER_EXT}", slug(name)));
    if path.exists() {
        return load_graph(path, reg).ok();
    }
    list_clusters_in(dir, reg).into_iter().find(|g| g.name == name)
}

pub fn load_cluster(name: &str, reg: Option<&Registry>) -> Option<Graph> {
    load_cluster_in(&cluster_dir(), name, reg)
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
    Ok(serde_json::to_string_pretty(&Versioned { format_version: GRAPH_FORMAT_VERSION, doc: p })?)
}

pub fn load_preset_str(text: &str) -> anyhow::Result<Preset> {
    let mut doc: serde_json::Value = serde_json::from_str(text)?;
    let version = doc.get(VERSION_KEY).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    if version > GRAPH_FORMAT_VERSION {
        anyhow::bail!("preset file is format version {version}, but this build reads up to {GRAPH_FORMAT_VERSION} — it was saved by a newer RingDesigner");
    }
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

pub fn list_presets() -> Vec<Preset> {
    list_presets_in(&preset_dir())
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
