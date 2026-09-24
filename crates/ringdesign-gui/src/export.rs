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
        match self.verdict_of(out) {
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

    /// The verdict the file carries: the snapshot, or with CAD parts on the band the field judged with them on `out`.
    fn verdict_of(&self, out: &ringdesign_core::BuildResult) -> Option<Verdict> {
        let parts = self.design.band_is_procedural() && self.design.cad.as_ref().is_some_and(|doc| !doc.attachments().is_empty());
        if !parts {
            return self.verdict;
        }
        let d = &self.design;
        Some(ringdesign_core::castability::judged_field_report(d, &self.lib, &d.draft, 192, 128, Some(out)).verdict)
    }

    fn build(&self) -> ringdesign_core::BuildResult {
        ringdesign_core::mesh::build(&self.design, &self.lib, self.params)
    }

    /// What a mould is made from: under sand, made settings are left out and each seat carries its
    /// drill mark. Mesh files are patterns; renders and GLB are the finished ring.
    fn build_pattern(&self) -> ringdesign_core::BuildResult {
        ringdesign_core::mesh::try_build_pattern(&self.design, &self.lib, self.params).unwrap_or_else(|_| self.build())
    }

    /// The factor the chosen metal's shrink cuts a pattern oversize by; 1 for nominal.
    fn scale(&self) -> f64 {
        self.shrink.and_then(|i| metal::METALS.get(i)).map_or(1.0, |m| metal::pattern_scale(m.shrink_pct))
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
        let out = job.build_pattern();
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
        let out = job.build_pattern();
        let (_, name) = job.pattern(&ringdesign_core::Mesh::default());
        // The band with its joined and cut parts is one object, each separate part its own.
        let objects: Vec<threemf::Object> = threemf::objects(&out, &name).iter().map(|o| o.scaled(job.scale())).collect();
        match stl::write_obj_objects(&path, &objects) {
            Ok(bytes) => format!(
                "Wrote {} • {} object{} • {} tris • {:.1} KB{}",
                path.display(),
                objects.len(),
                if objects.len() == 1 { "" } else { "s" },
                out.report.validation.triangle_count,
                bytes as f64 / 1024.0,
                job.caveats(&out)
            ),
            Err(e) => format!("OBJ export failed: {e}"),
        }
    });
}

pub fn export_step(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("STEP", &["step", "stp"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.step", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    export_step_to(app, path);
}

/// The whole ring as STEP at `path`, built and written off the UI thread, its band's facets held to
/// [`step::BAND_TOLERANCE_MM`](ringdesign_core::cad::step::BAND_TOLERANCE_MM) of every vertex of the export build.
pub(crate) fn export_step_to(app: &mut RingDesignerApp, path: PathBuf) {
    use ringdesign_core::cad::step;
    let job = ExportJob::snapshot(app);
    spawn_export(app, "STEP", move || {
        match step::ring_sized(&job.design, &job.lib, job.params, step::BAND_TOLERANCE_MM, &job.design.name) {
            Ok(sized) => {
                let text = &sized.text;
                let exact = text.matches("=MANIFOLD_SOLID_BREP(").count() + text.matches("=BREP_WITH_VOIDS(").count();
                let faceted = text.matches("=FACETED_BREP(").count();
                let nominal = if job.shrink.is_some() { " • nominal size, STEP is never scaled for shrink" } else { "" };
                match library::write_atomic(&path, text.as_bytes()) {
                    Ok(()) => format!(
                        "Wrote {} • {exact} exact and {faceted} faceted solid{} • {} • {}{nominal}",
                        path.display(),
                        if exact + faceted == 1 { "" } else { "s" },
                        size_words(text.len()),
                        band_words(sized.band.as_ref())
                    ),
                    Err(e) => format!("STEP export failed: {e}"),
                }
            }
            Err(e) => format!("STEP export failed: {e:#}"),
        }
    });
}

/// A file's size in the unit that reads.
pub(crate) fn size_words(bytes: usize) -> String {
    if bytes >= 1 << 20 { format!("{:.1} MB", bytes as f64 / 1048576.0) } else { format!("{:.1} KB", bytes as f64 / 1024.0) }
}

/// `n` with its thousands grouped.
pub(crate) fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What a STEP file's band holds: its facets, and how near every vertex of the export build stands to them.
fn band_words(band: Option<&ringdesign_core::cad::step::BandFacets>) -> String {
    match band {
        Some(b) if b.written < b.built => format!("band {} facets from {}, every vertex of the export build within {:.3} mm", grouped(b.written), grouped(b.built), b.deviation_mm),
        Some(b) => format!("band {} facets as the export build made them", grouped(b.written)),
        None => "no band: every part as it was built".into(),
    }
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
        let out = job.build_pattern();
        // The mesh in this file is scaled for the metal's shrink, bore and
        // all, so stamping it with the nominal size is the one mistake the
        // filename convention exists to prevent. Say which it is.
        let size = match job.shrink.and_then(|i| metal::METALS.get(i)) {
            Some(m) => format!(
                "{} pattern, cut +{:.1}% for {}",
                job.design.size.display(),
                m.shrink_pct,
                m.name
            ),
            None => job.design.size.display(),
        };
        let (_, name) = job.pattern(&ringdesign_core::Mesh::default());
        let objects: Vec<threemf::Object> = threemf::objects(&out, &name).iter().map(|o| o.scaled(job.scale())).collect();
        match threemf::write_3mf_objects(&path, &objects, &name, &size) {
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
        let field = ringdesign_core::castability::judged_field_report(
            &job.design,
            &job.lib,
            &job.design.draft,
            192,
            128,
            Some(&out),
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
            app.document_path = Some(path.clone());
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
            let named = app.stamps_named();
            app.design = d;
            app.follow_stamps(named);
            // A different file is a different session; the old timeline does
            // not describe it.
            app.history.reset(&app.design.clone());
            app.selected_layer = None;
            app.fit_pending = true;
            app.mark_dirty();
            app.document_path = Some(path.to_path_buf());
            app.push_recent(path);
            app.set_status(format!("Opened {}", path.display()));
        }
        Err(e) => app.set_status(format!("Open failed: {e}")),
    }
}

/// Start a template from the shared collection library.
pub fn load_catalog_template(app: &mut RingDesignerApp, t: &ringdesign_workbench::templates::Template) {
    match t.instantiate(&app.graph_reg, &app.lib) {
        Ok(design) => adopt_template(app, design, t.name),
        Err(e) => app.set_status(format!("Could not open template: {e}")),
    }
}

fn adopt_template(app: &mut RingDesignerApp, design: ringdesign_core::RingDesign, name: &str) {
    design.unpack_embedded(app.library_mut());
    design.bake_all(app.library_mut());
    app.document_path = None;
    let named = app.stamps_named();
    app.design = design;
    app.follow_stamps(named);
    app.history.reset(&app.design.clone());
    app.selected_layer = None;
    app.fit_pending = true;
    app.sync_graph();
    app.arrange_graph();
    app.mark_dirty();
    app.set_status(format!("New design from template: {name}"));
}

/// Import a part from STL, OBJ or STEP onto the ring.
pub fn import_part(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Part: STL, OBJ or STEP", ringdesign_mcp::import::EXTENSIONS)
        .set_directory(dir("exports"))
        .pick_file()
    else {
        return;
    };
    import_part_path(app, &path);
}

/// Part files up to this many bytes are read on the UI thread: measured in release at 13 ms or less in every format, inside a frame.
pub(crate) const SYNC_IMPORT_BYTES: u64 = 1 << 20;

/// What reads a part file, by its extension.
fn reader_of(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("stl") => "the STL reader",
        Some("obj") => "the OBJ reader",
        _ => "the STEP reader",
    }
}

/// The part `path` holds, joined at the top of the ring and chosen, one History entry, read off the UI thread past [`SYNC_IMPORT_BYTES`]; a STEP file goes to OpenCascade only for what the core leaves or refuses.
pub(crate) fn import_part_path(app: &mut RingDesignerApp, path: &std::path::Path) {
    let file = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let step = ringdesign_mcp::import::is_step(path);
    // Only a file known to be over the threshold goes to the slot.
    if std::fs::metadata(path).is_ok_and(|m| m.len() > SYNC_IMPORT_BYTES) {
        let (reader, path, occt) = (reader_of(path), path.to_path_buf(), app.occt.clone());
        app.start_import(file, reader, move |cancel| {
            let read = ringdesign_mcp::import::part_file(&path).map_err(|e| format!("{e:#}"));
            if step { crate::occt::after_core(&path, &occt, read, cancel) } else { read }
        });
        return;
    }
    let read = ringdesign_mcp::import::part_file(path).map_err(|e| format!("{e:#}"));
    if step && crate::occt::leaves_for_occt(&read) && crate::occt::available(&app.occt) {
        let (path, occt) = (path.to_path_buf(), app.occt.clone());
        app.start_import(file, "OpenCascade", move |cancel| crate::occt::after_core(&path, &occt, read, cancel));
        return;
    }
    match read {
        Ok(read) => land_part(app, read),
        Err(e) => app.set_status(e),
    }
}

/// A part read from a file, stood at the top of the ring, joined and chosen through the edit funnel: one History entry.
pub(crate) fn land_part(app: &mut RingDesignerApp, (feature, notes): (ringdesign_core::cad::Feature, Vec<String>)) {
    let name = feature.name.clone();
    let edits = ringdesign_core::cad::stored::import_edits(&app.design, feature);
    // A refused edit has said why on the status line already.
    let Ok(applied) = crate::cad_edit::apply(app, &edits) else { return };
    if let Some(id) = applied.last().and_then(|a| a.id) {
        app.selection.click(Some(ringdesign_workbench::viewport::Sel::Part(id)), ringdesign_workbench::viewport::selection::Mods::default());
    }
    let said: String = notes.iter().map(|n| format!(" • {n}")).collect();
    app.set_status(format!("Imported {name} at the top of the ring, joined: G moves it, R turns it{said}"));
}

/// The part being read, over the window's foot: what reads which file, for how long, and a way to stop waiting.
pub(crate) fn import_plate(app: &mut RingDesignerApp, ctx: &egui::Context) {
    let Some(p) = &app.importing else { return };
    let words = format!("Reading {} in {}… {:.1} s", p.file, p.reader, p.started.elapsed().as_secs_f64());
    let mut cancel = false;
    egui::Area::new(egui::Id::new("part-import"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            egui::Frame::new().fill(crate::theme::FLOAT).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(words);
                    cancel = ui.button("Cancel import").on_hover_text("Stop waiting for the part: nothing is imported, whatever the reader finds").clicked();
                });
            });
        });
    if cancel {
        app.cancel_import();
    }
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
