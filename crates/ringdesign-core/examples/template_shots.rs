//! Finished studio renders and measured gates for every starter and factory blank.
use ringdesign_core::{AlphaLibrary, BuildParams, RingDesign, cad, castability, csg, dfm, manufacturing, mesh, render, stones, templates};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Instant;

fn measure(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> anyhow::Result<(mesh::BuildResult, Value)> {
    let started = Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_ms = started.elapsed().as_millis();
    let field = castability::judged_field_report(d, lib, &d.draft, 192, 128, Some(&built));
    let settings = stones::report_built(d, field.parting_z_mm, &built);
    let mut parts = Vec::new();
    if let Some(e) = &built.parts.evaluated {
        for p in &e.components {
            if !p.settings.reference {
                let solid = csg::Solid { v: p.mesh.vertices.iter().map(|p| [p.0 as f64,p.1 as f64,p.2 as f64]).collect(), f: p.mesh.faces.clone() };
                parts.push(json!({"id":p.id,"name":p.name,"watertight":p.mesh.validate().watertight,"degenerate_faces":p.mesh.quality().degenerate_faces,"crossings":csg::self_crossings(&solid)}));
            }
        }
    }
    let mut setup = manufacturing::Setup::from_design(d);
    setup.sample_pitch_mm = 0.1;
    let inspection = manufacturing::inspect(d, lib, &setup, params);
    let release = match &inspection {
        Ok(i) => json!({"status":format!("{:?}",i.release.status),"obstructions":i.release.obstructions.len(),"unresolved":i.release.unresolved_rays,"wall":i.local_wall,"details":i.details}),
        Err(e) => json!({"error":format!("{e:#}")}),
    };
    let measured_wall = (d.name == "Split gallery").then(|| inspection.as_ref().ok().map(|i| cad::measure::thickness(&i.prepared.mesh.scaled(1.0/i.prepared.scale),0.8)));
    let gallery_sections = (d.name == "Split gallery").then(|| inspection.as_ref().ok().map(|i| templates::settings::gallery_rail_sections(&i.prepared.mesh.scaled(1.0/i.prepared.scale),d.profile.width_mm)));
    let warnings: Vec<_> = settings.iter().flat_map(|s| s.seats.iter().flat_map(|c| c.warnings.iter().map(|w| format!("{}: {w}",c.label)))).collect();
    let row = json!({
        "name":d.name,"process":format!("{:?}",d.draft.process),"build_ms":build_ms,"mesh":built.mesh.validate(),"degenerate_faces":built.mesh.quality().degenerate_faces,
        "solids_notes":built.solids.notes,"parts_notes":built.parts.notes,
        "features":built.parts.evaluated.as_ref().map(|e|&e.features),"made_parts":parts,
        "verdict":format!("{:?}",field.verdict),"undercut_pct":field.undercut_fraction()*100.0,"field_notes":field.notes,"thinnest_wall_mm":field.thinnest_wall_mm,
        "dfm":dfm::findings_in(d,lib).iter().map(|f|format!("{}: {}",f.label,f.message)).collect::<Vec<_>>(),
        "stone_groups":settings.as_ref().map(|s|s.seats.iter().map(|c|json!({"label":c.label,"count":c.count})).collect::<Vec<_>>()),
        "stone_count":settings.as_ref().map_or(0,|s|s.stone_count),"preview_count":stones::all_stone_frames_built(d,&built).len(),"stone_warnings":warnings,
        "tight_pairs":settings.as_ref().map_or(0,|s|s.tight_pairs),"crowding":settings.as_ref().and_then(|s|s.crowding_note()),"release":release,"gallery_wall":measured_wall,"gallery_sections":gallery_sections
    });
    Ok((built,row))
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("output directory");
    let filter = args.next().unwrap_or_default();
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;
    let lib = AlphaLibrary::builtin();
    let mut rows = Vec::new();
    let mut rings = Vec::new();
    let params = BuildParams { theta_steps: 512, profile_steps: 192, ..Default::default() };
    let mut shoot = |slug: String, d: RingDesign, view: (f64,f64), description: &str, open_ms: u128| -> anyhow::Result<()> {
        if !filter.is_empty() && !slug.contains(&filter) { return Ok(()); }
        eprintln!("measuring {slug}");
        match measure(&d,&lib,params) {
            Ok((built,mut row)) => {
                row["open_ms"] = json!(open_ms);
                let f = render::finished_from(&d,&lib,built);
                let parts = f.parts(render::GOLD);
                render::write_png_parts(dir.join(format!("{slug}.png")),&parts,view.0,view.1,700)?;
                let second = if view.1 < 0.5 { (0.55,1.12) } else { (0.25,0.35) };
                render::write_png_parts(dir.join(format!("{slug}-side.png")),&parts,second.0,second.1,700)?;
                println!("{slug}: {}",serde_json::to_string(&row)?);
                let mut spec = format!("{:.2} mm bore · {}",2.0*d.inner_radius_mm(),d.draft.process.label());
                if row["verdict"].as_str() != Some("Castable") {
                    spec.push_str(&format!(" · {}",row["verdict"].as_str().unwrap_or("review")));
                } else if d.name == "Split gallery" { spec.push_str(" · mould trial"); }
                else if d.name == "Half eternity" { spec.push_str(" · setter review"); }
                rows.push(row);
                rings.push(json!({"slug":slug,"title":d.name,"description":description,"spec":spec,"views":[{"name":"hero","label":"Portrait","render":false,"image":format!("{slug}.png")},{"name":"side","label":"Gallery","render":false,"image":format!("{slug}-side.png")}]}));
            }
            Err(e) => {
                eprintln!("{slug}: {e:#}");
                rows.push(json!({"name":d.name,"slug":slug,"error":format!("{e:#}")}));
            }
        }
        std::fs::write(dir.join("gates.json"),serde_json::to_string_pretty(&rows)?)?;
        Ok(())
    };
    for t in templates::all() {
        let slug = t.name.to_ascii_lowercase().replace(' ',"-");
        shoot(slug,t.design(),t.view,t.blurb,0)?;
    }
    for preset in ringdesign_core::imported_base::PRESETS {
        let slug = format!("stock-{}-{}",preset.id,preset.name.to_ascii_lowercase());
        if !filter.is_empty() && !slug.contains(&filter) { continue; }
        let at = Instant::now();
        let d = templates::stock(preset)?;
        let description = format!("Factory stock, hard angles where wall meets face; bare, ready for a theme. {}",templates::stock_process_note(preset).unwrap_or("Verified process at calibrated size."));
        shoot(slug,d,(0.55,1.12),&description,at.elapsed().as_millis())?;
    }
    std::fs::write(dir.join("collection.json"),serde_json::to_string_pretty(&json!({"collection":"starters","title":"Starters","metal":"gold","subtitle":"BLANKS FOR THE BENCH","description":"Bands, settings and factory stocks.","rings":rings}))?)?;
    Ok(())
}
