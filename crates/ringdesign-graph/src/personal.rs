//! Self-contained reusable ring templates. Defaults live in the saved graph.
use crate::{graph::Graph, registry::Registry};
use anyhow::{Result, ensure};
use ringdesign_core::{AlphaLibrary, RingDesign, library};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const EXT: &str = "ringtemplate.json";
pub const MAX_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Template {
    pub template_version: u32,
    pub name: String,
    pub description: String,
    /// A versioned RingDesign, including its artwork and editable graph.
    pub design: serde_json::Value,
    #[serde(default)]
    pub thumbnail_png: Vec<u8>,
}
impl Template {
    pub fn new(
        name: &str,
        description: &str,
        source: &RingDesign,
        graph: &Graph,
        lib: &AlphaLibrary,
    ) -> Result<Self> {
        ensure!(!name.trim().is_empty(), "Give the template a name");
        let mut source = source.clone();
        source.name = name.trim().into();
        source.graph = Some(serde_json::to_value(graph)?);
        let reg = Registry::builtin();
        let mut names: std::collections::BTreeSet<String> = source
            .layers
            .referenced_alphas()
            .into_iter()
            .map(str::to_owned)
            .collect();
        collect_artwork(graph, &reg, &mut names);
        source.embed_named_alphas(lib, names.iter().map(String::as_str));
        let template = Self {
            template_version: 1,
            name: source.name.clone(),
            description: description.trim().into(),
            design: serde_json::from_str(&library::design_json(&source)?)?,
            thumbnail_png: vec![],
        };
        template.validate(&Registry::builtin())?;
        Ok(template)
    }

    pub fn instantiate(&self) -> Result<RingDesign> {
        ensure!(
            self.template_version == 1,
            "This template needs a newer RingDesigner"
        );
        library::load_design_str(&serde_json::to_string(&self.design)?)
    }

    pub fn validate(&self, reg: &Registry) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.len() <= 240,
            "Use a template name of 1–240 bytes"
        );
        ensure!(
            self.description.len() <= 8000 && self.thumbnail_png.len() <= 512 * 1024,
            "Template description or preview is too large"
        );
        let design = self.instantiate()?;
        let graph: Graph = serde_json::from_value(
            design
                .graph
                .ok_or_else(|| anyhow::anyhow!("Template has no editable graph"))?,
        )?;
        let errors = graph.validate(Some(reg));
        ensure!(
            errors.is_empty(),
            "Invalid template graph: {}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
        let mut names = std::collections::BTreeSet::new();
        let mut inputs = std::collections::BTreeSet::new();
        for e in &graph.exposed {
            ensure!(
                names.insert(e.name.trim().to_lowercase()),
                "Control names must be unique: {}",
                e.name
            );
            ensure!(
                inputs.insert((e.node, &e.input)),
                "An input can only be exposed once: {}",
                e.name
            );
            ensure!(!e.name.trim().is_empty(), "Every control needs a name");
            let node = graph
                .node(e.node)
                .ok_or_else(|| anyhow::anyhow!("Control refers to a missing node"))?;
            let (pins, _) = reg
                .node_pins(node)
                .ok_or_else(|| anyhow::anyhow!("Control has no matching node type"))?;
            let pin = pins
                .iter()
                .find(|p| p.name == e.input)
                .ok_or_else(|| anyhow::anyhow!("Control refers to a missing input"))?;
            ensure!(
                graph.wire_into(e.node, &e.input).is_none(),
                "{} is driven by a wire; expose its source instead",
                e.name
            );
            if let Some([min, max]) = e.range {
                ensure!(
                    min.is_finite() && max.is_finite() && min < max,
                    "{} needs a finite minimum below its maximum",
                    e.name
                );
                ensure!(
                    matches!(
                        pin.kind,
                        crate::value::ValueKind::Int | crate::value::ValueKind::Number
                    ),
                    "{} is not a numeric control",
                    e.name
                );
                let default = node.inputs.get(&e.input).or(pin.default.as_ref());
                let number = match default {
                    Some(crate::value::Literal::Number(n)) => Some(*n),
                    Some(crate::value::Literal::Int(n)) => Some(*n as f64),
                    _ => None,
                };
                ensure!(
                    number.is_some_and(|n| n.is_finite() && n >= min && n <= max),
                    "{} needs a numeric default within its range",
                    e.name
                );
            }
        }
        Ok(())
    }

    pub fn save_new(&self, path: &Path) -> Result<()> {
        use std::io::Write;
        self.validate(&Registry::builtin())?;
        let bytes = serde_json::to_vec(self)?;
        ensure!(bytes.len() as u64 <= MAX_BYTES, "Template exceeds 128 MB");
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        // Never replace an existing template; a failed write removes only our new file.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(error.into());
        }
        Ok(())
    }
}

// Include literal artwork references even when a branch does not contribute to today's mesh.
fn collect_artwork(graph: &Graph, reg: &Registry, names: &mut std::collections::BTreeSet<String>) {
    fn literal(value: &crate::value::Literal, names: &mut std::collections::BTreeSet<String>) {
        match value {
            crate::value::Literal::Text(name) => {
                names.insert(name.clone());
            }
            crate::value::Literal::List(items) => {
                for item in items {
                    literal(item, names);
                }
            }
            _ => {}
        }
    }
    for node in &graph.nodes {
        if let Some(spec) = reg.get(&node.kind) {
            for pin in &spec.inputs {
                if pin.kind == crate::value::ValueKind::AlphaRef
                    || (node.kind == "alpha.library" && pin.name == "name")
                {
                    if let Some(value) = node.inputs.get(&pin.name).or(pin.default.as_ref()) {
                        literal(value, names);
                    }
                }
            }
        }
        if node.kind == crate::nodes::cluster::CLUSTER_KIND {
            if let Some(inner) = crate::nodes::cluster::embedded(node) {
                collect_artwork(&inner, reg, names);
            }
        }
    }
}

pub fn directory() -> PathBuf {
    library::data_root().join("templates")
}
pub fn load(path: &Path) -> Result<Template> {
    ensure!(
        std::fs::metadata(path)?.len() <= MAX_BYTES,
        "Template exceeds 128 MB"
    );
    let template: Template = serde_json::from_slice(&std::fs::read(path)?)?;
    template.validate(&Registry::builtin())?;
    Ok(template)
}
pub fn paths(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(EXT))
        .take(500)
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    fn template() -> Template {
        let mut graph = crate::templates::simple();
        graph
            .exposed
            .iter_mut()
            .find(|e| e.name == "Width")
            .unwrap()
            .range = Some([3., 12.]);
        Template::new(
            "Everyday band",
            "A reusable band",
            &RingDesign::default(),
            &graph,
            &AlphaLibrary::builtin(),
        )
        .unwrap()
    }

    #[test]
    fn saved_template_roundtrips_defaults_limits_and_cannot_overwrite_a_file() {
        let template = template();
        let dir = std::env::temp_dir().join(format!(
            "ring-template-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join(format!("band.{EXT}"));
        template.save_new(&path).unwrap();
        let saved = load(&path).unwrap();
        let design = saved.instantiate().unwrap();
        assert_eq!(design.name, "Everyday band");
        let graph: Graph = serde_json::from_value(design.graph.unwrap()).unwrap();
        let e = graph.exposed.iter().find(|e| e.name == "Width").unwrap();
        assert_eq!(e.range, Some([3., 12.]));
        assert_eq!(
            graph.node(e.node).unwrap().inputs["width_mm"],
            crate::value::Literal::Number(6.)
        );
        assert!(template.save_new(&path).is_err());
        assert_eq!(load(&path).unwrap().name, saved.name);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn artwork_on_an_unused_graph_branch_travels_with_the_template() {
        let mut graph = crate::templates::simple();
        let alpha = graph.add("alpha.library").unwrap();
        graph
            .set_input(
                alpha,
                "name",
                crate::value::Literal::Text("My engraving".into()),
            )
            .unwrap();
        let mut lib = AlphaLibrary::default();
        lib.insert(ringdesign_core::Alpha::new(
            "My engraving",
            8,
            8,
            vec![0.5; 64],
        ));
        let t = Template::new("Portable", "", &RingDesign::default(), &graph, &lib).unwrap();
        let design = t.instantiate().unwrap();
        let mut elsewhere = AlphaLibrary::default();
        design.unpack_embedded(&mut elsewhere);
        assert!(elsewhere.get("My engraving").is_some());
    }

    #[test]
    fn out_of_range_defaults_and_duplicate_control_names_are_rejected() {
        let mut t = template();
        let mut design = t.instantiate().unwrap();
        let mut graph: Graph = serde_json::from_value(design.graph.take().unwrap()).unwrap();
        graph
            .exposed
            .iter_mut()
            .find(|e| e.name == "Width")
            .unwrap()
            .range = Some([8., 12.]);
        design.graph = Some(serde_json::to_value(&graph).unwrap());
        t.design = serde_json::from_str(&library::design_json(&design).unwrap()).unwrap();
        assert!(
            t.validate(&Registry::builtin())
                .unwrap_err()
                .to_string()
                .contains("default within")
        );
        graph.exposed[1].name = graph.exposed[0].name.clone();
        design.graph = Some(serde_json::to_value(graph).unwrap());
        t.design = serde_json::from_str(&library::design_json(&design).unwrap()).unwrap();
        assert!(
            t.validate(&Registry::builtin())
                .unwrap_err()
                .to_string()
                .contains("unique")
        );
        t.template_version = 100;
        assert!(t.instantiate().is_err());
    }
}
