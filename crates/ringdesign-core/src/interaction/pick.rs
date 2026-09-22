//! One pick scene over a built ring — the fused band with its CAD parts, and the stones on it —
//! answered through a BVH, so hover, selection, snapping, headless tests and MCP name the same
//! entity for the same ray. A pick is what is visible: the surface under the cursor, and the part
//! vertices and edges within the aperture that lie on that surface and are not behind it.
//! Box selection is x-ray: a part and its faces, edges and vertices are judged on the part's own
//! placed geometry, whole; the band on the fused faces it owns; a stone on its facets.
use super::bvh::{self, Bvh, cross, dist2, dot, sub};
use crate::{AlphaLibrary, BuildResult, Mesh, RingDesign, sketch::Id};
use std::collections::HashMap;

/// A world-space ray; `pick` normalises the direction so depths are millimetres.
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: [f64; 3],
    pub direction: [f64; 3],
}

/// The screen's axes and scale at the cursor, so an aperture in pixels is a reach in millimetres.
#[derive(Clone, Copy, Debug)]
pub struct ViewScale {
    pub right: [f64; 3],
    pub up: [f64; 3],
    pub px_per_mm: f64,
}

/// What a pick names. Face, edge and vertex ordinals index the part's B-rep in its own order:
/// `PartTrace::face_kind`, `EvaluatedComponent::edges` and `PartTrace::vertices`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Entity {
    Band,
    Part { feature: Id },
    Face { feature: Id, face: u32 },
    Edge { feature: Id, edge: u32 },
    Vertex { feature: Id, vertex: u32 },
    Stone { path: Vec<usize> },
}

/// One thing under the cursor: where on it, the unit normal of the surface there, its depth along
/// the ray in mm and its distance from the cursor in pixels.
#[derive(Clone, Debug)]
pub struct Pick {
    pub entity: Entity,
    pub world: [f64; 3],
    pub normal: [f64; 3],
    pub depth: f64,
    pub px: f32,
}

/// Which entity classes a query may answer with.
#[derive(Clone, Copy, Debug)]
pub struct Filter {
    pub vertices: bool,
    pub edges: bool,
    pub faces: bool,
    pub parts: bool,
    pub band: bool,
    pub stones: bool,
}

impl Default for Filter {
    fn default() -> Self {
        Self { vertices: true, edges: true, faces: true, parts: true, band: true, stones: true }
    }
}

impl Filter {
    /// Nothing; a starting point for `Filter { edges: true, ..Filter::none() }`.
    pub fn none() -> Self {
        Self { vertices: false, edges: false, faces: false, parts: false, band: false, stones: false }
    }
}

/// A part's edge or vertex counts as exposed within this of the fused surface: the part's own
/// tessellation sags up to its 0.04 mm chord inside the true surface its edges are sampled on.
pub const ON_SURFACE_MM: f64 = 0.1;
/// A surface nearer than the point by less than this does not hide it.
pub const OCCLUSION_MM: f64 = 0.1;
/// A fused face whose centroid lies within this of a part's own surface is that part's.
const ON_PART_MM: f64 = 1e-4;
/// Owner of a fused face no part claims.
const BAND: u32 = u32::MAX;

/// One CAD part: its own placed tessellation for face ordinals and box selection, its B-rep edges and vertices.
struct Part {
    feature: Id,
    mesh: Mesh,
    bvh: Bvh,
    /// Face ordinal behind each of the part's triangles; `u32::MAX` for a stitched gap.
    tri_face: Vec<u32>,
    edges: Vec<Vec<[f64; 3]>>,
    vertices: Vec<[f64; 3]>,
    /// Face ordinals the body has.
    faces: u32,
}

/// Every stone's preview facets in one soup, each facet naming its stone.
struct Stones {
    paths: Vec<Vec<usize>>,
    mesh: Mesh,
    bvh: Bvh,
    owner: Vec<u32>,
}

pub struct PickScene {
    mesh: Mesh,
    bvh: Bvh,
    /// Per fused face: `BAND`, or the index into `parts`.
    owner: Vec<u32>,
    /// Per fused face: the owning part's face ordinal, or `u32::MAX`.
    ordinal: Vec<u32>,
    parts: Vec<Part>,
    stones: Option<Stones>,
}

/// Class order a pick is ranked by, before screen distance and depth.
fn rank(e: &Entity) -> u8 {
    match e {
        Entity::Vertex { .. } => 0,
        Entity::Edge { .. } => 1,
        Entity::Face { .. } => 2,
        Entity::Part { .. } => 3,
        Entity::Stone { .. } => 4,
        Entity::Band => 5,
    }
}

impl PickScene {
    /// The scene over a build: the fused mesh, the parts its origin names, and the design's stones.
    pub fn build(built: &BuildResult, design: &RingDesign) -> Self {
        let src = &built.mesh;
        let mesh = Mesh { vertices: src.vertices.clone(), faces: src.faces.clone(), ..Default::default() };
        let bvh = Bvh::build(&mesh);
        let mut parts = Vec::new();
        let mut by_feature: HashMap<Id, u32> = HashMap::new();
        if let Some(e) = &built.parts.evaluated {
            for c in &e.components {
                if c.settings.reference {
                    continue;
                }
                let own = Mesh { vertices: c.mesh.vertices.clone(), faces: c.mesh.faces.clone(), ..Default::default() };
                let bvh = Bvh::build(&own);
                by_feature.insert(c.id, parts.len() as u32);
                parts.push(Part {
                    feature: c.id,
                    mesh: own,
                    bvh,
                    tri_face: c.trace.tri_face.clone(),
                    edges: c.edges.clone(),
                    vertices: c.trace.vertices.clone(),
                    faces: c.trace.face_kind.len() as u32,
                });
            }
        }
        let n = mesh.faces.len();
        let mut owner = vec![BAND; n];
        let mut ordinal = vec![u32::MAX; n];
        if src.origin.len() == src.vertices.len() && !parts.is_empty() {
            for (i, f) in mesh.faces.iter().enumerate() {
                // Candidates are the parts the face's vertices name; the surface decides between them.
                let mut candidates = [BAND; 3];
                for (k, &v) in f.iter().enumerate() {
                    if let Some(p) = src.origin.get(v as usize).and_then(|o| built.parts.feature_of(*o)).and_then(|id| by_feature.get(&id)) {
                        candidates[k] = *p;
                    }
                }
                if candidates.iter().all(|c| *c == BAND) {
                    continue;
                }
                let Some((a, b, c)) = mesh.triangle(f) else { continue };
                let centroid: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
                for (k, &p) in candidates.iter().enumerate() {
                    if p == BAND || candidates[..k].contains(&p) {
                        continue;
                    }
                    let part = &parts[p as usize];
                    if let Some((tri, _)) = part.bvh.nearest(&part.mesh, centroid, ON_PART_MM) {
                        owner[i] = p;
                        ordinal[i] = part.tri_face.get(tri).copied().unwrap_or(u32::MAX);
                        break;
                    }
                }
            }
        }
        Self { mesh, bvh, owner, ordinal, parts, stones: stones_of(design) }
    }

    /// Faces the fused mesh holds.
    pub fn faces(&self) -> usize {
        self.mesh.faces.len()
    }

    /// CAD parts the scene names.
    pub fn parts(&self) -> usize {
        self.parts.len()
    }

    /// Stones the scene names.
    pub fn stones(&self) -> usize {
        self.stones.as_ref().map_or(0, |s| s.paths.len())
    }

    /// The fused mesh the picks are measured on: vertices and faces only.
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// The tree over the fused mesh.
    pub fn bvh(&self) -> &Bvh {
        &self.bvh
    }

    /// What the feature id behind a fused face is, when a part owns it.
    pub fn feature_of_face(&self, face: usize) -> Option<Id> {
        let owner = *self.owner.get(face)?;
        (owner != BAND).then(|| self.parts[owner as usize].feature)
    }

    /// Everything visible within `aperture_px` of the ray, best first: vertex over edge over face
    /// over part over stone over band, then nearer the cursor, then nearer the eye.
    pub fn pick(&self, ray: Ray, view: &ViewScale, aperture_px: f32, filter: Filter) -> Vec<Pick> {
        let len = dot(ray.direction, ray.direction).sqrt();
        if !(len > 0.0) || !len.is_finite() || !ray.origin.iter().all(|v| v.is_finite()) {
            return Vec::new();
        }
        let dir = ray.direction.map(|v| v / len);
        let o = ray.origin;
        let mut out = Vec::new();
        // Metal always hides what is behind it; stones left out of the filter are not in the scene.
        let metal = self.bvh.ray(&self.mesh, o, dir);
        let stone = self.stones.as_ref().filter(|_| filter.stones).and_then(|s| s.bvh.ray(&s.mesh, o, dir).map(|(f, t)| (s, f, t)));
        match (metal, stone) {
            (Some((_, t)), Some((s, sf, st))) if st < t => self.stone_pick(s, sf, st, o, dir, &mut out),
            (None, Some((s, sf, st))) => self.stone_pick(s, sf, st, o, dir, &mut out),
            (Some((f, t)), _) => {
                let world: [f64; 3] = std::array::from_fn(|k| o[k] + dir[k] * t);
                let normal = self.mesh.face_normal(&self.mesh.faces[f]).unwrap_or([0.0, 0.0, 1.0]);
                let owner = self.owner[f];
                if owner == BAND {
                    if filter.band {
                        out.push(Pick { entity: Entity::Band, world, normal, depth: t, px: 0.0 });
                    }
                } else {
                    let feature = self.parts[owner as usize].feature;
                    let face = self.ordinal[f];
                    if filter.faces && face != u32::MAX {
                        out.push(Pick { entity: Entity::Face { feature, face }, world, normal, depth: t, px: 0.0 });
                    }
                    if filter.parts {
                        out.push(Pick { entity: Entity::Part { feature }, world, normal, depth: t, px: 0.0 });
                    }
                }
            }
            (None, None) => {}
        }
        // Part vertices and edges within reach that lie on the visible surface.
        if (filter.vertices || filter.edges) && view.px_per_mm > 0.0 && aperture_px >= 0.0 {
            for part in &self.parts {
                if filter.vertices {
                    for (vi, &v) in part.vertices.iter().enumerate() {
                        let (t, px) = screen(o, dir, view, v);
                        if t <= 0.0 || px > aperture_px as f64 {
                            continue;
                        }
                        if let Some(normal) = self.exposed(o, v, filter.stones) {
                            out.push(Pick { entity: Entity::Vertex { feature: part.feature, vertex: vi as u32 }, world: v, normal, depth: t, px: px as f32 });
                        }
                    }
                }
                if filter.edges {
                    for (ei, poly) in part.edges.iter().enumerate() {
                        let mut best: Option<Pick> = None;
                        for w in poly.windows(2) {
                            let q = nearest_on_segment(o, dir, w[0], w[1]);
                            let (t, px) = screen(o, dir, view, q);
                            if t <= 0.0 || px > aperture_px as f64 {
                                continue;
                            }
                            if best.as_ref().is_some_and(|b| (b.px as f64, b.depth) <= (px, t)) {
                                continue;
                            }
                            if let Some(normal) = self.exposed(o, q, filter.stones) {
                                best = Some(Pick { entity: Entity::Edge { feature: part.feature, edge: ei as u32 }, world: q, normal, depth: t, px: px as f32 });
                            }
                        }
                        out.extend(best);
                    }
                }
            }
        }
        out.sort_by(|a, b| (rank(&a.entity), a.px, a.depth).partial_cmp(&(rank(&b.entity), b.px, b.depth)).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    fn stone_pick(&self, s: &Stones, face: usize, t: f64, o: [f64; 3], dir: [f64; 3], out: &mut Vec<Pick>) {
        let path = s.paths[s.owner[face] as usize].clone();
        let world: [f64; 3] = std::array::from_fn(|k| o[k] + dir[k] * t);
        let normal = s.mesh.face_normal(&s.mesh.faces[face]).unwrap_or([0.0, 0.0, 1.0]);
        out.push(Pick { entity: Entity::Stone { path }, world, normal, depth: t, px: 0.0 });
    }

    /// The surface normal at `q` when `q` lies on the visible surface: within `ON_SURFACE_MM` of
    /// the metal and with nothing nearer than it along the ray from `o`, stones included when asked.
    fn exposed(&self, o: [f64; 3], q: [f64; 3], stones: bool) -> Option<[f64; 3]> {
        let (face, _) = self.bvh.nearest(&self.mesh, q, ON_SURFACE_MM)?;
        let normal = self.mesh.face_normal(&self.mesh.faces[face])?;
        let to = sub(q, o);
        let reach = dot(to, to).sqrt();
        if reach <= 0.0 {
            return None;
        }
        let limit = 1.0 - OCCLUSION_MM / reach;
        if self.bvh.ray(&self.mesh, o, to).is_some_and(|(_, t)| t < limit) {
            return None;
        }
        if stones && self.stones.as_ref().and_then(|s| s.bvh.ray(&s.mesh, o, to)).is_some_and(|(_, t)| t < limit) {
            return None;
        }
        Some(normal)
    }

    /// Entities inside the four planes (`a·x + b·y + c·z + d ≥ 0` is inside): wholly inside for a
    /// window, touched at all when `crossing`.
    pub fn box_select(&self, planes: [[f64; 4]; 4], crossing: bool, filter: Filter) -> Vec<Entity> {
        let inside = |p: [f64; 3]| planes.iter().all(|pl| pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] + pl[3] >= 0.0);
        let tri_in = |m: &Mesh, f: &[u32; 3]| m.triangle(f).is_some_and(|(a, b, c)| inside(a) && inside(b) && inside(c));
        let tri_touch = |m: &Mesh, f: &[u32; 3]| m.triangle(f).is_some_and(|(a, b, c)| triangle_touches(&planes, a, b, c));
        let mut out = Vec::new();
        for part in &self.parts {
            let feature = part.feature;
            if filter.vertices {
                out.extend(part.vertices.iter().enumerate().filter(|(_, v)| inside(**v)).map(|(i, _)| Entity::Vertex { feature, vertex: i as u32 }));
            }
            if filter.edges {
                for (i, poly) in part.edges.iter().enumerate() {
                    let hit = if crossing {
                        poly.windows(2).any(|w| segment_touches(&planes, w[0], w[1])) || (poly.len() == 1 && inside(poly[0]))
                    } else {
                        !poly.is_empty() && poly.iter().all(|p| inside(*p))
                    };
                    if hit {
                        out.push(Entity::Edge { feature, edge: i as u32 });
                    }
                }
            }
            if filter.faces {
                // Per ordinal: every triangle inside, or any triangle touching.
                let mut all_in = vec![true; part.faces as usize];
                let mut seen = vec![false; part.faces as usize];
                let mut any_touch = vec![false; part.faces as usize];
                for (ti, f) in part.mesh.faces.iter().enumerate() {
                    let Some(&ord) = part.tri_face.get(ti) else { continue };
                    if ord == u32::MAX || ord >= part.faces {
                        continue;
                    }
                    let k = ord as usize;
                    seen[k] = true;
                    if crossing {
                        if !any_touch[k] && tri_touch(&part.mesh, f) {
                            any_touch[k] = true;
                        }
                    } else if all_in[k] && !tri_in(&part.mesh, f) {
                        all_in[k] = false;
                    }
                }
                for k in 0..part.faces as usize {
                    if seen[k] && if crossing { any_touch[k] } else { all_in[k] } {
                        out.push(Entity::Face { feature, face: k as u32 });
                    }
                }
            }
            if filter.parts {
                let hit = if crossing {
                    part.mesh.faces.iter().any(|f| tri_touch(&part.mesh, f))
                } else {
                    !part.mesh.vertices.is_empty() && part.mesh.vertices.iter().all(|v| inside([v.0 as f64, v.1 as f64, v.2 as f64]))
                };
                if hit {
                    out.push(Entity::Part { feature });
                }
            }
        }
        if filter.stones {
            if let Some(s) = &self.stones {
                let n = s.paths.len();
                let mut all_in = vec![true; n];
                let mut seen = vec![false; n];
                let mut any_touch = vec![false; n];
                for (fi, f) in s.mesh.faces.iter().enumerate() {
                    let k = s.owner[fi] as usize;
                    seen[k] = true;
                    if crossing {
                        if !any_touch[k] && tri_touch(&s.mesh, f) {
                            any_touch[k] = true;
                        }
                    } else if all_in[k] && !tri_in(&s.mesh, f) {
                        all_in[k] = false;
                    }
                }
                for k in 0..n {
                    if seen[k] && if crossing { any_touch[k] } else { all_in[k] } {
                        out.push(Entity::Stone { path: s.paths[k].clone() });
                    }
                }
            }
        }
        if filter.band {
            let band = self.mesh.faces.iter().zip(&self.owner).filter(|(_, o)| **o == BAND).map(|(f, _)| f);
            let hit = if crossing {
                band.clone().any(|f| tri_touch(&self.mesh, f))
            } else {
                let mut any = false;
                let all = band.clone().all(|f| {
                    any = true;
                    tri_in(&self.mesh, f)
                });
                any && all
            };
            if hit {
                out.push(Entity::Band);
            }
        }
        out
    }
}

/// The stones' preview facets, each facet given to the stone whose girdle centre is nearest.
fn stones_of(design: &RingDesign) -> Option<Stones> {
    let frames = crate::stones::stone_frames(design);
    if frames.is_empty() {
        return None;
    }
    let soup = crate::gems::preview_mesh(design, &AlphaLibrary::default())?;
    let owner = soup
        .faces
        .iter()
        .map(|f| {
            let Some((a, b, c)) = soup.triangle(f) else { return 0 };
            let centroid: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
            (0..frames.len()).min_by(|&i, &j| dist2(frames[i].1.girdle, centroid).total_cmp(&dist2(frames[j].1.girdle, centroid))).unwrap_or(0) as u32
        })
        .collect();
    let mesh = Mesh { vertices: soup.vertices, faces: soup.faces, ..Default::default() };
    let bvh = Bvh::build(&mesh);
    Some(Stones { paths: frames.into_iter().map(|(st, _)| st.path).collect(), mesh, bvh, owner })
}

/// Depth of `q` along the ray and its offset from the ray on the screen, in pixels: the
/// perpendicular from the ray, projected onto the view's image plane when the view looks along
/// the ray (within 60°, which any perspective ray does), else the perpendicular whole.
fn screen(o: [f64; 3], dir: [f64; 3], view: &ViewScale, q: [f64; 3]) -> (f64, f64) {
    let d = sub(q, o);
    let t = dot(d, dir);
    let mut perp: [f64; 3] = std::array::from_fn(|k| d[k] - dir[k] * t);
    let axis = cross(view.right, view.up);
    let len2 = dot(axis, axis);
    if len2 > 0.0 && dot(axis, dir).abs() > 0.5 * len2.sqrt() {
        let along = dot(perp, axis) / len2;
        perp = std::array::from_fn(|k| perp[k] - axis[k] * along);
    }
    (t, dot(perp, perp).sqrt() * view.px_per_mm)
}

/// The point of segment `ab` nearest the ray's line, the nearer end when they are parallel.
fn nearest_on_segment(o: [f64; 3], dir: [f64; 3], a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let e = sub(b, a);
    let w = sub(a, o);
    let (ee, ed, wd, we) = (dot(e, e), dot(e, dir), dot(w, dir), dot(w, e));
    let denom = ee - ed * ed;
    if ee <= 0.0 {
        return a;
    }
    if denom <= 1e-12 * ee {
        return if dot(sub(b, o), dir) < wd { b } else { a };
    }
    // Closest points of two lines: s along the segment, clamped to it.
    let s = ((ed * wd - we) / denom).clamp(0.0, 1.0);
    std::array::from_fn(|k| a[k] + e[k] * s)
}

/// Whether triangle `abc` has any point inside all four planes.
fn triangle_touches(planes: &[[f64; 4]; 4], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> bool {
    let mut poly = vec![a, b, c];
    for pl in planes {
        let side = |p: [f64; 3]| pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] + pl[3];
        let mut next = Vec::with_capacity(poly.len() + 1);
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                next.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                next.push(std::array::from_fn(|k| p[k] + (q[k] - p[k]) * t));
            }
        }
        if next.is_empty() {
            return false;
        }
        poly = next;
    }
    true
}

/// Whether segment `ab` has any point inside all four planes.
fn segment_touches(planes: &[[f64; 4]; 4], a: [f64; 3], b: [f64; 3]) -> bool {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for pl in planes {
        let sa = pl[0] * a[0] + pl[1] * a[1] + pl[2] * a[2] + pl[3];
        let sb = pl[0] * b[0] + pl[1] * b[1] + pl[2] * b[2] + pl[3];
        if sa < 0.0 && sb < 0.0 {
            return false;
        }
        if sa >= 0.0 && sb >= 0.0 {
            continue;
        }
        let t = sa / (sa - sb);
        if sa < 0.0 {
            t0 = t0.max(t);
        } else {
            t1 = t1.min(t);
        }
        if t0 > t1 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildParams,
        cad::{Attach, Component, Document, Feature, Operation, Placement, Stage},
        interaction::picking,
        templates,
    };

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
    }
    fn template(name: &str) -> RingDesign {
        templates::all().iter().find(|t| t.name == name).unwrap().design()
    }
    /// A joined part at the top of the Court band, its centre `height` above the surface: a part
    /// sunk into the band buries its lower features, the ones a flat foot on a dome would float.
    fn court_with(id: Id, name: &str, operation: Operation, attach: Attach, height: f64) -> RingDesign {
        let mut d = template("Court band");
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id,
            name: name.into(),
            enabled: true,
            operation,
            component: Component { attach, stage: Stage::Cast, placement: Placement::ring(90.0, height), ..Default::default() },
        })
        .unwrap();
        d.cad = Some(doc);
        d
    }
    fn top_view() -> ViewScale {
        ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 }
    }
    fn down_at(x: f64, z: f64) -> Ray {
        Ray { origin: [x, 40.0, z], direction: [0.0, -1.0, 0.0] }
    }
    fn normalize(v: [f64; 3]) -> [f64; 3] {
        let l = dot(v, v).sqrt();
        v.map(|x| x / l)
    }
    /// Per face ordinal, the mean radial height of its triangles at the top of the ring.
    fn ordinal_heights(c: &crate::cad::EvaluatedComponent) -> Vec<(u32, f64)> {
        let n = c.trace.face_kind.len();
        let (mut sum, mut count) = (vec![0.0; n], vec![0usize; n]);
        for (ti, f) in c.mesh.faces.iter().enumerate() {
            let Some(ord) = c.trace.face_of(ti) else { continue };
            let (a, b, cc) = c.mesh.triangle(f).unwrap();
            sum[ord as usize] += (a[1] + b[1] + cc[1]) / 3.0;
            count[ord as usize] += 1;
        }
        (0..n as u32).map(|k| (k, sum[k as usize] / count[k as usize].max(1) as f64)).collect()
    }

    #[test]
    fn the_scene_ray_matches_the_brute_force_on_a_ring_with_a_joined_cylinder() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert_eq!(built.parts.joined, 1);
        let scene = PickScene::build(&built, &d);
        assert_eq!(scene.faces(), built.mesh.faces.len());
        assert_eq!((scene.parts(), scene.stones()), (1, 0));
        let m = scene.mesh();
        let mut hits = 0;
        for (o, dir) in bvh::tests::random_rays(m, 1000, 3) {
            let a = scene.bvh().ray(m, o, dir);
            let b = bvh::tests::brute_ray(m, o, dir);
            match (a, b) {
                (Some((fa, ta)), Some((fb, tb))) => {
                    assert!(fa == fb || (ta - tb).abs() < 1e-9, "face {fa} at {ta} vs {fb} at {tb}");
                    assert!((ta - tb).abs() < 1e-9);
                    hits += 1;
                }
                (None, None) => {}
                (a, b) => panic!("{a:?} vs {b:?}"),
            }
        }
        assert!(hits > 500, "{hits} of 1000");
        // The same face `picking::raycast` names, at the same point.
        let (o, dir) = ([0.0, 40.0, 0.5], [0.0, -1.0, 0.0]);
        let (face, t) = scene.bvh().ray(m, o, dir).unwrap();
        let (bf, bp) = picking::raycast(&built.mesh, o.map(|v| v as f32), dir.map(|v| v as f32)).unwrap();
        assert_eq!(face, bf);
        assert!((bp[1] as f64 - (o[1] + dir[1] * t)).abs() < 1e-4);
    }

    #[test]
    fn a_pick_on_the_cylinder_top_names_its_face_with_a_radial_normal() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        let c = &built.parts.evaluated.as_ref().unwrap().components[0];
        let (top, top_y) = ordinal_heights(c).into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        assert!(c.trace.face_kind[top as usize] == crate::cad::SurfaceKind::Plane);
        let picks = scene.pick(down_at(0.0, 0.0), &top_view(), 8.0, Filter::default());
        let names: Vec<&Entity> = picks.iter().map(|p| &p.entity).collect();
        assert_eq!(names, vec![&Entity::Face { feature: 1, face: top }, &Entity::Part { feature: 1 }], "{picks:?}");
        let face = &picks[0];
        assert!(dot(face.normal, [0.0, 1.0, 0.0]) > 0.999, "{:?}", face.normal);
        // The top stands 1.5 mm over the crest at 10.65; the ray is along -y from y = 40.
        assert!((face.world[1] - 12.15).abs() < 0.05 && (top_y - 12.15).abs() < 0.05, "{face:?} top {top_y}");
        assert!(face.px == 0.0 && (face.depth - (40.0 - face.world[1])).abs() < 1e-9, "{face:?}");
        assert_eq!(scene.feature_of_face(scene.bvh().ray(scene.mesh(), [0.0, 40.0, 0.0], [0.0, -1.0, 0.0]).unwrap().0), Some(1));
        // The wall: a ray from the side at the cylinder's mid-height names the cylinder ordinal.
        let wall = ordinal_heights(c).into_iter().find(|(k, _)| c.trace.face_kind[*k as usize] == crate::cad::SurfaceKind::Cylinder).unwrap().0;
        let mid = top_y - 0.6;
        let side = ViewScale { right: [0.0, 1.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
        let picks = scene.pick(Ray { origin: [40.0, mid, 0.0], direction: [-1.0, 0.0, 0.0] }, &side, 0.0, Filter { faces: true, ..Filter::none() });
        assert_eq!(picks[0].entity, Entity::Face { feature: 1, face: wall }, "{picks:?}");
        assert!(dot(picks[0].normal, [1.0, 0.0, 0.0]) > 0.99, "{:?}", picks[0].normal);
        assert!((picks[0].world[0] - 1.5).abs() < 0.05, "{:?}", picks[0].world);
        // Filtered to the band, the cylinder hides it and nothing answers.
        assert!(scene.pick(down_at(0.0, 0.0), &top_view(), 8.0, Filter { band: true, ..Filter::none() }).is_empty());
    }

    #[test]
    fn a_pick_on_the_band_names_it_and_hit_recovers_theta_and_v() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        let ray = Ray { origin: [-40.0, 0.0, 0.0], direction: [1.0, 0.0, 0.0] };
        let side = ViewScale { right: [0.0, 1.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
        let picks = scene.pick(ray, &side, 8.0, Filter::default());
        assert_eq!(picks.len(), 1, "{picks:?}");
        assert_eq!(picks[0].entity, Entity::Band);
        assert!(dot(picks[0].normal, [-1.0, 0.0, 0.0]) > 0.99, "{:?}", picks[0].normal);
        let hit = picking::hit(&d, &lib, &built.mesh, [-40.0, 0.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
        assert!((hit.theta_deg - 180.0).abs() < 1e-3, "{}", hit.theta_deg);
        assert!((hit.v_mm - d.field_context().crest_v_mm).abs() < 0.1, "{} vs {}", hit.v_mm, d.field_context().crest_v_mm);
        for k in 0..3 {
            assert!((hit.world[k] as f64 - picks[0].world[k]).abs() < 1e-4, "{:?} vs {:?}", hit.world, picks[0].world);
        }
        assert!((picks[0].depth - (40.0 + hit.world[0] as f64)).abs() < 1e-4);
        // A ray down the finger hole answers nothing under either view; a zero direction too.
        let finger = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 10.0 };
        assert!(scene.pick(Ray { origin: [0.0, 0.0, 40.0], direction: [0.0, 0.0, -1.0] }, &finger, 8.0, Filter::default()).is_empty());
        assert!(scene.pick(Ray { origin: [0.0, 0.0, 40.0], direction: [0.0, 0.0, -1.0] }, &top_view(), 8.0, Filter::default()).is_empty());
        assert!(scene.pick(Ray { origin: [0.0, 0.0, 40.0], direction: [0.0; 3] }, &top_view(), 8.0, Filter::default()).is_empty());
    }

    #[test]
    fn an_edge_seen_through_the_band_from_behind_is_rejected() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        let c = &built.parts.evaluated.as_ref().unwrap().components[0];
        // The top rim: the edge whose points stand highest.
        let (rim, poly) = c.edges.iter().enumerate().max_by(|a, b| mean_y(a.1).total_cmp(&mean_y(b.1))).unwrap();
        let q = poly[poly.len() / 3];
        let edges = Filter { edges: true, ..Filter::none() };
        // From behind: the ray crosses the far side of the ring before it reaches the rim.
        let behind = [0.0, -40.0, 0.0];
        let picks = scene.pick(Ray { origin: behind, direction: sub(q, behind) }, &top_view(), 8.0, edges);
        assert!(picks.is_empty(), "{picks:?}");
        // From above: the same point names the rim.
        let above = [q[0], q[1] + 10.0, q[2]];
        let picks = scene.pick(Ray { origin: above, direction: [0.0, -1.0, 0.0] }, &top_view(), 8.0, edges);
        assert_eq!(picks.first().map(|p| &p.entity), Some(&Entity::Edge { feature: 1, edge: rim as u32 }), "{picks:?}");
        assert!(picks[0].px < 1e-6 && (picks[0].depth - 10.0).abs() < 0.05, "{:?}", picks[0]);
        assert!(dot(picks[0].normal, picks[0].normal) > 0.999);
        // The bottom rim is a millimetre under the band's surface and never answers, from anywhere.
        let (floor, low) = c.edges.iter().enumerate().min_by(|a, b| mean_y(a.1).total_cmp(&mean_y(b.1))).unwrap();
        assert!(mean_y(low) < mean_y(poly) - 2.0);
        for p in [low[0], low[low.len() / 4], low[low.len() / 2]] {
            let picks = scene.pick(Ray { origin: [p[0], p[1] + 10.0, p[2]], direction: [0.0, -1.0, 0.0] }, &top_view(), 40.0, edges);
            assert!(!picks.iter().any(|k| k.entity == Entity::Edge { feature: 1, edge: floor as u32 }), "{picks:?}");
            assert!(picks.iter().any(|k| k.entity == Entity::Edge { feature: 1, edge: rim as u32 }), "{picks:?}");
        }
    }
    fn mean_y(poly: &[[f64; 3]]) -> f64 {
        poly.iter().map(|p| p[1]).sum::<f64>() / poly.len().max(1) as f64
    }

    #[test]
    fn a_vertex_within_8_px_beats_the_face_under_it() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(2, "boss", Operation::Box { size: [2.0, 2.0, 2.0] }, Attach::Join, 0.0);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert_eq!(built.parts.joined, 1, "{:?}", built.parts.notes);
        let scene = PickScene::build(&built, &d);
        let c = &built.parts.evaluated.as_ref().unwrap().components[0];
        let top_y = c.trace.vertices.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
        let top: Vec<(usize, [f64; 3])> = c.trace.vertices.iter().copied().enumerate().filter(|(_, v)| v[1] > top_y - 0.5).collect();
        assert_eq!(top.len(), 4, "{:?}", c.trace.vertices);
        let centre: [f64; 3] = std::array::from_fn(|k| top.iter().map(|(_, v)| v[k]).sum::<f64>() / 4.0);
        let (ci, corner) = top[0];
        let inward = normalize(sub(centre, corner));
        let (top_face, _) = ordinal_heights(c).into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        // 6 px from the corner: the vertex, then its edges, then the face and the part.
        let at = |mm: f64| -> [f64; 3] { std::array::from_fn(|k| corner[k] + inward[k] * mm) };
        let near = at(0.6);
        let picks = scene.pick(down_at(near[0], near[2]), &top_view(), 8.0, Filter::default());
        let classes: Vec<u8> = picks.iter().map(|p| rank(&p.entity)).collect();
        assert_eq!(picks[0].entity, Entity::Vertex { feature: 2, vertex: ci as u32 }, "{picks:?}");
        assert!((picks[0].px - 6.0).abs() < 0.05 && dist2(picks[0].world, corner) < 1e-12, "{:?}", picks[0]);
        assert!(classes.windows(2).all(|w| w[0] <= w[1]), "{classes:?}");
        assert!(picks.iter().filter(|p| matches!(p.entity, Entity::Edge { .. })).count() >= 2, "{picks:?}");
        assert!(picks.iter().any(|p| p.entity == Entity::Face { feature: 2, face: top_face }), "{picks:?}");
        assert!(picks.iter().any(|p| p.entity == Entity::Part { feature: 2 }));
        // 12 px away the vertex is out of reach and the face is first.
        let far = at(1.2);
        let picks = scene.pick(down_at(far[0], far[2]), &top_view(), 8.0, Filter::default());
        assert_eq!(picks[0].entity, Entity::Face { feature: 2, face: top_face }, "{picks:?}");
        assert!(!picks.iter().any(|p| matches!(p.entity, Entity::Vertex { .. })));
        // The same spot with a wider aperture finds it again, ranked first.
        let picks = scene.pick(down_at(far[0], far[2]), &top_view(), 14.0, Filter::default());
        assert_eq!(picks[0].entity, Entity::Vertex { feature: 2, vertex: ci as u32 });
        assert!((picks[0].px - 12.0).abs() < 0.05);
        // The bottom corners are buried and never answer.
        let low: Vec<usize> = c.trace.vertices.iter().enumerate().filter(|(_, v)| v[1] < top_y - 1.0).map(|(i, _)| i).collect();
        assert_eq!(low.len(), 4);
        let picks = scene.pick(down_at(near[0], near[2]), &top_view(), 40.0, Filter { vertices: true, ..Filter::none() });
        assert!(picks.iter().all(|p| !low.contains(&match p.entity { Entity::Vertex { vertex, .. } => vertex as usize, _ => usize::MAX })), "{picks:?}");
        assert_eq!(picks.len(), 4, "{picks:?}");
    }

    #[test]
    fn box_select_window_takes_the_whole_cylinder_and_crossing_takes_the_band_too() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        let c = &built.parts.evaluated.as_ref().unwrap().components[0];
        let (faces, edges, vertices) = (c.trace.face_kind.len() as u32, c.edges.len() as u32, c.trace.vertices.len() as u32);
        assert!(faces >= 3 && edges >= 2, "faces {faces} edges {edges} vertices {vertices}");
        // Looking down on the top of the ring: x and z bound the window, y is free.
        let window = |x0: f64, x1: f64, z0: f64, z1: f64| [[1.0, 0.0, 0.0, -x0], [-1.0, 0.0, 0.0, x1], [0.0, 0.0, 1.0, -z0], [0.0, 0.0, -1.0, z1]];
        let whole = window(-3.5, 3.5, -3.5, 3.5);
        let got = scene.box_select(whole, false, Filter::default());
        let mut want: Vec<Entity> = (0..vertices).map(|k| Entity::Vertex { feature: 1, vertex: k }).collect();
        want.extend((0..edges).map(|k| Entity::Edge { feature: 1, edge: k }));
        want.extend((0..faces).map(|k| Entity::Face { feature: 1, face: k }));
        want.push(Entity::Part { feature: 1 });
        assert_eq!(got, want);
        let crossing = scene.box_select(whole, true, Filter::default());
        want.push(Entity::Band);
        assert_eq!(crossing, want);
        // A filter keeps to its classes.
        let only = scene.box_select(whole, true, Filter { faces: true, ..Filter::none() });
        assert_eq!(only, (0..faces).map(|k| Entity::Face { feature: 1, face: k }).collect::<Vec<_>>());
        // Half the cylinder: nothing round is wholly inside, everything round is touched.
        let half = window(0.0, 3.5, -3.5, 3.5);
        let got = scene.box_select(half, false, Filter { faces: true, edges: true, parts: true, ..Filter::none() });
        assert!(!got.iter().any(|e| matches!(e, Entity::Part { .. })), "{got:?}");
        let (top, _) = ordinal_heights(c).into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        assert!(!got.contains(&Entity::Face { feature: 1, face: top }), "{got:?}");
        let got = scene.box_select(half, true, Filter { faces: true, edges: true, parts: true, ..Filter::none() });
        assert!(got.contains(&Entity::Face { feature: 1, face: top }) && got.contains(&Entity::Part { feature: 1 }), "{got:?}");
        assert_eq!(got.iter().filter(|e| matches!(e, Entity::Face { .. })).count(), faces as usize);
        // A window beside the ring takes nothing; one round the whole ring takes the band as well.
        assert!(scene.box_select(window(30.0, 40.0, -1.0, 1.0), true, Filter::default()).is_empty());
        let all = scene.box_select(window(-30.0, 30.0, -30.0, 30.0), false, Filter::default());
        assert!(all.contains(&Entity::Band) && all.contains(&Entity::Part { feature: 1 }));
    }

    #[test]
    fn stones_are_picked_by_their_path_and_hide_the_band_under_them() {
        let lib = AlphaLibrary::builtin();
        let d = template("Cathedral solitaire stock");
        let stones = crate::setstone::set_stones(&d);
        assert!(!stones.is_empty());
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        assert_eq!(scene.stones(), stones.len());
        let frame = &crate::stones::stone_frames(&d)[0].1;
        let above: [f64; 3] = std::array::from_fn(|k| frame.girdle[k] + frame.normal[k] * 10.0);
        let ray = Ray { origin: above, direction: frame.normal.map(|v| -v) };
        let picks = scene.pick(ray, &top_view(), 8.0, Filter::default());
        assert_eq!(picks.first().map(|p| &p.entity), Some(&Entity::Stone { path: stones[0].path.clone() }), "{picks:?}");
        assert!(picks[0].depth < 10.0 && picks[0].depth > 10.0 - stones[0].gem.crown_mm() - 0.01, "{:?}", picks[0]);
        assert!(!picks.iter().any(|p| p.entity == Entity::Band));
        // Without stones the same ray reaches the seat under it.
        let picks = scene.pick(ray, &top_view(), 8.0, Filter { stones: false, ..Filter::default() });
        assert_eq!(picks.first().map(|p| &p.entity), Some(&Entity::Band), "{picks:?}");
        // Box selection round the stone names it in both modes.
        let r = frame.reach + 1.0;
        let g = frame.girdle;
        let planes = [[1.0, 0.0, 0.0, r - g[0]], [-1.0, 0.0, 0.0, r + g[0]], [0.0, 0.0, 1.0, r - g[2]], [0.0, 0.0, -1.0, r + g[2]]];
        let got = scene.box_select(planes, false, Filter { stones: true, ..Filter::none() });
        assert_eq!(got, vec![Entity::Stone { path: stones[0].path.clone() }]);
    }

    /// `cargo test --release -p ringdesign-core -- --ignored pick_1000` for the number.
    #[test]
    #[ignore]
    fn a_thousand_picks_on_an_export_build_cost_under_a_millisecond_each() {
        let lib = AlphaLibrary::builtin();
        let d = court_with(1, "bezel", Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, Attach::Join, 0.25);
        let built = crate::mesh::try_build(&d, &lib, BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() }).unwrap();
        let started = std::time::Instant::now();
        let scene = PickScene::build(&built, &d);
        let build_ms = started.elapsed().as_secs_f64() * 1e3;
        let rays = bvh::tests::random_rays(scene.mesh(), 1000, 5);
        let view = top_view();
        let started = std::time::Instant::now();
        let mut hits = 0;
        for (o, dir) in &rays {
            hits += usize::from(!scene.pick(Ray { origin: *o, direction: *dir }, &view, 8.0, Filter::default()).is_empty());
        }
        let per = started.elapsed().as_secs_f64() * 1e3 / rays.len() as f64;
        println!("{} faces: scene {build_ms:.1} ms, {per:.4} ms per pick, {hits} of 1000 hit", scene.faces());
        assert!(scene.faces() > 750_000, "{}", scene.faces());
        assert!(per < 1.0, "{per:.3} ms per pick");
    }

    #[test]
    fn a_ring_of_parts_only_names_each_part_and_never_the_band() {
        let lib = AlphaLibrary::builtin();
        let cylinder = || Operation::Cylinder { radius_mm: 1.5, height_mm: 3.0 };
        let mut doc = Document::default();
        for (id, name, operation) in [
            (1, "left", cylinder()),
            (2, "right stock", cylinder()),
            (3, "right", Operation::Transform { source: 2, translation: [5.0, 0.0, 0.0], rotation_deg: [0.0; 3] }),
        ] {
            doc.append(Feature { id, name: name.into(), enabled: true, operation, component: Component::default() }).unwrap();
        }
        let mut d = RingDesign::default();
        d.cad = Some(doc);
        assert!(!d.band_is_procedural());
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert_eq!((built.parts.first, built.parts.features.clone(), built.parts.separate), (0, vec![1, 3], 2));
        assert_eq!(built.mesh.origin.len(), built.mesh.vertices.len());
        let scene = PickScene::build(&built, &d);
        assert_eq!((scene.parts(), scene.stones()), (2, 0));
        assert!((0..scene.faces()).all(|f| scene.feature_of_face(f).is_some()), "every fused face is a part's");
        let down = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 10.0 };
        let e = built.parts.evaluated.as_ref().unwrap();
        for (x, feature) in [(0.0, 1), (5.0, 3)] {
            let c = e.components.iter().find(|c| c.id == feature).unwrap();
            let (top, _) = ordinal_heights_along(c, 2).into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
            let picks = scene.pick(Ray { origin: [x, 0.0, 40.0], direction: [0.0, 0.0, -1.0] }, &down, 0.0, Filter::default());
            let names: Vec<&Entity> = picks.iter().map(|p| &p.entity).collect();
            assert_eq!(names, vec![&Entity::Face { feature, face: top }, &Entity::Part { feature }], "{picks:?}");
            assert!((picks[0].world[2] - 1.5).abs() < 1e-3 && dot(picks[0].normal, [0.0, 0.0, 1.0]) > 0.999, "{:?}", picks[0]);
        }
        // Between them the ray meets nothing; box selection round both never says Band.
        assert!(scene.pick(Ray { origin: [2.5, 0.0, 40.0], direction: [0.0, 0.0, -1.0] }, &down, 0.0, Filter::default()).is_empty());
        let planes = [[1.0, 0.0, 0.0, 3.0], [-1.0, 0.0, 0.0, 8.0], [0.0, 1.0, 0.0, 3.0], [0.0, -1.0, 0.0, 3.0]];
        let all = scene.box_select(planes, true, Filter::default());
        assert!(!all.contains(&Entity::Band) && all.contains(&Entity::Part { feature: 1 }) && all.contains(&Entity::Part { feature: 3 }), "{all:?}");
    }

    /// Per face ordinal, the mean of its triangles' coordinate `axis`.
    fn ordinal_heights_along(c: &crate::cad::EvaluatedComponent, axis: usize) -> Vec<(u32, f64)> {
        let n = c.trace.face_kind.len();
        let (mut sum, mut count) = (vec![0.0; n], vec![0usize; n]);
        for (ti, f) in c.mesh.faces.iter().enumerate() {
            let Some(ord) = c.trace.face_of(ti) else { continue };
            let (a, b, cc) = c.mesh.triangle(f).unwrap();
            sum[ord as usize] += (a[axis] + b[axis] + cc[axis]) / 3.0;
            count[ord as usize] += 1;
        }
        (0..n as u32).map(|k| (k, sum[k as usize] / count[k as usize].max(1) as f64)).collect()
    }
}
