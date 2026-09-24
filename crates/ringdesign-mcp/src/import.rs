//! A part file read into a stored CAD feature, for the MCP tool and for any host that links this crate.
use anyhow::{Context, Result, bail};
use ringdesign_core::cad::{Feature, step, stored};
use std::path::Path;

/// The extensions a part file may carry.
pub const EXTENSIONS: &[&str] = &["stl", "obj", "step", "stp"];

/// Whether `path` names a STEP file.
pub fn is_step(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("step") || e.eq_ignore_ascii_case("stp"))
}

/// The part `path` holds as a stored feature and what reading it said: STL and OBJ by the solid crate's readers, STEP by the core's.
pub fn part_file(path: &Path) -> Result<(Feature, Vec<String>)> {
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("The file").to_string();
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    let (format, solids, notes) = match ext.as_str() {
        "stl" => ("stl", vec![ringdesign_solid::io::read_stl(path).with_context(|| format!("{file} does not read as STL"))?], Vec::new()),
        "obj" => ("obj", vec![ringdesign_solid::io::read_obj(path).with_context(|| format!("{file} does not read as OBJ"))?], Vec::new()),
        _ if is_step(path) => {
            let text = std::fs::read_to_string(path).with_context(|| format!("{file} could not be read"))?;
            let (meshes, notes) = step::solid_meshes(&text, &file)?;
            ("step", meshes, notes)
        }
        _ => bail!("{file}: a part comes in as STL, OBJ or STEP"),
    };
    Ok((stored::imported(&file, format, &solids)?, notes))
}
