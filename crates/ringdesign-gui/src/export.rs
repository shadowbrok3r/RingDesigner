//! File dialogs for mesh export, design persistence, and alpha import.

use std::path::PathBuf;

use ringdesign_core::castability::Verdict;
use ringdesign_core::{library, metal, stl, threemf};

use crate::app::RingDesignerApp;

/// Everything an export job needs, snapshotted so the build and the write
/// can run off the UI thread while the app keeps painting.
struct ExportJob {
    design: ringdesign_core::RingDesign,
    lib: std::sync::Arc<ringdesign_core::AlphaLibrary>,
    params: ringdesign_core::BuildParams,
    shrink: Option<usize>,
    /// The last settled field verdict. Snapshotted rather than recomputed:
    /// the worker already pays for it on every settled build, and an export
    /// that silently re-judged could disagree with the banner on screen.
    verdict: Option<Verdict>,
}

impl ExportJob {
    fn snapshot(app: &RingDesignerApp) -> Self {
        Self {
            design: app.design.clone(),
            lib: app.lib.clone(),
            params: app.export_params,
            shrink: app.shrink_metal,
            verdict: app.field.as_ref().map(|f| f.verdict),
        }
    }

    /// What the caster needs told before they cut a flask, appended to every
    /// success line. The export used to build, write, and report the byte
    /// count — so a `NotCastable` ring and a refinement that never reached its
    /// tolerance both left the app looking like a clean export.
    fn caveats(&self, out: &ringdesign_core::BuildResult) -> String {
        let mut w: Vec<String> = Vec::new();
        match self.verdict {
            Some(Verdict::NotCastable) => {
                w.push("the field verdict says this will NOT release".into())
            }
            Some(Verdict::Marginal) => w.push("the field verdict is marginal".into()),
            Some(Verdict::Castable) => {}
            None => w.push("not yet judged".into()),
        }
        if let Some(r) = &out.report.refine {
            if r.hit_cap {
                w.push("refinement hit its leaf cap, so the tolerance was not reached".into());
            } else if r.saturated_leaves > 0 {
                w.push(format!(
                    "{} leaves hit the depth limit — the worst-error figure is a floor, not a bound",
                    r.saturated_leaves
                ));
            }
        }
        if !out.report.validation.watertight {
            w.push("NOT watertight".into());
        }
        if w.is_empty() { String::new() } else { format!(" • {}", w.join(" • ")) }
    }

    fn build(&self) -> ringdesign_core::BuildResult {
        ringdesign_core::mesh::build(&self.design, &self.lib, self.params)
    }

    /// The mesh to write and the name to stamp it with: scaled oversize by
    /// the chosen metal's shrink, and *named* as such — a scaled file
    /// mistaken for nominal is a ring that comes out a size small.
    fn pattern(&self, mesh: &ringdesign_core::Mesh) -> (ringdesign_core::Mesh, String) {
        match self.shrink.and_then(|i| metal::METALS.get(i)) {
            Some(m) => (
                mesh.scaled(metal::pattern_scale(m.shrink_pct)),
                format!(
                    "{} [pattern +{:.1}% for {}]",
                    self.design.name, m.shrink_pct, m.name
                ),
            ),
            None => (mesh.clone(), self.design.name.clone()),
        }
    }
}

/// Run an export off the UI thread; the app's status line reports the
/// result when `poll_export` reaps it. One at a time — a second request
/// while one runs is refused with a message rather than queued silently.
fn spawn_export(
    app: &mut RingDesignerApp,
    label: &str,
    job: impl FnOnce() -> String + Send + 'static,
) {
    if app.exporting.is_some() {
        app.set_status("An export is already running — one at a time");
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("export".into())
        .spawn(move || {
            let _ = tx.send(job());
        })
        .expect("spawn export thread");
    app.exporting = Some(rx);
    app.set_status(format!("{label} — building in the background…"));
}

/// Directory a dialog opens in, created on demand so it is always there to
/// browse. Everything the app writes lands under one predictable tree.
fn dir(kind: &str) -> PathBuf {
    let path = match kind {
        "designs" => library::default_design_dir(),
        _ => library::default_design_dir().with_file_name(kind),
    };
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn export_stl(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Binary STL", &["stl"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.stl", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    spawn_export(app, "STL", move || {
        let out = job.build();
        let (mesh, name) = job.pattern(&out.mesh);
        match stl::write_stl(&path, &mesh, &name) {
            Ok(bytes) => {
                let warn = job.caveats(&out);
                format!(
                    "Wrote {} • {} tris • {:.1} KB{warn}",
                    path.display(),
                    out.report.validation.triangle_count,
                    bytes as f64 / 1024.0
                )
            }
            Err(e) => format!("STL export failed: {e}"),
        }
    });
}

pub fn export_obj(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Wavefront OBJ", &["obj"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.obj", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    spawn_export(app, "OBJ", move || {
        let out = job.build();
        let (mesh, name) = job.pattern(&out.mesh);
        match stl::write_obj(&path, &mesh, &name) {
            Ok(bytes) => format!(
                "Wrote {} • {} tris • {:.1} KB{}",
                path.display(),
                out.report.validation.triangle_count,
                bytes as f64 / 1024.0,
                job.caveats(&out)
            ),
            Err(e) => format!("OBJ export failed: {e}"),
        }
    });
}

pub fn export_3mf(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("3MF model", &["3mf"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.3mf", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    spawn_export(app, "3MF", move || {
        let out = job.build();
        let size = job.design.size.display();
        let (mesh, name) = job.pattern(&out.mesh);
        match threemf::write_3mf(&path, &mesh, &name, &size) {
            Ok(bytes) => format!(
                "Wrote {} • {} tris • {:.1} KB • units mm stated{}",
                path.display(),
                out.report.validation.triangle_count,
                bytes as f64 / 1024.0,
                job.caveats(&out)
            ),
            Err(e) => format!("3MF export failed: {e}"),
        }
    });
}

pub fn export_render(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("PNG image", &["png"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.png", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    let tint = crate::viewport::FINISHES[app.finish.min(crate::viewport::FINISHES.len() - 1)].rgb;
    spawn_export(app, "Render", move || {
        let out = job.build();
        match ringdesign_core::render::write_png(&path, &out.mesh, 0.55, 1.12, 1600, tint) {
            Ok(()) => format!("Wrote {}", path.display()),
            Err(e) => format!("Render failed: {e}"),
        }
    });
}

pub fn export_turntable(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("GIF animation", &["gif"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.gif", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    let tint = crate::viewport::FINISHES[app.finish.min(crate::viewport::FINISHES.len() - 1)].rgb;
    spawn_export(app, "Turntable", move || {
        let out = job.build();
        match ringdesign_core::render::write_turntable_gif(&path, &out.mesh, 36, 640, tint) {
            Ok(()) => format!("Wrote {}", path.display()),
            Err(e) => format!("Turntable failed: {e}"),
        }
    });
}

pub fn export_glb(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("glTF binary", &["glb"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.glb", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    let tint = crate::viewport::FINISHES[app.finish.min(crate::viewport::FINISHES.len() - 1)].rgb;
    spawn_export(app, "GLB", move || {
        let out = job.build();
        let (mesh, name) = job.pattern(&out.mesh);
        match ringdesign_core::gltf::write_glb(&path, &mesh, &name, tint) {
            Ok(bytes) => format!(
                "Wrote {} • {:.1} MB • metres, as glTF wants{}",
                path.display(),
                bytes as f64 / 1048576.0,
                job.caveats(&out)
            ),
            Err(e) => format!("GLB export failed: {e}"),
        }
    });
}

/// Export the parting line as a printable SVG: plan view plus the line's
/// height unrolled around the ring.
pub fn export_parting(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("SVG", &["svg"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}_parting.svg", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let line = ringdesign_core::castability::parting_line(&app.design, &app.lib, 512, 192);
    match ringdesign_core::castability::write_parting_svg(
        &path,
        &line,
        app.design.inner_radius_mm(),
        &app.design.name,
    ) {
        Ok(_) => app.set_status(format!("Wrote {}", path.display())),
        Err(e) => app.set_status(format!("Parting line failed: {e}")),
    }
}

/// Export the stone spacing map: every stone to scale, plan and unrolled,
/// with the census's tight gaps drawn.
pub fn export_stone_map(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("SVG", &["svg"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}_stones.svg", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let parting = app.field.as_ref().map(|f| f.parting_z_mm).unwrap_or(0.0);
    let report = ringdesign_core::stones::report(&app.design, parting);
    match ringdesign_core::stonemap::write_stone_map_svg(&path, &app.design, report.as_ref()) {
        Ok(_) => app.set_status(format!("Wrote {}", path.display())),
        Err(e) => app.set_status(format!("Stone map failed: {e}")),
    }
}

pub fn export_spec(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Casting sheet", &["html"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}_sheet.html", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    spawn_export(app, "Casting sheet", move || {
        let out = job.build();
        let field = ringdesign_core::castability::attributed_field_report(
            &job.design,
            &job.lib,
            &job.design.draft,
            192,
            128,
        );
        let stones = ringdesign_core::stones::report(&job.design, field.parting_z_mm);
        let dfm = ringdesign_core::dfm::findings_in(&job.design, &job.lib);
        let provenance = format!(
            "RingDesigner {} • {} x {} sweep",
            env!("CARGO_PKG_VERSION"),
            job.params.theta_steps,
            job.params.profile_steps
        );
        let page = ringdesign_core::spec::html(
            &job.design,
            &out.report,
            &field,
            stones.as_ref(),
            &dfm,
            &provenance,
        );
        match std::fs::write(&path, page) {
            Ok(()) => format!("Wrote {}", path.display()),
            Err(e) => format!("Sheet failed: {e}"),
        }
    });
}

/// Volume-and-weights JSON for the sibling cost calculator: one small file
/// any pricing tool can read without knowing ring formats.
pub fn export_cost_json(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}_cost.json", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    let job = ExportJob::snapshot(app);
    spawn_export(app, "Cost JSON", move || {
        let out = job.build();
        let metals: Vec<serde_json::Value> = out
            .report
            .metals
            .iter()
            .map(|m| serde_json::json!({ "metal": m.metal, "grams": m.grams, "dwt": m.dwt }))
            .collect();
        let doc = serde_json::json!({
            "name": job.design.name,
            "size_us": job.design.size.0,
            "volume_mm3": out.report.volume_mm3,
            "surface_mm2": out.report.surface_area_mm2,
            "metals": metals,
        });
        match std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default()) {
            Ok(()) => format!("Wrote {}", path.display()),
            Err(e) => format!("Cost JSON failed: {e}"),
        }
    });
}

pub fn save_design(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Ring design", &["json"])
        .set_directory(dir("designs"))
        .set_file_name(format!("{}.json", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    match library::save_design_embedded(&path, &app.design, &app.lib) {
        Ok(()) => {
            app.push_recent(&path);
            app.set_status(format!("Saved {}", path.display()));
        }
        Err(e) => app.set_status(format!("Save failed: {e}")),
    }
}

pub fn open_design(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Ring design", &["json"])
        .set_directory(dir("designs"))
        .pick_file()
    else {
        return;
    };
    open_design_path(app, &path);
}

/// Load a design file directly — the Recent menu's entry point.
pub fn open_design_path(app: &mut RingDesignerApp, path: &std::path::Path) {
    match library::load_design(path) {
        Ok(d) => {
            d.unpack_embedded(app.library_mut());
            d.bake_all(app.library_mut());
            app.design = d;
            // A different file is a different session; the old timeline does
            // not describe it.
            app.history.reset(&app.design.clone());
            app.selected_layer = None;
            app.fit_pending = true;
            app.mark_dirty();
            app.push_recent(path);
            app.set_status(format!("Opened {}", path.display()));
        }
        Err(e) => app.set_status(format!("Open failed: {e}")),
    }
}

/// Replace the design with a fresh template instance.
pub fn load_template(app: &mut RingDesignerApp, t: &ringdesign_core::templates::Template) {
    app.design = t.design();
    app.history.reset(&app.design.clone());
    app.selected_layer = None;
    app.fit_pending = true;
    app.mark_dirty();
    app.set_status(format!("New design from template: {}", t.name));
}

/// Import SVG files: the text travels in the design, the raster in the library.
pub fn import_svgs(app: &mut RingDesignerApp) {
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("SVG", &["svg"])
        .set_directory(library::default_alpha_dir())
        .pick_files()
    else {
        return;
    };
    let mut loaded = 0usize;
    for path in paths {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "svg".into());
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let entry = ringdesign_core::svg::SvgAlpha {
                    name: name.clone(),
                    svg: text,
                    invert: false,
                };
                let raster = entry.rasterize();
                if raster.is_empty() {
                    app.set_status(format!("{name}: not a renderable SVG"));
                    continue;
                }
                // Re-importing under the same name replaces the old vector.
                app.design.svgs.retain(|s| s.name != name);
                app.design.svgs.push(entry);
                app.library_mut().insert(raster);
                loaded += 1;
            }
            Err(e) => app.set_status(format!("{name}: {e}")),
        }
    }
    if loaded > 0 {
        app.set_status(format!(
            "Imported {loaded} SVG(s) — the vector text travels in the design"
        ));
    }
}

/// Import image files into the alpha library.
pub fn import_alphas(app: &mut RingDesignerApp) {
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
        .set_directory(library::default_alpha_dir())
        .pick_files()
    else {
        return;
    };
    let mut added = 0usize;
    let mut failed = 0usize;
    for p in paths {
        match ringdesign_core::alpha::Alpha::load(&p) {
            Ok(a) => {
                app.forget_thumbnail(&a.name);
                app.library_mut().insert(a);
                added += 1;
            }
            Err(e) => {
                log::warn!("import {}: {e}", p.display());
                failed += 1;
            }
        }
    }
    app.set_status(if failed == 0 {
        format!("Imported {added} alpha(s)")
    } else {
        format!("Imported {added}, {failed} failed")
    });
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "ring".into() } else { s }
}
