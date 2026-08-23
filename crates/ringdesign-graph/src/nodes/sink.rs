//! Sinks: the verdict, the builds, the reports, and the files. A SandRing
//! graph's file-writing sinks are judged first and refuse a ring that will
//! not release; Free mode adds the mesh verdict for what the field cannot
//! judge.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ringdesign_core::castability::{self, FieldReport, Verdict, attributed_field_report};
use ringdesign_core::mesh::{BuildParams, Report};
use ringdesign_core::refine::RefineParams;
use ringdesign_core::stones::StonesReport;
use ringdesign_core::{AlphaLibrary, Mesh, RingDesign, dfm, gltf, library, metal, refine, render, spec, stl, threemf};

use crate::eval::{FIELD_PROFILE_STEPS, FIELD_THETA_STEPS, OUTPUT_DESIGN_PIN, OUTPUT_KIND};
use crate::graph::{Mode, Node};
use crate::registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry, Widget};
use crate::value::{Value, ValueKind};

fn design_of(i: &Inputs, pin: &str) -> Result<Arc<RingDesign>, NodeError> {
    match i.get(pin) {
        Value::Design(d) => Ok(d.clone()),
        other => Err(NodeError::input(pin, format!("expected a design, got {}", other.summary()))),
    }
}

fn mesh_of(i: &Inputs, pin: &str) -> Result<Arc<Mesh>, NodeError> {
    match i.get(pin) {
        Value::Mesh(m) => Ok(m.clone()),
        other => Err(NodeError::input(pin, format!("expected a mesh, got {}", other.summary()))),
    }
}

/// The library with the design's own sources baked, when it has any.
fn baked<'a>(design: &RingDesign, lib: &'a AlphaLibrary) -> std::borrow::Cow<'a, AlphaLibrary> {
    let has_sources = !(design.texts.is_empty() && design.svgs.is_empty() && design.drawn.is_empty() && design.recipes.is_empty() && design.embedded.is_empty());
    if has_sources {
        let mut l = lib.clone();
        design.unpack_embedded(&mut l);
        design.bake_all(&mut l);
        std::borrow::Cow::Owned(l)
    } else {
        std::borrow::Cow::Borrowed(lib)
    }
}

fn field_for(design: &RingDesign, lib: &AlphaLibrary, theta: usize, profile: usize) -> FieldReport {
    let lib = baked(design, lib);
    attributed_field_report(design, &lib, &design.draft, theta, profile)
}

/// The verdict a file-writing sink is judged by in SandRing mode: the
/// wired field report, else one computed from the wired design.
fn judge(ctx: &EvalCtx<'_>, i: &Inputs, what: &str) -> Result<Option<Arc<FieldReport>>, NodeError> {
    if ctx.mode != Mode::SandRing {
        return Ok(None);
    }
    let field = match i.get("field") {
        Value::Field(f) => f.clone(),
        Value::Null => match i.get("design") {
            Value::Design(d) => Arc::new(field_for(d, ctx.lib, FIELD_THETA_STEPS, FIELD_PROFILE_STEPS)),
            Value::Null => return Err(NodeError::new(format!("a SandRing {what} is judged first: wire the design or its field verdict"))),
            other => return Err(NodeError::input("design", format!("expected a design, got {}", other.summary()))),
        },
        other => return Err(NodeError::input("field", format!("expected a field report, got {}", other.summary()))),
    };
    if field.verdict == Verdict::NotCastable {
        let why = field.notes.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        return Err(NodeError::new(format!("refused: this ring will not release from a two-part sand mould ({why})")));
    }
    Ok(Some(field))
}

fn path_of(i: &Inputs, pin: &str) -> Result<PathBuf, NodeError> {
    let p = i.text(pin)?.trim().to_string();
    if p.is_empty() {
        return Err(NodeError::input(pin, "a file path is needed"));
    }
    let path = PathBuf::from(p);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| NodeError::input(pin, format!("cannot create {}: {e}", parent.display())))?;
        }
    }
    Ok(path)
}

fn output(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, OUTPUT_DESIGN_PIN)?;
    Ok(Outputs::one("design", Value::Design(d)))
}

fn field_verdict(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let theta = i.int("theta_steps")?.clamp(16, 4096) as usize;
    let profile = i.int("profile_steps")?.clamp(8, 2048) as usize;
    let f = field_for(&d, ctx.lib, theta, profile);
    let notes: Vec<Value> = f.notes.iter().map(|n| Value::Text(n.clone())).collect();
    Ok(Outputs::one("field", Value::Field(Arc::new(f.clone())))
        .with("verdict", f.verdict.label())
        .with("castable", f.verdict != Verdict::NotCastable)
        .with("undercut_pct", f.undercut_fraction() * 100.0)
        .with("worst_draft_deg", f.worst_draft_deg)
        .with("thinnest_wall_mm", f.thinnest_wall_mm)
        .with("parting_z_mm", f.parting_z_mm)
        .with("notes", notes))
}

fn gate(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let field = match i.get("field") {
        Value::Field(f) => f.clone(),
        Value::Null => Arc::new(field_for(&d, ctx.lib, FIELD_THETA_STEPS, FIELD_PROFILE_STEPS)),
        other => return Err(NodeError::input("field", format!("expected a field report, got {}", other.summary()))),
    };
    let allow_marginal = i.bool("allow_marginal")?;
    let ok = match field.verdict {
        Verdict::Castable => true,
        Verdict::Marginal => allow_marginal,
        Verdict::NotCastable => false,
    };
    if !ok {
        let why = field.notes.iter().take(3).cloned().collect::<Vec<_>>().join("; ");
        return Err(NodeError::new(format!("{}: {why}", field.verdict.label())));
    }
    Ok(Outputs::one("design", Value::Design(d)).with("field", Value::Field(field)))
}

fn report_outputs(mut o: Outputs, mesh: Mesh, report: &Report) -> Result<Outputs, NodeError> {
    let json = serde_json::to_value(report).map_err(|e| NodeError::new(e.to_string()))?;
    let weights: Vec<Value> = report
        .metals
        .iter()
        .map(|w| Value::Json(Arc::new(serde_json::json!({"metal": w.metal, "grams": w.grams, "dwt": w.dwt}))))
        .collect();
    o = o
        .with("triangles", report.validation.triangle_count as i64)
        .with("watertight", report.validation.watertight)
        .with("volume_mm3", report.volume_mm3)
        .with("surface_area_mm2", report.surface_area_mm2)
        .with("weights", weights)
        .with("report", json)
        .with("mesh", Value::Mesh(Arc::new(mesh)));
    Ok(o)
}

fn build(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let mut params = d.build;
    let preset = i.text("preset")?;
    let (_, theta, profile) = BuildParams::PRESETS
        .iter()
        .find(|(n, _, _)| n.eq_ignore_ascii_case(preset))
        .ok_or_else(|| NodeError::input("preset", format!("{preset:?} is not one of {:?}", BuildParams::PRESETS.iter().map(|p| p.0).collect::<Vec<_>>())))?;
    params.theta_steps = *theta;
    params.profile_steps = *profile;
    if let Some(t) = i.get("theta_steps").as_int() {
        params.theta_steps = t.clamp(16, 8192) as usize;
    }
    if let Some(p) = i.get("profile_steps").as_int() {
        params.profile_steps = p.clamp(8, 4096) as usize;
    }
    params.refine = None;
    params.soften_mm = i.number("soften_mm")?.max(0.0);
    let lib = baked(&d, ctx.lib);
    let out = ringdesign_core::build(&d, &lib, params);
    report_outputs(Outputs::default(), out.mesh, &out.report)
}

fn refine_build(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let preset = i.text("preset")?;
    let mut params = RefineParams::preset(preset)
        .ok_or_else(|| NodeError::input("preset", format!("{preset:?} is not one of {:?}", RefineParams::PRESETS.iter().map(|p| p.0).collect::<Vec<_>>())))?;
    if let Some(t) = i.get("tolerance_mm").as_number() {
        params.tolerance_mm = t.clamp(0.002, 1.0);
    }
    if let Some(t) = i.get("normal_tolerance_deg").as_number() {
        params.normal_tolerance_deg = t.clamp(1.0, 60.0);
    }
    let lib = baked(&d, ctx.lib);
    let out = refine::build(&d, &lib, params, d.build.min_wall_mm);
    let stats = serde_json::to_value(&out.stats).map_err(|e| NodeError::new(e.to_string()))?;
    let v = out.mesh.validate();
    Ok(Outputs::one("stats", stats)
        .with("triangles", v.triangle_count as i64)
        .with("watertight", v.watertight)
        .with("volume_mm3", out.mesh.volume_mm3())
        .with("surface_area_mm2", out.mesh.surface_area_mm2())
        .with("relief", serde_json::json!([out.relief.0, out.relief.1]))
        .with("mesh", Value::Mesh(Arc::new(out.mesh))))
}

fn stones(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let parting = match i.get("field") {
        Value::Field(f) => f.parting_z_mm,
        _ => field_for(&d, ctx.lib, 96, 64).parting_z_mm,
    };
    let r = castability_stones(&d, parting);
    let warnings: Vec<Value> = r.seats.iter().flat_map(|s| s.warnings.iter().map(|w| Value::Text(w.clone()))).collect();
    Ok(Outputs::one("stones", Value::Stones(Arc::new(r.clone())))
        .with("count", i64::from(r.stone_count))
        .with("carats", r.total_carats)
        .with("tight_pairs", r.tight_pairs as i64)
        .with("crowding_note", r.crowding_note().unwrap_or_default())
        .with("warnings", warnings))
}

fn castability_stones(d: &RingDesign, parting: f64) -> StonesReport {
    ringdesign_core::stones::report(d, parting).unwrap_or_default()
}

fn dfm_findings(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let lib = baked(&d, ctx.lib);
    let f = dfm::findings_in(&d, &lib);
    let items: Vec<Value> = f
        .iter()
        .map(|x| Value::Json(Arc::new(serde_json::json!({"layer": x.layer, "label": x.label, "message": x.message}))))
        .collect();
    let summary = if f.is_empty() { "no DFM findings".to_string() } else { f.iter().map(|x| format!("{}: {}", x.label, x.message)).collect::<Vec<_>>().join("\n") };
    Ok(Outputs::one("findings", items).with("count", f.len() as i64).with("summary", summary))
}

fn sheet(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let d = design_of(i, "design")?;
    let lib = baked(&d, ctx.lib);
    let field = match i.get("field") {
        Value::Field(f) => (**f).clone(),
        _ => attributed_field_report(&d, &lib, &d.draft, FIELD_THETA_STEPS, FIELD_PROFILE_STEPS),
    };
    let report = {
        let mut params = d.build;
        params.theta_steps = 192;
        params.profile_steps = 96;
        params.refine = None;
        ringdesign_core::build(&d, &lib, params).report
    };
    let stones = match i.get("stones") {
        Value::Stones(s) => Some((**s).clone()),
        _ => ringdesign_core::stones::report(&d, field.parting_z_mm),
    };
    let findings = dfm::findings_in(&d, &lib);
    let html = spec::html(&d, &report, &field, stones.as_ref(), &findings, i.text("provenance")?);
    Ok(Outputs::one("html", html))
}

fn write_text(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let path = path_of(i, "path")?;
    let text = i.text("text")?;
    std::fs::write(&path, text).map_err(|e| NodeError::input("path", format!("cannot write {}: {e}", path.display())))?;
    Ok(Outputs::one("path", path.display().to_string()).with("bytes", text.len() as i64))
}

fn pattern_path(path: &Path, metal: &str) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "ring".into());
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let slug: String = metal.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    path.with_file_name(format!("{stem}-pattern-{slug}{ext}"))
}

fn export(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    judge(ctx, i, "export")?;
    let mesh = mesh_of(i, "mesh")?;
    let mut path = path_of(i, "path")?;
    let name = i.text("name")?.to_string();
    let name = if name.trim().is_empty() { "ring".to_string() } else { name };
    let format = i.text("format")?.to_lowercase();
    let shrink = i.text("shrink_metal")?.trim().to_string();
    let scaled;
    let mesh: &Mesh = if shrink.is_empty() || shrink.eq_ignore_ascii_case("none") {
        &mesh
    } else {
        let m = metal::find(&shrink).ok_or_else(|| NodeError::input("shrink_metal", format!("{shrink:?} is not a metal in the table")))?;
        scaled = mesh.scaled(metal::pattern_scale(m.shrink_pct));
        path = pattern_path(&path, m.name);
        &scaled
    };
    if path.extension().is_none() {
        path.set_extension(&format);
    }
    let size_label = i.text("size_label")?;
    let bytes = match format.as_str() {
        "stl" => stl::write_stl(&path, mesh, &name),
        "obj" => stl::write_obj(&path, mesh, &name),
        "ply" => stl::write_ply(&path, mesh, &name),
        "3mf" => threemf::write_3mf(&path, mesh, &name, size_label),
        "glb" => gltf::write_glb(&path, mesh, &name, tint_of(i.text("tint")?)),
        other => return Err(NodeError::input("format", format!("{other:?} is not stl, obj, ply, 3mf or glb"))),
    }
    .map_err(|e| NodeError::input("path", format!("{}: {e:#}", path.display())))?;
    Ok(Outputs::one("path", path.display().to_string()).with("bytes", bytes as i64))
}

fn tint_of(name: &str) -> [f32; 3] {
    match name.to_lowercase().as_str() {
        "silver" | "sterling" | "platinum" | "white" => [0.90, 0.90, 0.92],
        "rose" => [0.93, 0.66, 0.56],
        "bronze" => [0.80, 0.55, 0.30],
        _ => [1.00, 0.78, 0.36],
    }
}

fn render_sink(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mesh = mesh_of(i, "mesh")?;
    let path = path_of(i, "path")?;
    let edge = i.int("edge")?.clamp(32, 4096) as usize;
    let tint = tint_of(i.text("tint")?);
    if i.bool("turntable")? {
        let frames = i.int("frames")?.clamp(4, 120) as usize;
        render::write_turntable_gif(&path, &mesh, frames, edge, tint)
    } else {
        render::write_png(&path, &mesh, i.number("yaw")?, i.number("pitch")?, edge, tint)
    }
    .map_err(|e| NodeError::input("path", format!("{}: {e:#}", path.display())))?;
    Ok(Outputs::one("path", path.display().to_string()))
}

fn save_design(ctx: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    judge(ctx, i, "save")?;
    let d = design_of(i, "design")?;
    let mut path = path_of(i, "path")?;
    if !path.to_string_lossy().ends_with(".ring.json") {
        let stem = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "ring".into());
        let stem = stem.trim_end_matches(".json").to_string();
        path.set_file_name(format!("{stem}.ring.json"));
    }
    if i.bool("embed_alphas")? {
        let lib = baked(&d, ctx.lib);
        library::save_design_embedded(&path, &d, &lib)
    } else {
        library::save_design(&path, &d)
    }
    .map_err(|e| NodeError::input("path", format!("{}: {e:#}", path.display())))?;
    Ok(Outputs::one("path", path.display().to_string()))
}

fn mesh_verdict(_: &mut EvalCtx<'_>, _: &Node, i: &Inputs) -> Result<Outputs, NodeError> {
    let mesh = mesh_of(i, "mesh")?;
    let d = design_of(i, "design")?;
    let r = castability::analyze(&mesh, &d.draft, d.inner_radius_mm());
    let notes: Vec<Value> = r.notes.iter().map(|n| Value::Text(n.clone())).collect();
    Ok(Outputs::one("verdict", r.verdict.label())
        .with("castable", r.verdict != Verdict::NotCastable)
        .with("undercut_pct", r.undercut_fraction() * 100.0)
        .with("worst_draft_deg", r.worst_draft_deg)
        .with("parting_z_mm", r.parting_z_mm)
        .with("vertical_faces", r.vertical as i64)
        .with("notes", notes))
}

pub fn register(reg: &mut Registry) {
    let metals: Vec<String> = std::iter::once("none".to_string()).chain(metal::METALS.iter().map(|m| m.name.to_string())).collect();
    let tints = vec!["gold".to_string(), "silver".into(), "rose".into(), "white".into(), "bronze".into()];
    let specs = [
        NodeSpec::new(OUTPUT_KIND, "Output", Category::Sink)
            .doc("The design this graph is for. A SandRing graph evaluates to it with its field verdict.")
            .input(PinSpec::item(OUTPUT_DESIGN_PIN, ValueKind::Design).doc("The finished design."))
            .output(PinSpec::item("design", ValueKind::Design).doc("The same design, for chaining."))
            .eval(output),
        NodeSpec::new("sink.field_verdict", "Field verdict", Category::Sink)
            .doc("The castability verdict from the true surface: undercut, worst draft, thinnest wall, with every undercut arc attributed to the layer that causes it.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("theta_steps", ValueKind::Int).default(FIELD_THETA_STEPS as i64).doc("Samples round the ring."))
            .input(PinSpec::item("profile_steps", ValueKind::Int).default(FIELD_PROFILE_STEPS as i64).doc("Samples across the section."))
            .output(PinSpec::item("field", ValueKind::Field).doc("The report."))
            .output(PinSpec::item("verdict", ValueKind::Text).doc("Castable, Castable with care, or Will not release."))
            .output(PinSpec::item("castable", ValueKind::Bool).doc("Whether it releases at all."))
            .output(PinSpec::item("undercut_pct", ValueKind::Number).doc("Undercut share of the surface, %."))
            .output(PinSpec::item("worst_draft_deg", ValueKind::Number).doc("The worst draft angle."))
            .output(PinSpec::item("thinnest_wall_mm", ValueKind::Number).doc("The thinnest outer-to-bore wall, mm."))
            .output(PinSpec::item("parting_z_mm", ValueKind::Number).doc("Where the mould parts."))
            .output(PinSpec::list("notes", ValueKind::Text).doc("What the report has to say."))
            .eval(field_verdict),
        NodeSpec::new("gate.castable", "Castable gate", Category::Sink)
            .doc("Passes the design on only if it releases; otherwise this item fails with the verdict's notes, and nothing downstream runs on it.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("field", ValueKind::Field).optional().doc("Its field report; computed if unset."))
            .input(PinSpec::item("allow_marginal", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Let 'castable with care' through."))
            .output(PinSpec::item("design", ValueKind::Design).doc("The design, judged."))
            .output(PinSpec::item("field", ValueKind::Field).doc("The report it was judged by."))
            .eval(gate),
        NodeSpec::new("sink.build", "Build mesh", Category::Sink)
            .doc("The swept mesh at a preset resolution: watertight by construction, with its report and weights.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::select("preset", BuildParams::PRESETS.iter().map(|p| p.0.to_string()).collect()).default("Preview").doc("Resolution preset."))
            .input(PinSpec::item("theta_steps", ValueKind::Int).optional().doc("Override: steps round the ring."))
            .input(PinSpec::item("profile_steps", ValueKind::Int).optional().doc("Override: steps across the section."))
            .input(PinSpec::item("soften_mm", ValueKind::Number).default(0.0).doc("As-cast softening radius, mm; 0 for true geometry."))
            .output(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .output(PinSpec::item("triangles", ValueKind::Int).doc("Triangle count."))
            .output(PinSpec::item("watertight", ValueKind::Bool).doc("Whether validation found no open or non-manifold edges."))
            .output(PinSpec::item("volume_mm3", ValueKind::Number).doc("Volume, mm³."))
            .output(PinSpec::item("surface_area_mm2", ValueKind::Number).doc("Surface area, mm²."))
            .output(PinSpec::list("weights", ValueKind::Json).doc("Weight per metal: {metal, grams, dwt}."))
            .output(PinSpec::item("report", ValueKind::Json).doc("The whole build report."))
            .eval(build),
        NodeSpec::new("sink.refine", "Refine mesh", Category::Sink)
            .doc("The refined mesh: a tolerance instead of a step count, anisotropic, watertight by construction.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::select("preset", RefineParams::PRESETS.iter().map(|p| p.0.to_string()).collect()).default("Draft").doc("Tolerance preset."))
            .input(PinSpec::item("tolerance_mm", ValueKind::Number).optional().doc("Override: position tolerance, mm."))
            .input(PinSpec::item("normal_tolerance_deg", ValueKind::Number).optional().doc("Override: slope tolerance, degrees."))
            .output(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .output(PinSpec::item("triangles", ValueKind::Int).doc("Triangle count."))
            .output(PinSpec::item("watertight", ValueKind::Bool).doc("Whether validation found no open or non-manifold edges."))
            .output(PinSpec::item("volume_mm3", ValueKind::Number).doc("Volume, mm³."))
            .output(PinSpec::item("surface_area_mm2", ValueKind::Number).doc("Surface area, mm²."))
            .output(PinSpec::item("relief", ValueKind::Json).doc("[min, max] relief over the bare band, mm."))
            .output(PinSpec::item("stats", ValueKind::Json).doc("The refinement statistics."))
            .eval(refine_build),
        NodeSpec::new("sink.stones", "Stones report", Category::Sink)
            .doc("The bench check for every seat: footing, clearance, pavilion room, bridges, carats, and the pairwise crowding census.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("field", ValueKind::Field).optional().doc("Its field report, for the parting plane; computed coarsely if unset."))
            .output(PinSpec::item("stones", ValueKind::Stones).doc("The report."))
            .output(PinSpec::item("count", ValueKind::Int).doc("Stones."))
            .output(PinSpec::item("carats", ValueKind::Number).doc("Total estimated carats."))
            .output(PinSpec::item("tight_pairs", ValueKind::Int).doc("Pairs under the fill floor."))
            .output(PinSpec::item("crowding_note", ValueKind::Text).doc("The tightest pair, in words."))
            .output(PinSpec::list("warnings", ValueKind::Text).doc("Per-seat warnings."))
            .eval(stones),
        NodeSpec::new("sink.dfm", "DFM findings", Category::Sink)
            .doc("Each layer's finest feature against the sand's detail floor.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .output(PinSpec::list("findings", ValueKind::Json).doc("Findings: {layer, label, message}."))
            .output(PinSpec::item("count", ValueKind::Int).doc("How many."))
            .output(PinSpec::item("summary", ValueKind::Text).doc("One line each."))
            .eval(dfm_findings),
        NodeSpec::new("sink.sheet", "Casting sheet", Category::Sink)
            .doc("The printable casting sheet as HTML: dimensions, weights, the verdict, DFM, stones, provenance. Write it with sink.write_text.")
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("field", ValueKind::Field).optional().doc("Its field report; computed if unset."))
            .input(PinSpec::item("stones", ValueKind::Stones).optional().doc("Its stones report; computed if unset."))
            .input(PinSpec::item("provenance", ValueKind::Text).default("ringdesign-graph").widget(Widget::TextLine).doc("Who made it."))
            .output(PinSpec::item("html", ValueKind::Text).doc("The page."))
            .eval(sheet),
        NodeSpec::new("sink.write_text", "Write text file", Category::Sink)
            .doc("Write text to a file — the sheet, a manifest, a note.")
            .side_effect()
            .input(PinSpec::item("path", ValueKind::Text).default("").widget(Widget::TextLine).doc("Where."))
            .input(PinSpec::item("text", ValueKind::Text).default("").doc("What."))
            .output(PinSpec::item("path", ValueKind::Text).doc("The path written."))
            .output(PinSpec::item("bytes", ValueKind::Int).doc("Bytes written."))
            .eval(write_text),
        NodeSpec::new("sink.export", "Export mesh", Category::Sink)
            .doc("Write the mesh as STL, OBJ, PLY, 3MF or GLB. In SandRing mode the ring is judged first and a ring that will not release is refused; a shrink metal scales it to a pattern and names the file as one.")
            .side_effect()
            .input(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .input(PinSpec::item("path", ValueKind::Text).default("").widget(Widget::TextLine).doc("Where; the extension follows the format if missing."))
            .input(PinSpec::select("format", vec!["stl".into(), "obj".into(), "ply".into(), "3mf".into(), "glb".into()]).default("stl").doc("The file format."))
            .input(PinSpec::item("name", ValueKind::Text).default("").widget(Widget::TextLine).doc("The object's name inside the file."))
            .input(PinSpec::item("size_label", ValueKind::Text).default("").widget(Widget::TextLine).doc("The size, for 3MF metadata."))
            .input(PinSpec::select("shrink_metal", metals).default("none").doc("Scale to a pattern for this metal's shrink."))
            .input(PinSpec::select("tint", tints.clone()).default("gold").doc("GLB material tint."))
            .input(PinSpec::item("design", ValueKind::Design).optional().doc("The design, for the SandRing verdict."))
            .input(PinSpec::item("field", ValueKind::Field).optional().doc("Its field report, if already computed."))
            .output(PinSpec::item("path", ValueKind::Text).doc("The path written."))
            .output(PinSpec::item("bytes", ValueKind::Int).doc("Bytes written."))
            .eval(export),
        NodeSpec::new("sink.render", "Render", Category::Sink)
            .doc("A software-rendered PNG hero frame, or a looping turntable GIF.")
            .side_effect()
            .input(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .input(PinSpec::item("path", ValueKind::Text).default("").widget(Widget::TextLine).doc("Where."))
            .input(PinSpec::item("yaw", ValueKind::Number).default(0.6).doc("Camera yaw, radians."))
            .input(PinSpec::item("pitch", ValueKind::Number).default(1.12).doc("Camera pitch, radians."))
            .input(PinSpec::item("edge", ValueKind::Int).default(800i64).doc("Image edge, pixels."))
            .input(PinSpec::select("tint", tints).default("gold").doc("Metal tint."))
            .input(PinSpec::item("turntable", ValueKind::Bool).default(false).widget(Widget::Checkbox).doc("A spinning GIF instead of a frame."))
            .input(PinSpec::item("frames", ValueKind::Int).default(36i64).doc("Turntable frames."))
            .output(PinSpec::item("path", ValueKind::Text).doc("The path written."))
            .eval(render_sink),
        NodeSpec::new("sink.save_design", "Save design", Category::Sink)
            .doc("Write the design file (.ring.json), with its alphas embedded so it survives moving machines. In SandRing mode a ring that will not release is refused.")
            .side_effect()
            .input(PinSpec::item("design", ValueKind::Design).doc("The design."))
            .input(PinSpec::item("path", ValueKind::Text).default("").widget(Widget::TextLine).doc("Where."))
            .input(PinSpec::item("embed_alphas", ValueKind::Bool).default(true).widget(Widget::Checkbox).doc("Embed referenced alphas as PNG."))
            .input(PinSpec::item("field", ValueKind::Field).optional().doc("Its field report, if already computed."))
            .output(PinSpec::item("path", ValueKind::Text).doc("The path written."))
            .eval(save_design),
        NodeSpec::new("sink.mesh_verdict", "Mesh verdict", Category::Sink)
            .free_only()
            .doc("Castability read off a mesh's face normals — for meshes the field cannot judge. A bore reads as a vertical wall, never an undercut.")
            .input(PinSpec::item("mesh", ValueKind::Mesh).doc("The mesh."))
            .input(PinSpec::item("design", ValueKind::Design).doc("The design, for the sand's numbers and the bore."))
            .output(PinSpec::item("verdict", ValueKind::Text).doc("The verdict."))
            .output(PinSpec::item("castable", ValueKind::Bool).doc("Whether it releases."))
            .output(PinSpec::item("undercut_pct", ValueKind::Number).doc("Undercut share, %."))
            .output(PinSpec::item("worst_draft_deg", ValueKind::Number).doc("Worst draft."))
            .output(PinSpec::item("parting_z_mm", ValueKind::Number).doc("Where it parts."))
            .output(PinSpec::item("vertical_faces", ValueKind::Int).doc("Faces read as vertical walls."))
            .output(PinSpec::list("notes", ValueKind::Text).doc("Notes."))
            .eval(mesh_verdict),
    ];
    for s in specs {
        reg.register(s).expect("unique");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets, evaluate_design};
    use crate::graph::{Graph, NodeId};
    use crate::value::Literal;

    fn squared(g: &mut Graph) -> NodeId {
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("Flat".into())).unwrap();
        g.set_input(p, "width_mm", Literal::Number(6.0)).unwrap();
        g.set_input(p, "thickness_mm", Literal::Number(2.0)).unwrap();
        g.set_input(p, "flatten_sides", Literal::Bool(true)).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        d
    }

    /// A tiling on the crest of a dome at a relief no draft survives.
    fn locked(g: &mut Graph) -> NodeId {
        let p = g.add("band.profile").unwrap();
        g.set_input(p, "style", Literal::Text("HalfRound".into())).unwrap();
        let d = g.add("design.new").unwrap();
        g.connect(p, "profile", d, "profile").unwrap();
        let t = g.add("layer.tiling").unwrap();
        g.set_input(t, "height_mm", Literal::Number(0.8)).unwrap();
        g.set_input(t, "v_center_mm", Literal::Number(0.0)).unwrap();
        g.set_input(t, "repeats_around", Literal::Int(24)).unwrap();
        let st = g.add("stack").unwrap();
        g.connect(t, "layer", st, "entries").unwrap();
        let asm = g.add("design.assemble").unwrap();
        g.connect(d, "design", asm, "design").unwrap();
        g.connect(st, "stack", asm, "stack").unwrap();
        asm
    }

    fn run(g: &Graph, targets: Targets) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, &Registry::builtin(), &AlphaLibrary::builtin(), 0, targets)
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ringdesign-graph-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn the_verdict_and_the_gate_read_the_field() {
        let mut g = Graph::default();
        let good = squared(&mut g);
        let bad = locked(&mut g);
        let fv = g.add("sink.field_verdict").unwrap();
        g.connect(good, "design", fv, "design").unwrap();
        let fb = g.add("sink.field_verdict").unwrap();
        g.connect(bad, "design", fb, "design").unwrap();
        let gate_ok = g.add("gate.castable").unwrap();
        g.connect(good, "design", gate_ok, "design").unwrap();
        g.connect(fv, "field", gate_ok, "field").unwrap();
        let gate_bad = g.add("gate.castable").unwrap();
        g.connect(bad, "design", gate_bad, "design").unwrap();
        let after = g.add("design.info").unwrap();
        g.connect(gate_bad, "design", after, "design").unwrap();
        let r = run(&g, Targets::AllPure);
        assert_eq!(r.value(fv, "castable"), Some(&Value::Bool(true)));
        assert_eq!(r.value(fb, "castable"), Some(&Value::Bool(false)), "{:?}", r.value(fb, "notes"));
        assert!(r.value(fb, "undercut_pct").unwrap().as_number().unwrap() > 1.0);
        assert!(matches!(r.value(gate_ok, "design"), Some(Value::Design(_))));
        assert!(r.status[&gate_bad].failed());
        assert!(r.status[&gate_bad].errors[0].1.starts_with("Will not release"), "{:?}", r.status[&gate_bad].errors);
        assert!(r.status[&after].failed(), "nothing downstream runs on a refused ring");
    }

    #[test]
    fn builds_and_reports_come_off_the_design() {
        let mut g = Graph::default();
        let d = squared(&mut g);
        let gem = g.add("gem.calibrated").unwrap();
        g.set_input(gem, "w_mm", Literal::Number(2.0)).unwrap();
        let seat = g.add("layer.seat.fit").unwrap();
        g.connect(gem, "gem", seat, "gem").unwrap();
        let st = g.add("stack").unwrap();
        g.connect(seat, "layer", st, "entries").unwrap();
        let asm = g.add("design.assemble").unwrap();
        g.connect(d, "design", asm, "design").unwrap();
        g.connect(st, "stack", asm, "stack").unwrap();
        let b = g.add("sink.build").unwrap();
        g.connect(asm, "design", b, "design").unwrap();
        g.set_input(b, "preset", Literal::Text("Draft".into())).unwrap();
        let rf = g.add("sink.refine").unwrap();
        g.connect(asm, "design", rf, "design").unwrap();
        g.set_input(rf, "preset", Literal::Text("Coarse".into())).unwrap();
        let s = g.add("sink.stones").unwrap();
        g.connect(asm, "design", s, "design").unwrap();
        let dfm_ = g.add("sink.dfm").unwrap();
        g.connect(asm, "design", dfm_, "design").unwrap();
        let sheet_ = g.add("sink.sheet").unwrap();
        g.connect(asm, "design", sheet_, "design").unwrap();
        g.connect(s, "stones", sheet_, "stones").unwrap();
        let r = run(&g, Targets::AllPure);
        assert!(!r.any_failed(), "{:?}", r.notes(&g));
        assert_eq!(r.value(b, "triangles"), Some(&Value::Int(192 * 96 * 2)));
        assert_eq!(r.value(b, "watertight"), Some(&Value::Bool(true)));
        assert!(r.value(b, "volume_mm3").unwrap().as_number().unwrap() > 50.0);
        assert_eq!(r.value(rf, "watertight"), Some(&Value::Bool(true)));
        assert!(r.value(rf, "triangles").unwrap().as_int().unwrap() > 1000);
        assert_eq!(r.value(s, "count"), Some(&Value::Int(1)));
        assert!(r.value(s, "carats").unwrap().as_number().unwrap() > 0.0);
        assert!(r.value(dfm_, "count").unwrap().as_int().unwrap() >= 0);
        let html = r.value(sheet_, "html").unwrap().as_text().unwrap().to_string();
        assert!(html.contains("<html") && html.contains("Untitled"), "{}", &html[..200.min(html.len())]);
    }

    #[test]
    fn file_sinks_run_on_demand_and_sandring_refuses_a_locked_ring() {
        let mut g = Graph::default();
        let good = squared(&mut g);
        let bad = locked(&mut g);
        let bg = g.add("sink.build").unwrap();
        g.connect(good, "design", bg, "design").unwrap();
        g.set_input(bg, "preset", Literal::Text("Draft".into())).unwrap();
        let bb = g.add("sink.build").unwrap();
        g.connect(bad, "design", bb, "design").unwrap();
        g.set_input(bb, "preset", Literal::Text("Draft".into())).unwrap();
        let ok_path = tmp("good.stl");
        let bad_path = tmp("bad.stl");
        let ex_ok = g.add("sink.export").unwrap();
        g.connect(bg, "mesh", ex_ok, "mesh").unwrap();
        g.connect(good, "design", ex_ok, "design").unwrap();
        g.set_input(ex_ok, "path", Literal::Text(ok_path.display().to_string())).unwrap();
        g.set_input(ex_ok, "shrink_metal", Literal::Text(metal::METALS[0].name.into())).unwrap();
        let ex_bad = g.add("sink.export").unwrap();
        g.connect(bb, "mesh", ex_bad, "mesh").unwrap();
        g.connect(bad, "design", ex_bad, "design").unwrap();
        g.set_input(ex_bad, "path", Literal::Text(bad_path.display().to_string())).unwrap();
        let unjudged = g.add("sink.export").unwrap();
        g.connect(bg, "mesh", unjudged, "mesh").unwrap();
        g.set_input(unjudged, "path", Literal::Text(tmp("unjudged.stl").display().to_string())).unwrap();
        let save = g.add("sink.save_design").unwrap();
        g.connect(good, "design", save, "design").unwrap();
        g.set_input(save, "path", Literal::Text(tmp("good").display().to_string())).unwrap();
        let png = g.add("sink.render").unwrap();
        g.connect(bg, "mesh", png, "mesh").unwrap();
        g.set_input(png, "path", Literal::Text(tmp("good.png").display().to_string())).unwrap();
        g.set_input(png, "edge", Literal::Int(64)).unwrap();
        let _ = std::fs::remove_file(&ok_path);
        let _ = std::fs::remove_file(&bad_path);

        let r = run(&g, Targets::AllPure);
        assert!(r.status[&ex_ok].skipped && r.status[&save].skipped && r.status[&png].skipped, "nothing writes unless asked");
        let r = run(&g, Targets::Everything);
        let written = r.value(ex_ok, "path").unwrap().as_text().unwrap().to_string();
        assert!(written.contains("-pattern-"), "a shrunk export is named as a pattern: {written}");
        assert!(std::fs::metadata(&written).unwrap().len() > 84);
        assert!(r.status[&ex_bad].failed());
        assert!(r.status[&ex_bad].errors[0].1.starts_with("refused"), "{:?}", r.status[&ex_bad].errors);
        assert!(!bad_path.exists(), "a refused export writes nothing");
        assert!(r.status[&unjudged].errors[0].1.contains("judged first"), "{:?}", r.status[&unjudged].errors);
        let saved = r.value(save, "path").unwrap().as_text().unwrap().to_string();
        assert!(saved.ends_with("good.ring.json"));
        let back = library::load_design(&saved).unwrap();
        assert_eq!(back.profile.width_mm, 6.0);
        assert!(std::fs::metadata(r.value(png, "path").unwrap().as_text().unwrap()).unwrap().len() > 100);

        // Free mode does not judge an export, and has the mesh verdict.
        g.mode = Mode::Free;
        let mv = g.add("sink.mesh_verdict").unwrap();
        g.connect(bg, "mesh", mv, "mesh").unwrap();
        g.connect(good, "design", mv, "design").unwrap();
        let r = run(&g, Targets::Everything);
        assert!(!r.status[&unjudged].failed(), "{:?}", r.status[&unjudged].errors);
        assert_eq!(r.value(mv, "castable"), Some(&Value::Bool(true)), "{:?}", r.value(mv, "notes"));
        assert!(r.value(mv, "vertical_faces").unwrap().as_int().unwrap() > 0, "the bore reads as vertical");
        g.mode = Mode::SandRing;
        let r = run(&g, Targets::AllPure);
        assert!(r.errors.iter().any(|e| e.message.contains("does not run in SandRing")), "{:?}", r.errors);
    }

    #[test]
    fn the_output_sink_is_what_evaluate_design_reads() {
        let mut g = Graph::default();
        let d = squared(&mut g);
        let out = g.add(OUTPUT_KIND).unwrap();
        g.connect(d, "design", out, OUTPUT_DESIGN_PIN).unwrap();
        let res = evaluate_design(&mut Evaluator::new(), &g, &Registry::builtin(), &AlphaLibrary::builtin(), 0).unwrap();
        assert_eq!(res.design.profile.width_mm, 6.0);
        assert_ne!(res.field.verdict, Verdict::NotCastable);
        assert!(matches!(res.report.value(out, "design"), Some(Value::Design(_))));
    }
}
