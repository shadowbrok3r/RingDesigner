//! Workshop inspection, backed by the same prepared pattern used by exports.

use crate::{app::RingDesignerApp, pane::PaneKind, theme};
use egui::{Color32, Pos2, Rect, Stroke, vec2};
use ringdesign_core::manufacturing::{
    self as mf, BoreStrategy, Channel, ChannelKind, Inspection, Recipe, Setup,
    release::Status,
    repair::{self, Repair},
};
use ringdesign_core::{BuildParams, RingDesign};
use std::sync::{
    Arc,
    mpsc::{self, Receiver},
};

struct Preview {
    design: RingDesign,
    inspection: Arc<Inspection>,
    source: u64,
}
enum TaskResult {
    Inspection(Arc<Inspection>),
    Preview(Box<Preview>),
    Export(Result<String, String>),
    Orientations(Vec<mf::release::Orientation>),
    Rankings(Vec<repair::RankedRepair>),
}
struct Task {
    key: u64,
    receiver: Receiver<Result<TaskResult, String>>,
}

pub struct CastingState {
    task: Option<Task>,
    requested: u64,
    report_key: u64,
    inspection: Option<Arc<Inspection>>,
    preview: Option<Preview>,
    selected: Option<usize>,
    layer: Option<usize>,
    opening: f64,
    withdrawal: f64,
    show_cope: bool,
    show_drag: bool,
    view: usize,
    error: Option<String>,
    message: String,
    repair: Repair,
    diagnostic: bool,
    tab: usize,
    orientations: Vec<mf::release::Orientation>,
    orientation_key: u64,
    rankings: Vec<repair::RankedRepair>,
    ranking_key: u64,
    trial: mf::trials::Trial,
    calibration: Option<mf::trials::Calibration>,
}
impl Default for CastingState {
    fn default() -> Self {
        Self {
            task: None,
            requested: 0,
            report_key: 0,
            inspection: None,
            preview: None,
            selected: None,
            layer: None,
            opening: 0.0,
            withdrawal: 0.0,
            show_cope: true,
            show_drag: true,
            view: 0,
            error: None,
            message: String::new(),
            repair: Repair::ReduceRelief,
            diagnostic: false,
            tab: 0,
            orientations: Vec::new(),
            orientation_key: 0,
            rankings: Vec::new(),
            ranking_key: 0,
            trial: Default::default(),
            calibration: None,
        }
    }
}

fn color(s: Status) -> Color32 {
    match s {
        Status::Blocked | Status::Invalid => theme::BAD,
        Status::Review => theme::WARN,
        _ => theme::GOOD,
    }
}
fn params(app: &RingDesignerApp) -> BuildParams {
    BuildParams {
        theta_steps: app.preview_params.theta_steps.clamp(128, 384),
        profile_steps: app.preview_params.profile_steps.clamp(96, 192),
        refine: None,
        soften_mm: 0.0,
        ..app.preview_params
    }
}
fn key(app: &RingDesignerApp, s: &Setup) -> u64 {
    let bytes = serde_json::to_vec(&(&app.design, s, params(app), Arc::as_ptr(&app.lib) as usize))
        .unwrap_or_default();
    mf::package::fingerprint(&bytes)
}

pub fn active(app: &RingDesignerApp) -> bool {
    app.panes
        .get(app.active_pane)
        .is_some_and(|p| p.kind == PaneKind::Casting)
}
fn current_inspection(app: &RingDesignerApp) -> Option<&Inspection> {
    if !app.is_current() {
        return None;
    }
    let setup = app
        .design
        .manufacturing
        .clone()
        .unwrap_or_else(|| Setup::from_design(&app.design));
    let current = key(app, &setup);
    if let Some(preview) = app.casting.preview.as_ref().filter(|p| p.source == current) {
        return Some(&preview.inspection);
    }
    if app.casting.report_key == current {
        app.casting.inspection.as_deref()
    } else {
        None
    }
}
pub fn status_chip(app: &RingDesignerApp, ui: &mut egui::Ui) {
    if let Some(i) = current_inspection(app) {
        ui.colored_label(
            color(i.release.status),
            format!("Pattern: {}", i.release.status.label()),
        );
    } else {
        ui.weak("Pattern: inspection pending");
    }
}
pub fn report_panel(app: &RingDesignerApp, ui: &mut egui::Ui) {
    ui.strong("Pattern manufacturing check");
    let Some(i) = current_inspection(app) else {
        ui.weak("Design or recipe changed — inspection pending");
        return;
    };
    let r = &i.release;
    ui.colored_label(color(r.status), r.status.label());
    if app.casting.preview.is_some() {
        ui.colored_label(theme::INFO, "Correction preview");
    }
    ui.separator();
    for (label, value) in [
        ("Release", format!("{} obstructions", r.obstructions.len())),
        (
            "Draft",
            format!("{:.2} mm² below target", r.low_draft_area_mm2),
        ),
        ("Sand slots", format!("{} to review", r.sand_findings.len())),
        ("Detail", format!("{} findings", i.details.len())),
        (
            "Radial metal wall",
            i.field.as_ref().map_or_else(
                || "Use CAD dimensions".into(),
                |f| format!("{:.3} mm", f.thinnest_wall_mm),
            ),
        ),
        (
            "Flask fit",
            if r.fits_flask {
                "Fits with margin"
            } else {
                "Does not fit"
            }
            .into(),
        ),
        ("Pattern scale", format!("×{:.6}", i.prepared.scale)),
        ("Cast ring", format!("{:.2} g", i.ring_grams)),
        ("Channels", format!("{:.2} g estimated", i.channel_grams)),
        (
            "Charge",
            format!(
                "{:.2} g before handling losses",
                i.ring_grams + i.channel_grams
            ),
        ),
    ] {
        ui.horizontal_wrapped(|ui| {
            ui.weak(label);
            ui.label(value);
        });
    }
    if let Some(w) = &i.local_wall {
        ui.colored_label(
            if w.below_limit > 0 {
                theme::WARN
            } else {
                theme::TEXT
            },
            format!(
                "Sampled local wall: {:?} mm; {} below target; {} unresolved",
                w.sampled_min_mm, w.below_limit, w.unresolved
            ),
        );
        ui.weak(w.note);
    }
    if let Some((theta, modulus)) = i.hot_spot {
        ui.label(format!(
            "Thicker section near {theta:.0}°; feeding proxy {modulus:.2} mm"
        ));
    } else {
        ui.weak("No dominant section hot spot");
    }
    for note in &r.notes {
        ui.add_space(4.0);
        ui.weak(note);
    }
    if !i.prepared.bench_layers.is_empty() {
        ui.separator();
        ui.strong("Deferred to bench");
        for name in &i.prepared.bench_layers {
            ui.label(name);
        }
    }
}
fn spawn(
    state: &mut CastingState,
    key: u64,
    ctx: egui::Context,
    work: impl FnOnce() -> Result<TaskResult, String> + Send + 'static,
) {
    let (tx, receiver) = mpsc::channel();
    state.task = Some(Task { key, receiver });
    state.error = None;
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
            .unwrap_or_else(|_| Err("Inspection failed; edit the setup or retry".into()));
        let _ = tx.send(result);
        ctx.request_repaint();
    });
}

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    let mut state = std::mem::take(&mut app.casting);
    let mut setup = app
        .design
        .manufacturing
        .clone()
        .unwrap_or_else(|| Setup::from_design(&app.design));
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (n, label) in [
            "Recipe & pattern",
            "Release & repairs",
            "Channels & export",
            "Debug",
            "Shop data",
        ]
        .iter()
        .enumerate()
        {
            ui.selectable_value(&mut state.tab, n, *label);
        }
    });
    ui.separator();
    match state.tab {
        0 => {
            if let Some(doc) = &app.design.cad {
                egui::ComboBox::from_id_salt("casting_component")
                    .selected_text(
                        setup
                            .component
                            .and_then(|id| doc.features.iter().find(|f| f.id == id))
                            .map_or("Select component", |f| f.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for f in doc
                            .features
                            .iter()
                            .filter(|f| doc.outputs.contains(&f.id) && !f.component.reference)
                        {
                            if ui
                                .selectable_value(&mut setup.component, Some(f.id), &f.name)
                                .changed()
                            {
                                if let Some(recipe) = &f.component.manufacturing {
                                    let id = setup.component;
                                    setup = recipe.clone();
                                    setup.component = id;
                                }
                                changed = true;
                            }
                        }
                    });
            }
            changed |= settings(ui, &mut setup, &mut state);
        }
        2 => changed |= channels(ui, &mut setup),
        3 => {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Recheck geometry").clicked() {
                    state.requested = 0;
                }
                if ui
                    .button("Add test rail")
                    .on_hover_text(
                        "Adds a narrow crown rail to inspect and repair; undo removes it",
                    )
                    .clicked()
                    && !app.graph_driven()
                {
                    app.history.commit(&app.design);
                    let ctx = app.design.field_context();
                    let rail = ringdesign_core::curve::CurveLayer {
                        height_mm: 0.9,
                        width_mm: 0.35,
                        points: vec![
                            [0.0, ctx.crest_v_mm - 0.4],
                            [0.5, ctx.crest_v_mm + 0.4],
                            [1.0, ctx.crest_v_mm - 0.4],
                        ],
                        ..Default::default()
                    };
                    app.design
                        .layers
                        .layers
                        .push(ringdesign_core::field::LayerEntry::new(
                            "Release test rail",
                            ringdesign_core::Layer::Curve(rail),
                        ));
                    state.layer = Some(app.design.layers.layers.len() - 1);
                    app.mark_dirty();
                    app.history.commit(&app.design);
                }
            });
            changed |= number(ui, "Sample pitch", &mut setup.sample_pitch_mm, 0.025..=2.0);
            changed |= number(
                ui,
                "Obstruction tolerance",
                &mut setup.tolerance_mm,
                0.001..=0.25,
            );
            ui.weak("The samples and interval cross-section below are the actual data used by the release check. Smaller pitch costs more time and can reveal missed details.");
        }
        4 => changed |= shop_data(app, ui, &mut setup, &mut state),
        _ => {}
    }
    if changed {
        app.design.manufacturing = Some(setup.clone());
        app.mark_dirty();
        state.preview = None;
    }
    let current = key(app, &setup);
    if state.preview.as_ref().is_some_and(|p| p.source != current) {
        state.preview = None;
    }
    if let Some(task) = state.task.as_ref() {
        match task.receiver.try_recv() {
            Ok(result) => {
                let task_key = task.key;
                state.task = None;
                match result {
                    Ok(TaskResult::Export(Ok(msg))) => state.message = msg,
                    Ok(TaskResult::Export(Err(e))) => state.error = Some(e),
                    Ok(TaskResult::Orientations(rows)) if task_key == current => {
                        state.orientations = rows;
                        state.orientation_key = current;
                    }
                    Ok(TaskResult::Rankings(rows)) if task_key == current => {
                        state.rankings = rows;
                        state.ranking_key = current;
                    }
                    Ok(TaskResult::Inspection(i)) if task_key == current => {
                        state.inspection = Some(i);
                        state.report_key = current;
                    }
                    Ok(TaskResult::Preview(p)) if task_key == current => state.preview = Some(*p),
                    Err(e) if task_key == current => state.error = Some(e),
                    _ => {}
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.task = None;
                state.error = Some("Worker stopped; recheck to retry".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if state.task.is_none()
        && state.requested != current
        && state.preview.is_none()
        && app.is_current()
    {
        state.requested = current;
        let d = app.design.clone();
        let lib = app.lib.clone();
        let s = setup.clone();
        let p = params(app);
        spawn(&mut state, current, ui.ctx().clone(), move || {
            mf::inspect(&d, &lib, &s, p)
                .map(|i| TaskResult::Inspection(Arc::new(i)))
                .map_err(|e| e.to_string())
        });
    }
    if state.task.is_some() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.weak("Preparing and checking the pattern…");
        });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(60));
    }
    if let Some(e) = &state.error {
        ui.colored_label(theme::BAD, e);
    }
    if !state.message.is_empty() {
        ui.label(&state.message);
    }
    let fresh = state.report_key == current && app.is_current();
    let inspection = if app.is_current() {
        state
            .preview
            .as_ref()
            .map(|p| p.inspection.clone())
            .or_else(|| fresh.then(|| state.inspection.clone()).flatten())
    } else {
        None
    };
    if let Some(i) = inspection {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                color(i.release.status),
                egui::RichText::new(i.release.status.label()).strong(),
            );
            ui.label(format!(
                "{} obstructions • parting {:+.2} mm • pattern ×{:.5}",
                i.release.obstructions.len(),
                i.release.parting_mm,
                i.prepared.scale
            ));
            if state.preview.is_some() {
                ui.colored_label(theme::INFO, "Correction preview");
            }
        });
        if i.release.grid.contains(&0) {
            for note in &i.release.notes {
                ui.colored_label(theme::BAD, note);
            }
            app.casting = state;
            return;
        }
        if state.tab == 1 {
            repairs(app, ui, &setup, &i, &mut state, current);
        }
        if state.tab == 2 {
            if let Some((theta, modulus)) = i.hot_spot {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Feeding aid: thick section near {theta:.0}° (proxy {modulus:.2} mm)"
                    ));
                    if ui.button("Place gate near this section").clicked() {
                        let angle = theta.to_radians();
                        let radius = app.design.inner_radius_mm() + app.design.profile.thickness_mm;
                        let target = i.release.frame.project([
                            radius * angle.cos() * i.prepared.scale,
                            radius * angle.sin() * i.prepared.scale,
                            0.0,
                        ]);
                        if let Some(c) = i
                            .release
                            .columns
                            .iter()
                            .filter(|c| c.metal_at(i.release.parting_mm))
                            .min_by(|a, b| {
                                (a.x - target[0])
                                    .hypot(a.y - target[1])
                                    .total_cmp(&(b.x - target[0]).hypot(b.y - target[1]))
                            })
                        {
                            let len = c.x.hypot(c.y).max(1e-9);
                            let outer = setup.flask.width_mm.min(setup.flask.length_mm) * 0.5 - 2.0;
                            setup.channels.push(Channel {
                                kind: ChannelKind::Gate,
                                start: [c.x, c.y, i.release.parting_mm],
                                end: [c.x / len * outer, c.y / len * outer, i.release.parting_mm],
                                diameter_mm: 2.5,
                            });
                            app.design.manufacturing = Some(setup.clone());
                            app.mark_dirty();
                        }
                    }
                });
            }
            export_controls(app, ui, &setup, &mut state, current);
        }
        if state.tab == 3 {
            ui.label(format!(
                "{} × {} rays • {} occupied • {} unresolved • {:.3} × {:.3} mm spacing",
                i.release.grid[0],
                i.release.grid[1],
                i.release.occupied_rays,
                i.release.unresolved_rays,
                i.release.cell_mm[0],
                i.release.cell_mm[1]
            ));
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.view, 0, "Mold opening");
            ui.selectable_value(&mut state.view, 1, "Mold plan");
            ui.selectable_value(&mut state.view, 2, "Pull section");
            if ui.small_button("Closed").clicked() {
                state.opening = 0.0;
                state.withdrawal = 0.0;
            }
            if ui.small_button("Open upper mold").clicked() {
                state.opening = 1.0;
                state.withdrawal = 0.0;
            }
            if ui.small_button("Withdraw pattern").clicked() {
                state.opening = 1.0;
                state.withdrawal = 1.0;
            }
        });
        if state.view == 0 {
            ui.horizontal_wrapped(|ui| {
                ringdesign_workbench::controls::slider(ui, "Upper mold", egui::Slider::new(&mut state.opening, 0.0..=1.0));
                ringdesign_workbench::controls::slider(ui, "Pattern withdrawal", egui::Slider::new(&mut state.withdrawal, 0.0..=1.0));
                ui.checkbox(&mut state.show_cope, "Upper");
                ui.checkbox(&mut state.show_drag, "Lower");
            });
        }
        let space = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(
            vec2(space.x.max(100.0), (space.y - 52.0).max(200.0)),
            egui::Sense::click(),
        );
        ui.painter().rect_filled(rect, 4.0, theme::VIEWPORT_BG);
        match state.view {
            0 => opening(ui, rect, &setup, &i, &state),
            1 => plan(ui, rect, &setup, &i, &state),
            _ => section(ui, rect, &i, &state),
        }
        if resp.clicked() && state.view == 1 {
            if let Some(p) = resp.interact_pointer_pos() {
                let transform = plan_transform(rect, &setup);
                state.selected = i
                    .release
                    .obstructions
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        transform(a.point)
                            .distance_sq(p)
                            .total_cmp(&transform(b.point).distance_sq(p))
                    })
                    .map(|(k, _)| k);
            }
        }
        ui.weak("Red: blocked withdrawal. Amber: review. Mold surfaces show sampled cavities; inspect fine details in the ring view too.");
    } else if state.error.is_none() {
        ui.weak(
            "The current recipe and design need a new inspection. Previous results are hidden.",
        );
    }
    app.casting = state;
}

fn shop_data(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    setup: &mut Setup,
    state: &mut CastingState,
) -> bool {
    let mut recipe_changed = false;
    ui.weak("Local workshop observations feed measured shrink calibration and future ML experiments. Data stays in this project until you export it.");
    egui::ScrollArea::vertical().max_height(360.0).id_salt("trial_entry").show(ui,|ui|{
        ui.horizontal_wrapped(|ui|{
            if ui.button("New casting trial").clicked() {state.trial=mf::trials::Trial {id:format!("trial-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_millis())),design_family:app.design.name.clone(),recipe:setup.recipe.clone(),..Default::default()};}
            if ui.button("Use exported report…").clicked() {if let Some(path)=rfd::FileDialog::new().add_filter("Manufacturing report",&["json"]).pick_file() {
                let result=(||->anyhow::Result<()>{let v:serde_json::Value=serde_json::from_slice(&std::fs::read(path)?)?;state.trial.pattern_fingerprint=v["pattern_fingerprint"].as_str().ok_or_else(||anyhow::anyhow!("Report has no pattern fingerprint"))?.into();state.trial.recipe=serde_json::from_value(v["setup"]["recipe"].clone())?;Ok(())})();if let Err(e)=result {state.error=Some(e.to_string());}
            }}
        });
        for (label,value) in [("Trial ID",&mut state.trial.id),("Design family",&mut state.trial.design_family),("Casting run",&mut state.trial.run_id),("Printed pattern fingerprint",&mut state.trial.pattern_fingerprint),("Pattern material",&mut state.trial.pattern_material)] {ui.horizontal(|ui|{ui.label(label);ui.text_edit_singleline(value);});}
        ui.horizontal_wrapped(|ui|{ui.label("Actual release");for outcome in [mf::trials::ReleaseOutcome::NotTried,mf::trials::ReleaseOutcome::Clean,mf::trials::ReleaseOutcome::Dragged,mf::trials::ReleaseOutcome::BrokenMold] {ui.selectable_value(&mut state.trial.release,outcome,format!("{outcome:?}"));}ringdesign_workbench::controls::slider(ui, "Detail quality", egui::Slider::new(&mut state.trial.detail_quality,0..=5));});
        if ui.button("Add measured dimension").clicked()&&state.trial.measurements.len()<100 {state.trial.measurements.push(mf::trials::Measurement {label:"Bore diameter".into(),pattern_mm:0.0,as_cast_mm:0.0,finished_mm:None});}
        for m in &mut state.trial.measurements {ui.horizontal_wrapped(|ui|{ui.text_edit_singleline(&mut m.label);number(ui,"Printed",&mut m.pattern_mm,0.0..=999.0);number(ui,"As cast",&mut m.as_cast_mm,0.0..=999.0);let mut finished=m.finished_mm.is_some();if ui.checkbox(&mut finished,"Finished").changed() {m.finished_mm=finished.then_some(m.as_cast_mm);}if let Some(v)=&mut m.finished_mm {number(ui,"Final",v,0.0..=999.0);}});}
        ui.label("Defects / release notes");ui.text_edit_multiline(&mut state.trial.defects);
        if ui.button("Record observation in project").clicked() {
            match state.trial.validate() {Ok(()) if !app.design.casting_trials.iter().any(|t|t.id==state.trial.id)=>{app.history.commit(&app.design);app.design.casting_trials.push(state.trial.clone());app.mark_dirty();state.message="Workshop observation recorded locally".into();state.calibration=None;},Ok(())=>state.error=Some("Trial ID already exists; use a new trial ID".into()),Err(e)=>state.error=Some(e.to_string())}
        }
        ui.separator();ui.label(format!("{} recorded trials",app.design.casting_trials.len()));
        ui.horizontal_wrapped(|ui|{
            if ui.button("Calculate measured shrink").clicked() {match mf::trials::calibrate(&app.design.casting_trials,&setup.recipe) {Ok(c)=>state.calibration=Some(c),Err(e)=>state.error=Some(e.to_string())}}
            if ui.button("Export dataset…").clicked() {if let Some(path)=rfd::FileDialog::new().add_filter("JSON",&["json"]).add_filter("CSV",&["csv"]).set_file_name("casting-trials.json").save_file() {let result=(||->anyhow::Result<()>{let bytes=if path.extension().is_some_and(|e|e=="csv") {mf::trials::csv(&app.design.casting_trials)?.into_bytes()} else {serde_json::to_vec_pretty(&app.design.casting_trials)?};ringdesign_core::library::write_atomic(&path,&bytes)?;Ok(())})();match result {Ok(())=>state.message="Local dataset exported".into(),Err(e)=>state.error=Some(e.to_string())}}}
        });
        if let Some(c)=&state.calibration {
            ui.label(format!("{:.3}% shrink from {} dimensions / {} runs / {} families • fit error {:.4} mm",c.shrink_pct,c.dimensions,c.runs,c.families,c.training_mae_mm));
            ui.weak(c.held_out_family_mae_mm.map_or_else(||"More design families are needed for held-out evaluation".into(),|e|format!("Held-out family error: {e:.4} mm")));
            if ui.button("Apply measured shrink to recipe").clicked() {setup.recipe.shrink_pct=c.shrink_pct;setup.recipe.calibration_note=format!("{}: {} dimensions, {} runs; fit MAE {:.4} mm",c.model_version,c.dimensions,c.runs,c.training_mae_mm);recipe_changed=true;}
            ui.weak("This is a measured scale baseline. A neural model needs enough shop data to beat this baseline on held-out designs.");
        }
    });
    recipe_changed
}

fn number(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(v)
                .speed(0.02)
                .range(range)
                .suffix(" mm")
                .max_decimals(3),
        )
        .changed()
    })
    .inner
}
fn settings(ui: &mut egui::Ui, s: &mut Setup, state: &mut CastingState) -> bool {
    let mut c = false;
    ui.horizontal_wrapped(|ui| {
        for sand in ringdesign_core::castability::SandProcess::ALL {
            if ui.button(sand.label()).clicked() {
                s.recipe = Recipe::sand(*sand);
                c = true;
            }
        }
        if ui.button("Load recipe…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Recipe", &["json"])
                .pick_file()
            {
                match Recipe::load(path) {
                    Ok(r) => {
                        s.recipe = r;
                        c = true;
                    }
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
        }
        if ui.button("Save recipe…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("shop.recipe.json")
                .save_file()
            {
                match s.recipe.save(path) {
                    Ok(()) => state.message = "Recipe saved".into(),
                    Err(e) => state.error = Some(e.to_string()),
                }
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Recipe");
        c |= ui.text_edit_singleline(&mut s.recipe.name).changed();
        egui::ComboBox::from_id_salt("casting_alloy")
            .selected_text(&s.recipe.alloy)
            .show_ui(ui, |ui| {
                for metal in ringdesign_core::metal::METALS {
                    if ui
                        .selectable_value(&mut s.recipe.alloy, metal.name.to_string(), metal.name)
                        .changed()
                    {
                        s.recipe.shrink_pct = metal.shrink_pct;
                        c = true;
                    }
                }
            });
        for process in ringdesign_core::castability::CastProcess::ALL {
            c |= ui
                .selectable_value(&mut s.recipe.process, *process, process.label())
                .changed();
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Shrink");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.recipe.shrink_pct)
                    .range(0.0..=15.0)
                    .speed(0.05)
                    .suffix("%"),
            )
            .changed();
        ui.label("Min draft");
        c |= ui
            .add(
                egui::DragValue::new(&mut s.recipe.min_draft_deg)
                    .range(0.0..=30.0)
                    .speed(0.1)
                    .suffix("°"),
            )
            .changed();
        c |= number(
            ui,
            "Metal section",
            &mut s.recipe.min_section_mm,
            0.05..=10.0,
        );
        c |= number(ui, "Detail", &mut s.recipe.min_detail_mm, 0.01..=5.0);
    });
    egui::CollapsingHeader::new("Pull, flask, and finishing stock").show(ui,|ui|{
        ui.horizontal_wrapped(|ui|{
            ui.label("Pull");for v in &mut s.pull {c|=ui.add(egui::DragValue::new(v).range(-1.0..=1.0).speed(0.05)).changed();}
            for (name,pull) in [("Z",[0.0,0.0,1.0]),("X",[1.0,0.0,0.0]),("Y",[0.0,1.0,0.0])] {if ui.small_button(name).clicked() {s.pull=pull;c=true;}}
            c|=ui.checkbox(&mut s.auto_parting,"Find parting plane").changed();
            ui.add_enabled_ui(!s.auto_parting,|ui|{c|=number(ui,"Plane",&mut s.parting_mm,-100.0..=100.0);});
        });
        ui.horizontal_wrapped(|ui|{
            c|=number(ui,"Flask width",&mut s.flask.width_mm,1.0..=1000.0);c|=number(ui,"Length",&mut s.flask.length_mm,1.0..=1000.0);
            c|=number(ui,"Upper depth",&mut s.flask.cope_mm,1.0..=1000.0);c|=number(ui,"Lower depth",&mut s.flask.drag_mm,1.0..=1000.0);
        });
        ui.horizontal_wrapped(|ui|{
            c|=number(ui,"Sand margin",&mut s.recipe.sand_margin_mm,0.0..=50.0);c|=number(ui,"Min sand web",&mut s.recipe.min_sand_web_mm,0.05..=10.0);
            egui::ComboBox::from_id_salt("bore_strategy").selected_text(s.bore.label()).show_ui(ui,|ui|{for b in BoreStrategy::ALL {c|=ui.selectable_value(&mut s.bore,b,b.label()).changed();}});
        });
        ui.horizontal_wrapped(|ui|{
            c|=number(ui,"Radial stock",&mut s.radial_stock_mm,0.0..=2.0);c|=number(ui,"Stock each side",&mut s.axial_stock_mm,0.0..=2.0);c|=number(ui,"Bore stock",&mut s.bore_stock_mm,0.0..=2.0);
        });
        ui.label("Allowances enlarge the band profile before shrink compensation. Bore stock is metal left for reaming.");
        ui.label("Calibration / workshop evidence");c|=ui.text_edit_singleline(&mut s.recipe.calibration_note).changed();
        ui.label("Bench instructions");c|=ui.text_edit_multiline(&mut s.bench_notes).changed();
    });
    c
}

fn channels(ui: &mut egui::Ui, s: &mut Setup) -> bool {
    let mut changed = false;
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        ui.label("Channels cut into sand");
        for kind in ChannelKind::ALL {
            if ui.small_button(format!("Add {}", kind.label())).clicked() {
                s.channels.push(Channel {
                    kind,
                    start: [0.0, 12.0, 0.0],
                    end: [0.0, 25.0, 0.0],
                    diameter_mm: if kind == ChannelKind::Vent { 0.6 } else { 2.5 },
                });
                changed = true;
            }
        }
    });
    egui::ScrollArea::vertical()
        .max_height(140.0)
        .id_salt("channel_list")
        .show(ui, |ui| {
            for (n, c) in s.channels.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(c.kind.label());
                        ui.label("From");
                        for v in &mut c.start {
                            changed |= ui
                                .add(
                                    egui::DragValue::new(v)
                                        .speed(0.2)
                                        .range(-1000.0..=1000.0)
                                        .max_decimals(2),
                                )
                                .changed();
                        }
                        ui.label("To");
                        for v in &mut c.end {
                            changed |= ui
                                .add(
                                    egui::DragValue::new(v)
                                        .speed(0.2)
                                        .range(-1000.0..=1000.0)
                                        .max_decimals(2),
                                )
                                .changed();
                        }
                        changed |= number(ui, "Ø", &mut c.diameter_mm, 0.1..=20.0);
                        if ui.small_button("Remove").clicked() {
                            remove = Some(n);
                        }
                    });
                });
            }
        });
    if let Some(n) = remove {
        s.channels.remove(n);
        changed = true;
    }
    changed
}

fn repairs(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    s: &Setup,
    i: &Arc<Inspection>,
    state: &mut CastingState,
    current: u64,
) {
    if ui
        .add_enabled(
            state.task.is_none() && !app.graph_driven() && state.preview.is_none(),
            egui::Button::new("Compare available fixes"),
        )
        .clicked()
    {
        let d = app.design.clone();
        let lib = app.lib.clone();
        let s = s.clone();
        let selected = state.layer;
        let params = params(app);
        spawn(state, current, ui.ctx().clone(), move || {
            repair::rank(&d, &lib, &s, selected, params)
                .map(TaskResult::Rankings)
                .map_err(|e| e.to_string())
        });
    }
    if state.ranking_key == current {
        egui::CollapsingHeader::new("Measured repair comparison").default_open(true).show(ui,|ui|{ui.weak("Ordered by release status, obstruction count/area, and volume change. Select a row, then preview the correction.");for row in &state.rankings {if ui.selectable_label(state.repair==row.repair,format!("{} • {} • {} obstructions • {:+.2}% volume",row.repair.label(),row.status.label(),row.obstructions,row.volume_change_pct)).clicked() {state.repair=row.repair;}}});
    }

    let r = &i.release;
    egui::CollapsingHeader::new("Compare pull orientations").show(ui, |ui| {
        if ui
            .add_enabled(
                state.task.is_none(),
                egui::Button::new("Compare six directions"),
            )
            .clicked()
        {
            let mesh = i.prepared.mesh.clone();
            let setup = s.clone();
            spawn(state, current, ui.ctx().clone(), move || {
                mf::release::compare_orientations(&mesh, &setup)
                    .map(TaskResult::Orientations)
                    .map_err(|e| e.to_string())
            });
        }
        if state.orientation_key == current {
            for row in &state.orientations {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Pull {:?} • plane {:+.2} mm • {} obstructions • {:.1} mm² low draft{}",
                        row.pull,
                        row.parting_mm,
                        row.obstructions,
                        row.low_draft_area_mm2,
                        if row.fits_flask {
                            ""
                        } else {
                            " • flask too small"
                        }
                    ));
                    if ui.small_button("Apply orientation").clicked() {
                        app.history.commit(&app.design);
                        let mut next = s.clone();
                        next.pull = row.pull;
                        next.parting_mm = row.parting_mm;
                        next.auto_parting = false;
                        app.design.manufacturing = Some(next);
                        app.mark_dirty();
                        app.history.commit(&app.design);
                    }
                });
            }
        }
    });
    egui::ScrollArea::vertical()
        .max_height(150.0)
        .id_salt("release_findings")
        .show(ui, |ui| {
            for (n, o) in r.obstructions.iter().enumerate() {
                if ui
                    .selectable_label(
                        state.selected == Some(n),
                        format!(
                            "{} • {:.3} mm obstruction • {:.2} mm²",
                            o.half.label(),
                            o.depth_mm,
                            o.projected_area_mm2
                        ),
                    )
                    .clicked()
                {
                    state.selected = Some(n);
                    state.layer = repair::layer_at(
                        &app.design,
                        &app.lib,
                        o.world.map(|v| v / i.prepared.scale),
                    );
                    app.selected_layer = state.layer;
                    let theta = o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0);
                    for pane in &mut app.panes {
                        if pane.kind == PaneKind::Section {
                            pane.section_theta_deg = theta;
                        }
                    }
                    app.refresh_sections();
                }
            }
            for f in &r.sand_findings {
                ui.colored_label(theme::WARN, &f.message);
            }
            for detail in &i.details {
                ui.colored_label(theme::WARN, detail);
            }
            if let Some(field) = &i.field {
                if field.thinnest_wall_mm < s.recipe.min_section_mm {
                    ui.colored_label(
                        theme::WARN,
                        format!(
                            "Radial metal wall {:.3} mm; recipe asks for {:.3} mm",
                            field.thinnest_wall_mm, s.recipe.min_section_mm
                        ),
                    );
                }
            }
            for note in &r.notes {
                ui.weak(note);
            }
        });
    if app.graph_driven() {
        ui.horizontal_wrapped(|ui| {
            ui.weak("The graph owns geometry edits.");
            if ui.button("Make an editable copy").clicked() {
                app.history.commit(&app.design);
                app.design.name = format!("{} (editable)", app.design.name);
                app.bake_graph();
                app.history.commit(&app.design);
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("repair_layer")
            .selected_text(
                state
                    .layer
                    .and_then(|n| app.design.layers.layers.get(n))
                    .map_or("Select layer", |e| e.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for (n, e) in app.design.layers.layers.iter().enumerate() {
                    ui.selectable_value(
                        &mut state.layer,
                        Some(n),
                        format!("{}{}", e.name, if e.bench_only { " (bench)" } else { "" }),
                    );
                }
            });
        egui::ComboBox::from_id_salt("repair_kind")
            .selected_text(state.repair.label())
            .show_ui(ui, |ui| {
                for repair in Repair::ALL {
                    ui.selectable_value(&mut state.repair, repair, repair.label());
                }
            });
        if ui
            .add_enabled(
                state.task.is_none(),
                egui::Button::new("Preview correction"),
            )
            .clicked()
        {
            match repair::candidate(
                &app.design,
                s,
                state.layer,
                state.repair,
                r.suggested_parting_mm,
            ) {
                Ok(d) => {
                    let lib = app.lib.clone();
                    let p = params(app);
                    spawn(state, current, ui.ctx().clone(), move || {
                        let inspected = mf::inspect(&d, &lib, d.manufacturing.as_ref().unwrap(), p)
                            .map_err(|e| e.to_string())?;
                        Ok(TaskResult::Preview(Box::new(Preview {
                            design: d,
                            inspection: Arc::new(inspected),
                            source: current,
                        })))
                    });
                }
                Err(e) => state.error = Some(e.to_string()),
            }
        }
    });
    if let Some(preview) = &state.preview {
        if let Some(before) = &state.inspection {
            ui.label(format!(
                "Obstructions {} → {}; ring {:.2} → {:.2} g",
                before.release.obstructions.len(),
                preview.inspection.release.obstructions.len(),
                before.ring_grams,
                preview.inspection.ring_grams
            ));
        }
        ui.horizontal(|ui| {
            if ui.button("Apply correction").clicked() {
                if let Some(p) = state.preview.take() {
                    app.history.commit(&app.design);
                    app.design = p.design;
                    app.mark_dirty();
                    app.history.commit(&app.design);
                    state.message = "Correction applied; undo restores the previous design".into();
                }
            }
            if ui.button("Cancel preview").clicked() {
                state.preview = None;
            }
        });
    }
}

fn export_controls(
    app: &mut RingDesignerApp,
    ui: &mut egui::Ui,
    s: &Setup,
    state: &mut CastingState,
    current: u64,
) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut state.diagnostic, "Diagnostic package")
            .on_hover_text("Explicitly labels obstructed or unresolved patterns for inspection");
        if ui
            .add_enabled(
                state.task.is_none() && state.preview.is_none(),
                egui::Button::new("Export pattern package…"),
            )
            .clicked()
        {
            if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis());
                let path = parent.join(format!("ring-pattern-{stamp}"));
                let d = app.design.clone();
                let lib = app.lib.clone();
                let s = s.clone();
                let params = app.export_params;
                let diagnostic = state.diagnostic;
                spawn(state, current, ui.ctx().clone(), move || {
                    Ok(TaskResult::Export(
                        mf::package::export(&path, &d, &lib, &s, params, diagnostic)
                            .map(|_| format!("Package saved to {}", path.display()))
                            .map_err(|e| e.to_string()),
                    ))
                });
            }
        }
    });
    ui.weak("Exports STL, 3MF, recipe, source design, exact-pattern report, mold plan, and a printable HTML sheet. Open the sheet in a browser to print.");
}

fn plan_transform(rect: Rect, s: &Setup) -> impl Fn([f64; 3]) -> Pos2 {
    let scale = ((rect.width() - 28.0) / s.flask.width_mm as f32)
        .min((rect.height() - 28.0) / s.flask.length_mm as f32);
    let center = rect.center();
    move |p| center + vec2(p[0] as f32, -p[1] as f32) * scale
}
fn plan(ui: &egui::Ui, rect: Rect, s: &Setup, i: &Inspection, state: &CastingState) {
    let p = ui.painter_at(rect);
    let map = plan_transform(rect, s);
    let r = &i.release;
    let bounds = Rect::from_two_pos(
        map([-s.flask.width_mm / 2.0, -s.flask.length_mm / 2.0, 0.0]),
        map([s.flask.width_mm / 2.0, s.flask.length_mm / 2.0, 0.0]),
    );
    p.rect_filled(bounds, 2.0, Color32::from_rgb(60, 55, 49));
    for c in &r.columns {
        if c.intervals.is_empty() {
            continue;
        }
        let cell = Rect::from_two_pos(
            map([c.x - r.cell_mm[0] * 0.5, c.y - r.cell_mm[1] * 0.5, 0.0]),
            map([c.x + r.cell_mm[0] * 0.5, c.y + r.cell_mm[1] * 0.5, 0.0]),
        );
        let blocked = c.upper_block_mm > 0.0 || c.lower_block_mm > 0.0;
        p.rect_filled(
            cell.expand(0.2),
            0.0,
            if blocked {
                theme::BAD
            } else {
                Color32::from_rgb(151, 160, 179)
            },
        );
    }
    for c in &s.channels {
        p.line_segment(
            [map(c.start), map(c.end)],
            Stroke::new(
                c.diameter_mm as f32 * (bounds.width() / s.flask.width_mm as f32),
                theme::WARN,
            ),
        );
        p.text(
            map(c.end),
            egui::Align2::LEFT_BOTTOM,
            c.kind.label(),
            egui::FontId::proportional(12.0),
            theme::TEXT,
        );
    }
    for (n, o) in r.obstructions.iter().enumerate() {
        p.circle_stroke(
            map(o.point),
            if state.selected == Some(n) { 9.0 } else { 4.0 },
            Stroke::new(2.0, theme::BAD),
        );
    }
    p.text(
        rect.left_top() + vec2(10.0, 10.0),
        egui::Align2::LEFT_TOP,
        format!("Bore: {} • brown regions are sand", s.bore.label()),
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );
}

fn opening(ui: &egui::Ui, rect: Rect, s: &Setup, i: &Inspection, state: &CastingState) {
    let p = ui.painter_at(rect);
    let r = &i.release;
    let plane = r.parting_mm;
    let separation = s.flask.cope_mm * 1.5 + 12.0;
    let lift = state.opening * separation;
    let withdraw = state.withdrawal * (s.flask.drag_mm + 5.0);
    let scale = (rect.width() / (s.flask.width_mm + s.flask.length_mm) as f32 * 1.3).min(
        rect.height()
            / (s.flask.cope_mm + s.flask.drag_mm + separation + s.flask.length_mm * 0.65) as f32
            * 0.85,
    );
    let origin = rect.center() + vec2(0.0, rect.height() * 0.16);
    let map = |v: [f64; 3]| {
        origin
            + vec2(
                (v[0] - v[1] * 0.62) as f32,
                (v[0] * 0.20 + v[1] * 0.34 - v[2]) as f32,
            ) * scale
    };
    let wire_box = |bottom: f64, top: f64, tint: Color32| {
        let w = s.flask.width_mm / 2.0;
        let h = s.flask.length_mm / 2.0;
        let points = [
            [-w, -h, bottom],
            [w, -h, bottom],
            [w, h, bottom],
            [-w, h, bottom],
            [-w, -h, top],
            [w, -h, top],
            [w, h, top],
            [-w, h, top],
        ];
        for face in [[0, 1, 5, 4], [1, 2, 6, 5], [4, 5, 6, 7]] {
            p.add(egui::Shape::convex_polygon(
                face.map(|k| map(points[k])).to_vec(),
                tint.gamma_multiply(0.18),
                Stroke::new(1.0, tint),
            ));
        }
        for [a, b] in [
            [0, 1],
            [1, 2],
            [2, 3],
            [3, 0],
            [4, 5],
            [5, 6],
            [6, 7],
            [7, 4],
            [0, 4],
            [1, 5],
            [2, 6],
            [3, 7],
        ] {
            p.line_segment([map(points[a]), map(points[b])], Stroke::new(1.0, tint));
        }
    };
    if state.show_drag {
        wire_box(
            plane - s.flask.drag_mm,
            plane,
            Color32::from_rgb(120, 103, 80),
        );
    }
    // The sampled upper/lower boundaries are the mold cavity surfaces. Coarse
    // display decimation is independent of the full-resolution analysis.
    let nx = r.grid[0];
    let ny = r.grid[1];
    let step = (nx.max(ny) / 80).max(1);
    for j in (0..ny).step_by(step) {
        for x in (0..nx).step_by(step) {
            let c = &r.columns[j * nx + x];
            if c.intervals.is_empty() {
                continue;
            }
            let sx = r.cell_mm[0] * step as f64;
            let sy = r.cell_mm[1] * step as f64;
            let quad = |z: f64| {
                vec![
                    map([c.x - sx / 2.0, c.y - sy / 2.0, z]),
                    map([c.x + sx / 2.0, c.y - sy / 2.0, z]),
                    map([c.x + sx / 2.0, c.y + sy / 2.0, z]),
                    map([c.x - sx / 2.0, c.y + sy / 2.0, z]),
                ]
            };
            if state.show_drag {
                p.add(egui::Shape::convex_polygon(
                    quad(c.bottom(plane)),
                    Color32::from_rgba_unmultiplied(174, 149, 107, 110),
                    Stroke::NONE,
                ));
            }
            let blocked = c.upper_block_mm > 0.0 || c.lower_block_mm > 0.0;
            p.add(egui::Shape::convex_polygon(
                quad(c.top(plane) + withdraw),
                if blocked {
                    theme::BAD
                } else {
                    Color32::from_rgb(162, 171, 194)
                },
                Stroke::NONE,
            ));
            if state.show_cope {
                p.add(egui::Shape::convex_polygon(
                    quad(c.top(plane) + lift),
                    Color32::from_rgba_unmultiplied(160, 147, 126, 90),
                    Stroke::NONE,
                ));
            }
        }
    }
    if state.show_cope {
        wire_box(
            plane + lift,
            plane + lift + s.flask.cope_mm,
            Color32::from_rgb(167, 147, 116),
        );
    }
    for (n, o) in r.obstructions.iter().enumerate() {
        if state.selected == Some(n) {
            let mut v = o.point;
            v[2] += withdraw;
            p.circle_stroke(map(v), 10.0, Stroke::new(2.5, theme::BAD));
        }
    }
    let x = s.flask.width_mm * 0.52;
    let a = map([x, 0.0, plane]);
    let b = map([x, 0.0, plane + 15.0]);
    p.arrow(a, b - a, Stroke::new(2.0, theme::INFO));
    p.text(
        b,
        egui::Align2::LEFT_BOTTOM,
        "Pull",
        egui::FontId::proportional(12.0),
        theme::INFO,
    );
    p.text(
        rect.left_top() + vec2(12.0, 12.0),
        egui::Align2::LEFT_TOP,
        "Upper mold opens first; pattern then leaves the lower mold",
        egui::FontId::proportional(13.0),
        theme::TEXT,
    );
    p.text(
        rect.left_bottom() + vec2(12.0, -12.0),
        egui::Align2::LEFT_BOTTOM,
        format!("Finger-hole sand stays with the mold • {}", s.bore.label()),
        egui::FontId::proportional(12.0),
        theme::TEXT,
    );
}

fn section(ui: &egui::Ui, rect: Rect, i: &Inspection, state: &CastingState) {
    let r = &i.release;
    let p = ui.painter_at(rect);
    let target = state
        .selected
        .and_then(|n| r.obstructions.get(n))
        .map_or(0.0, |o| o.point[1]);
    let row = (((target - r.bounds[0][1]) / r.cell_mm[1]).floor() as isize)
        .clamp(0, r.grid[1] as isize - 1) as usize;
    let min = r.bounds[0];
    let max = r.bounds[1];
    let scale = ((rect.width() - 40.0) / (max[0] - min[0]) as f32)
        .min((rect.height() - 50.0) / (max[2] - min[2] + 2.0) as f32);
    let map = |x: f64, z: f64| {
        rect.center()
            + vec2(
                (x - (min[0] + max[0]) / 2.0) as f32,
                (-z + (min[2] + max[2]) / 2.0) as f32,
            ) * scale
    };
    let plane = r.parting_mm;
    p.line_segment(
        [map(min[0], plane), map(max[0], plane)],
        Stroke::new(1.0, theme::INFO),
    );
    for c in &r.columns[row * r.grid[0]..(row + 1) * r.grid[0]] {
        for &[a, b] in &c.intervals {
            p.rect_filled(
                Rect::from_two_pos(
                    map(c.x - r.cell_mm[0] / 2.0, a),
                    map(c.x + r.cell_mm[0] / 2.0, b),
                ),
                0.0,
                if c.upper_block_mm > 0.0 || c.lower_block_mm > 0.0 {
                    theme::BAD
                } else {
                    theme::TEXT_DIM
                },
            );
        }
    }
    p.text(
        rect.left_top() + vec2(12.0, 12.0),
        egui::Align2::LEFT_TOP,
        format!(
            "Actual metal intervals at y = {:.3} mm • blue = parting plane",
            r.columns[row * r.grid[0]].y
        ),
        egui::FontId::proportional(13.0),
        theme::TEXT,
    );
}
