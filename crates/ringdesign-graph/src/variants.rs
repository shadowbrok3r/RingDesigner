//! Shared graph-preserving size batches for desktop and CLI. Every result
//! carries component manufacturing findings and its exact editable source.
use crate::{
    eval::{Evaluator, evaluate_design},
    graph::Graph,
    registry::Registry,
};
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Mesh, RingDesign, manufacturing as mf, resize::Policy,
};
pub struct Variant {
    pub design: RingDesign,
    pub mesh: Mesh,
    pub report: serde_json::Value,
    pub stem: String,
}

pub fn evaluate(
    d: &RingDesign,
    bores: &[f64],
    policy: &Policy,
    lib: &AlphaLibrary,
    params: BuildParams,
    registry: &Registry,
    ev: &mut Evaluator,
) -> Result<Vec<Variant>> {
    ensure!(!bores.is_empty() && bores.len() <= 40, "Choose 1–40 bores");
    for bore in bores {
        ringdesign_core::resize::size_from_bore(*bore)?;
    }
    let original: Graph = if let Some(g) = &d.graph {
        serde_json::from_value(g.clone())?
    } else {
        crate::nodes::cad::from_document(d)?
    };
    let mut variants = Vec::new();
    for (index, bore) in bores.iter().enumerate() {
        let notes = ringdesign_core::resize::candidate(d, *bore, policy)?.notes;
        let mut graph = original.clone();
        crate::nodes::cad::append_resize(&mut graph, *bore, policy)?;
        let evaluated = evaluate_design(ev, &graph, registry, lib, 0)?;
        let mut variant = (*evaluated.design).clone();
        ensure!(
            (variant.size.inner_diameter_mm() - bore).abs() < 1e-7,
            "Resize graph did not produce the requested bore"
        );
        variant.graph = Some(serde_json::to_value(graph)?);
        variant.manufacturing = d.manufacturing.clone();
        variant.casting_trials = d.casting_trials.clone();
        variant.name = format!("{} — bore {bore:.3} mm", d.name);
        let resolved = mf::source_library(&variant, lib);
        let lib = resolved.as_ref();
        let built = ringdesign_core::mesh::try_build(&variant, lib, params)?;
        let setups: Vec<_> = if let Some(doc) = &variant.cad {
            doc.outputs
                .iter()
                .filter_map(|id| {
                    doc.features
                        .iter()
                        .find(|f| f.id == *id && !f.component.reference)
                })
                .map(|f| {
                    let mut s = f
                        .component
                        .manufacturing
                        .clone()
                        .or_else(|| variant.manufacturing.clone())
                        .unwrap_or_else(|| mf::Setup::from_design(&variant));
                    s.component = Some(f.id);
                    s
                })
                .collect()
        } else {
            vec![
                variant
                    .manufacturing
                    .clone()
                    .unwrap_or_else(|| mf::Setup::from_design(&variant)),
            ]
        };
        let reports: Vec<_> = setups
            .iter()
            .map(|s| match mf::inspect(&variant, lib, s, params) {
                Ok(i) => mf::package::report(&variant, s, &i, true),
                Err(e) => {
                    serde_json::json!({"component":s.component,"unassessed":format!("{e:#}")})
                }
            })
            .collect();
        let grams = if let Some(doc) = &variant.cad {
            let e = ringdesign_core::cad::evaluate(&variant, lib, params)?;
            let _ = doc;
            e.components
                .iter()
                .filter(|c| !c.settings.reference)
                .filter_map(|c| {
                    ringdesign_core::metal::find(&c.settings.material)
                        .map(|m| c.mesh.volume_mm3() * m.density / 1000.0)
                })
                .sum::<f64>()
        } else {
            let alloy = setups[0].recipe.alloy.as_str();
            ringdesign_core::metal::find(alloy)
                .map_or(0.0, |m| built.mesh.volume_mm3() * m.density / 1000.0)
        };
        let stem = format!("variant-{:02}-bore-{bore:.3}", index + 1);
        let report = serde_json::json!({"file":stem,"nominal_bore_mm":bore,"size":variant.size.display(),"grams":grams,"geometry":built.report,"manufacturing":reports,"resize_notes":notes,"stage":"nominal"});
        variants.push(Variant {
            design: variant,
            mesh: built.mesh,
            report,
            stem,
        });
    }
    Ok(variants)
}

pub fn export(dir: &std::path::Path, variants: &[Variant], lib: &AlphaLibrary) -> Result<()> {
    ensure!(!dir.exists(), "Variant destination already exists");
    ensure!(!variants.is_empty(), "No variants to export");
    let parent = dir
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".variants-{}.building",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&temp)?;
    let result = (|| -> Result<()> {
        for v in variants {
            let resolved = mf::source_library(&v.design, lib);
            ringdesign_core::library::save_design_embedded(
                temp.join(format!("{}.ring.json", v.stem)),
                &v.design,
                &resolved,
            )?;
            ringdesign_core::stl::write_stl(
                temp.join(format!("{}-nominal.stl", v.stem)),
                &v.mesh,
                &v.design.name,
            )?;
            std::fs::write(
                temp.join(format!("{}-report.json", v.stem)),
                serde_json::to_vec_pretty(&v.report)?,
            )?;
        }
        std::fs::write(
            temp.join("variants.json"),
            serde_json::to_vec_pretty(&variants.iter().map(|v| &v.report).collect::<Vec<_>>())?,
        )?;
        ensure!(!dir.exists(), "Destination appeared during export");
        std::fs::rename(&temp, dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&temp);
    }
    result
}
