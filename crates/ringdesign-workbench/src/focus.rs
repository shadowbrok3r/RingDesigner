//! What the chosen graph node does to the ring, as a highlight on the ring —
//! shared by the desktop and the phone.
//!
//! A node's reach comes from `ringdesign_graph::focus`. For layers it is
//! measured rather than guessed: the design is built once more with those
//! layers muted, at the parameters of the mesh on screen, and every vertex
//! that moved is the node's doing — a window, a mask, a blend that loses to
//! the layer above it and a group's Replace all come out right, because the
//! height field already composed them. The head is an arc and the band is
//! everything. The result is one weight a vertex for a renderer's focus
//! channel, the patch to turn the camera to, and two numbers for a caption.

use std::sync::Arc;

use ringdesign_core::RingDesign;
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::interaction::picking::{Hit, layers_at};
use ringdesign_core::mesh::{BuildParams, Mesh};
use ringdesign_graph::focus::{NodeEffect, Scope, without_layers};
use ringdesign_graph::graph::NodeId;

/// A vertex that moved less than this did not move: f32 noise and the far
/// tail of a fade are not the node's reach.
const MOVED_MM: f32 = 0.004;
/// Relief that reads at full strength; shallower relief fades toward it so a
/// pattern keeps its shape inside its own highlight.
const FULL_MM: f32 = 0.06;
/// The least a touched vertex shows, so faint relief is still found.
const FLOOR: f32 = 0.45;
/// Bins round the ring for finding where a reach sits.
const BINS: usize = 72;

pub struct Request {
    pub node: NodeId,
    /// The build the mesh on screen came from.
    pub generation: u64,
    /// The evaluated design, without its graph.
    pub design: RingDesign,
    pub lib: Arc<AlphaLibrary>,
    pub params: BuildParams,
    pub mesh: Arc<Mesh>,
    pub effect: NodeEffect,
    /// Where the camera stands now: of two equal patches the nearer is faced,
    /// and a reach that runs all the way round is seen from here.
    pub view_yaw: f32,
}

/// The patch of surface a camera should face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub reach_mm: f32,
}

pub struct Highlight {
    pub node: NodeId,
    pub generation: u64,
    /// One weight a mesh vertex; empty when nothing on the ring is reached.
    pub weights: Vec<f32>,
    /// `weights` in a renderer's own vertex order, when a worker staged them.
    pub staged: Vec<f32>,
    /// Share of the surface area reached, 0..1.
    pub share: f32,
    /// The furthest any vertex moved, mm; zero for the head and the band.
    pub moved_mm: f32,
    pub aim: Option<Aim>,
}

impl Highlight {
    pub fn is_empty(&self) -> bool {
        self.share <= 0.0
    }

    /// The measured half of a caption.
    pub fn words(&self, effect: &NodeEffect) -> String {
        match effect.scope {
            Scope::Layers if self.is_empty() => "no metal moves (off, masked out, or under another layer)".into(),
            Scope::Layers => format!("{:.0}% of the surface, up to {:.2} mm", (self.share * 100.0).max(1.0), self.moved_mm),
            Scope::Head => format!("{:.0}% of the surface", self.share * 100.0),
            _ => String::new(),
        }
    }
}

fn head_weights(design: &RingDesign, mesh: &Mesh) -> Vec<f32> {
    let head = &design.shank.head;
    let r = (design.inner_radius_mm() + design.profile.thickness_mm).max(1.0);
    let half_face = (head.length_mm * 0.5 / r).to_degrees();
    let reach = (half_face + head.shoulder_deg).clamp(5.0, 175.0);
    mesh.vertices
        .iter()
        .map(|p| {
            let theta = f64::from(p.1).atan2(f64::from(p.0)).to_degrees();
            let off = ringdesign_core::field::wrap_delta(theta - head.theta_deg, 360.0).abs();
            if off <= half_face {
                1.0
            } else if off <= reach {
                (1.0 - (off - half_face) / (reach - half_face).max(1e-6) * 0.55) as f32
            } else {
                0.0
            }
        })
        .collect()
}

fn layer_weights(req: &Request) -> (Vec<f32>, f32) {
    let none = (Vec::new(), 0.0);
    let without = without_layers(&req.design, &req.effect.layers);
    let Ok(before) = ringdesign_core::mesh::try_build(&without, &req.lib, req.params) else { return none };
    let (a, b) = (&req.mesh, &before.mesh);
    // Seats resolved as solids cut the two meshes differently, so they are compared through the band
    // vertex each came from; a solid's own vertices light with the layer that carries its stone.
    let plain = a.origin.is_empty() && b.origin.is_empty();
    if plain && (a.vertices.len() != b.vertices.len() || a.faces.len() != b.faces.len()) {
        return none;
    }
    let mut was: std::collections::HashMap<u32, ringdesign_core::Vec3> = std::collections::HashMap::new();
    if !plain {
        for (i, p) in b.vertices.iter().enumerate() {
            let o = b.origin.get(i).copied().unwrap_or(i as u32);
            if o < ringdesign_core::mesh::SOLID_VERTEX { was.insert(o, *p); }
        }
    }
    let lit: Vec<bool> = before.solids.paths.iter().map(|path| req.effect.layers.iter().any(|l| path.starts_with(l))).collect();
    let mut furthest = 0.0f32;
    let weights = a
        .vertices
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let o = if plain { i as u32 } else { a.origin.get(i).copied().unwrap_or(i as u32) };
            if o >= ringdesign_core::mesh::SOLID_VERTEX {
                return if lit.get((o - ringdesign_core::mesh::SOLID_VERTEX) as usize).copied().unwrap_or(false) { 1.0 } else { 0.0 };
            }
            let Some(q) = (if plain { b.vertices.get(i).copied() } else { was.get(&o).copied() }) else { return 0.0 };
            let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2) + (p.2 - q.2).powi(2)).sqrt();
            if !d.is_finite() || d < MOVED_MM {
                return 0.0;
            }
            furthest = furthest.max(d);
            FLOOR + (1.0 - FLOOR) * (d / FULL_MM).min(1.0)
        })
        .collect();
    (weights, furthest)
}

struct Patch {
    area: f64,
    mid: [f32; 3],
    normal: [f32; 3],
    bin: usize,
}

fn angle_between(a: f32, b: f32) -> f32 {
    ((a - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI).abs()
}

/// Where a camera should stand for a reach that has no one side: the view
/// it has, lifted to a three-quarter look — or over the side faces, when
/// that is where the reach lives. If the reach has a gap and the camera
/// stands in it, the camera steps round to the near end of the reach.
fn wrapped_aim(mesh: &Mesh, patches: &[Patch], occupied: &[bool], view_yaw: f32) -> Aim {
    let bin_of = |yaw: f32| ((yaw.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * BINS as f32) as usize).min(BINS - 1);
    let here = bin_of(view_yaw);
    let view_yaw = if occupied.get(here).copied().unwrap_or(true) {
        view_yaw
    } else {
        let step = (1..=BINS / 2).find_map(|k| {
            let (fwd, back) = ((here + k) % BINS, (here + BINS - k) % BINS);
            if occupied[fwd] { Some(k as f32) } else if occupied[back] { Some(-(k as f32)) } else { None }
        });
        match step {
            // A little past the edge, so the reach is in front rather than on the horizon.
            Some(k) => view_yaw + (k + 5.0 * k.signum()) / BINS as f32 * std::f32::consts::TAU,
            None => view_yaw,
        }
    };
    let area: f64 = patches.iter().map(|p| p.area).sum();
    let axial: f64 = patches.iter().map(|p| f64::from(p.normal[2]) * p.area).sum::<f64>() / area.max(1e-9);
    let steep: f64 = patches.iter().map(|p| f64::from(p.normal[2].abs()) * p.area).sum::<f64>() / area.max(1e-9);
    let pitch = if steep > 0.6 { if axial < -0.05 { -1.05f32 } else { 1.05 } } else { 0.5 };
    let (point, reach_mm) = match mesh.bounds() {
        Some((lo, hi)) => ([(lo.0 + hi.0) * 0.5, (lo.1 + hi.1) * 0.5, (lo.2 + hi.2) * 0.5], ((hi.0 - lo.0).powi(2) + (hi.1 - lo.1).powi(2) + (hi.2 - lo.2).powi(2)).sqrt() * 0.5),
        None => ([0.0; 3], 12.0),
    };
    Aim { point, normal: [pitch.cos() * view_yaw.cos(), pitch.cos() * view_yaw.sin(), pitch.sin()], reach_mm }
}

/// Area-weighted reach over the faces, and the patch to face: the largest
/// run of it round the ring, the nearer of equals. A layer mirrored onto
/// both shoulders has a mean that points at the face between them, where
/// none of it is; one shoulder is what there is to look at.
fn survey(mesh: &Mesh, weight: &[f32], view_yaw: f32) -> (f32, Option<Aim>) {
    let (mut total, mut hit) = (0.0f64, 0.0f64);
    let mut patches: Vec<Patch> = Vec::new();
    for face in &mesh.faces {
        let Some([a, b, c]) = face.iter().map(|&i| mesh.vertices.get(i as usize).copied()).collect::<Option<Vec<_>>>().and_then(|v| <[_; 3]>::try_from(v).ok()) else { continue };
        let (e1, e2) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let area = f64::from(len) * 0.5;
        if !area.is_finite() || area <= 0.0 {
            continue;
        }
        total += area;
        if face.iter().all(|&i| weight.get(i as usize).copied().unwrap_or(0.0) <= 0.0) {
            continue;
        }
        hit += area;
        let mid = [(a.0 + b.0 + c.0) / 3.0, (a.1 + b.1 + c.1) / 3.0, (a.2 + b.2 + c.2) / 3.0];
        // The bore faces the finger: counted as reach, but it is never the
        // side to look at, and its normals would cancel the outside's.
        if n[0] * mid[0] + n[1] * mid[1] < 0.0 {
            continue;
        }
        let theta = mid[1].atan2(mid[0]).rem_euclid(std::f32::consts::TAU);
        let bin = ((theta / std::f32::consts::TAU * BINS as f32) as usize).min(BINS - 1);
        patches.push(Patch { area, mid, normal: [n[0] / len, n[1] / len, n[2] / len], bin });
    }
    if total <= 0.0 || hit <= 0.0 {
        return (0.0, None);
    }
    let share = (hit / total) as f32;
    if patches.is_empty() {
        return (share, None);
    }
    let mut bins = [0.0f64; BINS];
    for p in &patches {
        bins[p.bin] += p.area;
    }
    let floor = bins.iter().copied().fold(0.0, f64::max) * 0.04;
    let occupied: Vec<bool> = bins.iter().map(|a| *a > floor).collect();
    // Most of the way round is all the way round: its middle is no more
    // the place to look than anywhere else on it.
    if occupied.iter().filter(|o| **o).count() * 100 >= BINS * 55 {
        return (share, Some(wrapped_aim(mesh, &patches, &occupied, view_yaw)));
    }
    // Runs of occupied bins round the circle, a one-bin gap bridged.
    let start = (0..BINS).find(|i| !occupied[*i] && !occupied[(*i + 1) % BINS]).unwrap_or(0);
    let mut runs: Vec<(Vec<usize>, f64)> = Vec::new();
    let mut run: Vec<usize> = Vec::new();
    for k in 1..=BINS {
        let i = (start + k) % BINS;
        if occupied[i] || (!run.is_empty() && occupied[(i + 1) % BINS]) {
            run.push(i);
        } else if !run.is_empty() {
            let area = run.iter().map(|b| bins[*b]).sum();
            runs.push((std::mem::take(&mut run), area));
        }
    }
    if !run.is_empty() {
        let area = run.iter().map(|b| bins[*b]).sum();
        runs.push((run, area));
    }
    let largest = runs.iter().map(|r| r.1).fold(0.0, f64::max);
    let centre = |bins: &[usize]| {
        let (s, c) = bins.iter().fold((0.0f32, 0.0f32), |(s, c), b| {
            let t = (*b as f32 + 0.5) / BINS as f32 * std::f32::consts::TAU;
            (s + t.sin(), c + t.cos())
        });
        s.atan2(c)
    };
    let Some((chosen, _)) = runs
        .iter()
        .filter(|r| r.1 >= largest * 0.7)
        .min_by(|a, b| angle_between(centre(&a.0), view_yaw).total_cmp(&angle_between(centre(&b.0), view_yaw)))
    else {
        return (share, None);
    };
    let (mut area, mut point, mut normal) = (0.0f64, [0.0f64; 3], [0.0f64; 3]);
    for p in patches.iter().filter(|p| chosen.contains(&p.bin)) {
        area += p.area;
        for k in 0..3 {
            point[k] += f64::from(p.mid[k]) * p.area;
            normal[k] += f64::from(p.normal[k]) * p.area;
        }
    }
    let point = point.map(|v| (v / area.max(1e-9)) as f32);
    let facing = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt() / area.max(1e-9);
    if facing < 0.2 {
        return (share, Some(wrapped_aim(mesh, &patches, &occupied, view_yaw)));
    }
    let reach_mm = patches.iter().filter(|p| chosen.contains(&p.bin)).map(|p| ((p.mid[0] - point[0]).powi(2) + (p.mid[1] - point[1]).powi(2) + (p.mid[2] - point[2]).powi(2)).sqrt()).fold(0.0f32, f32::max);
    (share, Some(Aim { point, normal: normal.map(|v| v as f32), reach_mm }))
}

/// The highlight for one request. Pure: workers and tests call it alike.
pub fn compute(req: &Request) -> Highlight {
    let (weights, moved_mm) = match req.effect.scope {
        Scope::Layers => layer_weights(req),
        Scope::Head => (head_weights(&req.design, &req.mesh), 0.0),
        Scope::Band => (vec![0.5; req.mesh.vertices.len()], 0.0),
        Scope::Settings | Scope::Nothing => (Vec::new(), 0.0),
    };
    let (share, aim) = if weights.is_empty() { (0.0, None) } else { survey(&req.mesh, &weights, req.view_yaw) };
    let weights = if share > 0.0 { weights } else { Vec::new() };
    Highlight { node: req.node, generation: req.generation, weights, staged: Vec::new(), share, moved_mm, aim }
}

/// The layer that made the metal under a tap: of those with relief there,
/// the one whose absence changes the surface most. A layer can have relief
/// at a point and still lose its blend to the one above it; the first with
/// relief is not the one the eye is on.
pub fn layer_behind(design: &RingDesign, lib: &AlphaLibrary, hit: &Hit) -> Option<usize> {
    let candidates = layers_at(design, lib, hit);
    let ctx = design.field_context();
    let uv = ringdesign_core::field::Uv { u: ctx.u_of_theta(hit.theta_deg), v: hit.v_mm };
    let full = design.layers.height(uv, &ctx, lib);
    let mut stack = design.layers.clone();
    let mut best: Option<(usize, f64)> = None;
    for &i in &candidates {
        stack.layers[i].enabled = false;
        let moved = (full - stack.height(uv, &ctx, lib)).abs();
        stack.layers[i].enabled = true;
        if moved > 0.002 && best.is_none_or(|(_, m)| moved > m) {
            best = Some((i, moved));
        }
    }
    best.map(|(i, _)| i).or_else(|| candidates.first().copied())
}

/// The highlight's strength as it settles: bright when a node is first
/// chosen, so the eye finds it, then steady. Returns the strength and
/// whether it is still moving.
pub fn pulse(since_s: f32) -> (f32, bool) {
    const FLASH_S: f32 = 0.9;
    let t = (since_s / FLASH_S).clamp(0.0, 1.0);
    let ease = 1.0 - (1.0 - t) * (1.0 - t);
    (0.9 - 0.32 * ease, t < 1.0)
}

/// An orbit camera's pose, as both apps' cameras hold it: angles in radians
/// round a target, a zoom where 1 frames the model, a pan in view-plane mm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub yaw: f32,
    pub pitch: f32,
    /// Turn about the view axis; what lets a ring be looked at upside down.
    pub roll: f32,
    pub zoom: f32,
    pub pan: [f32; 2],
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let len = dot(a, a).sqrt();
    if len > 1e-9 { [a[0] / len, a[1] / len, a[2] / len] } else { [0.0, 0.0, 1.0] }
}

/// The view's right and up for a pose, world up being the finger axis,
/// turned about the view axis by `roll`.
pub fn view_axes(yaw: f32, pitch: f32, roll: f32) -> ([f32; 3], [f32; 3]) {
    let d = [pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()];
    // The way the eye moves as it tilts, as the cameras have it: continuous over the poles.
    let up = [-pitch.sin() * yaw.cos(), -pitch.sin() * yaw.sin(), pitch.cos()];
    let f = [-d[0], -d[1], -d[2]];
    let s = unit(cross(f, up));
    let u = cross(s, f);
    let (sin, cos) = roll.sin_cos();
    ([s[0] * cos + u[0] * sin, s[1] * cos + u[1] * sin, s[2] * cos + u[2] * sin], [u[0] * cos - s[0] * sin, u[1] * cos - s[1] * sin, u[2] * cos - s[2] * sin])
}

/// The pose that looks straight at a patch: down its outward normal, the
/// patch in the middle of the view, enough of the ring round it to say
/// where it is. `radius` is the fitted bounding radius the camera frames
/// at zoom 1 (times its own 1.15 margin), `target` what it orbits.
pub fn aim_pose(current: Pose, target: [f32; 3], radius: f32, aim: &Aim) -> Pose {
    use std::f32::consts::FRAC_PI_2;
    let n = unit(aim.normal);
    // Straight down the finger axis there is no yaw to read; keep the one in hand.
    let yaw = if n[0].hypot(n[1]) > 0.05 { n[1].atan2(n[0]) } else { current.yaw };
    let pitch = n[2].clamp(-1.0, 1.0).asin().clamp(-FRAC_PI_2 + 0.02, FRAC_PI_2 - 0.02);
    let (s, u) = view_axes(yaw, pitch, current.roll);
    let rel = [aim.point[0] - target[0], aim.point[1] - target[1], aim.point[2] - target[2]];
    // A seat is a millimetre or two across: framed alone it is a pink disc
    // with nothing round it to say where on the ring it is.
    let zoom = (radius * 1.15 / (aim.reach_mm.max(3.0) * 2.4)).clamp(1.0, 3.5);
    Pose { yaw, pitch, roll: current.roll, zoom, pan: [dot(rel, s), dot(rel, u)] }
}

/// Part of the way from one pose to another, yaw taking the short way round.
pub fn ease(from: Pose, to: Pose, t: f32) -> Pose {
    let t = t.clamp(0.0, 1.0);
    let e = t * t * (3.0 - 2.0 * t);
    let lerp = |a: f32, b: f32| a + (b - a) * e;
    let short = |a: f32, b: f32| (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let turn = short(from.yaw, to.yaw);
    Pose { yaw: from.yaw + turn * e, pitch: from.pitch + short(from.pitch, to.pitch) * e, roll: from.roll + short(from.roll, to.roll) * e, zoom: lerp(from.zoom, to.zoom), pan: [lerp(from.pan[0], to.pan[0]), lerp(from.pan[1], to.pan[1])] }
}

/// A camera turn under way.
#[derive(Clone, Copy, Debug)]
pub struct Turn {
    pub from: Pose,
    pub to: Pose,
    pub since: std::time::Instant,
}

impl Turn {
    pub const SECONDS: f32 = 0.42;

    pub fn new(from: Pose, to: Pose) -> Self {
        Self { from, to, since: std::time::Instant::now() }
    }

    /// The pose now, and whether the turn has arrived.
    pub fn now(&self) -> (Pose, bool) {
        let t = self.since.elapsed().as_secs_f32() / Self::SECONDS;
        (ease(self.from, self.to, t), t >= 1.0)
    }
}

/// A renderer's staging of per-vertex weights into its own vertex order.
pub type Stage = fn(&Mesh, &[f32]) -> Vec<f32>;

/// Highlights are computed off the UI thread; only the newest request matters.
#[cfg(not(target_arch = "wasm32"))]
pub struct Worker {
    jobs: std::sync::mpsc::Sender<Request>,
    done: std::sync::mpsc::Receiver<Highlight>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Worker {
    pub fn spawn(wake: impl Fn() + Send + 'static, stage: Stage) -> Self {
        let (jobs, jobs_rx) = std::sync::mpsc::channel::<Request>();
        let (done_tx, done) = std::sync::mpsc::channel::<Highlight>();
        std::thread::Builder::new()
            .name("ring-focus".into())
            .spawn(move || {
                while let Ok(mut job) = jobs_rx.recv() {
                    while let Ok(newer) = jobs_rx.try_recv() {
                        job = newer;
                    }
                    let mut hl = compute(&job);
                    if !hl.weights.is_empty() {
                        hl.staged = stage(&job.mesh, &hl.weights);
                        hl.weights = Vec::new();
                    }
                    if done_tx.send(hl).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .expect("spawn focus worker");
        Self { jobs, done }
    }

    pub fn request(&self, req: Request) -> bool {
        self.jobs.send(req).is_ok()
    }

    pub fn poll(&self) -> Option<Highlight> {
        self.done.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_graph::eval::{Evaluator, evaluate_design};
    use ringdesign_graph::registry::Registry;

    const PARAMS: BuildParams = BuildParams { theta_steps: 192, profile_steps: 72, min_wall_mm: 0.5, adaptive: false, refine: None, soften_mm: 0.0 };

    fn evaluated(name: &str) -> (ringdesign_graph::graph::Graph, RingDesign, Arc<AlphaLibrary>, std::collections::BTreeMap<NodeId, NodeEffect>) {
        let reg = Registry::builtin();
        let lib = AlphaLibrary::builtin();
        let g = ringdesign_graph::templates::graph(name).expect("bundled");
        let out = evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).expect("evaluates");
        let fx = ringdesign_graph::focus::effects(&g, &reg, &out.report.values, &out.design);
        let lib = out.baked_library.clone().unwrap_or_else(|| Arc::new(lib));
        (g, (*out.design).clone(), lib, fx)
    }

    fn request(design: &RingDesign, lib: &Arc<AlphaLibrary>, node: NodeId, effect: &NodeEffect) -> Request {
        let mesh = Arc::new(ringdesign_core::mesh::build(design, lib, PARAMS).mesh);
        Request { node, generation: 7, design: design.clone(), lib: lib.clone(), params: PARAMS, mesh, effect: effect.clone(), view_yaw: 0.3 }
    }

    #[test]
    fn a_windowed_layer_lights_its_arc_and_its_parts_cast_the_same_light() {
        let (g, design, lib, fx) = evaluated("Shouldered cushion signet");
        let entry = g.entry_nodes()[0];
        let req = request(&design, &lib, entry, &fx[&entry]);
        let hl = compute(&req);
        assert_eq!((hl.node, hl.generation), (entry, 7));
        assert_eq!(hl.weights.len(), req.mesh.vertices.len(), "one weight a vertex");
        assert!(hl.share > 0.02 && hl.share < 0.9, "a window reaches part of the ring: {}", hl.share);
        assert!(hl.moved_mm > 0.05, "{}", hl.moved_mm);
        assert!(hl.weights.iter().all(|w| *w == 0.0 || (*w >= FLOOR && *w <= 1.0)));
        assert!(hl.words(&fx[&entry]).contains("mm"));
        for w in g.wires_into(entry) {
            let again = compute(&request(&design, &lib, w.from, &fx[&w.from]));
            assert_eq!(again.weights, hl.weights, "{}", g.node(w.from).unwrap().kind);
        }
    }

    #[test]
    fn a_seat_with_a_claw_head_lights_the_head_and_the_camera_faces_it() {
        use ringdesign_core::field::{Layer, LayerEntry, SeatPadLayer};
        use ringdesign_core::gem::{Gem, GemCut};
        let lib = Arc::new(AlphaLibrary::builtin());
        let mut design = RingDesign::default();
        let v = design.field_context().crest_v_mm;
        for (name, theta) in [("Centre", 90.0), ("Other", 250.0)] {
            let mut pad = SeatPadLayer { theta_deg: theta, v_mm: v, blend_mm: 0.4, solid: ringdesign_core::setting::SolidKind::Prong, ..Default::default() };
            pad.fit_stone(Gem::calibrated(GemCut::Round, 3.0));
            pad.height_mm = 0.3;
            design.layers.layers.push(LayerEntry::new(name, Layer::SeatPad(pad)));
        }
        let effect = NodeEffect { scope: Scope::Layers, layers: vec![vec![0]], names: vec!["Centre".into()] };
        let req = request(&design, &lib, NodeId(3), &effect);
        assert!(!req.mesh.origin.is_empty(), "the head is resolved into the band");
        let hl = compute(&req);
        assert_eq!(hl.weights.len(), req.mesh.vertices.len());
        let solid = |i: usize| req.mesh.origin[i] >= ringdesign_core::mesh::SOLID_VERTEX;
        let (mut mine, mut theirs) = (0, 0);
        for (i, w) in hl.weights.iter().enumerate().filter(|(i, _)| solid(*i)) {
            let top = req.mesh.vertices[i].1 > 0.0;
            if top { assert_eq!(*w, 1.0, "the chosen seat's head is lit whole"); mine += 1; } else { assert_eq!(*w, 0.0, "and the other seat's is not"); theirs += 1; }
        }
        assert!(mine > 500 && theirs > 500, "{mine} {theirs}");
        let aim = hl.aim.expect("a head to face");
        assert!((aim.normal[1].atan2(aim.normal[0]).to_degrees() - 90.0).abs() < 25.0, "{:?}", aim.normal);
    }

    #[test]
    fn a_reach_on_both_shoulders_is_faced_on_the_nearer_one_not_between_them() {
        use ringdesign_core::field::{BorderLayer, Layer, LayerEntry};
        let lib = Arc::new(AlphaLibrary::builtin());
        let mut design = RingDesign::default();
        let ctx = design.field_context();
        // One rail on each shoulder: the mean of the two points at the top
        // between them, where neither is.
        for theta in [30.0, 150.0] {
            let mut e = LayerEntry::new(format!("rail {theta}"), Layer::Border(BorderLayer { v_mm: ctx.crest_v_mm, width_mm: 1.4, height_mm: 0.3, mirror: false, ..Default::default() }));
            e.window.enabled = true;
            e.window.theta_deg = theta;
            e.window.span_deg = 30.0;
            e.window.fade_deg = 4.0;
            design.layers.layers.push(e);
        }
        let effect = NodeEffect { scope: Scope::Layers, layers: vec![vec![0], vec![1]], names: vec!["a".into(), "b".into()] };
        for (view, want) in [(0.2f32, 30.0f32), (2.9, 150.0)] {
            let mut req = request(&design, &lib, NodeId(1), &effect);
            req.view_yaw = view;
            let hl = compute(&req);
            assert!(hl.share > 0.005 && hl.share < 0.2, "{}", hl.share);
            let aim = hl.aim.expect("a patch to face");
            let yaw = aim.normal[1].atan2(aim.normal[0]).to_degrees();
            assert!((yaw - want).abs() < 8.0, "from {view} the camera goes to the rail at {want}, not {yaw}");
            assert!(aim.reach_mm < 6.0, "one rail, not both: {}", aim.reach_mm);
        }
    }

    #[test]
    fn a_reach_with_a_gap_is_entered_from_its_near_end() {
        // The cushion signet's tiling runs everywhere but the top third: a
        // camera on the face steps round to where the pattern starts.
        let (g, design, lib, fx) = evaluated("Shouldered cushion signet");
        let entry = g.entry_nodes()[0];
        let head = design.shank.head.theta_deg.to_radians() as f32;
        let mut req = request(&design, &lib, entry, &fx[&entry]);
        req.view_yaw = head + 0.1;
        let aim = compute(&req).aim.expect("a view");
        let off = angle_between(aim.normal[1].atan2(aim.normal[0]), head);
        assert!(off > 1.0 && off < 2.2, "past the window's edge and not round at the palm: {off}");
        req.view_yaw = head + 2.4;
        let kept = compute(&req).aim.unwrap();
        assert!(angle_between(kept.normal[1].atan2(kept.normal[0]), head + 2.4) < 1e-3, "already over the reach: stay");
    }

    #[test]
    fn the_head_is_an_arc_the_band_frames_the_ring_and_settings_are_nothing() {
        let (g, design, lib, fx) = evaluated("Shouldered cushion signet");
        let of = |kind: &str| g.nodes.iter().find(|n| n.kind == kind).map(|n| n.id);
        let head = of("shank.signet").expect("the signet node");
        assert_eq!(fx[&head].scope, Scope::Head);
        let hl = compute(&request(&design, &lib, head, &fx[&head]));
        assert!(hl.share > 0.1 && hl.share < 0.8, "{}", hl.share);
        let aim = hl.aim.expect("a head has a side to face");
        let top = design.shank.head.theta_deg.to_radians();
        let n = unit(aim.normal);
        assert!((n[0] - top.cos() as f32).abs() < 0.15 && (n[1] - top.sin() as f32).abs() < 0.15, "{n:?}");

        let profile = of("band.profile").unwrap();
        assert_eq!(fx[&profile].scope, Scope::Band);
        let hl = compute(&request(&design, &lib, profile, &fx[&profile]));
        assert!(hl.share > 0.99);
        let aim = hl.aim.expect("the whole band is framed from where the camera stands");
        assert!(aim.reach_mm > 8.0 && (aim.normal[1].atan2(aim.normal[0]) - 0.3).abs() < 1e-4);

        let hl = compute(&request(&design, &lib, profile, &NodeEffect { scope: Scope::Settings, ..Default::default() }));
        assert!(hl.weights.is_empty() && hl.is_empty() && hl.aim.is_none());
    }

    #[test]
    fn a_full_ring_layer_is_seen_from_where_the_camera_stands_and_a_muted_one_moves_nothing() {
        let (g, mut design, lib, fx) = evaluated("Braided band");
        let entry = g.entry_nodes()[0];
        let hl = compute(&request(&design, &lib, entry, &fx[&entry]));
        assert!(hl.share > 0.2, "{}", hl.share);
        let aim = hl.aim.expect("a wrapped reach still gets a view");
        assert!((aim.normal[1].atan2(aim.normal[0]) - 0.3).abs() < 1e-4, "the yaw in hand is kept");
        let path = fx[&entry].layers[0].clone();
        design.layers.layers[path[0]].enabled = false;
        let off = compute(&request(&design, &lib, entry, &fx[&entry]));
        assert!(off.weights.is_empty() && off.aim.is_none());
        assert!(off.words(&fx[&entry]).contains("no metal moves"));
    }

    #[test]
    fn a_tap_finds_the_layer_that_wins_the_blend_there() {
        use ringdesign_core::field::{Blend, BorderLayer, Layer, LayerEntry};
        let lib = AlphaLibrary::builtin();
        let mut design = RingDesign::default();
        let ctx = design.field_context();
        let rail = |height_mm: f64| Layer::Border(BorderLayer { v_mm: ctx.crest_v_mm, width_mm: 1.2, height_mm, mirror: false, ..Default::default() });
        let mut low = LayerEntry::new("low rail", rail(0.05));
        low.blend = Blend::Max;
        let mut high = LayerEntry::new("high rail", rail(0.40));
        high.blend = Blend::Max;
        design.layers.layers = vec![high, low];
        let hit = Hit { ray: ([0.0; 3], [0.0, 0.0, 1.0]), world: [0.0; 3], face: 0, theta_deg: 40.0, v_mm: ctx.crest_v_mm, radial_wall_mm: 2.0, relief_mm: 0.4 };
        assert_eq!(layers_at(&design, &lib, &hit).first(), Some(&1));
        assert_eq!(layer_behind(&design, &lib, &hit), Some(0));
        let bare = Hit { v_mm: ctx.crest_v_mm + 2.5, ..hit };
        assert_eq!(layer_behind(&design, &lib, &bare), None, "bare band has no layer behind it");
    }

    #[test]
    fn aiming_centres_the_patch_faces_it_and_eases_the_short_way_round() {
        let current = Pose { yaw: 3.0, pitch: 0.2, roll: 0.0, zoom: 3.0, pan: [4.0, -2.0] };
        let target = [0.5, 1.5, 0.0];
        for (point, normal) in [([0.0, 10.5, 0.0], [0.0, 1.0, 0.0]), ([-9.0, 2.0, 3.5], [0.0, 0.1, 1.0]), ([7.0, -7.0, -1.0], [0.7, -0.7, -0.2])] {
            let pose = aim_pose(current, target, 17.0, &Aim { point, normal, reach_mm: 2.5 });
            let (s, u) = view_axes(pose.yaw, pose.pitch, pose.roll);
            let rel = [point[0] - target[0], point[1] - target[1], point[2] - target[2]];
            assert!((dot(rel, s) - pose.pan[0]).abs() < 1e-4 && (dot(rel, u) - pose.pan[1]).abs() < 1e-4, "the patch is the middle of the view");
            let d = [pose.pitch.cos() * pose.yaw.cos(), pose.pitch.cos() * pose.yaw.sin(), pose.pitch.sin()];
            assert!(dot(d, unit(normal)) > 0.9, "the camera stands out along the patch's normal");
            assert!((1.0..=3.5).contains(&pose.zoom));
        }
        assert_eq!(aim_pose(current, target, 17.0, &Aim { point: target, normal: [0.0, 1.0, 0.0], reach_mm: 40.0 }).zoom, 1.0);
        let to = Pose { yaw: -3.0, zoom: 2.0, ..current };
        let mid = ease(current, to, 0.5);
        assert!(mid.yaw > 3.0 && mid.yaw < 3.3, "through pi, not back through zero: {}", mid.yaw);
        let end = ease(current, to, 1.0);
        assert!((end.yaw.sin() - to.yaw.sin()).abs() < 1e-5 && end.zoom == 2.0);
        // Upside down the patch is still the middle of the view, and the turn rolls the short way.
        let flipped = Pose { roll: std::f32::consts::PI, ..current };
        let pose = aim_pose(flipped, target, 17.0, &Aim { point: [0.0, 10.5, 1.0], normal: [0.0, 1.0, 0.0], reach_mm: 2.5 });
        let (s, u) = view_axes(pose.yaw, pose.pitch, pose.roll);
        let (s0, u0) = view_axes(pose.yaw, pose.pitch, 0.0);
        assert!((dot(s, s0) + 1.0).abs() < 1e-5 && (dot(u, u0) + 1.0).abs() < 1e-5, "rolled half way round, right is left and up is down");
        let rel = [0.0 - target[0], 10.5 - target[1], 1.0 - target[2]];
        assert!((dot(rel, s) - pose.pan[0]).abs() < 1e-4 && (dot(rel, u) - pose.pan[1]).abs() < 1e-4 && pose.roll == flipped.roll);
        let half = ease(Pose { roll: 3.0, ..current }, Pose { roll: -3.0, ..current }, 0.5);
        assert!(half.roll > 3.0 && half.roll < 3.3, "{}", half.roll);
        let (start, moving) = pulse(0.0);
        let (held, still) = pulse(5.0);
        assert!(start > held && moving && !still && (0.5..0.7).contains(&held));
    }
}
