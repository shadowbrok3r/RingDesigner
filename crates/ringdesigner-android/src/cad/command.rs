//! A live command on the phone's ring: a gizmo handle dragged by one finger, a press-pull's arrow, an array awaiting its count.
use std::cell::RefCell;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use egui::{Pos2, Rect};
use egui_mobile::egui;
use ringdesign_core::{
    Mesh, RingDesign, Vec3,
    cad::{Attach, EvaluatedComponent, Feature, Operation, Placement, Stage, pattern},
    castability::{
        CastProcess, FieldReport,
        ghost::{GhostJudge, GhostRead},
    },
    interaction::pick::{Ray, ViewScale},
    sketch::Id,
};
use ringdesign_workbench::command::pattern::{ArrayCmd, PressPullCmd};
use ringdesign_workbench::command::{
    Affine, Axis, BandSurface, DimEvent, DimensionBar, Dofs, Grid, GripCmd, MoveCmd, Outcome, PlaceCmd, RingFeatures, RingPoint, RotateCmd, Scene, Session, SnapGeometry, SnapHit, Snapper, StepInput, ViewCommand, along_line, land, placed_ghost,
};
use ringdesign_workbench::gizmo::{self, Gizmo, Handle, Layout};
use ringdesign_workbench::touch;
use ringdesign_workbench::viewport::{Selection, pins::Pin};

use super::{Built, Request, Then};
use crate::camera::OrbitCamera;
use crate::viewport::GpuMeshRenderer;

/// The grid a carried part snaps to on the ring: 5° round, 0.5 mm across and out.
pub const GRID: Grid = Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 };
/// How long a committed ghost waits for the rebuild before it goes, seconds.
const LINGER_S: f64 = 3.0;
/// A press-pull arrow's length on screen, points.
pub const PULL_PT: f32 = 64.0;
/// The dimension bar's id salt.
const BAR_ID: &str = "phone-ring-dimensions";
/// The dimension bar's height until it has been drawn once, points.
const BAR_PT: f32 = 44.0;
/// A view shorter than this, as with the keyboard up, keeps the command's bars at its bottom and close to the edge, points.
const SHORT_VIEW_PT: f32 = 360.0;
/// Clearance kept under the command's bars on a tall view, for the view's own status line, points.
const FOOT_PT: f32 = 48.0;

/// The area the live command's words and its Done and Cancel stand in.
pub fn caption_area() -> egui::Id {
    egui::Id::new("phone-command-bar")
}

/// The area the dimension bar draws its fields in.
pub fn fields_area() -> egui::Id {
    egui::Id::new(BAR_ID).with("area")
}

/// What a live command reads the ring against.
pub struct Ctx<'a> {
    pub rect: Rect,
    pub camera: &'a OrbitCamera,
    pub design: &'a RingDesign,
    pub build: Option<&'a Built>,
    pub band: Option<&'a Arc<BandSurface>>,
    pub field: Option<&'a FieldReport>,
    pub pins: &'a [Pin],
    pub selection: &'a Selection,
}

impl Ctx<'_> {
    /// The world ray under a screen point.
    pub fn ray(&self, p: Pos2) -> Ray {
        let (o, d) = self.camera.ray(self.rect, p);
        Ray { origin: o.map(f64::from), direction: d.map(f64::from) }
    }

    /// The screen's axes and scale at `p`, and the ray there.
    fn view_at(&self, p: Pos2) -> (ViewScale, Ray) {
        ringdesign_workbench::hover::view_scale(p, &|q| self.camera.ray(self.rect, q))
    }

    fn feature(&self, id: Id) -> Option<Feature> {
        self.design.cad.as_ref()?.feature(id).cloned()
    }

    fn component(&self, id: Id) -> Option<&EvaluatedComponent> {
        self.build?.evaluated()?.components.iter().find(|c| c.id == id)
    }

    fn project(&self, p: [f64; 3]) -> Pos2 {
        self.camera.projector(self.rect).at(p.map(|v| v as f32))
    }
}

/// What a finger holds down.
#[derive(Clone, Debug)]
enum Held {
    /// A dot standing on the part: a drag once the finger moves, a tap on the part if it lifts where it landed.
    Armed { part: Id, handle: Handle, gizmo: Box<Gizmo>, at: Pos2 },
    /// A gizmo handle, with the gizmo as it stood at the press, which the drag is read against.
    Handle { handle: Handle, gizmo: Box<Gizmo> },
    /// A press-pull's arrow.
    Pull,
}

/// What the preview buffer holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Staged {
    /// A part's placed tessellation from the build it came from.
    Part { build: usize, feature: Id },
    /// A menu command's own ghost, already in the world.
    Own,
}

/// A menu command's ghost, drawn from the command as it stands.
type OwnGhost = Box<dyn Fn(&dyn ViewCommand) -> Option<Mesh>>;

/// The carried ghost read for castability: the judge by build, where it was read, and what it says.
#[derive(Default)]
struct Tint {
    judge: Option<(usize, Arc<GhostJudge>)>,
    at: Option<(Staged, Affine)>,
    read: Option<GhostRead>,
    /// What the part's stage and the process add to the caption.
    note: &'static str,
}

/// The session a finger drives, and what it keeps between frames.
pub struct Live {
    pub session: Session,
    pub bar: DimensionBar,
    snapper: Snapper,
    /// The part the live command carries, as the document held it when the command started.
    target: Option<Feature>,
    /// What the finger holds down, and whether it has left the press point.
    hold: Option<(Held, bool)>,
    /// A press-pull's face: its centre and outward normal.
    pull: Option<([f64; 3], [f64; 3])>,
    own: Option<(OwnGhost, Option<String>)>,
    /// Part vertices and edges to snap to, by build and carried part.
    snaps: Option<(usize, Option<Id>, Vec<[f64; 3]>, Vec<Vec<[f64; 3]>>)>,
    /// The ring's own snap targets, by build, carried part and pins.
    features: Option<(usize, Option<Id>, u64, Arc<RingFeatures>)>,
    tint: Tint,
    staged: Option<Staged>,
    /// The committed ghost holds until a new build lands: the build it was committed over, and when.
    linger: Option<(usize, Instant)>,
    /// What the last pointer landed on.
    snapped: Option<SnapHit>,
    /// A field the bar gives the keyboard once it has been drawn.
    focus: Option<&'static str>,
    /// The build on screen.
    build: usize,
    /// Where the finger last pressed or dragged.
    finger: Option<Pos2>,
    /// The world point the live command carries: the gizmo's origin, the face pushed, the part patterned.
    watch: Option<[f64; 3]>,
    /// Whether the command's bars stand at the view's top, settled when they are first drawn.
    bars_top: Option<bool>,
}

impl Default for Live {
    fn default() -> Self {
        Self {
            session: Session::default(),
            bar: DimensionBar::new(BAR_ID),
            snapper: Snapper { grid: Some(GRID), crest: true, ..Snapper::default() },
            target: None,
            hold: None,
            pull: None,
            own: None,
            snaps: None,
            features: None,
            tint: Tint::default(),
            staged: None,
            linger: None,
            snapped: None,
            focus: None,
            build: 0,
            finger: None,
            watch: None,
            bars_top: None,
        }
    }
}

/// Where `handle`'s drag goes and how to type it, for the status line.
fn hint(handle: Handle) -> &'static str {
    match handle {
        Handle::Move(_) => "drag along the arrow and lift to set it; tap the arrow instead to type a distance",
        Handle::Turn(_) => "drag round the ring and lift to set it; tap it instead to type degrees",
        Handle::Dial => "drag round the ring on the 5° grid and lift to set it; tap it instead to type θ",
        Handle::Grip(_) => "drag the grip and lift to set it; tap it instead to type a size",
    }
}

/// A map as the renderer takes it: the column-major 4x4 and the normals' column-major 3x3.
pub fn gl_model(m: &Affine) -> ([f32; 16], [f32; 9]) {
    let c = m.cofactors();
    let sign = if m.determinant() < 0.0 { -1.0 } else { 1.0 };
    let mut n = [0.0f32; 9];
    for col in 0..3 {
        for row in 0..3 {
            n[col * 3 + row] = (c[row][col] * sign) as f32;
        }
    }
    (m.gl(), n)
}

/// A hash of the pins, which the ring's features are kept by.
fn pins_key(pins: &[Pin]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in pins {
        p.name.hash(&mut h);
        p.world.map(f64::to_bits).hash(&mut h);
    }
    h.finish()
}

/// The chosen part and its gizmo: one part chosen and nothing live.
pub fn gizmo_of(c: &Ctx, live: &Live) -> Option<(Id, Gizmo)> {
    if live.session.is_live() || live.hold.is_some() {
        return None;
    }
    let id = c.selection.one_part()?;
    let f = c.feature(id)?;
    if matches!(f.operation, Operation::Band | Operation::Sketch { .. }) {
        return None;
    }
    let band = c.band.map(|b| b.as_ref());
    let part = c.component(id);
    let g = match &f.component.placement {
        placement @ Placement::Ring { .. } => {
            let g = Gizmo::on_ring(c.design, band, placement, 0.0)?;
            // The part's reach about the origin the build seated it by.
            let reach_mm = part.map_or(0.0, |x| gizmo::reach(&x.mesh, x.frame.origin));
            Gizmo { reach_mm, ..g }
        }
        Placement::Free => {
            let (lo, hi) = part?.mesh.bounds()?;
            let centre = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
            Gizmo::free(centre, part.map_or(0.0, |x| gizmo::reach(&x.mesh, centre)))
        }
    };
    Some((id, g.with_grips(&f.operation)))
}

/// The gizmo on the view's screen.
pub fn layout(c: &Ctx, g: &Gizmo) -> Layout {
    let proj = c.camera.projector(c.rect);
    let (view, ray) = c.view_at(c.rect.center());
    let project = |p: [f64; 3]| proj.at(p.map(|v| v as f32));
    g.layout(&gizmo::View { project: &project, forward: ray.direction, px_per_mm: view.px_per_mm })
}

/// The chosen part on screen: where its origin lands and how far its reach runs, points.
pub fn part_disc(c: &Ctx, g: &Gizmo) -> (Pos2, f32) {
    let (view, _) = c.view_at(c.rect.center());
    (c.project(g.origin), (g.reach_mm * view.px_per_mm) as f32)
}

/// A press-pull's arrow on screen: from the face where the pull has taken it, out along its normal.
fn pull_arrow(c: &Ctx, centre: [f64; 3], normal: [f64; 3], pulled: f64) -> (Pos2, Pos2) {
    let (view, _) = c.view_at(c.rect.center());
    let mm = f64::from(PULL_PT) / view.px_per_mm.max(1e-9);
    let at = |t: f64| c.project(std::array::from_fn(|k| centre[k] + normal[k] * t));
    (at(pulled), at(pulled + mm))
}

fn segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

impl Live {
    pub fn is_live(&self) -> bool {
        self.session.is_live()
    }

    /// Whether a finger holds a handle or an arrow, so the view neither orbits nor pinches.
    pub fn holding(&self) -> bool {
        self.hold.is_some()
    }

    /// How far a live press-pull has taken its face, mm.
    fn pulled(&self) -> f64 {
        self.session.dimensions().iter().find(|d| d.key == "distance").map_or(0.0, |d| d.value)
    }

    /// A new build is on screen: a lingering ghost may go.
    pub fn landed(&mut self, build: &Built) {
        self.build = build.key();
    }

    /// The part the live command carries.
    pub fn target(&self) -> Option<Id> {
        self.target.as_ref().map(|f| f.id)
    }

    /// The carried ghost's castability as last read.
    pub fn ghost_read(&self) -> Option<&GhostRead> {
        self.tint.read.as_ref()
    }

    /// Ends a live command without committing it; whether one was live.
    pub fn cancel(&mut self, out: &mut Vec<Request>) -> bool {
        if !self.session.is_live() {
            return false;
        }
        let o = self.session.feed(StepInput::Cancel);
        self.hold = None;
        self.outcome(o, None, out);
        true
    }

    fn start(&mut self, cmd: Box<dyn ViewCommand>, target: Option<Feature>, own: Option<OwnGhost>) {
        self.session.start(cmd);
        self.target = target;
        self.own = own.map(|g| (g, None));
        self.hold = None;
        self.pull = None;
        self.linger = None;
        self.snapped = None;
        self.focus = None;
        self.watch = None;
        self.bars_top = None;
        self.tint.at = None;
        self.tint.read = None;
        self.bar.reset();
    }

    /// A first finger at `p`: a gizmo handle or a live press-pull's arrow takes it and holds the view.
    pub fn press(&mut self, p: Pos2, c: &Ctx, out: &mut Vec<Request>) {
        self.finger = Some(p);
        if self.session.is_live() {
            if let Some((centre, normal)) = self.pull {
                let (a, b) = pull_arrow(c, centre, normal, self.pulled());
                if segment(p, a, b) <= touch::FINGER_PT {
                    self.hold = Some((Held::Pull, false));
                    self.feed_pull(c, p, out);
                }
            }
            return;
        }
        let Some((id, gizmo)) = gizmo_of(c, self) else { return };
        let layout = layout(c, &gizmo);
        let Some(handle) = touch::handle_at(&layout, p, touch::FINGER_PT, Some(part_disc(c, &gizmo))) else { return };
        if matches!(handle, Handle::Grip(_) | Handle::Dial) {
            // A dot on the part is taken by a drag; a tap on it goes on to the part.
            self.hold = Some((Held::Armed { part: id, handle, gizmo: Box::new(gizmo), at: p }, false));
            return;
        }
        self.start_drag(c, id, gizmo, handle, p, out);
    }

    /// Lets go of a dot pressed and not dragged; whether there was one, so the tap goes on to the part.
    pub fn tap_through(&mut self) -> bool {
        if matches!(self.hold, Some((Held::Armed { .. }, _))) {
            self.hold = None;
            return true;
        }
        false
    }

    /// The held finger moved to `at`.
    pub fn drag(&mut self, at: Pos2, c: &Ctx, out: &mut Vec<Request>) {
        self.finger = Some(at);
        let Some((held, moved)) = self.hold.as_mut() else { return };
        *moved = true;
        match held.clone() {
            Held::Armed { part, handle, gizmo, at: from } => {
                // The dot is taken where it was pressed, then follows the finger.
                self.start_drag(c, part, (*gizmo).clone(), handle, from, out);
                if let Some((_, moved)) = self.hold.as_mut() {
                    *moved = true;
                    self.feed_handle(c, &gizmo, handle, at, out);
                }
            }
            Held::Handle { handle, gizmo } => self.feed_handle(c, &gizmo, handle, at, out),
            Held::Pull => self.feed_pull(c, at, out),
        }
    }

    /// The held finger lifted at `at`: a drag or a typed value commits as one undo step, a tap waits for a number in the bar.
    pub fn release(&mut self, at: Pos2, moved: bool, c: &Ctx, out: &mut Vec<Request>) {
        let Some((held, dragged)) = self.hold.take() else { return };
        if !self.session.is_live() {
            return;
        }
        if moved {
            match &held {
                Held::Handle { handle, gizmo } => self.feed_handle(c, gizmo, *handle, at, out),
                Held::Pull => self.feed_pull(c, at, out),
                Held::Armed { .. } => {}
            }
        }
        let typed = self.session.dimensions().iter().any(|d| d.locked);
        if (moved || dragged || typed) && self.session.is_live() {
            let o = self.session.enter();
            self.outcome(o, Some(c.design), out);
            return;
        }
        // Taken and let go in place: the command waits for a typed value.
        self.focus = match &held {
            Held::Handle { handle, gizmo } | Held::Armed { handle, gizmo, .. } => gizmo.key(*handle),
            Held::Pull => Some("distance"),
        };
        if let Some(key) = self.focus {
            self.bar.prefer(Some(key));
        }
        out.push(Request::Status(format!("{}: type a value, then Done", self.session.prompt())));
    }

    /// A second finger landed or the touch was taken away: what the finger held lets go and nothing moves.
    pub fn let_go(&mut self, out: &mut Vec<Request>) {
        let Some((held, _)) = self.hold.take() else { return };
        match held {
            Held::Armed { .. } => {}
            // A handle's command was the drag itself.
            Held::Handle { .. } => {
                let o = self.session.feed(StepInput::Cancel);
                self.outcome(o, None, out);
                out.push(Request::Status("Two fingers: the handle let go and nothing moved".into()));
            }
            // A press-pull outlives its arrow's drag.
            Held::Pull => out.push(Request::Status("Two fingers: the face let go; drag its arrow again or type a distance".into())),
        }
    }

    /// Starts the command a handle drags, locked to the handle, and reads the press as its first pointer.
    fn start_drag(&mut self, c: &Ctx, id: Id, gizmo: Gizmo, handle: Handle, at: Pos2, out: &mut Vec<Request>) {
        let Some(f) = c.feature(id) else { return };
        let fresh = c.design.cad.as_ref().map_or(1, |d| d.fresh_id());
        let (cmd, lock): (Box<dyn ViewCommand>, Option<Axis>) = match handle {
            Handle::Move(axis) => (Box::new(MoveCmd::of(&f, fresh)), Some(axis)),
            Handle::Turn(axis) => (Box::new(RotateCmd::of(&f, fresh).about(gizmo.pivot())), Some(axis)),
            Handle::Dial => (Box::new(PlaceCmd::new(f.id, f.component.placement.clone())), Some(Axis::Theta)),
            Handle::Grip(i) => match gizmo.grips.get(i).and_then(|g| GripCmd::new(f.id, f.operation.clone(), g.grip.key, &gizmo.frame)) {
                Some(cmd) => (Box::new(cmd), None),
                None => return,
            },
        };
        self.start(cmd, Some(f), None);
        self.watch = Some(gizmo.origin);
        if let Some(axis) = lock {
            self.session.feed(StepInput::Lock(axis));
        }
        self.bar.prefer(gizmo.key(handle));
        // The drag is measured from where the handle was taken.
        self.feed_handle(c, &gizmo, handle, at, out);
        self.hold = Some((Held::Handle { handle, gizmo: Box::new(gizmo) }, false));
        out.push(Request::Status(format!("{} · {}", self.session.prompt(), hint(handle))));
    }

    /// Feeds a handle's pointer at `at`: an arrow or the dial lands the part on the ring's snaps, the dial on its grid.
    fn feed_handle(&mut self, c: &Ctx, gizmo: &Gizmo, handle: Handle, at: Pos2, out: &mut Vec<Request>) {
        let Some(token) = gizmo.token(handle, c.ray(at), None) else { return };
        let (o, snap) = match (gizmo.dofs(handle), c.build) {
            (Some(dofs), Some(build)) => {
                let (view, _) = c.view_at(at);
                let grid = (handle == Handle::Dial).then_some(Grid { theta_deg: GRID.theta_deg, across_mm: 0.0, height_mm: 0.0 });
                let snapper = Snapper { grid, ..self.snapper };
                self.feed_landed(c, build, token, view, dofs, snapper)
            }
            _ => (self.session.feed(token), None),
        };
        self.snapped = snap;
        self.outcome(o, Some(c.design), out);
    }

    /// Feeds a press-pull the point along its face's normal nearest the finger at `at`.
    fn feed_pull(&mut self, c: &Ctx, at: Pos2, out: &mut Vec<Request>) {
        let Some((centre, normal)) = self.pull else { return };
        let ray = c.ray(at);
        let Some(t) = along_line(ray, centre, normal) else { return };
        let world: [f64; 3] = std::array::from_fn(|k| centre[k] + normal[k] * t);
        let token = StepInput::Pointer { world, normal, theta_deg: world[1].atan2(world[0]).to_degrees(), across_mm: world[2], height_mm: 0.0, snapped: None, dragging: true };
        let o = self.session.feed(token);
        self.outcome(o, Some(c.design), out);
    }

    /// Feeds `token`, landing the carried part on the best snap of `dofs` over the ring's features; the hit it landed on.
    fn feed_landed(&mut self, c: &Ctx, build: &Built, token: StepInput, view: ViewScale, dofs: Dofs, snapper: Snapper) -> (Outcome, Option<SnapHit>) {
        self.snaps_for(build);
        let features = self.features_for(c, build);
        let Live { session, snaps, .. } = self;
        let (vertices, edges) = snaps.as_ref().map_or((&[][..], &[][..]), |(_, _, v, e)| (v.as_slice(), e.as_slice()));
        let nominal = c.design.inner_radius_mm() + c.design.profile.thickness_mm;
        let band = c.band.map(|b| b.as_ref());
        let world_of = |p: RingPoint| match band {
            Some(b) => b.world(p),
            None => {
                let (s, co) = p.theta_deg.to_radians().sin_cos();
                let r = nominal + p.height_mm;
                Some([r * co, r * s, p.across_mm])
            }
        };
        let scene = Scene { view, aperture_px: touch::APERTURE_PT, geometry: SnapGeometry { vertices, edges }, features: &features, design: Some(c.design), world_of: &world_of };
        let hit = RefCell::new(None);
        let snap = |p: RingPoint| {
            let h = snapper.snap_ring(world_of(p)?, p, dofs, &scene);
            hit.replace(h.clone());
            h
        };
        let o = land(session, token, &snap);
        (o, hit.take())
    }

    /// Part vertices and edges a carried part may snap to: every part's but its own.
    fn snaps_for(&mut self, build: &Built) {
        let carried = self.target();
        if self.snaps.as_ref().is_some_and(|(k, c, ..)| *k == build.key() && *c == carried) {
            return;
        }
        let (mut vertices, mut edges) = (Vec::new(), Vec::new());
        if let Some(e) = build.evaluated() {
            for comp in e.components.iter().filter(|x| Some(x.id) != carried && !x.settings.reference) {
                vertices.extend_from_slice(&comp.trace.vertices);
                edges.extend(comp.edges.iter().cloned());
            }
        }
        self.snaps = Some((build.key(), carried, vertices, edges));
    }

    /// The ring's snap targets for this build less the carried part: named angles, parting line, side faces, stones, parts, pins.
    fn features_for(&mut self, c: &Ctx, build: &Built) -> Arc<RingFeatures> {
        let carried = self.target();
        let pins = pins_key(c.pins);
        if let Some((b, cr, p, f)) = &self.features
            && (*b, *cr, *p) == (build.key(), carried, pins)
        {
            return f.clone();
        }
        let parting = c.field.map_or(0.0, |f| f.parting_z_mm);
        let features = Arc::new(RingFeatures::of(c.design, parting).with_stones(c.design).with_parts(build.evaluated(), carried).with_points(c.pins.iter().map(Pin::target)));
        self.features = Some((build.key(), carried, pins, features.clone()));
        features
    }

    /// Does what a command's outcome asks: effects leave as one funnel edit, a refusal is said, an end drops the ghost.
    fn outcome(&mut self, o: Outcome, design: Option<&RingDesign>, out: &mut Vec<Request>) {
        match o {
            Outcome::Commit(effects) => {
                let Some(design) = design else { return };
                let (edits, added) = touch::parts::effect_edits(design, effects);
                out.push(Request::Edit { edits, then: if added { Then::LastAdded } else { Then::Keep } });
                self.linger = Some((self.build, Instant::now()));
                self.ended();
            }
            Outcome::Refused(why) => out.push(Request::Status(why)),
            Outcome::Cancelled => {
                self.linger = None;
                self.ended();
                out.push(Request::Status("Cancelled; nothing changed".into()));
            }
            Outcome::NextStep => out.push(Request::Status(self.session.prompt())),
            Outcome::Continue => {}
        }
    }

    fn ended(&mut self) {
        self.target = None;
        self.hold = None;
        self.pull = None;
        self.focus = None;
        self.bar.reset();
    }

    /// Starts copies of `f` round the ring, or round stone `about`, waiting for how many.
    pub fn array(&mut self, c: &Ctx, f: Feature, attach: Attach, about: Option<Id>) -> String {
        let ghost = c.build.zip(c.component(f.id)).map(|(b, comp)| copies_ghost(b, comp.mesh.clone()));
        let watch = c.component(f.id).map(|comp| comp.frame.origin);
        self.start(Box::new(ArrayCmd::new(0, f, attach, about)), None, ghost);
        self.watch = watch;
        self.focus = Some("count");
        self.bar.prefer(Some("count"));
        format!("{}: type how many, then Done", self.session.prompt())
    }

    /// Starts pushing or pulling planar face `face` of part `feature` along its normal: its arrow drags, its bar types.
    pub fn press_pull(&mut self, c: &Ctx, feature: Id, face: u32) -> Result<String, String> {
        let f = c.feature(feature).ok_or_else(|| format!("Part #{feature} is not in the document"))?;
        let comp = c.component(feature).ok_or_else(|| format!("#{feature} {} did not build; mend it on the strip first", f.name))?;
        if f.component.reference {
            return Err("A reference stone is never metal".into());
        }
        let (signed, centre, normal) = pattern::planar_face(comp, face).map_err(|e| format!("{e:#}"))?;
        let ghost = face_ghost(comp, face, normal);
        let attach = comp.attach;
        self.start(Box::new(PressPullCmd::new(f, signed, attach, centre, normal, 0)), None, Some(ghost));
        self.pull = Some((centre, normal));
        self.watch = Some(centre);
        Ok(format!("{}: drag the arrow out or in and lift, or type a distance", self.session.prompt()))
    }

    /// Draws the ghost, the gizmo or the handle held, the press-pull's arrow, the snap landed on, and the bars.
    pub fn draw(&mut self, ui: &mut egui::Ui, c: &Ctx, renderer: &Mutex<GpuMeshRenderer>, out: &mut Vec<Request>) {
        self.ghost(c, renderer);
        let painter = ui.painter_at(c.rect);
        match &self.hold {
            Some((Held::Handle { handle, gizmo }, _)) => {
                let mut gizmo = (**gizmo).clone();
                let preview = self.session.preview().unwrap_or_default();
                // The dial's marker rides the angle being previewed.
                if let (Handle::Dial, Some(d), Some(theta)) = (*handle, gizmo.dial.as_mut(), preview.placement.as_ref().and_then(Placement::theta_deg)) {
                    d.theta_deg = theta;
                }
                gizmo::paint(&painter, &layout(c, &gizmo).only(*handle), None, Some(*handle));
            }
            _ => {
                if let Some((_, g)) = gizmo_of(c, self) {
                    gizmo::paint(&painter, &layout(c, &g), None, None);
                }
            }
        }
        if let Some((centre, normal)) = self.pull {
            let (a, b) = pull_arrow(c, centre, normal, self.pulled());
            let lit = matches!(self.hold, Some((Held::Pull, _)));
            let color = if lit { egui::Color32::from_rgb(255, 226, 110) } else { crate::theme::AQUA };
            painter.line_segment([a, b], egui::Stroke::new(if lit { 4.0 } else { 3.0 }, color));
            painter.circle(b, 9.0, color, egui::Stroke::new(1.5, egui::Color32::BLACK));
        }
        if let (true, Some(snap)) = (self.session.is_live(), &self.snapped) {
            let p = c.project(snap.world);
            if c.rect.contains(p) {
                let s = 7.0;
                painter.add(egui::Shape::closed_line(vec![p + egui::vec2(0.0, -s), p + egui::vec2(s, 0.0), p + egui::vec2(0.0, s), p + egui::vec2(-s, 0.0)], egui::Stroke::new(2.0, crate::theme::AQUA)));
                painter.text(p + egui::vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, &snap.label, egui::FontId::proportional(12.0), crate::theme::AQUA);
            }
        }
        if self.session.is_live() {
            self.bars(ui, c, out);
        }
    }

    /// The live command's words, its typed fields, and Done and Cancel a thumb high.
    fn bars(&mut self, ui: &mut egui::Ui, c: &Ctx, out: &mut Vec<Request>) {
        let ctx = ui.ctx().clone();
        let preview = self.session.preview().unwrap_or_default();
        let mut lines = vec![(self.session.prompt(), crate::theme::AQUA), (preview.caption.clone(), crate::theme::INK)];
        if let Some(read) = self.tint.read.as_ref() {
            let class = if read.locks() { ringdesign_core::FaceClass::Undercut } else { ringdesign_core::FaceClass::Good };
            let [r, g, b] = class.rgb().map(|v| (v * 255.0).round() as u8);
            lines.push((format!("Ghost {}{}", read.caption(), self.tint.note), egui::Color32::from_rgb(r, g, b)));
        }
        let mut done = false;
        let mut cancel = false;
        let width = (c.rect.width() - 16.0).max(120.0);
        let tall_view = c.rect.height() > SHORT_VIEW_PT;
        // The end of the view away from the finger, else from what the command carries, kept for the command's life.
        let top = match self.bars_top {
            Some(top) => top,
            None => {
                let watched = self.finger.filter(|_| self.hold.is_some()).or_else(|| self.watch.map(|w| c.project(w)));
                let top = tall_view && watched.is_some_and(|p| p.y > c.rect.center().y);
                self.bars_top = Some(top);
                top
            }
        };
        let (pivot, at) = if top {
            (egui::Align2::LEFT_TOP, c.rect.left_top() + egui::vec2(8.0, 8.0))
        } else {
            (egui::Align2::LEFT_BOTTOM, c.rect.left_bottom() + egui::vec2(8.0, if tall_view { -FOOT_PT } else { -6.0 }))
        };
        let caption = egui::Area::new(caption_area())
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(at)
            .constrain_to(c.rect)
            .show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgba_unmultiplied(12, 12, 18, 236)).show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    for (text, color) in lines.iter().filter(|(t, _)| !t.is_empty()) {
                        ui.label(egui::RichText::new(text).color(*color).size(12.0));
                    }
                    ui.horizontal(|ui| {
                        let size = egui::vec2(96.0, touch::TARGET_PT);
                        done = ui.add(egui::Button::image_and_text(ringdesign_workbench::icons::Icon::Check.image(ui, 20.0), "Done").min_size(size)).clicked();
                        cancel = ui.add(egui::Button::image_and_text(ringdesign_workbench::icons::Icon::Close.image(ui, 20.0), "Cancel").min_size(size)).clicked();
                    });
                });
            });
        // The fields stand on the caption's inner side, at the height they last drew, and never above the view.
        let mut dims = self.session.dimensions();
        let tall = ctx.memory(|m| m.area_rect(fields_area())).map_or(BAR_PT, |r| r.height());
        let shown = caption.response.rect;
        let y = if top { shown.bottom() + 6.0 } else { (shown.top() - 6.0 - tall).max(c.rect.top() + 4.0) };
        // The bar draws 18 points right of and below its anchor.
        let anchor = egui::pos2(c.rect.left() + 8.0, y) - egui::vec2(18.0, 18.0);
        for e in self.bar.show(&ctx, anchor, &mut dims) {
            let o = match e {
                DimEvent::Typed { key, value } => self.session.feed(StepInput::Typed { key, value }),
                DimEvent::Cleared { key } => self.session.feed(StepInput::Cleared { key }),
                DimEvent::Confirm => self.session.enter(),
                DimEvent::Escape => self.session.escape(),
                DimEvent::Focused { .. } => Outcome::Continue,
            };
            self.outcome(o, Some(c.design), out);
        }
        if let Some(key) = self.focus.take().filter(|_| self.session.is_live()) {
            self.bar.focus_field(&ctx, key);
        }
        if done && self.session.is_live() {
            let o = self.session.enter();
            self.outcome(o, Some(c.design), out);
        } else if cancel && self.session.is_live() {
            let o = self.session.feed(StepInput::Cancel);
            self.outcome(o, Some(c.design), out);
        }
    }

    /// Stages the ghost once and sets this frame's matrix: the carried part to its preview, or a menu command's own.
    fn ghost(&mut self, c: &Ctx, renderer: &Mutex<GpuMeshRenderer>) {
        let Some(build) = c.build else { return };
        if !self.session.is_live() {
            let done = self.linger.is_none_or(|(b, at)| b != build.key() || at.elapsed().as_secs_f64() > LINGER_S);
            if done && (self.staged.is_some() || self.linger.is_some()) {
                self.staged = None;
                self.linger = None;
                self.own = None;
                self.tint.read = None;
                self.tint.at = None;
                if let Ok(mut r) = renderer.lock() {
                    r.set_pending_preview(Vec::new());
                    r.set_preview_model(None);
                    r.set_preview_draft(false);
                }
            }
            return;
        }
        if let Some((stage, shown)) = &mut self.own {
            let Some(cmd) = self.session.command() else { return };
            let preview = cmd.preview();
            let key = format!("{:?} {:?}", preview.operation, preview.placement);
            if shown.as_ref() != Some(&key) {
                let mesh = stage(cmd).unwrap_or_default();
                if let Ok(mut r) = renderer.lock() {
                    r.set_pending_preview(GpuMeshRenderer::stage_part(&mesh));
                    r.set_preview_model((!mesh.faces.is_empty()).then(|| gl_model(&Affine::IDENTITY)));
                    r.set_preview_draft(false);
                }
                *shown = Some(key);
                self.staged = Some(Staged::Own);
            }
            return;
        }
        let (Some(preview), Some(target)) = (self.session.preview(), self.target.clone()) else { return };
        let band = c.band.map(|b| b.as_ref());
        let want = Staged::Part { build: build.key(), feature: target.id };
        let model = placed_ghost(c.design, band, &target, &preview);
        let part = c.component(target.id);
        // A reference stone is not read.
        let judge = (part.is_some_and(|x| !x.settings.reference) && model.is_some()).then(|| self.judge_for(c, build));
        let mut restage = self.staged != Some(want);
        let tinted = match (part, model, judge) {
            (Some(part), Some(m), Some(judge)) => {
                if self.tint.at != Some((want, m)) {
                    let read = judge.read(&part.mesh, &m.0, part.attach == Attach::Cut);
                    restage |= self.tint.read.as_ref().is_none_or(|r| r.classes != read.classes);
                    self.tint.read = Some(read);
                    self.tint.at = Some((want, m));
                    let sand = c.design.draft.process == CastProcess::SandTwoPart;
                    self.tint.note = match (sand, part.stage) {
                        (false, _) => " (lost wax: read, not judged)",
                        (true, Stage::Bench) => " (bench: soldered on after the pour)",
                        _ => "",
                    };
                }
                true
            }
            _ => {
                restage |= self.tint.read.is_some();
                self.tint.read = None;
                self.tint.at = None;
                false
            }
        };
        if restage {
            let verts = match (part, self.tint.read.as_ref()) {
                (Some(part), Some(read)) => GpuMeshRenderer::stage_part_classes(&part.mesh, &read.classes),
                (Some(part), None) => GpuMeshRenderer::stage_part(&part.mesh),
                (None, _) => Vec::new(),
            };
            if let Ok(mut r) = renderer.lock() {
                r.set_pending_preview(verts);
            }
            self.staged = Some(want);
        }
        if let Ok(mut r) = renderer.lock() {
            r.set_preview_model(model.as_ref().map(gl_model));
            r.set_preview_draft(tinted);
        }
    }

    /// The ghost's judge for this build: the field verdict's parting plane and the band's bore.
    fn judge_for(&mut self, c: &Ctx, build: &Built) -> Arc<GhostJudge> {
        if let Some((b, judge)) = &self.tint.judge
            && *b == build.key()
        {
            return judge.clone();
        }
        let parting = c.field.map_or(0.0, |f| f.parting_z_mm);
        let judge = Arc::new(GhostJudge::new(c.design, c.band.map(|b| b.mesh()), parting));
        self.tint.judge = Some((build.key(), judge.clone()));
        judge
    }
}

/// The part's mesh carried onto every copy a pattern command would add.
fn copies_ghost(build: &Built, source: Mesh) -> OwnGhost {
    let frames: Vec<(Id, _)> = build.evaluated().iter().flat_map(|e| e.components.iter().map(|c| (c.id, c.frame)).chain(e.planes.iter().map(|p| (p.id, p.placement())))).collect();
    Box::new(move |cmd| {
        let Some(Operation::Pattern { kind, .. }) = cmd.preview().operation else { return None };
        let frame_of = |id: Id| frames.iter().find(|(f, _)| *f == id).map(|(_, p)| *p);
        let motions = pattern::world_motions(&kind, &frame_of).ok()?;
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
fn face_ghost(c: &EvaluatedComponent, face: u32, normal: [f64; 3]) -> OwnGhost {
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
