//! One record per stone the design sets — a pad, a run's station, a pavé
//! seat, a halo marker, a stone its CAD parts carry — read by the report's
//! census, the gem preview and the section view, so no two of them can
//! disagree about where a stone is.

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;

use cadkernel::brep::Placement as Motion;

use crate::cad::{self, builders, pattern, Document, Evaluated, EvaluatedComponent, FaceSeat, Feature, Operation, PatternKind, Placement, PlaneBase};
use crate::field::{wrap_delta, FieldContext, Layer, LayerEntry, LayerStack, SeatPadLayer, Uv};
use crate::gem::Gem;
use crate::mesh::{BuildResult, Mesh};
use crate::profile::ProfileLoop;
use crate::sketch::Id;
use crate::RingDesign;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoneSource {
    /// A seat pad of its own.
    Pad,
    /// Station `station` of a seat run.
    Run { station: u32 },
    /// Stone `copy` that CAD feature `feature` carries: a stone part, a halo's melee, a pattern's copies.
    Cad { feature: Id, copy: u32 },
}

/// A stone where the design sets it.
#[derive(Clone, Debug)]
pub struct SetStone {
    /// The entry's path, `"Halo / Centre"` for a nested layer; the carrying feature's name for a CAD stone.
    pub label: String,
    /// The same path as indices: the top-level entry, then each group's own; empty for a CAD stone.
    pub path: Vec<usize>,
    pub source: StoneSource,
    pub theta_deg: f64,
    pub v_mm: f64,
    pub gem: Gem,
    /// The seat as this stone meets it: a run's seat fitted to its gem and
    /// scaled with the grade; for a CAD stone, its plan and stand-off over the bare band.
    pub seat: SeatPadLayer,
    /// A CAD stone's girdle: centre at the origin, x along its length, y across it, z up its table.
    /// `None` for a seat, whose frame the band gives.
    pub frame: Option<Motion>,
}

impl SetStone {
    /// Height of the girdle over the bare band, mm.
    pub fn stand_off_mm(&self) -> f64 {
        self.seat.stand_off_mm(self.gem)
    }

    /// Bearing of the stone's long axis in the chart, degrees.
    pub fn rot_deg(&self) -> f64 {
        self.seat.rot_deg
    }

    /// How far the seat reaches round the ring at the crest radius, degrees.
    pub fn reach_deg(&self, ctx: &FieldContext) -> f64 {
        self.seat.chart_reach_mm(ctx).0 / ctx.crest_radius_mm.max(1e-9) * 180.0
            / std::f64::consts::PI
    }

    pub fn carats(&self) -> f64 {
        self.gem.carats()
    }

    /// The CAD feature carrying the stone, when its parts set it.
    pub fn cad_feature(&self) -> Option<Id> {
        match self.source {
            StoneSource::Cad { feature, .. } => Some(feature),
            _ => None,
        }
    }
}

/// Whether a station survives its entry's window.
pub fn kept(entry: &LayerEntry, ctx: &FieldContext, theta_deg: f64, v_mm: f64) -> bool {
    let uv = Uv { u: ctx.u_of_theta(theta_deg.rem_euclid(360.0)), v: v_mm };
    entry.window.mask(uv, ctx) > 0.5
}

/// Every stone the design sets, in stack order, windows honoured, then the
/// stones its CAD parts carry, seated on the bare band. A seat without a
/// stone is stock, not a stone, and is not listed.
pub fn set_stones(design: &RingDesign) -> Vec<SetStone> {
    record(design, None).stones
}

/// [`set_stones`] with the CAD stones read off `built`'s own evaluation, where its parts stand.
pub fn set_stones_built(design: &RingDesign, built: &BuildResult) -> Vec<SetStone> {
    record(design, Some(built)).stones
}

/// The stones the layer stack's seats carry, and none of the CAD parts'.
pub fn seat_stones(design: &RingDesign) -> Vec<SetStone> {
    seat_stones_in(design, &design.field_context())
}

/// The stones the layer stack's seats carry.
fn seat_stones_in(design: &RingDesign, ctx: &FieldContext) -> Vec<SetStone> {
    let mut out = Vec::new();
    walk(ctx, &design.layers, "", &mut Vec::new(), &mut out);
    out
}

/// The stones whose seat reaches a slice at `theta_deg`, never narrower
/// than `min_deg` either side.
pub fn stones_near<'a>(
    stones: &'a [SetStone],
    ctx: &FieldContext,
    theta_deg: f64,
    min_deg: f64,
) -> Vec<&'a SetStone> {
    stones
        .iter()
        .filter(|st| wrap_delta(theta_deg - st.theta_deg, 360.0).abs() <= st.reach_deg(ctx).max(min_deg))
        .collect()
}

fn walk(ctx: &FieldContext, stack: &LayerStack, prefix: &str, path: &mut Vec<usize>, out: &mut Vec<SetStone>) {
    for (index, entry) in stack.layers.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        path.push(index);
        match &entry.layer {
            Layer::SeatPad(seat) => {
                if let Some(gem) = seat.gem {
                    if kept(entry, ctx, seat.theta_deg, seat.v_mm) {
                        out.push(SetStone {
                            label: format!("{prefix}{}", entry.name),
                            path: path.clone(),
                            source: StoneSource::Pad,
                            theta_deg: seat.theta_deg,
                            v_mm: seat.v_mm,
                            gem,
                            seat: *seat,
                            frame: None,
                        });
                    }
                }
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let mut fitted = run.seat;
                fitted.fit_stone(run.gem);
                let fitted = run.turned(fitted);
                for k in 0..n {
                    let theta = run.theta_of_station(k as f64, ctx);
                    if !kept(entry, ctx, theta, fitted.v_mm) {
                        continue;
                    }
                    // The field scales the whole seat with its stone.
                    let mut seat = fitted;
                    seat.height_mm *= run.scale_at(theta);
                    out.push(SetStone {
                        label: format!("{prefix}{}", entry.name),
                        path: path.clone(),
                        source: StoneSource::Run { station: k },
                        theta_deg: theta,
                        v_mm: fitted.v_mm,
                        gem: run.gem_at(theta),
                        seat,
                        frame: None,
                    });
                }
            }
            Layer::Group(g) => walk(ctx, &g.stack, &format!("{prefix}{} / ", entry.name), path, out),
            _ => {}
        }
        path.pop();
    }
}

// --- Stones the CAD parts carry ---------------------------------------------------------------

/// Most stones the CAD parts carry into the record; a part carrying past it is counted to it and named.
pub const MAX_CAD_STONES: usize = 480;
/// How deep a chain of patterns and heads is followed to the stones it carries.
const MAX_CARRY_DEPTH: u32 = 16;
/// Closer than this, two stones of the same size and cut are one stone, mm.
const SAME_STONE_MM: f64 = 1e-3;
/// Samples round a section a CAD stone is seated on and charted against.
const SECTION_STEPS: usize = 192;
/// Pruned part evaluations a thread keeps for stones seated on a part's face.
const FACE_PARTS_KEPT: usize = 8;
/// Half the angle between the two sections a seat's lean round the ring is read from, degrees.
const NORMAL_STEP_DEG: f64 = 0.05;
/// Band recipes a thread keeps seated points for.
const BANDS_KEPT: usize = 4;
/// Seated points a thread keeps per band before it starts over.
const POINTS_KEPT: usize = 16_384;

/// The stones the design sets, and the CAD parts carrying more than [`MAX_CAD_STONES`] let in.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub stones: Vec<SetStone>,
    /// Each part whose stones ran past the cap, with a stone it carries.
    pub past_cap: Vec<(Id, Gem)>,
}

/// [`set_stones`], or [`set_stones_built`] with `built`, and the parts whose stones ran past the cap.
pub fn record(design: &RingDesign, built: Option<&BuildResult>) -> Record {
    let ctx = design.field_context();
    let mut stones = seat_stones_in(design, &ctx);
    if design.cad.is_none() {
        return Record { stones, past_cap: Vec::new() };
    }
    let (cad, past_cap) = match built.and_then(|b| b.parts.evaluated.as_ref().map(|e| (b, e))) {
        Some((b, e)) => cad_stones(design, &ctx, &Built { design, e, surface: b.band.as_deref(), bare: Analytic::new(design) }),
        None => cad_stones(design, &ctx, &Analytic::new(design)),
    };
    stones.extend(cad);
    Record { stones, past_cap }
}

/// The head or halo holding the stones feature `id` carries: a stone's own head, a head or halo itself, a head a
/// pattern copies, a Transform moves or a Boolean keeps. A pattern of a bare stone copies no head, so its copies are
/// held by none, and a stone a Transform moved is held by a head built round the moved stone.
pub fn holder(doc: &Document, id: Id) -> Option<&Feature> {
    held_by(doc, id, false, 0)
}

fn held_by(doc: &Document, id: Id, copied: bool, depth: u32) -> Option<&Feature> {
    let f = doc.feature(id)?;
    if depth > MAX_CARRY_DEPTH {
        return None;
    }
    match &f.operation {
        _ if is_stone(doc, f) => (!copied).then(|| builders::head_on(doc, id)).flatten().filter(|h| !moved_away(doc, h.id)),
        Operation::Builder { key, .. } if key == builders::HALO || builders::HEADS.contains(&key.as_str()) => Some(f),
        Operation::Pattern { sources, .. } => sources.iter().find_map(|id| held_by(doc, *id, true, depth + 1)),
        Operation::Fillet { source, .. }
        | Operation::Chamfer { source, .. }
        | Operation::Shell { source, .. }
        | Operation::PressPull { source, .. }
        | Operation::Transform { source, .. } => held_by(doc, *source, copied, depth + 1),
        Operation::Boolean { a, b, kind } => {
            held_by(doc, *a, copied, depth + 1).or_else(|| (*kind != cad::Boolean::Subtract).then(|| held_by(doc, *b, copied, depth + 1)).flatten())
        }
        _ => None,
    }
}

/// The nearest point of the bare band's surface section to a stone, at the stone's own bearing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foot {
    pub r: f64,
    pub z: f64,
    pub nr: f64,
    pub nz: f64,
    /// Its arc along the section's surface from the low bore edge, mm.
    pub along_mm: f64,
    /// The section's whole surface arc, mm.
    pub surface_len_mm: f64,
    /// The section's reach along the finger, mm.
    pub width_mm: f64,
}

/// The bare band's sections the CAD stones are charted on, remembered on this thread with the record's.
pub struct BareBand<'a>(Analytic<'a>);

impl<'a> BareBand<'a> {
    pub fn new(design: &'a RingDesign) -> Self {
        Self(Analytic::new(design))
    }

    /// The bare band under a CAD stone's girdle, where the record charted it; `None` for a seat's stone.
    pub fn under(&self, st: &SetStone) -> Option<Foot> {
        self.0.foot(st.frame?.origin)
    }
}

/// Where the CAD stones stand: off an evaluation of the parts as built, or off the band's own sections.
trait Frames {
    /// The parts that come out of the document, in order.
    fn outputs(&self, doc: &Document) -> Vec<Id>;
    /// A stone part's gem and girdle frame.
    fn stone(&self, f: &Feature) -> Option<(Gem, Motion)>;
    /// The motions carrying `source` onto each copy of pattern `f`.
    fn copies(&self, f: &Feature, source: Id, kind: &PatternKind) -> Vec<Motion>;
    /// The bare band's surface under a stone's girdle centre, for charting it.
    fn foot(&self, origin: [f64; 3]) -> Option<Foot>;
    /// Where a ring placement seats a part.
    fn seat(&self, p: &Placement) -> Option<Motion>;
    /// Whether feature `id` built.
    fn built(&self, id: Id) -> bool;
}

/// The frames an evaluation of the parts gave, dropped on `surface`: what a build shows.
struct Built<'a> {
    design: &'a RingDesign,
    e: &'a Evaluated,
    surface: Option<&'a Mesh>,
    /// The bare band's sections, which chart each stone.
    bare: Analytic<'a>,
}

impl Frames for Built<'_> {
    fn outputs(&self, _doc: &Document) -> Vec<Id> {
        self.e.components.iter().map(|c| c.id).collect()
    }
    fn stone(&self, f: &Feature) -> Option<(Gem, Motion)> {
        if let Some(c) = self.e.components.iter().find(|c| c.id == f.id) {
            return Some((c.made.as_ref()?.gem?, c.frame));
        }
        // A stone a later part consumed, seated as the build seated it; one on a part's face off the bare band.
        let Operation::Builder { on: None, params, .. } = &f.operation else { return self.bare.stone(f) };
        let gem = builders::gem_of(params).ok()?;
        match &f.component.placement {
            Placement::Free => Some((gem, Motion::IDENTITY)),
            p => Some((gem, stone_turn(&self.seat(p)?))),
        }
    }
    fn copies(&self, _f: &Feature, source: Id, kind: &PatternKind) -> Vec<Motion> {
        pattern::copy_motions(self.design, self.surface, self.e, source, kind).unwrap_or_default()
    }
    fn foot(&self, origin: [f64; 3]) -> Option<Foot> {
        self.bare.foot(origin)
    }
    fn seat(&self, p: &Placement) -> Option<Motion> {
        p.frame_on(self.design, self.surface).ok()
    }
    fn built(&self, id: Id) -> bool {
        self.e.status_of(id).is_some_and(cad::FeatureStatus::is_ok)
    }
}

/// What rays and stones met one band's surface at, remembered across calls on a thread.
#[derive(Default)]
struct Seated {
    /// A ray's hit and normal by its angle and offset along the finger.
    hits: HashMap<[u64; 2], Option<([f64; 3], [f64; 3])>>,
    /// A stone's foot by its girdle centre.
    feet: HashMap<[u64; 3], Option<Foot>>,
}

thread_local! {
    static BANDS: RefCell<Vec<(u64, Rc<RefCell<Seated>>)>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
thread_local! {
    static SECTIONS_SAMPLED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts one bare section sampled on this thread.
#[cfg(test)]
pub(crate) fn section_sampled() {
    SECTIONS_SAMPLED.with(|c| c.set(c.get() + 1));
}

/// A hash of everything a bare section of the band is made from; `None` when it cannot be read.
fn band_key(design: &RingDesign) -> Option<u64> {
    let mut h = std::hash::DefaultHasher::new();
    serde_json::to_vec(&(design.size, &design.profile, &design.shank)).ok()?.hash(&mut h);
    if let Some(b) = &design.imported_base {
        b.source.fingerprint().hash(&mut h);
        serde_json::to_vec(&b.chart).ok()?.hash(&mut h);
        (b.bare, b.sand_envelope).hash(&mut h);
    }
    SECTION_STEPS.hash(&mut h);
    Some(h.finish())
}

/// The seated points this thread keeps for the design's band, a fresh set for a band that cannot be keyed.
fn seated_for(design: &RingDesign) -> Rc<RefCell<Seated>> {
    let Some(key) = band_key(design) else { return Rc::default() };
    BANDS.with(|b| {
        let mut b = b.borrow_mut();
        if let Some(at) = b.iter().position(|(k, _)| *k == key) {
            let hit = b.remove(at);
            let seated = hit.1.clone();
            b.push(hit);
            return seated;
        }
        if b.len() >= BANDS_KEPT {
            b.remove(0);
        }
        let seated = Rc::<RefCell<Seated>>::default();
        b.push((key, seated.clone()));
        seated
    })
}

/// The frames the placements give on the bare band's own sections, with no build: `Placement::frame_on`
/// read off the section at the part's angle rather than off a swept mesh.
struct Analytic<'a> {
    design: &'a RingDesign,
    reference: std::cell::OnceCell<ProfileLoop>,
    /// One section serves every angle.
    uniform: bool,
    sections: RefCell<HashMap<i64, Rc<ProfileLoop>>>,
    seated: Rc<RefCell<Seated>>,
}

impl<'a> Analytic<'a> {
    fn new(design: &'a RingDesign) -> Self {
        Self { design, reference: Default::default(), uniform: uniform(design), sections: RefCell::default(), seated: seated_for(design) }
    }

    fn doc(&self) -> Option<&'a Document> {
        self.design.cad.as_ref()
    }

    /// The bare band's section at `theta_deg`, sampled once per angle.
    fn at(&self, theta_deg: f64) -> Rc<ProfileLoop> {
        let key = if self.uniform { 0 } else { (theta_deg.rem_euclid(360.0) * 1e6).round() as i64 };
        self.sections
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                #[cfg(test)]
                section_sampled();
                let reference = self.reference.get_or_init(|| self.design.reference_loop());
                Rc::new(self.design.section_at(theta_deg, SECTION_STEPS, None, Some(reference)))
            })
            .clone()
    }

    /// Where a radial ray in the finger's plane at `theta_deg`, `across_mm` along the finger, meets the band's
    /// section, and the section's own tangent there, pointing across the band.
    fn crossing(&self, theta_deg: f64, across_mm: f64) -> Option<([f64; 3], [f64; 3])> {
        let s = self.at(theta_deg);
        let (sin, cos) = theta_deg.to_radians().sin_cos();
        let n = s.pts.len();
        let mut best: Option<(f64, f64, f64)> = None;
        for i in 0..n {
            let (a, b) = (&s.pts[i], &s.pts[(i + 1) % n]);
            if (a.z - across_mm) * (b.z - across_mm) > 0.0 || (b.z - a.z).abs() < 1e-12 {
                continue;
            }
            let t = ((across_mm - a.z) / (b.z - a.z)).clamp(0.0, 1.0);
            let r = a.r + (b.r - a.r) * t;
            if best.is_none_or(|(r0, _, _)| r > r0) {
                best = Some((r, a.nr + (b.nr - a.nr) * t, a.nz + (b.nz - a.nz) * t));
            }
        }
        let (r, nr, nz) = best?;
        let len = nr.hypot(nz);
        if nr <= 0.0 || len < 1e-9 {
            return None;
        }
        Some(([r * cos, r * sin, across_mm], [-nz / len * cos, -nz / len * sin, nr / len]))
    }

    /// [`Self::crossing`] with the surface's normal, which leans round the ring wherever the band's crest falls.
    fn hit(&self, theta_deg: f64, across_mm: f64) -> Option<([f64; 3], [f64; 3])> {
        let key = [theta_deg.to_bits(), across_mm.to_bits()];
        if let Some(hit) = self.seated.borrow().hits.get(&key) {
            return *hit;
        }
        let hit = self.hit_on_sections(theta_deg, across_mm);
        let mut seated = self.seated.borrow_mut();
        if seated.hits.len() >= POINTS_KEPT {
            seated.hits.clear();
        }
        seated.hits.insert(key, hit);
        hit
    }

    fn hit_on_sections(&self, theta_deg: f64, across_mm: f64) -> Option<([f64; 3], [f64; 3])> {
        let (p, across) = self.crossing(theta_deg, across_mm)?;
        let (sin, cos) = theta_deg.to_radians().sin_cos();
        let radial = [cos, sin, 0.0];
        let flat = cross(across, [-sin, cos, 0.0]);
        let round = match (self.uniform, self.crossing(theta_deg - NORMAL_STEP_DEG, across_mm), self.crossing(theta_deg + NORMAL_STEP_DEG, across_mm)) {
            (false, Some((a, _)), Some((b, _))) => sub(b, a),
            _ => [-sin, cos, 0.0],
        };
        let n = cross(round, across);
        let len = norm(n);
        let n = if len > 1e-12 { n.map(|v| v / len) } else { flat.map(|v| -v) };
        Some((p, if dot(n, radial) < 0.0 { n.map(|v| -v) } else { n }))
    }

    /// The bare section's surface point nearest `origin`, at `origin`'s own bearing.
    fn foot(&self, origin: [f64; 3]) -> Option<Foot> {
        let key = origin.map(f64::to_bits);
        if let Some(foot) = self.seated.borrow().feet.get(&key) {
            return *foot;
        }
        let foot = self.foot_on_section(origin);
        let mut seated = self.seated.borrow_mut();
        if seated.feet.len() >= POINTS_KEPT {
            seated.feet.clear();
        }
        seated.feet.insert(key, foot);
        foot
    }

    fn foot_on_section(&self, o: [f64; 3]) -> Option<Foot> {
        let section = self.at(bearing(o));
        let (r, z) = (o[0].hypot(o[1]), o[2]);
        let (lo, hi) = section.z_range();
        let n = section.pts.len();
        let mut best: Option<(f64, Foot)> = None;
        for i in 0..n {
            let (a, b) = (&section.pts[i], &section.pts[(i + 1) % n]);
            if !(a.surface && b.surface) {
                continue;
            }
            let (dr, dz) = (b.r - a.r, b.z - a.z);
            let t = (((r - a.r) * dr + (z - a.z) * dz) / (dr * dr + dz * dz).max(1e-18)).clamp(0.0, 1.0);
            let (fr, fz) = (a.r + dr * t, a.z + dz * t);
            let d = (r - fr).hypot(z - fz);
            if best.is_none_or(|b| d < b.0) {
                let lerp = |p: f64, q: f64| p + (q - p) * t;
                let foot = Foot {
                    r: fr,
                    z: fz,
                    nr: lerp(a.nr, b.nr),
                    nz: lerp(a.nz, b.nz),
                    along_mm: lerp(a.v_mm, b.v_mm),
                    surface_len_mm: section.surface_len_mm,
                    width_mm: (hi - lo).max(0.0),
                };
                best = Some((d, foot));
            }
        }
        best.map(|(_, f)| f)
    }

    /// [`Placement::frame_on`] with the bare band's section for the surface; `frame()` where the parts are the ring.
    fn seated(&self, p: &Placement) -> anyhow::Result<Motion> {
        let Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } = *p else { return p.frame(self.design) };
        if !self.design.band_is_procedural() {
            return p.frame(self.design);
        }
        anyhow::ensure!([theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg].iter().all(|v| v.is_finite()), "Invalid ring placement");
        let Some((hit, z)) = self.hit(theta_deg, across_mm) else { return p.frame(self.design) };
        let along = [0.0, 0.0, -1.0];
        let d = dot(along, z);
        let x: [f64; 3] = std::array::from_fn(|k| along[k] - z[k] * d);
        let len = norm(x);
        if len < 1e-6 {
            return p.frame(self.design);
        }
        let x = x.map(|v| v / len);
        let y = cross(z, x);
        let lean = nalgebra::Rotation3::from_euler_angles(tilt_deg.to_radians(), cant_deg.to_radians(), spin_deg.to_radians());
        let l = lean.matrix();
        let col = |i: usize| -> [f64; 3] { std::array::from_fn(|k| x[k] * l[(0, i)] + y[k] * l[(1, i)] + z[k] * l[(2, i)]) };
        Ok(Motion { x_axis: col(0), y_axis: col(1), z_axis: col(2), origin: std::array::from_fn(|k| hit[k] + z[k] * height_mm) })
    }

    /// The frame feature `id` stands in, as the evaluation would record it; `None` for a part standing free.
    fn frame_of(&self, id: Id, depth: u32) -> Option<Motion> {
        let f = self.doc()?.feature(id)?;
        if depth > MAX_CARRY_DEPTH {
            return None;
        }
        match &f.operation {
            Operation::Builder { key, .. } if key == builders::STONE => self.stone(f).map(|(_, m)| m),
            Operation::Builder { on: Some(stone), .. } => self.frame_of(*stone, depth + 1),
            Operation::Plane { base, offset_mm } => self.plane(base, *offset_mm),
            Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } | Operation::PressPull { source, .. } => {
                self.frame_of(*source, depth + 1)
            }
            Operation::Transform { source, translation, rotation_deg } => {
                let inner = self.frame_of(*source, depth + 1).map(|i| crate::cad::rotate_place(*translation, *rotation_deg).ok().map(|m| pattern::then(&m, &i)));
                self.placed(f, inner.flatten())
            }
            Operation::Boolean { a, b, .. } if self.doc()?.band().is_some_and(|band| band == *a || band == *b) => {
                let other = if self.doc()?.band() == Some(*a) { *b } else { *a };
                self.placed(f, self.frame_of(other, depth + 1))
            }
            _ => self.placed(f, None),
        }
    }

    /// `inner`, the frame a part's body stands in, carried on by its own ring placement.
    fn placed(&self, f: &Feature, inner: Option<Motion>) -> Option<Motion> {
        match &f.component.placement {
            Placement::Free => inner,
            p => {
                let seat = self.seated(p).ok()?;
                Some(inner.map_or(seat, |i| pattern::then(&seat, &i)))
            }
        }
    }

    /// A work plane's frame where no part's face is needed for it.
    fn plane(&self, base: &PlaneBase, offset_mm: f64) -> Option<Motion> {
        let (origin, x, y, n) = match base {
            PlaneBase::Section { theta_deg } => {
                let (s, c) = theta_deg.to_radians().sin_cos();
                ([0.0; 3], [c, s, 0.0], [0.0, 0.0, 1.0], [s, -c, 0.0])
            }
            PlaneBase::Parting => ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
            PlaneBase::Tangent { theta_deg, across_mm } => {
                let (s, c) = theta_deg.to_radians().sin_cos();
                let (hit, n) = self.hit(*theta_deg, *across_mm).unwrap_or_else(|| {
                    let r = self.design.inner_radius_mm() + self.design.profile.thickness_mm;
                    ([r * c, r * s, *across_mm], [c, s, 0.0])
                });
                let round = [-s, c, 0.0];
                let d = dot(round, n);
                let x: [f64; 3] = std::array::from_fn(|k| round[k] - n[k] * d);
                let len = norm(x);
                if len < 1e-12 {
                    return None;
                }
                let x = x.map(|v| v / len);
                (hit, x, cross(n, x), n)
            }
            PlaneBase::Face { .. } => return None,
        };
        offset_mm.is_finite().then(|| Motion { x_axis: x, y_axis: y, z_axis: n, origin: std::array::from_fn(|k| origin[k] + n[k] * offset_mm) })
    }

    /// Part `part` evaluated alone, where no surface seats it, and the motion from there to where the bare band seats it.
    fn face_part(&self, part: Id) -> Option<(Arc<EvaluatedComponent>, Motion)> {
        let doc = self.doc()?;
        let mut keep = BTreeSet::new();
        let mut open = vec![part];
        while let Some(id) = open.pop() {
            if keep.insert(id) {
                open.extend(doc.feature(id).map(|f| f.operation.sources()).unwrap_or_default());
            }
        }
        let pruned = Document {
            features: doc.features.iter().filter(|f| keep.contains(&f.id)).cloned().collect(),
            outputs: vec![part],
            ..Document::default()
        };
        let alone = RingDesign {
            imported_base: self.design.imported_base.clone(),
            size: self.design.size,
            profile: self.design.profile.clone(),
            shank: self.design.shank.clone(),
            cad: Some(pruned),
            ..RingDesign::default()
        };
        let c = evaluated_alone(&alone, part)?;
        let correction = match pattern::seat_of(doc, part) {
            Some((_, p)) => pattern::then(&self.seated(&p).ok()?, &pattern::inverse(&p.frame(self.design).ok()?)),
            None => Motion::IDENTITY,
        };
        Some((c, correction))
    }
}

/// `c` moved by `m`, a reflection re-handed so the stone's x and z keep their sense.
fn moved(m: &Motion, f: &Motion) -> Motion {
    let mut t = pattern::then(m, f);
    if t.reflects() {
        t.y_axis = cross(t.z_axis, t.x_axis);
    }
    t
}

/// `m` read in the frame `c` carries the world to: `c ∘ m ∘ c⁻¹`.
fn conjugate(c: &Motion, m: &Motion) -> Motion {
    pattern::then(c, &pattern::then(m, &pattern::inverse(c)))
}

impl Frames for Analytic<'_> {
    fn outputs(&self, doc: &Document) -> Vec<Id> {
        let Some(through) = doc.through else { return doc.outputs.clone() };
        let mut out: Vec<Id> = Vec::new();
        for f in &doc.features {
            for id in f.operation.consumes() {
                out.retain(|v| *v != id);
            }
            if f.operation.has_body() {
                out.push(f.id);
            }
            if f.id == through {
                break;
            }
        }
        out
    }

    fn stone(&self, f: &Feature) -> Option<(Gem, Motion)> {
        let Operation::Builder { on, params, .. } = &f.operation else { return None };
        let gem = builders::gem_of(params).ok()?;
        let frame = match on {
            None => match &f.component.placement {
                Placement::Free => Motion::IDENTITY,
                p => stone_turn(&self.seated(p).ok()?),
            },
            Some(part) => {
                let seat = FaceSeat::of(params).ok().flatten()?;
                let (c, correction) = self.face_part(*part)?;
                pattern::then(&correction, &seat.frame(&seat.face_of(&c).ok()?, &c.frame))
            }
        };
        Some((gem, frame))
    }

    fn copies(&self, _f: &Feature, source: Id, kind: &PatternKind) -> Vec<Motion> {
        let Some(doc) = self.doc() else { return Vec::new() };
        if let (PatternKind::Ring { .. }, Some((stone, part, seat))) = (kind, cad::face_stone(doc, source)) {
            let Some((c, correction)) = self.face_part(part) else { return Vec::new() };
            let Some(f) = doc.feature(stone) else { return Vec::new() };
            let Some((_, at)) = self.stone(f) else { return Vec::new() };
            let at = pattern::then(&pattern::inverse(&correction), &at);
            let (Ok(face), Ok(outline)) = (seat.face_of(&c), pattern::FaceOutline::of_seat(&seat, &c)) else { return Vec::new() };
            return pattern::face_instances(kind, &at, &face, &outline)
                .map(|all| all.into_iter().filter(|i| !i.off_face).map(|i| conjugate(&correction, &i.motion)).collect())
                .unwrap_or_default();
        }
        let seat = pattern::seat_of(doc, source).and_then(|(_, p)| self.seated(&p).ok().map(|used| (p, used)));
        pattern::motions_seated(kind, self.design, &|p: &Placement| self.seated(p), seat.as_ref().map(|(p, used)| (p, used)), &|id| self.frame_of(id, 0))
            .unwrap_or_default()
    }

    fn foot(&self, origin: [f64; 3]) -> Option<Foot> {
        Analytic::foot(self, origin)
    }
    fn seat(&self, p: &Placement) -> Option<Motion> {
        self.seated(p).ok()
    }
    fn built(&self, _id: Id) -> bool {
        true
    }
}

/// The stone frame of a seat: turned a quarter about its normal, so the stone's length runs round the ring at no spin.
fn stone_turn(seat: &Motion) -> Motion {
    Motion { x_axis: seat.y_axis, y_axis: seat.x_axis.map(|v| -v), z_axis: seat.z_axis, origin: seat.origin }
}

/// Whether one section of the bare band serves every angle round it.
fn uniform(design: &RingDesign) -> bool {
    design.imported_base.is_none() && design.shank.kind == crate::profile::ShankKind::Uniform && design.profile.morph.is_none()
}

thread_local! {
    static FACE_PARTS: RefCell<Vec<(u64, Option<Arc<EvaluatedComponent>>)>> = const { RefCell::new(Vec::new()) };
}

/// Part `part` of a pruned design evaluated with no surface under it, remembered per thread by its recipe.
fn evaluated_alone(design: &RingDesign, part: Id) -> Option<Arc<EvaluatedComponent>> {
    let key = {
        let mut h = std::hash::DefaultHasher::new();
        serde_json::to_vec(&(&design.cad, design.size, &design.profile, &design.shank)).ok()?.hash(&mut h);
        design.imported_base.as_ref().map(|b| Arc::as_ptr(&b.source) as usize).hash(&mut h);
        part.hash(&mut h);
        h.finish()
    };
    if let Some(hit) = FACE_PARTS.with(|c| c.borrow().iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())) {
        return hit;
    }
    let built = cad::evaluate(design, &crate::AlphaLibrary::default(), crate::BuildParams::default())
        .ok()
        .and_then(|e| e.components.into_iter().find(|c| c.id == part))
        .map(Arc::new);
    FACE_PARTS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= FACE_PARTS_KEPT {
            c.remove(0);
        }
        c.push((key, built.clone()));
    });
    built
}

/// The stones feature `f` carries to wherever it is copied or moved — a stone its own, a head its stone, a halo its melee, a
/// pattern each copy's, a Transform its source's moved, a Boolean both operands' but a subtracted one's — at most
/// [`MAX_CAD_STONES`] of them, and whether it carries more.
fn carried(doc: &Document, frames: &dyn Frames, f: &Feature, depth: u32) -> (Vec<(Gem, Motion)>, bool) {
    if !f.enabled || depth > MAX_CARRY_DEPTH || !frames.built(f.id) {
        return (Vec::new(), false);
    }
    let follow = |id: Id| doc.feature(id).map_or_else(|| (Vec::new(), false), |s| carried(doc, frames, s, depth + 1));
    let stone_of = |id: Id| doc.feature(id).filter(|s| is_stone(doc, s)).map_or_else(|| (Vec::new(), false), |s| carried(doc, frames, s, depth + 1));
    match &f.operation {
        Operation::Builder { key, .. } if key == builders::STONE => (frames.stone(f).into_iter().collect(), false),
        Operation::Builder { key, on: Some(stone), params } if key == builders::HALO => {
            let Some((gem, frame)) = stone_of(*stone).0.into_iter().next() else { return (Vec::new(), false) };
            let Ok((melee, stations)) = builders::halo_melee(gem, params) else { return (Vec::new(), false) };
            let over = stations.len() > MAX_CAD_STONES;
            (stations.iter().take(MAX_CAD_STONES).map(|p| (melee, Motion { origin: frame.point(*p), ..frame })).collect(), over)
        }
        Operation::Builder { key, on: Some(stone), .. } if builders::HEADS.contains(&key.as_str()) => stone_of(*stone),
        Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } | Operation::PressPull { source, .. } => {
            follow(*source)
        }
        Operation::Transform { source, .. } if is_head(doc, finished_from(doc, *source)) => (Vec::new(), false),
        Operation::Transform { source, translation, rotation_deg } => {
            let Some(m) = crate::cad::rotate_place(*translation, *rotation_deg).ok() else { return (Vec::new(), false) };
            let m = match &f.component.placement {
                Placement::Free => m,
                p => match frames.seat(p) {
                    Some(seat) => pattern::then(&seat, &m),
                    None => return (Vec::new(), false),
                },
            };
            let (inner, over) = follow(*source);
            (inner.iter().map(|(g, s)| (*g, moved(&m, s))).collect(), over)
        }
        Operation::Boolean { a, b, kind } => {
            let (mut out, mut over) = follow(*a);
            if *kind != cad::Boolean::Subtract {
                let (more, past) = follow(*b);
                over |= past || out.len() + more.len() > MAX_CAD_STONES;
                out.extend(more);
                out.truncate(MAX_CAD_STONES);
            }
            (out, over)
        }
        Operation::Pattern { sources, kind } => {
            let (mut inner, mut over) = (Vec::new(), false);
            for id in sources.iter() {
                let (stones, past) = follow(*id);
                inner.extend(stones);
                over |= past || inner.len() > MAX_CAD_STONES;
                inner.truncate(MAX_CAD_STONES);
            }
            let Some(first) = sources.first().filter(|_| !inner.is_empty()) else { return (Vec::new(), over) };
            let copies = frames.copies(f, first, kind);
            let over = over || copies.len().saturating_mul(inner.len()) > MAX_CAD_STONES;
            (copies.iter().flat_map(|m| inner.iter().map(move |(g, s)| (*g, moved(m, s)))).take(MAX_CAD_STONES).collect(), over)
        }
        _ => (Vec::new(), false),
    }
}

/// The part a chain of fillets, chamfers, shells and press-pulls starts from.
fn finished_from(doc: &Document, id: Id) -> Id {
    let mut at = id;
    for _ in 0..=MAX_CARRY_DEPTH {
        match doc.feature(at).map(|f| &f.operation) {
            Some(Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } | Operation::PressPull { source, .. }) => at = *source,
            _ => break,
        }
    }
    at
}

/// Whether part `id` is a head built round a stone.
fn is_head(doc: &Document, id: Id) -> bool {
    doc.feature(id).is_some_and(|f| matches!(&f.operation, Operation::Builder { key, .. } if builders::HEADS.contains(&key.as_str())))
}

/// Whether an enabled Transform moves head `id`, which leaves its stone where it was.
fn moved_away(doc: &Document, id: Id) -> bool {
    doc.features.iter().any(|t| t.enabled && matches!(&t.operation, Operation::Transform { source, .. } if finished_from(doc, *source) == id))
}

/// Whether `f` is a stone: a stone part, or one a Transform moved.
fn is_stone(doc: &Document, f: &Feature) -> bool {
    let mut at = f;
    for _ in 0..=MAX_CARRY_DEPTH {
        match &at.operation {
            Operation::Builder { key, .. } => return key == builders::STONE,
            Operation::Transform { source, .. } => match doc.feature(*source) {
                Some(s) => at = s,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// A stone found among the parts' outputs, and the part it is counted under.
struct Found<'d> {
    feature: Id,
    name: &'d str,
    /// Whether a head or halo holds the stones `feature` carries.
    held: bool,
    gem: Gem,
    frame: Motion,
}

/// The cell of the grid [`SAME_STONE_MM`] on a side that holds `p`.
fn cell_of(p: [f64; 3]) -> [i64; 3] {
    p.map(|v| (v / SAME_STONE_MM).floor() as i64)
}

/// Every stone the document's parts carry, each once and at most [`MAX_CAD_STONES`] of them — a stone part, a halo's
/// melee, every copy a pattern makes of a stone, a head or a halo, and whatever a Transform moves or a Boolean keeps of
/// them — and the parts carrying past the cap. A head standing on a stone adds none; its stone is its own part, or the
/// part that moved it. Two parts carrying one stone count it under the one that holds it.
fn cad_stones(design: &RingDesign, ctx: &FieldContext, frames: &dyn Frames) -> (Vec<SetStone>, Vec<(Id, Gem)>) {
    let Some(doc) = &design.cad else { return Default::default() };
    let mut found: Vec<Found> = Vec::new();
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    let mut past_cap = Vec::new();
    for id in frames.outputs(doc) {
        let Some(f) = doc.feature(id).filter(|f| f.enabled) else { continue };
        if matches!(&f.operation, Operation::Builder { key, .. } if builders::HEADS.contains(&key.as_str())) {
            continue;
        }
        let (stones, mut over) = carried(doc, frames, f, 0);
        if stones.is_empty() && !over {
            continue;
        }
        let held = holder(doc, id).is_some();
        let first = stones.first().map(|(g, _)| *g);
        for (gem, frame) in stones {
            let cell = cell_of(frame.origin);
            let same = |k: &usize| {
                let s = &found[*k];
                s.gem.cut == gem.cut
                    && s.gem.form == gem.form
                    && (s.gem.w_mm - gem.w_mm).abs() < 1e-9
                    && (s.gem.l_mm - gem.l_mm).abs() < 1e-9
                    && norm(sub(s.frame.origin, frame.origin)) < SAME_STONE_MM
            };
            let near = (0..27).find_map(|n| {
                let c = [cell[0] + n % 3 - 1, cell[1] + n / 3 % 3 - 1, cell[2] + n / 9 - 1];
                grid.get(&c).and_then(|ks| ks.iter().copied().find(|k| same(k)))
            });
            match near {
                Some(k) if held && !found[k].held => found[k] = Found { feature: id, name: &f.name, held, ..found[k] },
                Some(_) => {}
                None if found.len() < MAX_CAD_STONES => {
                    grid.entry(cell).or_default().push(found.len());
                    found.push(Found { feature: id, name: &f.name, held, gem, frame });
                }
                None => over = true,
            }
        }
        if let (true, Some(gem)) = (over, first) {
            past_cap.push((id, gem));
        }
    }
    let mut copies: HashMap<Id, u32> = HashMap::new();
    let stones = found
        .into_iter()
        .map(|Found { feature, name, gem, frame, .. }| {
            let copy = copies.entry(feature).or_default();
            let st = charted(ctx, frames.foot(frame.origin), name, StoneSource::Cad { feature, copy: *copy }, gem, frame);
            *copy += 1;
            st
        })
        .collect();
    (stones, past_cap)
}

/// A point's angle round the ring, degrees in [0, 360).
fn bearing(p: [f64; 3]) -> f64 {
    p[1].atan2(p[0]).to_degrees().rem_euclid(360.0)
}

/// A CAD stone in the record: its chart point where its girdle stands over the band, and the seat that holds
/// its plan at its bearing and its girdle at its stand-off.
fn charted(ctx: &FieldContext, foot: Option<Foot>, label: &str, source: StoneSource, gem: Gem, frame: Motion) -> SetStone {
    let o = frame.origin;
    let theta = bearing(o);
    let (sin, cos) = theta.to_radians().sin_cos();
    let (v_mm, stand_off, rot) = match foot {
        Some(Foot { r, z, nr, nz, along_mm, surface_len_mm, .. }) => {
            let foot = [r * cos, r * sin, z];
            let (t, across) = ([-sin, cos, 0.0], [-nz * cos, -nz * sin, nr]);
            let x = frame.x_axis;
            let v_chart = along_mm / surface_len_mm.max(1e-9) * ctx.band_v_len_mm;
            (v_chart, dot(sub(o, foot), frame.z_axis), dot(x, across).atan2(dot(x, t)).to_degrees())
        }
        None => (ctx.crest_v_mm, 0.0, 0.0),
    };
    let seat = SeatPadLayer {
        theta_deg: theta,
        v_mm,
        diameter_mm: gem.w_mm,
        elong: (gem.l_mm / gem.w_mm.max(1e-9)).max(1.0),
        plan_pow: gem.cut.plan_pow(),
        rot_deg: rot,
        height_mm: stand_off.max(0.0),
        set_depth_mm: Some(0.0),
        blend_mm: 0.0,
        gem: Some(gem),
        ..SeatPadLayer::default()
    };
    SetStone { label: label.to_string(), path: Vec::new(), source, theta_deg: theta, v_mm, gem, seat, frame: Some(frame) }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{GroupLayer, SeatRunLayer, Window};
    use crate::gem::GemCut;

    fn pad(theta: f64, v: f64, gem: Option<Gem>) -> SeatPadLayer {
        let mut s = SeatPadLayer::default();
        s.theta_deg = theta;
        s.v_mm = v;
        s.gem = gem;
        if let Some(g) = gem {
            s.fit_stone(g);
        }
        s
    }

    fn design() -> (RingDesign, usize) {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let v = ctx.band_v_len_mm * 0.5;
        let round = |mm: f64| Gem::calibrated(GemCut::Round, mm);
        d.layers.layers.push(LayerEntry::new("Centre", Layer::SeatPad(pad(90.0, v, Some(round(4.0))))));
        d.layers.layers.push(LayerEntry::new("Blank stock", Layer::SeatPad(pad(270.0, v, None))));
        let mut off = LayerEntry::new("Off", Layer::SeatPad(pad(180.0, v, Some(round(2.0)))));
        off.enabled = false;
        d.layers.layers.push(off);
        let mut run = SeatRunLayer::default();
        run.gem = round(1.5);
        run.seat.v_mm = v;
        run.count = 20;
        run.solve_spacing(&ctx);
        let mut row = LayerEntry::new("Row", Layer::SeatRun(run));
        row.window = Window::around(270.0, 120.0);
        let kept_stations = (0..20)
            .filter(|&k| {
                let t = run.theta_of_station(k as f64, &ctx);
                kept(&row, &ctx, t, v)
            })
            .count();
        d.layers.layers.push(row);
        let mut g = GroupLayer::default();
        g.stack.layers.push(LayerEntry::new("Seat 1", Layer::SeatPad(pad(30.0, v, Some(round(1.2))))));
        g.stack.layers.push(LayerEntry::new("Seat 2", Layer::SeatPad(pad(40.0, v, Some(round(1.2))))));
        g.stack.layers.push(LayerEntry::new("Marker", Layer::SeatPad(pad(50.0, v, None))));
        d.layers.layers.push(LayerEntry::new("Pavé", Layer::Group(g)));
        (d, 1 + kept_stations + 2)
    }

    /// The claw solitaire on the Court band, its head arrayed three round the ring: stone #2, head #3, bur #4,
    /// array #5 of the head.
    fn arrayed_claws() -> RingDesign {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        let doc = d.cad.as_mut().unwrap();
        let operation = Operation::Pattern { sources: 3.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } };
        doc.append(Feature { id: 5, name: "Ring array of Four-claw head".into(), enabled: true, operation, component: builders::component(builders::CLAW) }).unwrap();
        d
    }

    fn preview() -> crate::BuildParams {
        crate::BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() }
    }

    /// The Court band with `gem` in a four-claw head at the top and a halo of melee round it.
    fn halo(gem: Gem) -> RingDesign {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Default::default() }).unwrap();
        doc.append(builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm("claw4", gem)))).unwrap();
        let mut next = 2;
        for f in builders::setting_features("halo", 2, gem, false, &mut || { next += 1; next }).unwrap() {
            doc.append(f).unwrap();
        }
        RingDesign { cad: Some(doc), ..court }
    }

    /// With `RD_STONE_SHOTS=/dir`, the rings these tests set stones in, finished in studio gold with their stones.
    #[test]
    fn the_rings_render_with_their_stones_set() {
        let Some(dir) = std::env::var_os("RD_STONE_SHOTS") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let lib = crate::AlphaLibrary::builtin();
        let sapphire = Gem { preview_tint: Some([0.08, 0.16, 0.62]), ..Gem::calibrated(GemCut::Oval, 5.0) };
        let params = crate::BuildParams { theta_steps: 512, profile_steps: 192, ..Default::default() };
        for (name, d, yaw, pitch) in [("arrayed-claws", arrayed_claws(), 0.55, 1.12), ("halo-sapphire", halo(sapphire), 0.4, 0.9)] {
            let f = crate::render::finished(&d, &lib, params).unwrap();
            crate::render::write_png_parts(dir.join(format!("{name}.png")), &f.parts(crate::render::GOLD), yaw, pitch, 700).unwrap();
            eprintln!("{name}: {} stones, {} stone triangles in {} colours", set_stones(&d).len(), f.stone_faces(), f.stones.len());
        }
    }

    #[test]
    fn every_consumer_counts_the_same_stones() {
        let (d, expected) = design();
        let stones = set_stones(&d);
        assert_eq!(stones.len(), expected);
        assert!(expected > 4, "the window keeps some stations: {expected}");
        assert!(stones.iter().any(|s| s.label == "Pavé / Seat 2"));
        assert!(stones.iter().all(|s| s.label != "Off" && s.label != "Blank stock"));
        let lib = crate::AlphaLibrary::builtin();
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count as usize, stones.len(), "the report counts the record's stones");
        assert!((report.total_carats - stones.iter().map(|s| s.carats()).sum::<f64>()).abs() < 1e-9);
        let mesh = crate::gems::preview_mesh(&d, &lib).unwrap();
        assert_eq!(mesh.faces.len() % stones.len(), 0, "every stone draws the same facet count");

        // The same seats with a claw head arrayed three round the ring: the stone and two copies of it.
        let mut both = arrayed_claws();
        both.layers = d.layers.clone();
        let stones = set_stones(&both);
        let cad: Vec<&SetStone> = stones.iter().filter(|s| s.frame.is_some()).collect();
        assert_eq!((stones.len(), cad.len()), (expected + 3, 3), "the seats and three CAD stones");
        let report = crate::stones::report(&both, 0.0).unwrap();
        assert_eq!(report.stone_count as usize, stones.len(), "the report counts the CAD stones too");
        assert!((report.total_carats - stones.iter().map(|s| s.carats()).sum::<f64>()).abs() < 1e-9);
        let map = crate::stonemap::stone_map_svg(&both, Some(&report)).unwrap();
        assert_eq!(map.matches("class=\"stone\"").count(), 2 * stones.len(), "the map draws every one, CAD stones among them");
        let built = crate::mesh::try_build(&both, &lib, preview()).unwrap();
        let exact = set_stones_built(&both, &built);
        assert_eq!(exact.len(), stones.len(), "the build sets the stones the record lists");
        let soup = crate::gems::built_vertices(&both, &lib, &built);
        let per = crate::gems::preview_vertices(&RingDesign { cad: None, ..both.clone() }, &lib).len() / expected;
        assert_eq!(soup.len() % 12, 0);
        assert!(soup.len() >= per * expected + 3 * 12 * 32, "every CAD stone draws: {} floats", soup.len());
    }

    #[test]
    fn a_ring_array_of_claw_heads_carries_a_stone_per_copy_where_the_build_puts_it() {
        let d = arrayed_claws();
        let lib = crate::AlphaLibrary::builtin();
        let analytic = set_stones(&d);
        assert_eq!(analytic.len(), 3, "the stone and two copies");
        let thetas: Vec<f64> = analytic.iter().map(|s| s.theta_deg).collect();
        for (got, want) in thetas.iter().zip([90.0, 210.0, 330.0]) {
            assert!(wrap_delta(got - want, 360.0).abs() < 1e-6, "{thetas:?}");
        }
        assert!(matches!(analytic[0].source, StoneSource::Cad { feature: 2, copy: 0 }));
        assert!(matches!(analytic[2].source, StoneSource::Cad { feature: 5, copy: 1 }));
        assert_eq!(analytic[1].label, "Ring array of Four-claw head");
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let exact = set_stones_built(&d, &built);
        assert_eq!(exact.len(), 3);
        let mut worst: f64 = 0.0;
        for (a, b) in analytic.iter().zip(&exact) {
            let (fa, fb) = (a.frame.unwrap(), b.frame.unwrap());
            worst = worst.max(norm(sub(fa.origin, fb.origin)));
            assert!(dot(fa.z_axis, fb.z_axis) > 0.9999 && dot(fa.x_axis, fb.x_axis) > 0.9999, "the same bearing either way");
        }
        eprintln!("arrayed claws: analytic against built, worst {worst:.5} mm");
        assert!(worst < 0.02, "the bare section seats a stone within {worst:.4} mm of the build");
        // The girdle stands the claw setting's own height over the band.
        let want = builders::stand_off_mm("claw4", analytic[0].gem);
        for s in &analytic {
            assert!((s.stand_off_mm() - want).abs() < 0.02, "{} against {want}", s.stand_off_mm());
            assert!(s.rot_deg().abs() < 1e-6, "its length runs round the ring");
        }
        // The pattern's copies keep the stone their head was made for.
        let e = built.parts.evaluated.as_ref().unwrap();
        let array = e.components.iter().find(|c| c.id == 5).unwrap();
        assert_eq!(array.made.as_ref().unwrap().gem, Some(analytic[0].gem));
        // Every copy is drawn from the stone part's own mesh, standing where the build puts it.
        let stone = e.components.iter().find(|c| c.id == 2).unwrap();
        let soup = crate::gems::built_vertices(&d, &lib, &built);
        assert_eq!(soup.len(), 3 * stone.mesh.faces.len() * 36, "three stones of {} facets", stone.mesh.faces.len());
        for (k, s) in exact.iter().enumerate() {
            let tris = &soup[k * stone.mesh.faces.len() * 36..(k + 1) * stone.mesh.faces.len() * 36];
            let n = (tris.len() / 12) as f64;
            let mean: [f64; 3] = std::array::from_fn(|i| tris.chunks_exact(12).map(|v| f64::from(v[i])).sum::<f64>() / n);
            let local = pattern::inverse(&s.frame.unwrap()).point(mean);
            assert!(local[0].hypot(local[1]) < 0.05, "copy {k} is centred on its girdle: {local:?}");
        }
    }

    #[test]
    fn a_stone_moved_after_a_pattern_of_it_draws_each_copy_as_one_stone() {
        let lib = crate::AlphaLibrary::builtin();
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        let doc = d.cad.as_mut().unwrap();
        let ring = Operation::Pattern { sources: 2.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } };
        doc.append(Feature { id: 5, name: "Ring array of stone".into(), enabled: true, operation: ring, component: builders::component(builders::STONE) }).unwrap();
        let lift = Operation::Transform { source: 2, translation: [0.0, 0.0, 0.3], rotation_deg: [0.0; 3] };
        doc.append(Feature { id: 6, name: "Move".into(), enabled: true, operation: lift, component: builders::component(builders::STONE) }).unwrap();
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let exact = set_stones_built(&d, &built);
        assert_eq!(exact.len(), 3);
        let one = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 6).unwrap().mesh.faces.len() * 36;
        let soup = crate::gems::built_vertices(&d, &lib, &built);
        assert_eq!(soup.len(), 3 * one, "three stones' worth of facets");
        let girdles: Vec<[f64; 3]> = exact.iter().map(|s| s.frame.unwrap().origin).collect();
        let far = soup.chunks_exact(12).map(|v| girdles.iter().map(|g| norm(sub([f64::from(v[0]), f64::from(v[1]), f64::from(v[2])], *g))).fold(f64::MAX, f64::min)).fold(0.0, f64::max);
        assert!(far < 0.6 * exact[0].gem.w_mm, "every drawn point stands within its stone of a girdle: {far:.2} mm");
    }

    #[test]
    fn a_halos_melee_are_stones_where_its_collets_stand() {
        let gem = Gem::calibrated(GemCut::Round, 6.0);
        let d = halo(gem);
        let clock = std::time::Instant::now();
        let stones = set_stones(&d);
        eprintln!("a halo's record: {} stones in {:.2} ms", stones.len(), clock.elapsed().as_secs_f64() * 1e3);
        let (melee, stations) = builders::halo_melee(gem, &serde_json::json!({})).unwrap();
        assert_eq!(stones.len(), 1 + stations.len());
        assert!(stones[1..].iter().all(|s| s.gem == melee && s.label == "Halo"));
        let built = crate::mesh::try_build(&d, &crate::AlphaLibrary::builtin(), preview()).unwrap();
        let halo = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.settings.role == cad::ComponentRole::Setting && c.name == "Halo").unwrap();
        let exact = set_stones_built(&d, &built);
        for (s, p) in exact[1..].iter().zip(&halo.made.as_ref().unwrap().stations) {
            assert!(norm(sub(s.frame.unwrap().origin, *p)) < 1e-9, "each melee stands on its collet");
        }
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count as usize, stones.len());
        assert!(report.closest.as_ref().is_some_and(|p| p.a == "Halo" || p.b == "Halo"), "the census measures the melee");
    }

    #[test]
    fn a_stone_on_a_plates_face_stands_on_it_without_a_build() {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let gem = Gem::calibrated(GemCut::Round, 3.0);
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Default::default() }).unwrap();
        let plate = Feature {
            id: 2,
            name: "Plate".into(),
            enabled: true,
            operation: Operation::Box { size: [6.0, 6.0, 1.0] },
            component: cad::Component { placement: Placement::ring(90.0, 0.3), attach: cad::Attach::Join, ..Default::default() },
        };
        doc.append(plate).unwrap();
        let d0 = RingDesign { cad: Some(doc.clone()), ..court.clone() };
        let lib = crate::AlphaLibrary::builtin();
        let bare = crate::mesh::try_build(&d0, &lib, preview()).unwrap();
        let c = bare.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap().clone();
        let top = (0..c.body.faces.len() as u32).find(|&k| FaceSeat::on(&c, k, None, 0.4).and_then(|s| s.face_of(&c)).is_ok_and(|f| f.normal[1] > 0.9)).unwrap();
        let seat = FaceSeat::on(&c, top, None, 0.4).unwrap();
        doc.append(cad::stone_on_face(3, gem, 2, &seat)).unwrap();
        let d = RingDesign { cad: Some(doc), ..court };
        let analytic = set_stones(&d);
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let exact = set_stones_built(&d, &built);
        assert_eq!((analytic.len(), exact.len()), (1, 1));
        let gap = norm(sub(analytic[0].frame.unwrap().origin, exact[0].frame.unwrap().origin));
        eprintln!("stone on a plate: analytic against built, {gap:.5} mm");
        assert!(gap < 0.02, "the pruned plate seats its stone within {gap:.4} mm of the build");
        assert!(analytic[0].stand_off_mm() > 1.0, "the plate stands the stone off the band: {}", analytic[0].stand_off_mm());
    }

    /// Measured at 256 slices the sweep's normal lags the shoulder's fall by 7.6° (0.19 mm at the girdle); at 1024 it is 0.0001 mm.
    #[test]
    fn stones_on_a_signets_table_and_shoulder_stand_where_the_build_puts_them() {
        let mut d = RingDesign::default();
        d.profile.width_mm = 12.0;
        d.shank.apply_signet(12.0);
        let gem = Gem::calibrated(GemCut::Round, 3.0);
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Default::default() }).unwrap();
        let lift = builders::stand_off_mm("claw4", gem);
        for (id, theta, across) in [(2, 90.0, 0.0), (3, 55.0, 0.0), (4, 90.0, 2.5)] {
            doc.append(builders::stone_feature(id, gem, Placement::Ring { theta_deg: theta, across_mm: across, height_mm: lift, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 })).unwrap();
        }
        d.cad = Some(doc);
        let analytic = set_stones(&d);
        let built = crate::mesh::try_build(&d, &crate::AlphaLibrary::builtin(), crate::BuildParams { theta_steps: 1024, profile_steps: 128, ..Default::default() }).unwrap();
        let exact = set_stones_built(&d, &built);
        assert_eq!((analytic.len(), exact.len()), (3, 3));
        let doc = d.cad.as_ref().unwrap();
        let mut missed: f64 = 0.0;
        for (a, b) in analytic.iter().zip(&exact) {
            let gap = norm(sub(a.frame.unwrap().origin, b.frame.unwrap().origin));
            let turn = dot(a.frame.unwrap().z_axis, b.frame.unwrap().z_axis).clamp(-1.0, 1.0).acos().to_degrees();
            let miss = norm(sub(doc.feature(a.cad_feature().unwrap()).unwrap().component.placement.frame(&d).unwrap().origin, b.frame.unwrap().origin));
            missed = missed.max(miss);
            eprintln!("{:.0}° across {:.1}: {gap:.4} mm, {turn:.3}° from the build; the reference crest misses by {miss:.3} mm", a.theta_deg, a.frame.unwrap().origin[2]);
            assert!(gap < 0.05 && turn < 1.0, "{} at {:.0}°: {gap:.4} mm and {turn:.3}° from the build", a.label, a.theta_deg);
        }
        assert!(missed > 0.25, "the head stands off the reference crest: {missed:.3} mm");
    }

    /// The claw solitaire's cad on `band`, its head arrayed `count` round the ring as feature #5.
    fn heads_round(band: RingDesign, count: u32) -> RingDesign {
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..band };
        let operation = Operation::Pattern { sources: 3.into(), kind: PatternKind::Ring { count, span_deg: 360.0 } };
        let array = Feature { id: 5, name: "Ring array of Four-claw head".into(), enabled: true, operation, component: builders::component(builders::CLAW) };
        d.cad.as_mut().unwrap().append(array).unwrap();
        d
    }

    fn sampled() -> usize {
        SECTIONS_SAMPLED.with(|c| c.get())
    }

    /// Three arrays of 120 nested in one another carry 1.7 million stones on paper; the record counts 480 and names the parts past it.
    #[test]
    fn nested_arrays_are_counted_to_the_cap_and_the_report_says_so() {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = heads_round(court, 120);
        let doc = d.cad.as_mut().unwrap();
        for (id, source) in [(6, 5), (7, 6)] {
            let operation = Operation::Pattern { sources: source.into(), kind: PatternKind::Ring { count: 120, span_deg: 11.9 } };
            doc.append(Feature { id, name: format!("Nest {id}"), enabled: true, operation, component: builders::component(builders::CLAW) }).unwrap();
        }
        let clock = std::time::Instant::now();
        let record = record(&d, None);
        let took = clock.elapsed().as_secs_f64();
        eprintln!("nested arrays: {} stones, {} parts past the cap, in {:.1} ms", record.stones.len(), record.past_cap.len(), took * 1e3);
        assert_eq!(record.stones.len(), MAX_CAD_STONES, "the record stops at the cap");
        let past: Vec<Id> = record.past_cap.iter().map(|(id, _)| *id).collect();
        assert!(past.contains(&6) && past.contains(&7) && !past.contains(&5), "the nests are named, the plain array is not: {past:?}");
        assert!(took < 20.0, "counted in {took:.1} s");
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count as usize, MAX_CAD_STONES);
        let capped: Vec<&crate::stones::SeatCheck> = report.seats.iter().filter(|s| s.warnings.iter().any(|w| w.contains("480"))).collect();
        assert_eq!(capped.len(), 2, "each nest says it runs past the cap");
        assert!(capped.iter().all(|s| s.made.as_deref() == Some("Four-claw head")));
    }

    /// A redraw asks for the same stones again and samples no section for them; a changed band samples afresh.
    #[test]
    fn a_second_look_at_the_same_band_samples_no_section() {
        let mut signet = RingDesign::default();
        signet.profile.width_mm = 12.0;
        signet.shank.apply_signet(12.0);
        let mut pinched = RingDesign::default();
        pinched.shank.kind = crate::profile::ShankKind::Pinched;
        for (name, band) in [("lofted signet", signet), ("pinched", pinched)] {
            let d = heads_round(band, 24);
            let (before, clock) = (sampled(), std::time::Instant::now());
            let first = set_stones(&d);
            let (cold, cold_ms) = (sampled() - before, clock.elapsed().as_secs_f64() * 1e3);
            let (before, clock) = (sampled(), std::time::Instant::now());
            let again = set_stones(&d);
            let (warm, warm_ms) = (sampled() - before, clock.elapsed().as_secs_f64() * 1e3);
            eprintln!("{name}: 24 heads, {cold} sections in {cold_ms:.2} ms, then {warm} in {warm_ms:.2} ms");
            assert_eq!(first.len(), 24);
            assert!(cold > 24 && cold <= 4 * 24, "{cold} sections the first time");
            assert_eq!(warm, 0, "nothing sampled again");
            let (before, clock) = (sampled(), std::time::Instant::now());
            let report = crate::stones::report(&d, 0.0).unwrap();
            eprintln!("{name}: the report after it in {:.2} ms", clock.elapsed().as_secs_f64() * 1e3);
            assert_eq!((report.stone_count, sampled() - before), (24, 0), "the report reads the same sections");
            for (a, b) in first.iter().zip(&again) {
                assert_eq!((a.theta_deg, a.v_mm, a.frame.unwrap().origin, a.seat.height_mm), (b.theta_deg, b.v_mm, b.frame.unwrap().origin, b.seat.height_mm));
            }
            let mut wider = d.clone();
            wider.profile.width_mm += 0.5;
            let before = sampled();
            let moved = set_stones(&wider);
            assert!(sampled() > before, "a new band is sampled");
            let shift = first.iter().zip(&moved).map(|(a, b)| norm(sub(a.frame.unwrap().origin, b.frame.unwrap().origin))).fold(0.0, f64::max);
            assert!(shift > 1e-3, "{name}: the stones move with the band, {shift:.4} mm");
        }
    }

    /// A pattern of a bare stone copies no claws; the same copies carried by a pattern of the head are held by it.
    #[test]
    fn a_pattern_of_a_bare_stone_is_held_by_nothing_until_its_head_is_arrayed_with_it() {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        let doc = d.cad.as_mut().unwrap();
        let operation = Operation::Pattern { sources: 2.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } };
        doc.append(Feature { id: 5, name: "Ring array of Stone".into(), enabled: true, operation, component: builders::component(builders::STONE) }).unwrap();
        let report = crate::stones::report(&d, 0.0).unwrap();
        let bare = report.seats.iter().find(|s| s.label == "Ring array of Stone").unwrap();
        assert_eq!((bare.count, bare.made.as_deref()), (2, None), "two copies, no claws");
        assert!(bare.warnings.iter().any(|w| w.starts_with("no setting holds this stone")));
        let own = report.seats.iter().find(|s| s.count == 1 && s.gem.is_some()).unwrap();
        assert_eq!(own.made.as_deref(), Some("Four-claw head"), "the stone the head stands on is held");
        let lib = crate::AlphaLibrary::builtin();
        let built = crate::mesh::try_build(&d, &lib, crate::BuildParams { theta_steps: 128, profile_steps: 64, ..Default::default() }).unwrap();
        let field = crate::castability::analyze_field(&d, &lib, &d.draft, 96, 64);
        let stones = crate::stones::report_built(&d, field.parting_z_mm, &built);
        let sheet = crate::spec::html(&d, &built.report, &field, stones.as_ref(), &[], "test build");
        assert!(sheet.contains("Ring array of Stone ×2</td>"), "the sheet names no head for the copies");

        let operation = Operation::Pattern { sources: 3.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } };
        d.cad.as_mut().unwrap().append(Feature { id: 6, name: "Ring array of Four-claw head".into(), enabled: true, operation, component: builders::component(builders::CLAW) }).unwrap();
        let stones = set_stones(&d);
        assert_eq!(stones.len(), 3, "the arrays carry the same two copies");
        assert!(stones[1..].iter().all(|s| s.cad_feature() == Some(6)), "counted under the array that holds them");
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert!(report.seats.iter().all(|s| s.made.as_deref() == Some("Four-claw head") && s.warnings.is_empty()), "{:?}", report.seats.iter().map(|s| (&s.label, &s.made, &s.warnings)).collect::<Vec<_>>());
    }

    /// The claw solitaire's stone moved half a millimetre up the finger by Transform #9, its head re-pointed onto the moved stone.
    fn moved_solitaire() -> RingDesign {
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        let doc = d.cad.as_mut().unwrap();
        let lift = Operation::Transform { source: 2, translation: [0.0, 0.0, 0.5], rotation_deg: [0.0; 3] };
        let feature = Feature { id: 9, name: "Move".into(), enabled: true, operation: lift, component: builders::component(builders::STONE) };
        doc.apply(&cad::edit::CadEdit::Add { feature, after: Some(2) }).unwrap();
        let head = Operation::Builder { key: builders::CLAW.into(), on: Some(9), params: serde_json::json!({ "prongs": 4 }) };
        doc.apply(&cad::edit::CadEdit::Operation { id: 3, operation: head }).unwrap();
        d
    }

    #[test]
    fn a_stone_a_transform_moved_is_counted_where_the_build_moved_it() {
        let lib = crate::AlphaLibrary::builtin();
        let before = set_stones(&RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..moved_solitaire() });
        let d = moved_solitaire();
        let stones = set_stones(&d);
        assert_eq!(stones.len(), 1, "the moved stone is the one stone");
        assert!(matches!(stones[0].source, StoneSource::Cad { feature: 9, copy: 0 }));
        let shift = sub(stones[0].frame.unwrap().origin, before[0].frame.unwrap().origin);
        assert!(norm(sub(shift, [0.0, 0.0, 0.5])) < 1e-9, "moved by the Transform: {shift:?}");
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let exact = set_stones_built(&d, &built);
        let part = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 9).unwrap();
        assert_eq!(exact.len(), 1);
        let gap = norm(sub(exact[0].frame.unwrap().origin, part.frame.origin));
        assert!(gap < 1e-9 && dot(exact[0].frame.unwrap().z_axis, part.frame.z_axis) > 1.0 - 1e-12, "the moved part's own frame: {gap:.2e} mm");
        assert!(norm(sub(stones[0].frame.unwrap().origin, part.frame.origin)) < 0.02, "the bare band seats it within 0.02 mm of the build");
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count, 1);
        let check = report.seats.iter().find(|s| s.gem.is_some()).unwrap();
        assert_eq!((check.label.as_str(), check.made.as_deref()), ("Move", Some("Four-claw head")), "held by the head built round the moved stone");
        assert!(check.warnings.is_empty(), "{:?}", check.warnings);
        assert!(crate::stonemap::stone_map_svg(&d, Some(&report)).is_some(), "the map draws it");
        assert_eq!(crate::gems::built_vertices(&d, &lib, &built).len(), part.mesh.faces.len() * 36, "drawn once, from the moved part's mesh");

        // Turned as well as moved, the record turns it the way the evaluation does.
        let mut turned = d.clone();
        let lift = Operation::Transform { source: 2, translation: [0.3, -0.2, 0.5], rotation_deg: [2.0, -3.0, 10.0] };
        turned.cad.as_mut().unwrap().apply(&cad::edit::CadEdit::Operation { id: 9, operation: lift }).unwrap();
        let turned_build = crate::mesh::try_build(&turned, &lib, preview()).unwrap();
        let at = turned_build.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 9).unwrap().frame;
        for s in [set_stones_built(&turned, &turned_build)[0].frame.unwrap(), set_stones(&turned)[0].frame.unwrap()] {
            let axes = [(s.x_axis, at.x_axis), (s.y_axis, at.y_axis), (s.z_axis, at.z_axis)];
            assert!(norm(sub(s.origin, at.origin)) < 0.02 && axes.iter().all(|(a, b)| dot(*a, *b) > 1.0 - 1e-6), "{s:?} against {at:?}");
        }
        let exact = set_stones_built(&turned, &turned_build)[0].frame.unwrap();
        assert!(norm(sub(exact.origin, at.origin)) < 1e-9 && dot(exact.x_axis, at.x_axis) > 1.0 - 1e-12, "the built record is the part's own frame");

        // A ring array of the head round the moved stone carries a stone in every copy.
        let mut arrayed = d.clone();
        let operation = Operation::Pattern { sources: 3.into(), kind: PatternKind::Ring { count: 3, span_deg: 360.0 } };
        arrayed.cad.as_mut().unwrap().append(Feature { id: 5, name: "Ring array of Four-claw head".into(), enabled: true, operation, component: builders::component(builders::CLAW) }).unwrap();
        let stones = set_stones(&arrayed);
        assert_eq!(stones.len(), 3, "the moved stone and two copies");
        assert!(stones.iter().all(|s| (s.frame.unwrap().origin[2] - part.frame.origin[2]).abs() < 0.02), "every copy stands half a millimetre up the finger");
        let built = crate::mesh::try_build(&arrayed, &lib, preview()).unwrap();
        assert_eq!(set_stones_built(&arrayed, &built).len(), 3);
        let report = crate::stones::report(&arrayed, 0.0).unwrap();
        assert!(report.seats.iter().all(|s| s.made.as_deref() == Some("Four-claw head")), "every copy is held");
    }

    #[test]
    fn a_transform_round_a_head_leaves_its_stone_where_it_was_and_unheld() {
        let lib = crate::AlphaLibrary::builtin();
        let court = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut d = RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court };
        let original = set_stones(&d)[0].frame.unwrap().origin;
        let lift = Operation::Transform { source: 3, translation: [0.0, 0.0, 0.8], rotation_deg: [0.0; 3] };
        d.cad.as_mut().unwrap().append(Feature { id: 9, name: "Move head".into(), enabled: true, operation: lift, component: builders::component(builders::CLAW) }).unwrap();
        let stones = set_stones(&d);
        assert_eq!(stones.len(), 1, "{:?}", stones.iter().map(|s| (&s.label, s.source)).collect::<Vec<_>>());
        assert!(matches!(stones[0].source, StoneSource::Cad { feature: 2, copy: 0 }));
        assert!(norm(sub(stones[0].frame.unwrap().origin, original)) < 1e-9, "the stone stays where it was");
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        assert_eq!(set_stones_built(&d, &built).len(), 1);
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert_eq!(report.stone_count, 1);
        let check = report.seats.iter().find(|s| s.gem.is_some()).unwrap();
        assert_eq!(check.made, None, "the moved head holds nothing");
        assert!(check.warnings.iter().any(|w| w.starts_with("no setting holds this stone")), "{:?}", check.warnings);
    }

    #[test]
    fn a_boolean_keeps_the_stones_its_operands_carry_and_a_subtracted_one_carries_none() {
        let lib = crate::AlphaLibrary::builtin();
        let d = arrayed_claws();
        assert_eq!(set_stones(&d).len(), 3);
        let crest = d.inner_radius_mm() + d.profile.thickness_mm;
        let rail = Feature { id: 6, name: "Rail".into(), enabled: true, operation: Operation::Torus { major_mm: crest + 1.0, minor_mm: 0.4 }, component: Default::default() };
        let union = |kind, a, b| Feature { id: 7, name: "Heads and rail".into(), enabled: true, operation: Operation::Boolean { a, b, kind }, component: Default::default() };
        let mut joined = d.clone();
        let doc = joined.cad.as_mut().unwrap();
        doc.append(rail.clone()).unwrap();
        doc.append(union(cad::Boolean::Union, 5, 6)).unwrap();
        let stones = set_stones(&joined);
        assert_eq!(stones.len(), 3, "the three heads still hold three stones");
        assert!(matches!(stones[0].source, StoneSource::Cad { feature: 2, copy: 0 }) && stones[1..].iter().all(|s| s.cad_feature() == Some(7)));
        let built = crate::mesh::try_build(&joined, &lib, preview()).unwrap();
        let e = built.parts.evaluated.as_ref().unwrap();
        assert!(e.status_of(7).is_some_and(cad::FeatureStatus::is_ok), "{:?}", e.first_error());
        let exact = set_stones_built(&joined, &built);
        assert_eq!(exact.len(), 3, "the build's record keeps them too");
        for (a, b) in stones.iter().zip(&exact) {
            assert!(norm(sub(a.frame.unwrap().origin, b.frame.unwrap().origin)) < 0.02);
        }
        let report = crate::stones::report(&joined, 0.0).unwrap();
        assert_eq!(report.stone_count, 3);
        assert!(report.seats.iter().all(|s| s.made.as_deref() == Some("Four-claw head")), "the union's copies are held by the head it copies");

        let mut cut = d.clone();
        let doc = cut.cad.as_mut().unwrap();
        doc.append(rail).unwrap();
        doc.append(union(cad::Boolean::Subtract, 6, 5)).unwrap();
        assert_eq!(set_stones(&cut).len(), 1, "heads subtracted from the rail hold nothing; the stone part stays");
    }

    #[test]
    fn a_graded_run_records_each_stone_at_its_own_size() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.0);
        run.seat.v_mm = ctx.band_v_len_mm * 0.5;
        run.taper = 0.6;
        run.taper_theta_deg = 90.0;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Graded", Layer::SeatRun(run)));
        let stones = set_stones(&d);
        let big = stones.iter().max_by(|a, b| a.gem.w_mm.total_cmp(&b.gem.w_mm)).unwrap();
        let small = stones.iter().min_by(|a, b| a.gem.w_mm.total_cmp(&b.gem.w_mm)).unwrap();
        assert!(wrap_delta(big.theta_deg - 90.0, 360.0).abs() < 20.0, "largest at the pole");
        assert!(small.gem.w_mm < big.gem.w_mm * 0.6);
        assert!(small.seat.height_mm < big.seat.height_mm, "the seat scales with its stone");
        assert!(matches!(small.source, StoneSource::Run { .. }));
    }

    #[test]
    fn stones_near_picks_the_slices_neighbours() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let v = ctx.band_v_len_mm * 0.5;
        let g = Gem::calibrated(GemCut::Round, 3.0);
        d.layers.layers.push(LayerEntry::new("Top", Layer::SeatPad(pad(90.0, v, Some(g)))));
        d.layers.layers.push(LayerEntry::new("Bottom", Layer::SeatPad(pad(270.0, v, Some(g)))));
        let stones = set_stones(&d);
        let near = stones_near(&stones, &ctx, 92.0, 2.0);
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].label, "Top");
        assert!(stones_near(&stones, &ctx, 180.0, 2.0).is_empty());
    }
}
