//! Keep immutable packed mesh bytes out of the CAD editor's per-frame graph copies and hashes.
//! Tokens are local to this pane. Evaluation and commits always restore the complete data.
use ringdesign_graph::graph::Graph;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

const PREFIX: &str = "@cad-mesh:";

#[derive(Clone, Default)]
pub(super) struct Meshes(BTreeMap<String, Arc<str>>);

impl Meshes {
    pub(super) fn compact(&mut self, graph: Graph) -> anyhow::Result<Graph> {
        let mut json = serde_json::to_value(graph)?;
        self.compact_value(&mut json);
        Ok(serde_json::from_value(json)?)
    }

    pub(super) fn compact_value(&mut self, json: &mut Value) {
        visit(json, &mut |data| {
            if !data.starts_with(PREFIX) {
                let key = format!("{PREFIX}{:x}", Sha256::digest(data.as_bytes()));
                let bytes = std::mem::replace(data, key.clone());
                self.0.entry(key).or_insert_with(|| Arc::from(bytes));
            }
            Ok(())
        }).expect("compacting mesh bytes cannot fail");
    }

    pub(super) fn expand(&self, graph: &Graph) -> anyhow::Result<Graph> {
        let mut json = serde_json::to_value(graph)?;
        self.expand_value(&mut json)?;
        Ok(serde_json::from_value(json)?)
    }

    pub(super) fn expand_value(&self, json: &mut Value) -> anyhow::Result<()> {
        visit(json, &mut |data| {
            if data.starts_with(PREFIX) {
                let bytes = self.0.get(data).ok_or_else(|| anyhow::anyhow!("The CAD candidate's stored mesh is unavailable; reopen the design"))?;
                *data = bytes.to_string();
            }
            Ok(())
        })
    }
}

/// Match the packed mesh's fields, including meshes nested in graph patches and clusters.
fn visit(json: &mut Value, f: &mut impl FnMut(&mut String) -> anyhow::Result<()>) -> anyhow::Result<()> {
    match json {
        Value::Object(map) => {
            if map.contains_key("vertices") && map.contains_key("triangles") && map.contains_key("faces") {
                if let Some(Value::String(data)) = map.get_mut("data") {
                    return f(data);
                }
            }
            for item in map.values_mut() { visit(item, f)?; }
        }
        Value::Array(items) => for item in items { visit(item, f)?; },
        _ => {}
    }
    Ok(())
}
