//! OpenCascade in the CAD pane: fillets, shells and the torus-and-cylinder junction made by a worker process and committed as one stored-mesh feature.
use crate::app::RingDesignerApp;
use crate::occt_embedded;
use crate::theme;
use ringdesign_core::cad::{BuildCtx, EdgeRef, EvaluatedComponent, Feature, Operation, Placement, edge_signature, edit::CadEdit, evaluate_with};
use ringdesign_core::sketch::Id;
use ringdesign_occt::client::Locator;
use ringdesign_occt::parts;
use ringdesign_occt::protocol::{Request, Response, Tolerance};
use ringdesign_workbench::{icons::Icon, viewport::Sel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Longest a worker may run before it is stopped.
const TIMEOUT: Duration = Duration::from_secs(60);

/// What the pane asked OpenCascade for and what came back.
#[derive(Default)]
struct Job {
    running: bool,
    /// The edit a finished run makes, or why it made none.
    outcome: Option<Result<CadEdit, String>>,
    message: Option<String>,
}
type Shared = Arc<Mutex<Job>>;

/// The part the Ring viewport has chosen edges of, with those edges; `None` unless they are all one part's.
fn chosen_edges(app: &RingDesignerApp) -> Option<(Id, Vec<usize>)> {
    let mut part = None;
    let mut edges = Vec::new();
    for item in &app.selection.items {
        let Sel::Edge { feature, edge } = item else { continue };
        if part.is_some_and(|p| p != *feature) {
            return None;
        }
        part = Some(*feature);
        edges.push(*edge as usize);
    }
    part.map(|p| (p, edges))
}

/// The part the Ring viewport has chosen faces of, with those faces; `None` unless they are all one part's.
fn chosen_faces(app: &RingDesignerApp) -> Option<(Id, Vec<u32>)> {
    let mut part = None;
    let mut faces = Vec::new();
    for item in &app.selection.items {
        let Sel::Face { feature, face } = item else { continue };
        if part.is_some_and(|p| p != *feature) {
            return None;
        }
        part = Some(*feature);
        faces.push(*face);
    }
    part.map(|p| (p, faces))
}

/// A point on face `face` of part `c`, in the frame the build seated the part by: the centroid of its first triangle.
fn face_point(c: &EvaluatedComponent, face: u32) -> Option<[f64; 3]> {
    let t = c.trace.tri_face.iter().position(|f| *f == face)?;
    let corners = c.mesh.faces.get(t)?.map(|v| c.trace.positions.get(v as usize).copied());
    let [Some(a), Some(b), Some(d)] = corners else { return None };
    let world: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + d[k]) / 3.0);
    Some(ringdesign_core::cad::pattern::inverse(&c.frame).point(world))
}

/// Whether `locator` finds a worker to run OpenCascade, embedded, beside the app or named.
pub fn available(locator: &Locator) -> bool {
    occt_embedded::missing(locator).is_none()
}

/// The torus and the cylinder a junction fuses: two chosen parts, a torus standing free and a cylinder, in either order.
fn chosen_junction(app: &RingDesignerApp) -> Option<(Id, Id)> {
    let doc = app.design.cad.as_ref()?;
    let parts: Vec<Id> = app.selection.items.iter().filter_map(|s| if let Sel::Part(id) = s { Some(*id) } else { None }).collect();
    let [a, b] = parts[..] else { return None };
    let is_torus = |id: Id| doc.feature(id).is_some_and(|f| matches!(f.operation, Operation::Torus { .. }) && f.component.placement == Placement::Free);
    let is_cylinder = |id: Id| doc.feature(id).is_some_and(|f| matches!(f.operation, Operation::Cylinder { .. }));
    match (is_torus(a) && is_cylinder(b), is_torus(b) && is_cylinder(a)) {
        (true, _) => Some((a, b)),
        (_, true) => Some((b, a)),
        _ => None,
    }
}

/// A STEP file's solids read by OpenCascade in a worker process, kept as one stored part at the top of the ring and joined.
pub fn import_step(path: &std::path::Path, locator: &Locator, cancel: &std::sync::atomic::AtomicBool) -> Result<(Feature, Vec<String>), String> {
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("The file").to_string();
    let step = std::fs::read_to_string(path).map_err(|e| format!("{file} could not be read: {e}"))?;
    let request = Request::Import { step, tolerance: Tolerance::EXPORT };
    let response = occt_embedded::worker(locator, cancel)?.run_cancellable(&request, TIMEOUT, cancel).map_err(|e| e.to_string())?;
    let (solids, notes) = match response {
        Response::Done { solids, notes, .. } => (solids, notes),
        Response::Refused { message } => return Err(format!("{file}: {message}")),
    };
    let meshes: Vec<&ringdesign_core::cad::stored::Packed> = solids.iter().map(|s| &s.mesh).collect();
    let merged = ringdesign_core::cad::stored::merged(&meshes).map_err(|e| format!("{file}: {e:#}"))?;
    let params = serde_json::json!({ "file": file, "tolerance": Tolerance::EXPORT });
    let feature = ringdesign_core::cad::stored::imported_packed(&file, "occt", params, merged).map_err(|e| format!("{e:#}"))?;
    Ok((feature, notes))
}

/// Whether the core's reading of a STEP file left a solid for OpenCascade or refused the file.
pub fn leaves_for_occt(core: &Result<(Feature, Vec<String>), String>) -> bool {
    core.as_ref().map_or(true, |(_, notes)| notes.iter().any(|n| n.contains("OpenCascade")))
}

/// A STEP file after the core has read it: OpenCascade reads it whole when the core left a solid for it or refused it and `locator` finds a worker, else the core's reading stands.
pub fn after_core(path: &std::path::Path, locator: &Locator, core: Result<(Feature, Vec<String>), String>, cancel: &std::sync::atomic::AtomicBool) -> Result<(Feature, Vec<String>), String> {
    if !leaves_for_occt(&core) || !available(locator) {
        return core;
    }
    match (import_step(path, locator, cancel), core) {
        (Ok(read), _) => Ok(read),
        (Err(why), Ok((feature, mut notes))) => {
            notes.push(format!("OpenCascade did not read the rest: {why}"));
            Ok((feature, notes))
        }
        (Err(why), Err(core)) => Err(format!("{core}; OpenCascade did not read it either: {why}")),
    }
}

/// Why chosen part `id` has no build to hand OpenCascade: a later feature consumed it, or it is not built yet.
fn unbuilt(doc: Option<&ringdesign_core::cad::Document>, id: Id, what: &str) -> String {
    let consumer = doc.and_then(|d| d.features.iter().find(|f| f.enabled && f.id != id && f.operation.consumes().contains(&id)));
    match consumer {
        Some(f) => format!("The chosen {what} is used up by {}, a later feature; suppress that feature to reach it", f.name),
        None => format!("The chosen {what} is not built yet"),
    }
}

/// The chosen part as built, when one is chosen.
fn chosen_part(app: &RingDesignerApp) -> Option<Id> {
    app.selection.items.iter().find_map(|s| match s {
        Sel::Part(id) | Sel::Edge { feature: id, .. } | Sel::Face { feature: id, .. } => Some(*id),
        _ => None,
    })
}

fn component(app: &RingDesignerApp, id: Id) -> Option<EvaluatedComponent> {
    app.build.as_ref()?.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == id).cloned()
}

/// Part `id` as built now, evaluated up to it on the band the build seated parts on when a later feature consumed it.
fn as_built(app: &RingDesignerApp, id: Id) -> Result<EvaluatedComponent, String> {
    if let Some(c) = component(app, id) {
        return Ok(c);
    }
    let mut rolled = app.design.clone();
    rolled.cad.as_mut().ok_or("The design has no parts")?.through = Some(id);
    let never = std::sync::atomic::AtomicBool::new(false);
    let band = app.build.as_ref().and_then(|b| b.band.clone());
    let ctx = match band.as_deref() {
        Some(mesh) => BuildCtx::new(&never).with_surface(mesh),
        None => BuildCtx::new(&never),
    };
    let e = evaluate_with(&rolled, &app.lib, app.preview_params, &ctx).map_err(|e| format!("{e:#}"))?;
    e.components.into_iter().find(|c| c.id == id).ok_or_else(|| format!("Part #{id} did not build"))
}

/// `request` run on a worker thread by the worker `locator` finds; its outcome becomes `edit` once the worker answers.
fn start(job: &Shared, ctx: &egui::Context, locator: &Locator, request: Request, edit: impl FnOnce(&ringdesign_occt::protocol::Built) -> CadEdit + Send + 'static) {
    let (job, ctx, locator) = (job.clone(), ctx.clone(), locator.clone());
    if let Ok(mut j) = job.lock() {
        j.running = true;
        j.message = Some(if occt_embedded::unpacks_first(&locator) { "Unpacking OpenCascade for its first use" } else { "OpenCascade is working" }.into());
    }
    std::thread::spawn(move || {
        let never = std::sync::atomic::AtomicBool::new(false);
        let outcome = occt_embedded::worker(&locator, &never).and_then(|w| w.run(&request, TIMEOUT).map_err(|e| e.to_string())).and_then(|response| match response {
            Response::Done { solids, .. } => solids.first().map(edit).ok_or_else(|| "OpenCascade built nothing".to_string()),
            Response::Refused { message } => Err(message),
        });
        if let Ok(mut j) = job.lock() {
            j.running = false;
            j.outcome = Some(outcome);
        }
        ctx.request_repaint();
    });
}

/// Edges of `source` a stored fillet names, found again by signature on the part as built now.
fn edges_again(source: &EvaluatedComponent, params: &serde_json::Value) -> Result<Vec<usize>, String> {
    let refs: Vec<EdgeRef> = serde_json::from_value(params.get("edges").cloned().unwrap_or_default()).map_err(|e| format!("The fillet's edges do not read: {e}"))?;
    let body = source.brep().ok_or("The source is not a kernel part")?;
    refs.iter()
        .map(|r| {
            let Some(expected) = &r.signature else { return Ok(r.ordinal) };
            let holds = |i: usize| edge_signature(body, i, &source.frame).is_some_and(|now| expected.matches(&now));
            if holds(r.ordinal) {
                return Ok(r.ordinal);
            }
            (0..body.edges.len()).find(|i| holds(*i)).ok_or_else(|| format!("Edge {} is no longer a {}; pick it again", r.ordinal, expected.describe()))
        })
        .collect()
}

/// The CAD pane's OpenCascade tools for the chosen part.
pub fn cad_pane(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let id = egui::Id::new("occt-pane");
    let job: Shared = ctx.data_mut(|d| d.get_temp_mut_or_default::<Shared>(id).clone());
    let mut radius = ctx.data_mut(|d| *d.get_temp_mut_or(id.with("radius"), 0.3_f64));
    let mut wall = ctx.data_mut(|d| *d.get_temp_mut_or(id.with("wall"), 0.6_f64));
    let finished = job.lock().ok().and_then(|mut j| j.outcome.take());
    match finished {
        Some(Ok(edit)) => {
            let message = match crate::cad_edit::apply(app, &[edit]) {
                Ok(applied) => applied.first().map_or_else(String::new, |a| a.label.clone()),
                Err(e) => e,
            };
            if let Ok(mut j) = job.lock() {
                j.message = Some(message);
            }
        }
        Some(Err(message)) => {
            app.set_status(message.clone());
            if let Ok(mut j) = job.lock() {
                j.message = Some(message);
            }
        }
        None => {}
    }
    let (running, message) = job.lock().map(|j| (j.running, j.message.clone())).unwrap_or_default();
    let chosen = chosen_edges(app);
    let target = chosen.as_ref().and_then(|(part, _)| component(app, *part));
    let faces = chosen_faces(app);
    let hollowed = faces.as_ref().and_then(|(part, _)| component(app, *part));
    let doc = app.design.cad.clone();
    // A chosen stored fillet whose source has changed since it was made is offered a rerun.
    let rerun = chosen_part(app).and_then(|id| {
        let f = doc.as_ref()?.feature(id)?.clone();
        let Operation::Stored { recipe, sources, .. } = &f.operation else { return None };
        (recipe.kernel == "occt" && recipe.op == "fillet" && sources.len() == 1 && recipe.digest != ringdesign_core::cad::stored::digest(doc.as_ref()?, sources)).then_some(f)
    });
    let missing = occt_embedded::missing(&app.occt);
    let has_torus = doc.as_ref().is_some_and(|d| d.features.iter().any(|f| matches!(f.operation, Operation::Torus { .. })));
    let junction = chosen_junction(app);
    let post = junction.and_then(|(_, cylinder)| component(app, cylinder));
    let (show_fillet, show_shell) = (chosen.is_some() || rerun.is_some(), faces.is_some());
    let canvas = app.cad.canvas();
    let corner = canvas.map_or_else(|| ui.max_rect().right_bottom() + egui::vec2(-8.0, -44.0), |c| c.right_bottom() - egui::vec2(8.0, 8.0));
    // Anchored at the canvas's bottom-right corner.
    egui::Area::new(ui.id().with("occt-pane")).order(egui::Order::Middle).fixed_pos(corner).pivot(egui::Align2::RIGHT_BOTTOM).show(&ctx, |ui| {
        egui::Frame::new().fill(theme::FLOAT.gamma_multiply(0.94)).corner_radius(4).inner_margin(6).show(ui, |ui| {
            // At most the canvas's width less the plate's gaps and margins.
            if let Some(c) = canvas {
                ui.set_max_width((c.width() - 28.0).max(96.0));
            }
            ui.horizontal_wrapped(|ui| {
                let about = missing.as_deref().unwrap_or("Each run starts a worker process of its own; a crash or a hang stops only it");
                ui.strong("OpenCascade").on_hover_text(about);
                if !(show_fillet || show_shell || has_torus) {
                    ui.weak(if missing.is_some() { "no worker in this build" } else { "choose a part's edges to fillet, or its faces to shell" }).on_hover_text(about);
                }
                if running {
                    ui.spinner();
                }
            });
            if show_fillet {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut radius).range(0.01..=5.0).speed(0.01).suffix(" mm").prefix("r "));
                    let why = match (&chosen, &target) {
                        _ if missing.is_some() => missing.clone(),
                        _ if running => Some("OpenCascade is already working".into()),
                        (None, _) => Some("Choose one part's edges in the Ring viewport: click an edge, Shift adds more".into()),
                        (Some((part, _)), None) => Some(unbuilt(doc.as_ref(), *part, "part")),
                        (Some(_), Some(c)) if c.brep().is_none() => Some("OpenCascade rounds a kernel part; this one is a mesh".into()),
                        _ => None,
                    };
                    let label = chosen.as_ref().map_or("Fillet with OpenCascade".to_string(), |(_, e)| format!("Fillet {} edge{} with OpenCascade", e.len(), if e.len() == 1 { "" } else { "s" }));
                    let clicked = ui
                        .add_enabled(why.is_none(), egui::Button::new((Icon::CadFillet.image(ui, 18.0), label)))
                        .on_hover_text("Round the chosen edges in OpenCascade, in a process of its own; the result is kept in the file as a mesh every build can show")
                        .on_disabled_hover_text(why.unwrap_or_default())
                        .clicked();
                    if let (true, Some((part, edges)), Some(c), Some(doc)) = (clicked, &chosen, &target, &doc) {
                        match parts::fillet(c, edges, radius, Tolerance::EXPORT) {
                            Ok((request, params)) => {
                                let (doc, part, request2) = (doc.clone(), *part, request.clone());
                                let settings = doc.feature(part).map(|f| f.component.clone()).unwrap_or_default();
                                start(&job, ui.ctx(), &app.occt, request, move |built| CadEdit::Add { feature: parts::stored_feature(&doc, &[part], &request2, params, built, settings), after: None });
                            }
                            Err(e) => app.set_status(format!("{e:#}")),
                        }
                    }
                    if let Some(f) = &rerun {
                        let again = ui
                            .add_enabled(!running && missing.is_none(), egui::Button::new((Icon::Rebuild.image(ui, 18.0), "Run again")))
                            .on_hover_text("Its source changed since OpenCascade made it: fillet the same edges of the part as it is now")
                            .on_disabled_hover_text(missing.as_deref().unwrap_or("OpenCascade is already working"));
                        if again.clicked() {
                            rerun_fillet(app, &job, ui.ctx(), f);
                        }
                    }
                });
            }
            if show_shell {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::DragValue::new(&mut wall).range(0.05..=5.0).speed(0.01).suffix(" mm").prefix("wall "));
                    let why = match (&faces, &hollowed) {
                        _ if missing.is_some() => missing.clone(),
                        _ if running => Some("OpenCascade is already working".into()),
                        (None, _) => Some("Choose the faces to leave open, all of one part: click a face, Shift adds more".into()),
                        (Some((part, _)), None) => Some(unbuilt(doc.as_ref(), *part, "part")),
                        (Some(_), Some(c)) if c.brep().is_none() => Some("OpenCascade hollows a kernel part; this one is a mesh".into()),
                        _ => None,
                    };
                    let label = faces.as_ref().map_or("Shell with OpenCascade".to_string(), |(_, f)| format!("Shell with OpenCascade, {} face{} open", f.len(), if f.len() == 1 { "" } else { "s" }));
                    let clicked = ui
                        .add_enabled(why.is_none(), egui::Button::new((Icon::CadShell.image(ui, 18.0), label)))
                        .on_hover_text("Hollow the chosen part to walls this thick in OpenCascade, the chosen faces left open; the result is kept in the file as a mesh every build can show")
                        .on_disabled_hover_text(why.unwrap_or_default())
                        .clicked();
                    if let (true, Some((part, open)), Some(c), Some(doc)) = (clicked, &faces, &hollowed, &doc) {
                        let points: Option<Vec<[f64; 3]>> = open.iter().map(|f| face_point(c, *f)).collect();
                        let prepared = points.ok_or_else(|| "A chosen face has no triangles to point at".to_string()).and_then(|points| parts::shell(c, &points, wall, Tolerance::EXPORT).map_err(|e| format!("{e:#}")));
                        match prepared {
                            Ok((request, params)) => {
                                let (doc, part, request2) = (doc.clone(), *part, request.clone());
                                let settings = doc.feature(part).map(|f| f.component.clone()).unwrap_or_default();
                                start(&job, ui.ctx(), &app.occt, request, move |built| CadEdit::Add { feature: parts::stored_feature(&doc, &[part], &request2, params, built, settings), after: None });
                            }
                            Err(e) => app.set_status(e),
                        }
                    }
                });
            }
            if has_torus {
                ui.horizontal_wrapped(|ui| {
                    if !show_fillet {
                        ui.add(egui::DragValue::new(&mut radius).range(0.01..=5.0).speed(0.01).suffix(" mm").prefix("r "));
                    }
                    let why = match (junction, &post) {
                        _ if missing.is_some() => missing.clone(),
                        _ if running => Some("OpenCascade is already working".into()),
                        (None, _) => Some("Choose a torus standing free round the finger and a cylinder on it: click one part, Shift adds the other".into()),
                        (Some((_, cylinder)), None) => Some(unbuilt(doc.as_ref(), cylinder, "cylinder")),
                        _ => None,
                    };
                    let clicked = ui
                        .add_enabled(why.is_none(), egui::Button::new((Icon::CadUnion.image(ui, 18.0), "Fuse torus and cylinder with OpenCascade")))
                        .on_hover_text("Fuse the chosen torus and cylinder in OpenCascade and round every edge where they meet by r; the result replaces both and is kept in the file as a mesh every build can show")
                        .on_disabled_hover_text(why.unwrap_or_default())
                        .clicked();
                    if let (true, Some((torus, cylinder)), Some(c), Some(doc)) = (clicked, junction, &post, &doc) {
                        match parts::junction(doc, torus, c, radius, Tolerance::EXPORT) {
                            Ok((request, params)) => {
                                let (doc, request2) = (doc.clone(), request.clone());
                                let settings = doc.feature(torus).map(|f| f.component.clone()).unwrap_or_default();
                                start(&job, ui.ctx(), &app.occt, request, move |built| CadEdit::Add { feature: parts::stored_feature(&doc, &[torus, cylinder], &request2, params, built, settings), after: None });
                            }
                            Err(e) => app.set_status(format!("{e:#}")),
                        }
                    }
                });
            }
            if let Some(m) = message.filter(|m| !m.is_empty()) {
                let colour = if running { ui.visuals().weak_text_color() } else if m.starts_with("Add ") || m.starts_with("Edit ") { theme::GOOD } else { theme::BAD };
                ui.colored_label(colour, m);
            }
        });
    });
    ctx.data_mut(|d| d.insert_temp(id.with("radius"), radius));
    ctx.data_mut(|d| d.insert_temp(id.with("wall"), wall));
}

/// A stored fillet made again from its source as built now, replacing its operation in one edit.
fn rerun_fillet(app: &mut RingDesignerApp, job: &Shared, ctx: &egui::Context, f: &Feature) {
    let Operation::Stored { recipe, sources, .. } = &f.operation else { return };
    let radius = recipe.params.get("radius_mm").and_then(serde_json::Value::as_f64).unwrap_or(0.3);
    let tolerance = recipe.params.get("tolerance").and_then(|t| serde_json::from_value(t.clone()).ok()).unwrap_or(Tolerance::EXPORT);
    let prepared = sources
        .first()
        .ok_or_else(|| "The fillet names no source".to_string())
        .and_then(|s| as_built(app, *s))
        .and_then(|source| edges_again(&source, &recipe.params).and_then(|edges| parts::fillet(&source, &edges, radius, tolerance).map_err(|e| format!("{e:#}"))));
    let (request, params) = match prepared {
        Ok(p) => p,
        Err(e) => {
            app.set_status(e);
            return;
        }
    };
    let Some(doc) = app.design.cad.clone() else { return };
    let (id, sources, request2, settings) = (f.id, sources.clone(), request.clone(), f.component.clone());
    start(job, ctx, &app.occt, request, move |built| {
        let fresh = parts::stored_feature(&doc, &sources, &request2, params, built, settings);
        CadEdit::Operation { id, operation: fresh.operation }
    });
}

#[cfg(test)]
mod tests {
    use crate::interaction_tests::{harness, wait_for_build};
    use egui_kittest::kittest::Queryable;
    use ringdesign_core::RingDesign;
    use ringdesign_core::cad::{Attach, Component, ComponentRole, Document, Feature, Operation, Placement};
    use ringdesign_occt::client::Locator;
    use ringdesign_workbench::viewport::Sel;

    /// The worker in the test's target directory, built by `cargo build -p ringdesign-occt --features kernel-occt --bin occt-worker`, as a locator naming it; `None` when it is not built.
    fn worker() -> Option<Locator> {
        let worker = std::env::current_exe().unwrap().parent().and_then(|d| d.parent()).map(|d| d.join(ringdesign_occt::client::WORKER_NAME)).unwrap();
        if !worker.is_file() {
            eprintln!("{} is not built; OpenCascade is not driven", worker.display());
            return None;
        }
        Some(Locator::named(worker))
    }

    /// A scratch folder of this test's own.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ringdesign-occt-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 2 x 3 x 4 block and a pin of radius 1 and height 2 as STEP, the block's first plane written as a coplanar bilinear B-spline, as a vendor would.
    fn vendor_step() -> String {
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Block".into(), enabled: true, operation: Operation::Box { size: [2.0, 3.0, 4.0] }, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "Pin".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: Component::default() }).unwrap();
        let d = RingDesign { cad: Some(doc), ..RingDesign::default() };
        let text = ringdesign_core::cad::step::export(&ringdesign_core::cad::evaluate(&d, &lib, ringdesign_core::BuildParams::default()).unwrap(), "Parts").unwrap();
        let record = |id: usize| text.lines().find_map(|l| l.strip_prefix(&format!("#{id}="))).unwrap().to_string();
        let refs = |r: &str| -> Vec<usize> { r.split('#').skip(1).map(|t| t.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap()).collect() };
        let xyz = |id: usize| -> [f64; 3] {
            let r = record(id);
            let inside = r.rsplit_once('(').unwrap().1.split(')').next().unwrap().to_string();
            let v: Vec<f64> = inside.split(',').map(|n| n.trim().parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        };
        let line = text.lines().find(|l| l.contains("=PLANE(")).unwrap();
        let plane: usize = line[1..line.find('=').unwrap()].parse().unwrap();
        let axis = refs(&record(refs(&record(plane))[0]));
        let (o, n, x) = (xyz(axis[0]), xyz(axis[1]), xyz(axis[2]));
        let y = [n[1] * x[2] - n[2] * x[1], n[2] * x[0] - n[0] * x[2], n[0] * x[1] - n[1] * x[0]];
        let last: usize = text.lines().filter_map(|l| l.strip_prefix('#')?.split('=').next()?.parse().ok()).max().unwrap();
        // Ten millimetres either way along the plane's own axes, so the patch's normal is the plane's.
        let corner = |i: f64, j: f64| std::array::from_fn::<f64, 3, _>(|k| o[k] + 10.0 * ((2.0 * i - 1.0) * x[k] + (2.0 * j - 1.0) * y[k]));
        let points: String = [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)]
            .iter()
            .enumerate()
            .map(|(k, (i, j))| {
                let p = corner(*i, *j);
                format!("#{}=CARTESIAN_POINT('',({:.6},{:.6},{:.6}));\n", last + 1 + k, p[0], p[1], p[2])
            })
            .collect();
        let spline = format!("#{plane}=B_SPLINE_SURFACE_WITH_KNOTS('',1,1,((#{},#{}),(#{},#{})),.UNSPECIFIED.,.F.,.F.,.F.,(2,2),(2,2),(0.,1.),(0.,1.),.UNSPECIFIED.);", last + 1, last + 2, last + 3, last + 4);
        text.replace(line, &spline).replace("ENDSEC;\nEND-ISO", &format!("{points}ENDSEC;\nEND-ISO"))
    }

    /// Imports `path` through File > Import part and waits for it to land or be refused.
    fn import(h: &mut egui_kittest::Harness<'static, crate::app::RingDesignerApp>, path: &std::path::Path) {
        crate::export::import_part_path(h.state_mut(), path);
        let started = std::time::Instant::now();
        while h.state().importing.is_some() {
            h.run_steps(1);
            assert!(started.elapsed() < std::time::Duration::from_secs(60), "{} never landed", path.display());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// The last feature of the design's parts, as a stored import: its name, kernel and recipe parameters.
    fn last_import(h: &egui_kittest::Harness<'static, crate::app::RingDesignerApp>) -> (String, String, serde_json::Value, f64) {
        let doc = h.state().design.cad.clone().unwrap();
        let f = doc.features.last().unwrap();
        let Operation::Stored { recipe, mesh, .. } = &f.operation else { panic!("{:?}", f.operation) };
        (f.name.clone(), recipe.kernel.clone(), recipe.params.clone(), mesh.made().unwrap().solid().volume())
    }

    #[test]
    fn a_step_import_reads_in_the_core_first_and_hands_opencascade_only_what_it_leaves() {
        let dir = scratch("routing");
        // A file that is no program stands in for the worker: anything handed to it fails, and says so.
        let fake = dir.join("not-a-worker");
        std::fs::write(&fake, "not a program").unwrap();
        let mut h = crate::interaction_tests::harness();
        h.state_mut().occt = Locator::named(&fake);
        // A ring the core reads whole never reaches the worker, and keeps the core's recipe.
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let params = ringdesign_core::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let court = dir.join("court.step");
        std::fs::write(&court, ringdesign_core::cad::step::ring(&court_with_block(), &lib, params, "Court").unwrap()).unwrap();
        let start = h.state().history.present();
        import(&mut h, &court);
        assert_eq!(h.state().history.present(), start + 1, "{}", h.state().status);
        let (name, kernel, recipe, _) = last_import(&h);
        assert_eq!((name.as_str(), recipe["format"].as_str()), ("court", Some("step")), "{kernel}");
        assert!(!h.state().status.contains("OpenCascade"), "{}", h.state().status);
        // A solid the core leaves goes to the worker; when it fails, what the core read lands and the line says why.
        let vendor = dir.join("vendor.step");
        std::fs::write(&vendor, vendor_step()).unwrap();
        import(&mut h, &vendor);
        assert_eq!(h.state().history.present(), start + 2, "{}", h.state().status);
        let (name, _, recipe, volume) = last_import(&h);
        assert_eq!((name.as_str(), recipe["format"].as_str()), ("vendor", Some("step")));
        assert!((volume - std::f64::consts::TAU).abs() < 0.01 * std::f64::consts::TAU, "the pin alone: {volume}");
        let status = h.state().status.clone();
        assert!(status.contains("vendor.step: the exact solid Block reads only where OpenCascade is (it carries a B-spline surface), and was left out"), "{status}");
        assert!(status.contains(" • OpenCascade did not read the rest: OpenCascade's worker would not start: "), "{status}");
        // A file the core refuses goes to the worker too, and both reasons are said.
        let block = dir.join("block.step");
        std::fs::write(&block, vendor_step().replacen("MANIFOLD_SOLID_BREP('Pin'", "UNREAD_SOLID('Pin'", 1)).unwrap();
        import(&mut h, &block);
        assert_eq!(h.state().history.present(), start + 2);
        let status = h.state().status.clone();
        assert!(status.starts_with("block.step holds no solid that reads without OpenCascade; its 1 exact solid(s) (Block) read only where OpenCascade is: it carries a B-spline surface; OpenCascade did not read it either: OpenCascade's worker would not start: "), "{status}");
        // With no worker at all, the core's reading stands as it did.
        h.state_mut().occt = Locator::nowhere();
        import(&mut h, &vendor);
        assert_eq!(h.state().history.present(), start + 3, "{}", h.state().status);
        assert!(h.state().status.contains("and was left out") && !h.state().status.contains("did not read"), "{}", h.state().status);
        import(&mut h, &block);
        assert_eq!(h.state().status, "block.step holds no solid that reads without OpenCascade; its 1 exact solid(s) (Block) read only where OpenCascade is: it carries a B-spline surface");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opencascade_reads_the_b_spline_solid_the_core_leaves() {
        let Some(occt) = worker() else { return };
        let dir = scratch("vendor");
        let vendor = dir.join("vendor.step");
        std::fs::write(&vendor, vendor_step()).unwrap();
        let mut h = crate::interaction_tests::harness();
        h.state_mut().occt = occt;
        let start = h.state().history.present();
        import(&mut h, &vendor);
        assert_eq!(h.state().history.present(), start + 1, "{}", h.state().status);
        let (name, kernel, recipe, volume) = last_import(&h);
        assert_eq!((name.as_str(), kernel.as_str(), recipe["file"].as_str()), ("vendor", "occt", Some("vendor.step")));
        // The block through its spline face and the pin, as one part of their metal.
        let whole = 24.0 + std::f64::consts::TAU;
        assert!((volume - whole).abs() < 0.01 * whole, "{volume} against {whole}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The Court band carrying a 4 x 6 x 2 block joined at its top.
    fn court_with_block() -> RingDesign {
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
        let block = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Block".into(), enabled: true, operation: Operation::Box { size: [4.0, 6.0, 2.0] }, component: block }).unwrap();
        let base = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        RingDesign { cad: Some(doc), ..base }
    }

    #[test]
    fn without_a_worker_the_opencascade_rows_are_greyed_and_say_why() {
        use egui_kittest::kittest::NodeT;
        let mut h = harness();
        {
            let app = h.state_mut();
            app.occt = Locator::nowhere();
            app.switch_desktop(crate::dock::Desktop::Cad);
            app.design = court_with_block();
            app.history.commit(&app.design);
            app.rebuild_now();
        }
        wait_for_build(&mut h);
        h.state_mut().selection.items.clear();
        h.run_steps(3);
        assert!(h.query_by_label("no worker in this build").is_some() && h.query_by_label_contains("with OpenCascade").is_none(), "one line until a row applies");
        h.state_mut().selection.items = vec![Sel::Edge { feature: 2, edge: 0 }, Sel::Face { feature: 2, face: 0 }];
        h.run_steps(3);
        let reason = format!("This build carries no OpenCascade worker: put occt-worker{} beside RingDesigner, or name one in RINGDESIGN_OCCT_WORKER", std::env::consts::EXE_SUFFIX);
        assert_eq!(crate::occt_embedded::missing(&h.state().occt), Some(reason));
        assert!(!super::available(&h.state().occt));
        for label in ["Fillet 1 edge with OpenCascade", "Shell with OpenCascade, 1 face open"] {
            assert!(h.get_by_label(label).accesskit_node().is_disabled(), "{label}");
        }
        assert!(h.query_by_label("Fuse torus and cylinder with OpenCascade").is_none(), "offered only beside a torus");
    }

    #[test]
    fn a_cylinder_a_later_feature_consumed_is_named_as_used_up_not_unbuilt() {
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Shank".into(), enabled: true, operation: Operation::Torus { major_mm: 9.85, minor_mm: 1.2 }, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.5 }, component: Component::default() }).unwrap();
        assert_eq!(super::unbuilt(Some(&doc), 2, "cylinder"), "The chosen cylinder is not built yet");
        doc.append(Feature { id: 3, name: "Nudge".into(), enabled: true, operation: Operation::Transform { source: 2, translation: [0.0, 0.0, 0.1], rotation_deg: [0.0; 3] }, component: Component::default() }).unwrap();
        assert_eq!(super::unbuilt(Some(&doc), 2, "cylinder"), "The chosen cylinder is used up by Nudge, a later feature; suppress that feature to reach it");
        doc.features[2].enabled = false;
        assert_eq!(super::unbuilt(Some(&doc), 2, "cylinder"), "The chosen cylinder is not built yet");
    }

    #[test]
    fn the_opencascade_plate_stands_over_the_canvas_and_clear_of_a_wrapped_footer() {
        let mut h = egui_kittest::Harness::builder().with_size([760., 640.]).build_eframe(|cc| {
            crate::theme::install(&cc.egui_ctx);
            let mut app = crate::app::RingDesignerApp::new(cc);
            app.updater.automatic = false;
            app.auto_rebuild = false;
            app
        });
        {
            let app = h.state_mut();
            app.occt = Locator::nowhere();
            app.switch_desktop(crate::dock::Desktop::Cad);
            app.design = court_with_block();
            app.history.commit(&app.design);
            app.rebuild_now();
        }
        wait_for_build(&mut h);
        h.run_steps(4);
        let plate = h.get_by_label("OpenCascade").rect().union(h.get_by_label("no worker in this build").rect());
        let canvas = h.state().cad.canvas().expect("the canvas is laid out");
        assert!(canvas.contains_rect(plate), "{plate:?} on {canvas:?}");
        // The view's footer, under the canvas: its own controls and the state it reports at its right end.
        let footer: Vec<(String, egui::Rect)> = ["Parts only", "Display", "History", "Committed · mm", "Preview pending", "Evaluating…"]
            .into_iter()
            .flat_map(|l| h.query_all_by_label(l).map(|n| (l.to_string(), n.rect())).collect::<Vec<_>>())
            .filter(|(_, r)| r.top() >= canvas.bottom() && r.left() >= canvas.left() - 1.0)
            .collect();
        let row = |l: &str| footer.iter().find(|(n, _)| n == l).map(|(_, r)| *r).unwrap_or_else(|| panic!("{l} in {footer:?}"));
        let state = footer.iter().find(|(n, _)| !["Parts only", "Display", "History"].contains(&n.as_str())).expect("the footer's state").1;
        assert!(state.top() > row("Parts only").bottom(), "the footer wraps onto a second line at this width");
        for (label, rect) in &footer {
            assert!(!rect.intersects(plate.expand(6.0)), "the plate covers {label}: {plate:?} over {rect:?}");
        }
    }

    #[test]
    fn a_torus_and_a_cylinder_fuse_in_opencascade_and_replace_both() {
        let Some(occt) = worker() else { return };
        let (major, minor) = (9.85, 1.2);
        let design = |post: Placement| {
            let mut doc = Document::default();
            let shank = Component { role: ComponentRole::Shank, ..Component::default() };
            doc.append(Feature { id: 1, name: "Shank".into(), enabled: true, operation: Operation::Torus { major_mm: major, minor_mm: minor }, component: shank }).unwrap();
            doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 0.8, height_mm: 2.5 }, component: Component { placement: post, ..Component::default() } }).unwrap();
            RingDesign { cad: Some(doc), ..RingDesign::default() }
        };
        let mut h = harness();
        h.state_mut().occt = occt;
        let seat = |h: &mut egui_kittest::Harness<'static, crate::app::RingDesignerApp>, d: RingDesign| {
            let app = h.state_mut();
            app.design = d;
            app.history.commit(&app.design);
            app.rebuild_now();
            wait_for_build(h);
            h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().frame.origin[1]
        };
        h.state_mut().switch_desktop(crate::dock::Desktop::Cad);
        // The post's foot half a millimetre into the tube's crest, however the seat is read.
        let r0 = seat(&mut h, design(Placement::ring(90.0, 0.0)));
        let seated = seat(&mut h, design(Placement::ring(90.0, major + minor + 1.25 - 0.5 - r0)));
        assert!((seated - (major + minor + 0.75)).abs() < 1e-6, "{seated}");
        h.state_mut().selection.items = vec![Sel::Part(2), Sel::Part(1)];
        h.run_steps(3);
        let entries = h.state().history.timeline().len();
        h.get_by_label("Fuse torus and cylinder with OpenCascade").click();
        let started = std::time::Instant::now();
        while !h.state().design.cad.as_ref().unwrap().features.iter().any(|f| matches!(f.operation, Operation::Stored { .. })) {
            h.run_steps(2);
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "OpenCascade never answered");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let doc = h.state().design.cad.clone().unwrap();
        let f = doc.features.last().unwrap();
        let Operation::Stored { recipe, sources, mesh } = &f.operation else { unreachable!() };
        assert_eq!((f.name.as_str(), recipe.op.as_str(), sources.as_slice(), doc.outputs.as_slice()), ("Junction (OpenCascade)", "junction", &[1, 2][..], &[f.id][..]));
        assert_eq!(f.component.role, ComponentRole::Shank);
        let timeline = h.state().history.timeline();
        assert_eq!((timeline.len(), timeline.last().unwrap().0.as_str()), (entries + 1, "Add Junction (OpenCascade)"), "one History entry");
        // The tube and the post less their overlap, with a little back in the rounded corner.
        let (tube, post) = (2.0 * std::f64::consts::PI.powi(2) * major * minor * minor, std::f64::consts::PI * 0.8 * 0.8 * 2.5);
        let volume = mesh.made().unwrap().solid().volume();
        assert!(volume > tube && volume < tube + post, "{volume} against {tube} + {post}");
        h.state_mut().rebuild_now();
        wait_for_build(&mut h);
        let built = h.state().build.clone().unwrap();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
    }

    #[test]
    fn chosen_faces_shell_in_opencascade_and_land_as_one_history_entry() {
        let Some(occt) = worker() else { return };
        let mut h = harness();
        {
            let app = h.state_mut();
            app.occt = occt;
            app.switch_desktop(crate::dock::Desktop::Cad);
            app.design = court_with_block();
            app.history.commit(&app.design);
            app.rebuild_now();
        }
        wait_for_build(&mut h);
        // The block's top face, where it stands proud of the band, left open.
        let top = {
            let c = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().clone();
            (0..c.trace.face_kind.len() as u32).find(|f| super::face_point(&c, *f).is_some_and(|p| p[2] > 0.99)).unwrap()
        };
        h.state_mut().selection.items = vec![Sel::Face { feature: 2, face: top }];
        h.run_steps(3);
        let entries = h.state().history.timeline().len();
        h.get_by_label("Shell with OpenCascade, 1 face open").click();
        let started = std::time::Instant::now();
        while !h.state().design.cad.as_ref().unwrap().features.iter().any(|f| matches!(f.operation, Operation::Stored { .. })) {
            h.run_steps(2);
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "OpenCascade never answered");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let doc = h.state().design.cad.clone().unwrap();
        let f = doc.features.last().unwrap();
        let Operation::Stored { recipe, sources, mesh } = &f.operation else { unreachable!() };
        assert_eq!((f.name.as_str(), recipe.op.as_str(), sources.as_slice(), doc.outputs.as_slice()), ("Shell (OpenCascade)", "shell", &[2][..], &[1, f.id][..]));
        let timeline = h.state().history.timeline();
        assert_eq!((timeline.len(), timeline.last().unwrap().0.as_str()), (entries + 1, "Add Shell (OpenCascade)"), "one History entry");
        // A 4 x 6 x 2 block less its hollow: 0.6 mm walls, open at the top.
        let hollow = 48.0 - (4.0 - 1.2) * (6.0 - 1.2) * (2.0 - 0.6);
        let volume = mesh.made().unwrap().solid().volume();
        assert!((volume - hollow).abs() < 2e-3 * hollow, "{volume} against {hollow}");
        h.state_mut().rebuild_now();
        wait_for_build(&mut h);
        let built = h.state().build.clone().unwrap();
        assert!(built.report.validation.watertight && built.parts.joined == 1, "{:?} {:?}", built.report.validation, built.parts.notes);
    }

    #[test]
    fn a_step_file_imports_through_opencascade_exact_solids_and_all() {
        let Some(occt) = worker() else { return };
        let dir = std::env::temp_dir().join(format!("ringdesign-occt-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let params = ringdesign_core::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let d = court_with_block();
        let step = dir.join("block.step");
        std::fs::write(&step, ringdesign_core::cad::step::ring(&d, &lib, params, "Court").unwrap()).unwrap();
        let started = std::time::Instant::now();
        let (f, notes) = super::import_step(&step, &occt, &std::sync::atomic::AtomicBool::new(false)).unwrap();
        eprintln!("OpenCascade read block.step in {:.0} ms: {notes:?}", started.elapsed().as_secs_f64() * 1e3);
        let Operation::Stored { recipe, mesh, .. } = &f.operation else { panic!() };
        assert_eq!((f.name.as_str(), recipe.kernel.as_str(), recipe.op.as_str(), recipe.params["file"].as_str()), ("block", "occt", "import", Some("block.step")));
        assert_eq!((f.component.attach, &f.component.placement), (Attach::Join, &Placement::ring(90.0, 0.0)));
        // The exact block and the faceted band both come back, as one part of their metal.
        let solids = ringdesign_core::cad::step::read_solids(&std::fs::read_to_string(&step).unwrap()).unwrap();
        let band = solids.iter().find(|s| s.faceted).and_then(|s| s.mesh.as_ref()).unwrap().volume_mm3();
        let volume = mesh.made().unwrap().solid().volume();
        assert!((volume - (band + 48.0)).abs() < 5e-3 * (band + 48.0), "{volume} against {band} + 48");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_chosen_edge_fillets_in_opencascade_and_lands_as_one_history_entry() {
        let Some(occt) = worker() else { return };
        let mut h = harness();
        {
            let app = h.state_mut();
            app.occt = occt;
            app.switch_desktop(crate::dock::Desktop::Cad);
            let mut doc = Document::default();
            doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
            let block = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
            doc.append(Feature { id: 2, name: "Block".into(), enabled: true, operation: Operation::Box { size: [4.0, 6.0, 2.0] }, component: block }).unwrap();
            let base = ringdesign_core::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
            app.design = RingDesign { cad: Some(doc), ..base };
            app.history.commit(&app.design);
            app.rebuild_now();
        }
        wait_for_build(&mut h);
        let edge = {
            let c = h.state().build.as_ref().unwrap().parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().clone();
            let body = ringdesign_occt::parts::local_body(&c).unwrap();
            let top = |p: [f64; 3]| p[2] > 0.99;
            body.edges.iter().position(|(k, _)| body.edge_endpoints(k).is_some_and(|(a, b)| top(a) && top(b))).unwrap()
        };
        h.state_mut().selection.items = vec![Sel::Edge { feature: 2, edge: edge as u32 }];
        h.run_steps(3);
        let entries = h.state().history.timeline().len();
        h.get_by_label("Fillet 1 edge with OpenCascade").click();
        let started = std::time::Instant::now();
        while !h.state().design.cad.as_ref().unwrap().features.iter().any(|f| matches!(f.operation, Operation::Stored { .. })) {
            h.run_steps(2);
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "OpenCascade never answered");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let doc = h.state().design.cad.clone().unwrap();
        let f = doc.features.last().unwrap();
        let Operation::Stored { recipe, sources, mesh } = &f.operation else { unreachable!() };
        assert_eq!((f.name.as_str(), recipe.op.as_str(), sources.as_slice(), doc.outputs.as_slice()), ("Fillet (OpenCascade)", "fillet", &[2][..], &[1, f.id][..]));
        assert!(mesh.triangles > 12, "{}", mesh.triangles);
        let timeline = h.state().history.timeline();
        assert_eq!((timeline.len(), timeline.last().unwrap().0.as_str()), (entries + 1, "Add Fillet (OpenCascade)"), "one History entry");
        h.state_mut().rebuild_now();
        wait_for_build(&mut h);
        let built = h.state().build.clone().unwrap();
        assert!(built.report.validation.watertight && built.parts.joined == 1, "{:?} {:?}", built.report.validation, built.parts.notes);
        // The block grown a millimetre leaves the fillet stale; Run again rounds the same edge of the block as it is now.
        let (id, before) = (f.id, mesh.clone());
        {
            let app = h.state_mut();
            app.design.cad.as_mut().unwrap().features[1].operation = Operation::Box { size: [4.0, 6.0, 3.0] };
            app.history.commit(&app.design);
            app.rebuild_now();
        }
        wait_for_build(&mut h);
        h.state_mut().selection.items = vec![Sel::Part(id)];
        h.run_steps(3);
        let entries = h.state().history.timeline().len();
        h.get_by_label("Run again").click();
        let started = std::time::Instant::now();
        let after = loop {
            h.run_steps(2);
            let f = h.state().design.cad.as_ref().unwrap().feature(id).cloned().unwrap();
            if let Operation::Stored { mesh, .. } = f.operation
                && mesh != before
            {
                break mesh;
            }
            assert!(started.elapsed() < std::time::Duration::from_secs(30), "OpenCascade never answered the rerun");
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let volume = |p: &ringdesign_core::cad::stored::Packed| p.made().unwrap().solid().volume();
        // A 4 x 6 x 3 block less the same 0.5 mm round along 4 mm, against 4 x 6 x 2.
        assert!((volume(&after) - volume(&before) - 24.0).abs() < 1e-2, "{} then {}", volume(&before), volume(&after));
        let timeline = h.state().history.timeline();
        assert_eq!((timeline.len(), timeline.last().unwrap().0.as_str()), (entries + 1, "Edit Fillet (OpenCascade)"));
        let doc = h.state().design.cad.clone().unwrap();
        let Some(Operation::Stored { recipe, sources, .. }) = doc.feature(id).map(|f| &f.operation) else { unreachable!() };
        assert_eq!(recipe.digest, ringdesign_core::cad::stored::digest(&doc, sources), "fresh again");
    }
}
