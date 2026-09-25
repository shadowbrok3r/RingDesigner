//! Shared set-up for the batch-15 spikes: a sand casting set-up, factory stock as its sand master, and a summary of a pull.
#![allow(dead_code)]
use anyhow::{Context, Result};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    castability::{CastProcess, SandProcess},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    manufacturing as mf,
};

/// A two-part Delft pour judged at `pitch_mm`, parting on z = 0, no channels.
pub fn sand_setup(pitch_mm: f64) -> mf::Setup {
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.sample_pitch_mm = pitch_mm;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    setup
}

/// Give a design the recipe's own draft settings and carry the set-up.
pub fn cast_in(d: &mut RingDesign, setup: &mf::Setup) {
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
    d.manufacturing = Some(setup.clone());
}

/// The same set-up judged for lost wax at the investment floors.
pub fn wax_setup(pitch_mm: f64) -> mf::Setup {
    let mut setup = sand_setup(pitch_mm);
    setup.recipe.process = CastProcess::LostWax;
    setup.recipe.sand = None;
    setup.recipe.min_draft_deg = 0.0;
    setup.recipe.min_detail_mm = 0.15;
    setup.recipe.min_section_mm = 0.8;
    setup
}

/// Factory stock `id`, as its sand master when `sand`, its face `(length, width)` mm or its own, on the masterworks' Flat chart.
pub fn stock(id: &str, sand: bool, face: Option<(f64, f64)>) -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == id).with_context(|| format!("no stock {id}"))?;
    let source = preset.load()?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, if sand { sand_master(source)? } else { source })?;
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    let (length, width) = face.unwrap_or((d.shank.head.length_mm, d.profile.width_mm));
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.shank.head.length_mm = length;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = BuildParams { theta_steps: 900, profile_steps: 448, refine: None, ..Default::default() };
    cast_in(&mut d, &sand_setup(0.1));
    Ok(d)
}

/// One line on a pull: the ray release's status, obstructions, their worst depth and area, and the unresolved rays.
pub fn release_line(r: &mf::release::ReleaseReport) -> String {
    let depth = r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max);
    let area: f64 = r.obstructions.iter().map(|o| o.projected_area_mm2).sum();
    format!("{:?}, {} obstructions (deepest {depth:.3} mm, {area:.3} mm²), {} unresolved", r.status, r.obstructions.len(), r.unresolved_rays)
}

/// Inspect `d` under its own set-up at `params`, and again with the rays at `fine_pitch`.
pub fn pull(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, fine_pitch: f64) -> Result<(mf::Inspection, mf::release::ReleaseReport)> {
    let setup = d.manufacturing.clone().unwrap_or_else(|| sand_setup(0.1));
    let inspection = mf::inspect(d, lib, &setup, params)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = fine_pitch;
    let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    Ok((inspection, release))
}
