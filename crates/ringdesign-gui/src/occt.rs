//! OpenCascade in the CAD pane: the chosen edges filleted by a worker process and committed as one stored-mesh feature.
use crate::app::RingDesignerApp;
use crate::theme;
use ringdesign_core::cad::{BuildCtx, EdgeRef, EvaluatedComponent, Feature, Operation, edge_signature, edit::CadEdit, evaluate_with};
use ringdesign_core::sketch::Id;
use ringdesign_occt::client::Worker;
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

/// A STEP file's solids read by OpenCascade in a worker process, kept as one stored part at the top of the ring and joined.
pub fn import_step(path: &std::path::Path, cancel: &std::sync::atomic::AtomicBool) -> Result<(Feature, Vec<String>), String> {
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("The file").to_string();
    let step = std::fs::read_to_string(path).map_err(|e| format!("{file} could not be read: {e}"))?;
    let request = Request::Import { step, tolerance: Tolerance::EXPORT };
    let response = Worker::locate().map_err(|e| e.to_string())?.run_cancellable(&request, TIMEOUT, cancel).map_err(|e| e.to_string())?;
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

/// `request` run on a worker thread; its outcome becomes `edit` once the worker answers.
fn start(job: &Shared, ctx: &egui::Context, request: Request, edit: impl FnOnce(&ringdesign_occt::protocol::Built) -> CadEdit + Send + 'static) {
    let (job, ctx) = (job.clone(), ctx.clone());
    if let Ok(mut j) = job.lock() {
        j.running = true;
        j.message = Some("OpenCascade is working".into());
    }
    std::thread::spawn(move || {
        let outcome = Worker::locate().map_err(|e| e.to_string()).and_then(|w| w.run(&request, TIMEOUT).map_err(|e| e.to_string())).and_then(|response| match response {
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
    let rect = ui.max_rect();
    egui::Area::new(ui.id().with("occt-pane")).order(egui::Order::Middle).fixed_pos(rect.left_bottom() + egui::vec2(8.0, -8.0)).pivot(egui::Align2::LEFT_BOTTOM).show(&ctx, |ui| {
        egui::Frame::new().fill(theme::FLOAT.gamma_multiply(0.94)).corner_radius(4).inner_margin(6).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("OpenCascade");
                ui.add(egui::DragValue::new(&mut radius).range(0.01..=5.0).speed(0.01).suffix(" mm").prefix("r "));
                let why = match (&chosen, &target) {
                    _ if running => Some("OpenCascade is already working"),
                    (None, _) => Some("Choose one part's edges in the Ring viewport: click an edge, Shift adds more"),
                    (Some(_), None) => Some("The chosen part is not built yet"),
                    (Some(_), Some(c)) if c.brep().is_none() => Some("OpenCascade rounds a kernel part; this one is a mesh"),
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
                            start(&job, ui.ctx(), request, move |built| CadEdit::Add { feature: parts::stored_feature(&doc, &[part], &request2, params, built, settings), after: None });
                        }
                        Err(e) => app.set_status(format!("{e:#}")),
                    }
                }
                if let Some(f) = &rerun {
                    let again = ui.add_enabled(!running, egui::Button::new((Icon::Rebuild.image(ui, 18.0), "Run again"))).on_hover_text("Its source changed since OpenCascade made it: fillet the same edges of the part as it is now");
                    if again.clicked() {
                        rerun_fillet(app, &job, ui.ctx(), f);
                    }
                }
                if running {
                    ui.spinner();
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut wall).range(0.05..=5.0).speed(0.01).suffix(" mm").prefix("wall "));
                let why = match (&faces, &hollowed) {
                    _ if running => Some("OpenCascade is already working"),
                    (None, _) => Some("Choose the faces to leave open, all of one part: click a face, Shift adds more"),
                    (Some(_), None) => Some("The chosen part is not built yet"),
                    (Some(_), Some(c)) if c.brep().is_none() => Some("OpenCascade hollows a kernel part; this one is a mesh"),
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
                            start(&job, ui.ctx(), request, move |built| CadEdit::Add { feature: parts::stored_feature(&doc, &[part], &request2, params, built, settings), after: None });
                        }
                        Err(e) => app.set_status(e),
                    }
                }
            });
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
    start(job, ctx, request, move |built| {
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
    use ringdesign_workbench::viewport::Sel;

    /// The worker in the test's target directory, built by `cargo build -p ringdesign-occt --features kernel-occt --bin occt-worker`, and named to the client; `None` when it is not built.
    fn worker() -> Option<std::path::PathBuf> {
        let worker = std::env::current_exe().unwrap().parent().and_then(|d| d.parent()).map(|d| d.join(ringdesign_occt::client::WORKER_NAME)).unwrap();
        if !worker.is_file() {
            eprintln!("{} is not built; OpenCascade is not driven", worker.display());
            return None;
        }
        // SAFETY: set before any worker thread that reads it starts, always to the same path.
        unsafe { std::env::set_var(ringdesign_occt::client::WORKER_ENV, &worker) };
        Some(worker)
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
    fn chosen_faces_shell_in_opencascade_and_land_as_one_history_entry() {
        if worker().is_none() {
            return;
        }
        let mut h = harness();
        {
            let app = h.state_mut();
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
        if worker().is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("ringdesign-occt-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lib = ringdesign_core::AlphaLibrary::builtin();
        let params = ringdesign_core::BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() };
        let d = court_with_block();
        let step = dir.join("block.step");
        std::fs::write(&step, ringdesign_core::cad::step::ring(&d, &lib, params, "Court").unwrap()).unwrap();
        let started = std::time::Instant::now();
        let (f, notes) = super::import_step(&step, &std::sync::atomic::AtomicBool::new(false)).unwrap();
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
        // The worker in the test's target directory, built by `cargo build -p ringdesign-occt --features kernel-occt --bin occt-worker`.
        let worker = std::env::current_exe().unwrap().parent().and_then(|d| d.parent()).map(|d| d.join(ringdesign_occt::client::WORKER_NAME)).unwrap();
        if !worker.is_file() {
            eprintln!("{} is not built; the OpenCascade pane is not driven", worker.display());
            return;
        }
        // SAFETY: set before the worker thread that reads it starts, and read by nothing else.
        unsafe { std::env::set_var(ringdesign_occt::client::WORKER_ENV, &worker) };
        let mut h = harness();
        {
            let app = h.state_mut();
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
