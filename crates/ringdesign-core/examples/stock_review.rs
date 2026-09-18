//! Geometry-first stock selection for the imported showcase collection.
use anyhow::Result;
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    imported_base::{ImportedBase, PRESETS},
    manufacturing as mf, render,
};
fn main() -> Result<()> {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or("target/stock-review".into()),
    );
    std::fs::create_dir_all(&out)?;
    let mut records = Vec::new();
    for preset in PRESETS {
        let mut d = RingDesign::default();
        ImportedBase::attach(&mut d, preset.load()?)?;
        let built =
            ringdesign_core::mesh::try_build(&d, &AlphaLibrary::default(), BuildParams::default())?;
        let mut setup = mf::Setup::default();
        setup.sample_pitch_mm = 0.1;
        let release = mf::release::analyze(&built.mesh, &setup)?;
        let id = &preset.name[..3];
        render::write_png(
            out.join(format!("{id}.png")),
            &built.mesh,
            0.48,
            1.0,
            600,
            render::GOLD,
        )?;
        let row =
            serde_json::json!({"id": id, "name":preset.name,"release":release,"mesh":built.report});
        println!(
            "{id}: {:?}, {} obstructions, {} unresolved",
            release.status,
            release.obstructions.len(),
            release.unresolved_rays
        );
        records.push(row);
    }
    std::fs::write(
        out.join("stocks.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}
