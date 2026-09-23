//! Patterns of a part and press-pull from the Ring viewport's right-click, each one funnel commit.
use std::sync::{Arc, Mutex};

use ringdesign_core::{
    BuildResult, Mesh, Vec3,
    cad::{Attach, EvaluatedComponent, Feature, MirrorPlane, Operation, PatternKind, builders, edit::CadEdit, pattern},
    sketch::Id,
};
use ringdesign_workbench::command::pattern::{ArrayCmd, PressPullCmd, pattern_feature};
use ringdesign_workbench::command::{Affine, Dimension, Outcome, Preview, StepInfo, StepInput, ViewCommand};
use ringdesign_workbench::viewport::{Mods, Sel, patterns as keys};
use ringdesign_workbench::visual::Tool;

use crate::app::RingDesignerApp;
use crate::viewport::GpuMeshRenderer;

/// How far from a part a stone may stand and still be the one an array turns about, mm.
pub const STONE_REACH_MM: f64 = 8.0;
/// Closer than this to the plane it would be mirrored across, a part is its own mirror, mm or degrees.
const ON_PLANE: f64 = 0.05;

#[cfg(test)]
thread_local! {
    /// The last ghost a pattern or a press-pull staged, for the tests to read.
    pub(crate) static STAGED: std::cell::RefCell<Mesh> = std::cell::RefCell::new(Mesh::default());
}

/// A part as the last build evaluated it.
fn component(build: &BuildResult, id: Id) -> Option<&EvaluatedComponent> {
    build.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == id)
}

/// The part `feature` from the document and the last build, or why the gesture cannot start.
fn part(app: &RingDesignerApp, feature: Id) -> Result<(Feature, Arc<BuildResult>), String> {
    let f = app.design.cad.as_ref().and_then(|d| d.feature(feature)).cloned().ok_or_else(|| format!("Part #{feature} is not in the document"))?;
    let build = app.build.clone().ok_or("The ring has not built yet; try once it has")?;
    if component(&build, feature).is_none() {
        return Err(format!("#{feature} {} did not build; mend it on the timeline first", f.name));
    }
    if f.component.reference {
        return Err("A reference stone is patterned with its setting; pattern the setting instead".into());
    }
    Ok((f, build))
}

/// Starts the pattern named by `key` on a part: an array round the ring or round its stone, or a mirror.
pub fn start(app: &mut RingDesignerApp, pane: usize, feature: u64, key: &'static str) {
    let (f, build) = match part(app, feature) {
        Ok(p) => p,
        Err(why) => return app.set_status(why),
    };
    let c = component(&build, feature).expect("checked by part");
    let attach = c.attach;
    match key {
        keys::RING_ARRAY => {
            let ghost = copies_ghost(app, &build, f.id, c.mesh.clone());
            begin(app, pane, Ghosted::new(ArrayCmd::new(0, f, attach, None), app.renderer.clone(), ghost));
        }
        keys::STONE_ARRAY => match stone_by(app, &build, &f, c) {
            Some(stone) => {
                let ghost = copies_ghost(app, &build, f.id, c.mesh.clone());
                begin(app, pane, Ghosted::new(ArrayCmd::new(0, f, attach, Some(stone)), app.renderer.clone(), ghost));
            }
            None => app.set_status(format!("#{} {} stands on no stone and by none within {STONE_REACH_MM} mm; set a stone first", f.id, f.name)),
        },
        keys::MIRROR_BAND => mirror(app, &f, attach, MirrorPlane::Band),
        keys::MIRROR_HEAD => {
            let theta_deg = app.design.shank.head.theta_deg;
            mirror(app, &f, attach, MirrorPlane::Section { theta_deg })
        }
        _ => app.set_status(format!("No pattern called {key}")),
    }
}

/// Starts pushing or pulling a planar face of a part along its normal.
pub fn press_pull(app: &mut RingDesignerApp, pane: usize, feature: u64, face: u32) {
    let (f, build) = match part(app, feature) {
        Ok(p) => p,
        Err(why) => return app.set_status(why),
    };
    let c = component(&build, feature).expect("checked by part");
    let (signed, centre, normal) = match pattern::planar_face(c, face) {
        Ok(found) => found,
        Err(e) => return app.set_status(format!("{e:#}")),
    };
    let ghost = face_ghost(c, face, normal);
    begin(app, pane, Ghosted::new(PressPullCmd::new(f, signed, c.attach, centre, normal, 0), app.renderer.clone(), ghost));
}

/// Adds the mirror of part `feature` across work plane `plane` as one funnel commit and chooses it.
pub fn mirror_across(app: &mut RingDesignerApp, feature: Id, plane: Id) {
    let (f, build) = match part(app, feature) {
        Ok(p) => p,
        Err(why) => return app.set_status(why),
    };
    let attach = component(&build, feature).map_or(f.component.attach, |c| c.attach);
    mirror(app, &f, attach, MirrorPlane::Plane { feature: plane });
}

/// Adds the mirror of `f` across `plane` as one funnel commit and chooses it; a part standing on the plane is its own mirror.
fn mirror(app: &mut RingDesignerApp, f: &Feature, attach: Attach, plane: MirrorPlane) {
    if let Some(doc) = app.design.cad.as_ref()
        && let Some((_, ringdesign_core::cad::Placement::Ring { theta_deg, across_mm, .. })) = pattern::seat_of(doc, f.id)
    {
        let on = match &plane {
            MirrorPlane::Band => across_mm.abs() < ON_PLANE,
            MirrorPlane::Section { theta_deg: through } => ((theta_deg - through + 180.0).rem_euclid(360.0) - 180.0).abs() < ON_PLANE,
            MirrorPlane::Plane { .. } => false,
        };
        if on {
            return app.set_status(format!("#{} {} stands on that plane already; its mirror would be itself", f.id, f.name));
        }
    }
    let feature = pattern_feature(0, f, attach, PatternKind::Mirror { plane });
    if let Ok(applied) = crate::cad_edit::apply(app, &[CadEdit::Add { feature, after: None }])
        && let Some(id) = applied.last().and_then(|a| a.id)
    {
        app.selection.click(Some(Sel::Part(id)), Mods::default());
    }
}

/// The stone `f` is built round or seated by, else the nearest stone within reach of it.
fn stone_by(app: &RingDesignerApp, build: &BuildResult, f: &Feature, c: &EvaluatedComponent) -> Option<Id> {
    let doc = app.design.cad.as_ref()?;
    let is_stone = |id: Id| doc.feature(id).is_some_and(|s| matches!(&s.operation, Operation::Builder { key, .. } if key == builders::STONE));
    if let Some((seat, _)) = pattern::seat_of(doc, f.id).filter(|(seat, _)| is_stone(*seat)) {
        return Some(seat);
    }
    let (lo, hi) = c.mesh.bounds()?;
    let centre = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
    let gap = |p: [f64; 3]| (0..3).map(|k| (p[k] - centre[k]).powi(2)).sum::<f64>().sqrt();
    build
        .parts
        .evaluated
        .as_ref()?
        .components
        .iter()
        .filter(|s| is_stone(s.id))
        .map(|s| (s.id, gap(s.frame.origin)))
        .filter(|(_, d)| *d <= STONE_REACH_MM)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// Makes `cmd` the Ring viewport's live command, ending any other, with its prompt on the status line.
fn begin(app: &mut RingDesignerApp, pane: usize, mut cmd: Ghosted) {
    crate::command::cancel(app);
    if app.visual.tool != Tool::Select {
        app.visual.select(Tool::Select);
    }
    app.active_pane = pane;
    cmd.restage();
    app.command.session.start(Box::new(cmd));
    app.command.bar.reset();
    let prompt = app.command.session.prompt();
    app.set_status(prompt);
}

/// What a ghost is drawn from: the command as it stands, read for its triangles in the world.
type Stage = Box<dyn Fn(&dyn ViewCommand) -> Option<Mesh>>;

/// The part's mesh carried onto every copy a pattern command would add, each where the build puts it: a seated part dropped onto the band again at its copy's angle.
fn copies_ghost(app: &RingDesignerApp, build: &BuildResult, source_id: Id, source: Mesh) -> Stage {
    let frames: Vec<(Id, _)> = build.parts.evaluated.iter().flat_map(|e| e.components.iter().map(|c| (c.id, c.frame)).chain(e.planes.iter().map(|p| (p.id, p.placement())))).collect();
    let design = app.design.clone();
    let surface = build.band.clone();
    // The seat as the evaluation reads it: the placement, and the frame it stood by on the band.
    let seat = app.design.cad.as_ref().and_then(|doc| pattern::seat_of(doc, source_id)).and_then(|(_, p)| {
        let used = p.frame_on(&design, surface.as_deref()).ok()?;
        Some((p, used))
    });
    Box::new(move |cmd| {
        let Some(Operation::Pattern { kind, .. }) = cmd.preview().operation else { return None };
        let frame_of = |id: Id| frames.iter().find(|(f, _)| *f == id).map(|(_, p)| *p);
        let motions = pattern::motions(&kind, &design, surface.as_deref(), seat.as_ref().map(|(p, used)| (p, used)), &frame_of).ok()?;
        let mut out = Mesh::default();
        for m in motions {
            let base = out.vertices.len() as u32;
            out.vertices.extend(source.vertices.iter().map(|v| {
                let p = m.point([v.0 as f64, v.1 as f64, v.2 as f64]);
                Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
            }));
            let flip = m.reflects();
            out.faces.extend(source.faces.iter().map(|f| if flip { [f[0] + base, f[2] + base, f[1] + base] } else { f.map(|i| i + base) }));
        }
        Some(out)
    })
}

/// The ghost of a pushed face: the face carried along its normal by the distance, walled to where it stood.
fn face_ghost(c: &EvaluatedComponent, face: u32, normal: [f64; 3]) -> Stage {
    let tris: Vec<[u32; 3]> = c.mesh.faces.iter().enumerate().filter(|(t, _)| c.trace.face_of(*t) == Some(face)).map(|(_, f)| *f).collect();
    let vertices = c.mesh.vertices.clone();
    Box::new(move |cmd| {
        let d = cmd.dimensions().first().map(|dim| dim.value)?;
        let at = |i: u32, by: f64| {
            let v = vertices[i as usize];
            Vec3(v.0 + (normal[0] * by) as f32, v.1 + (normal[1] * by) as f32, v.2 + (normal[2] * by) as f32)
        };
        let mut out = Mesh::default();
        let mut index = std::collections::HashMap::new();
        let mut vertex = |i: u32, moved: bool, out: &mut Mesh| {
            *index.entry((i, moved)).or_insert_with(|| {
                out.vertices.push(at(i, if moved { d } else { 0.0 }));
                out.vertices.len() as u32 - 1
            })
        };
        // Each edge used once by the face's triangles is on its rim, and stands a wall.
        let mut uses: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        for t in &tris {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                *uses.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        for t in &tris {
            let moved = t.map(|i| vertex(i, true, &mut out));
            out.faces.push(moved);
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                if uses[&(a.min(b), a.max(b))] == 1 {
                    let (a0, b0, a1, b1) = (vertex(a, false, &mut out), vertex(b, false, &mut out), vertex(a, true, &mut out), vertex(b, true, &mut out));
                    out.faces.push([a0, b0, b1]);
                    out.faces.push([a0, b1, a1]);
                }
            }
        }
        Some(out)
    })
}

/// A command that stages its own ghost and clears it when cancelled or dropped uncommitted.
pub struct Ghosted {
    cmd: Box<dyn ViewCommand>,
    renderer: Arc<Mutex<GpuMeshRenderer>>,
    stage: Stage,
    /// What the staged ghost was drawn for.
    shown: Option<String>,
    committed: bool,
}

impl Ghosted {
    fn new(cmd: impl ViewCommand + 'static, renderer: Arc<Mutex<GpuMeshRenderer>>, stage: Stage) -> Self {
        Self { cmd: Box::new(cmd), renderer, stage, shown: None, committed: false }
    }

    /// Stages the ghost again when the preview has moved on since it was drawn.
    fn restage(&mut self) {
        let preview = self.cmd.preview();
        let key = format!("{:?} {:?}", preview.operation, preview.placement);
        if self.shown.as_ref() == Some(&key) {
            return;
        }
        let mesh = (self.stage)(self.cmd.as_ref()).unwrap_or_default();
        if let Ok(mut r) = self.renderer.lock() {
            r.prepare_preview(GpuMeshRenderer::stage_part(&mesh));
            r.set_preview_model((!mesh.faces.is_empty()).then(|| crate::command::gl_model(&Affine::IDENTITY)));
        }
        #[cfg(test)]
        STAGED.with(|s| *s.borrow_mut() = mesh);
        self.shown = Some(key);
    }

    fn clear(&mut self) {
        if let Ok(mut r) = self.renderer.lock() {
            r.prepare_preview(Vec::new());
            r.set_preview_model(None);
        }
        #[cfg(test)]
        STAGED.with(|s| *s.borrow_mut() = Mesh::default());
        self.shown = None;
    }
}

impl Drop for Ghosted {
    fn drop(&mut self) {
        if !self.committed {
            self.clear();
        }
    }
}

impl ViewCommand for Ghosted {
    fn key(&self) -> &'static str {
        self.cmd.key()
    }
    fn title(&self) -> String {
        self.cmd.title()
    }
    fn step(&self) -> usize {
        self.cmd.step()
    }
    fn steps(&self) -> Vec<StepInfo> {
        self.cmd.steps()
    }
    fn dimensions(&self) -> Vec<Dimension> {
        self.cmd.dimensions()
    }
    fn feed(&mut self, input: &StepInput) -> Outcome {
        let out = self.cmd.feed(input);
        match &out {
            Outcome::Cancelled => self.clear(),
            Outcome::Commit(_) => self.committed = true,
            _ => self.restage(),
        }
        out
    }
    fn preview(&self) -> Preview {
        self.cmd.preview()
    }
}
