//! The ring frame under the pointer: the band parts seat on, a pointer's (θ, across, height), and a ghost's matrix.
use super::commands::Primitive;
use super::session::{Outcome, Preview, Session, StepInput};
use super::snap::{RingPoint, SnapGeometry, SnapHit, Snapper};
use std::sync::Arc;
use ringdesign_core::{
    AlphaLibrary, BuildParams, Mesh, RingDesign, Vec3,
    cad::{self, Component, Document, Feature, Operation, Placement},
    interaction::{
        bvh::Bvh,
        pick::{Entity, Pick, Ray, ViewScale},
    },
    sketch::Id,
};

/// The offsets along the finger `cad::surface_hit` tries its radial ray at, in its order.
const DZ: [f64; 5] = [1e-4, -1e-4, 0.0, 1e-3, -1e-3];

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn at(v: Vec3) -> [f64; 3] {
    [v.0 as f64, v.1 as f64, v.2 as f64]
}

/// The band built without its parts, with a tree and each vertex's faces, for `surface_hit` on a small patch.
pub struct BandSurface {
    mesh: Arc<Mesh>,
    bvh: Bvh,
    /// Past every vertex's radius, where `surface_hit` starts its rays.
    far: f64,
    /// The mesh's bounding corners, which give a patch the same `far` as the whole.
    corners: [Vec3; 2],
    /// Faces around each vertex: `incident[first[v]..first[v + 1]]`.
    first: Vec<u32>,
    incident: Vec<u32>,
}

impl BandSurface {
    pub fn new(mesh: Mesh) -> Self {
        Self::shared(Arc::new(mesh))
    }

    /// The surface over a band the build already holds, without copying it.
    pub fn shared(mesh: Arc<Mesh>) -> Self {
        let (lo, hi) = mesh.bounds().unwrap_or_default();
        let far = (lo.0.abs().max(hi.0.abs()) as f64).hypot(lo.1.abs().max(hi.1.abs()) as f64) + 1.0;
        let mut first = vec![0u32; mesh.vertices.len() + 1];
        for f in &mesh.faces {
            for &v in f {
                if let Some(c) = first.get_mut(v as usize + 1) {
                    *c += 1;
                }
            }
        }
        for i in 1..first.len() {
            first[i] += first[i - 1];
        }
        let mut fill = first.clone();
        let mut incident = vec![0u32; first.last().copied().unwrap_or(0) as usize];
        for (i, f) in mesh.faces.iter().enumerate() {
            for &v in f {
                if let Some(slot) = fill.get_mut(v as usize) {
                    incident[*slot as usize] = i as u32;
                    *slot += 1;
                }
            }
        }
        let bvh = Bvh::build(&mesh);
        Self { mesh, bvh, far, corners: [lo, hi], first, incident }
    }

    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }

    /// The band this surface reads, as the build shares it.
    pub fn shared_mesh(&self) -> &Arc<Mesh> {
        &self.mesh
    }

    /// Where a ring point stands in the world: its seat on the band, out along the normal by its height.
    pub fn world(&self, p: RingPoint) -> Option<[f64; 3]> {
        let (hit, n) = self.hit(p.theta_deg, p.across_mm)?;
        Some(std::array::from_fn(|k| hit[k] + n[k] * p.height_mm))
    }

    /// The faces around the first hits of `surface_hit`'s rays at (θ, across), in mesh order, within the mesh's bounds.
    fn patch(&self, theta_deg: f64, across_mm: f64) -> Mesh {
        let (sin, cos) = theta_deg.to_radians().sin_cos();
        // Rounded through f32 as `surface_hit` rounds them.
        let direction = [f64::from((-cos) as f32), f64::from((-sin) as f32), 0.0];
        let mut faces: Vec<u32> = Vec::new();
        for dz in DZ {
            let origin = [f64::from((self.far * cos) as f32), f64::from((self.far * sin) as f32), f64::from((across_mm + dz) as f32)];
            if let Some((f, _)) = self.bvh.ray(&self.mesh, origin, direction) {
                for &v in &self.mesh.faces[f] {
                    let (a, b) = (self.first[v as usize] as usize, self.first[v as usize + 1] as usize);
                    faces.extend_from_slice(&self.incident[a..b]);
                }
            }
        }
        faces.sort_unstable();
        faces.dedup();
        let mut out = Mesh::default();
        let mut index: Vec<(u32, u32)> = Vec::new();
        let smooth = self.mesh.normals.len() == self.mesh.vertices.len();
        if !faces.is_empty() {
            out.vertices.extend(self.corners);
            if smooth {
                out.normals.extend([Vec3(0.0, 0.0, 1.0); 2]);
            }
        }
        for f in faces {
            let face = self.mesh.faces[f as usize];
            let mut local = [0u32; 3];
            for (k, &v) in face.iter().enumerate() {
                local[k] = match index.iter().find(|(from, _)| *from == v) {
                    Some((_, to)) => *to,
                    None => {
                        let to = out.vertices.len() as u32;
                        out.vertices.push(self.mesh.vertices[v as usize]);
                        if smooth {
                            out.normals.push(self.mesh.normals[v as usize]);
                        }
                        index.push((v, to));
                        to
                    }
                };
            }
            out.faces.push(local);
        }
        out
    }

    /// Where a radial ray at (θ, across) meets the band and its outward normal, as `cad::surface_hit` finds it.
    pub fn hit(&self, theta_deg: f64, across_mm: f64) -> Option<([f64; 3], [f64; 3])> {
        cad::surface_hit(&self.patch(theta_deg, across_mm), theta_deg, across_mm)
    }

    /// A placement's frame seated on the band, as `Placement::frame_on` seats it in the build.
    pub fn frame(&self, placement: &Placement, design: &RingDesign) -> Option<Affine> {
        let f = match placement {
            Placement::Ring { theta_deg, across_mm, .. } => placement.frame_on(design, Some(&self.patch(*theta_deg, *across_mm))),
            Placement::Free => placement.frame(design),
        }
        .ok()?;
        Some(Affine::frame(f.x_axis, f.y_axis, f.z_axis, f.origin))
    }

    /// The first band point along a ray, with the smooth normal there turned toward the ray's origin.
    pub fn ray(&self, ray: Ray) -> Option<([f64; 3], [f64; 3])> {
        let (face, t) = self.bvh.ray(&self.mesh, ray.origin, ray.direction)?;
        let point: [f64; 3] = std::array::from_fn(|k| ray.origin[k] + ray.direction[k] * t);
        let f = self.mesh.faces[face];
        let facet = self.mesh.face_normal(&f)?;
        let n = if self.mesh.normals.len() == self.mesh.vertices.len() {
            let sum = f.iter().fold([0.0; 3], |acc, &v| {
                let n = at(self.mesh.normals[v as usize]);
                [acc[0] + n[0], acc[1] + n[1], acc[2] + n[2]]
            });
            let len = dot(sum, sum).sqrt();
            if len > 1e-9 { sum.map(|v| v / len) } else { facet }
        } else {
            facet
        };
        let n = if dot(n, ray.direction) > 0.0 { n.map(|v| -v) } else { n };
        Some((point, n))
    }
}

/// A world point as (θ, across, height): height off the band along its normal, or past the reference crest without one.
pub fn ring_point(world: [f64; 3], surface: Option<&BandSurface>, nominal_r: f64) -> RingPoint {
    let mut p = RingPoint::of_world(world, 0.0);
    p.height_mm = match surface {
        Some(s) => s.hit(p.theta_deg, p.across_mm).map_or(0.0, |(hit, n)| dot(sub(world, hit), n)),
        None => world[0].hypot(world[1]) - nominal_r,
    };
    p
}

/// An angle in [-180, 180).
fn wrap180(deg: f64) -> f64 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}

/// Feeds `raw`, snaps where the live command lands its part on the ring, and feeds the pointer shifted by that snap.
pub fn land(session: &mut Session, raw: StepInput, snap: &dyn Fn(RingPoint) -> Option<SnapHit>) -> Outcome {
    let out = session.feed(raw.clone());
    if !matches!(out, Outcome::Continue) {
        return out;
    }
    let StepInput::Pointer { world, normal, theta_deg, across_mm, height_mm, dragging, .. } = raw else { return out };
    let Some(Placement::Ring { theta_deg: t, across_mm: a, height_mm: h, .. }) = session.preview().and_then(|p| p.placement) else { return out };
    let landing = RingPoint { theta_deg: t, across_mm: a, height_mm: h };
    let Some(hit) = snap(landing) else { return out };
    // A command that seats where it points takes the snapped point; one carrying a part keeps the pointer's.
    let pointed = wrap180(theta_deg - t).abs() < 1e-9 && (across_mm - a).abs() < 1e-9;
    let (theta_deg, across_mm) = if pointed {
        (hit.ring.theta_deg, hit.ring.across_mm)
    } else {
        (theta_deg + wrap180(hit.ring.theta_deg - t), across_mm + (hit.ring.across_mm - a))
    };
    session.feed(StepInput::Pointer {
        world: if pointed { hit.world } else { world },
        normal,
        theta_deg,
        across_mm,
        height_mm: height_mm + (hit.ring.height_mm - h),
        snapped: Some(hit),
        dragging,
    })
}

/// An affine map `p' = L p + t` as three rows of `[L | t]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine(pub [[f64; 4]; 3]);

impl Affine {
    pub const IDENTITY: Self = Self([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]);

    /// The map whose columns are a frame's axes and whose translation is its origin.
    pub fn frame(x: [f64; 3], y: [f64; 3], z: [f64; 3], origin: [f64; 3]) -> Self {
        Self(std::array::from_fn(|r| [x[r], y[r], z[r], origin[r]]))
    }

    pub fn scale(s: [f64; 3]) -> Self {
        Self([[s[0], 0.0, 0.0, 0.0], [0.0, s[1], 0.0, 0.0], [0.0, 0.0, s[2], 0.0]])
    }

    /// `self` after `rhs`: a point goes through `rhs` first.
    pub fn mul(&self, rhs: &Affine) -> Affine {
        let (a, b) = (&self.0, &rhs.0);
        Self(std::array::from_fn(|r| {
            let mut row = [0.0; 4];
            for c in 0..4 {
                row[c] = (0..3).map(|k| a[r][k] * b[k][c]).sum::<f64>() + if c == 3 { a[r][3] } else { 0.0 };
            }
            row
        }))
    }

    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|r| self.0[r][0] * p[0] + self.0[r][1] * p[1] + self.0[r][2] * p[2] + self.0[r][3])
    }

    /// A direction through the linear part alone.
    pub fn turn(&self, v: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|r| self.0[r][0] * v[0] + self.0[r][1] * v[1] + self.0[r][2] * v[2])
    }

    /// Where the map takes the origin.
    pub fn origin(&self) -> [f64; 3] {
        [self.0[0][3], self.0[1][3], self.0[2][3]]
    }

    /// Column `i` of the linear part: where the map sends that unit axis.
    pub fn axis(&self, i: usize) -> [f64; 3] {
        std::array::from_fn(|r| self.0[r][i.min(2)])
    }

    /// The linear part's cofactors: the inverse transpose times the determinant, which carries normals.
    pub fn cofactors(&self) -> [[f64; 3]; 3] {
        let m = &self.0;
        let c = |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
        [
            [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
            [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
            [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
        ]
    }

    pub fn determinant(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    /// The inverse map; `None` when the linear part is singular.
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.determinant();
        if !(det.abs() > 1e-12) || !det.is_finite() {
            return None;
        }
        let cof = self.cofactors();
        // The inverse is the adjugate over the determinant; the adjugate is the cofactors transposed.
        let l: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|c| cof[c][r] / det));
        let t = [self.0[0][3], self.0[1][3], self.0[2][3]];
        Some(Self(std::array::from_fn(|r| [l[r][0], l[r][1], l[r][2], -(l[r][0] * t[0] + l[r][1] * t[1] + l[r][2] * t[2])])))
    }

    /// Column-major 4x4 for OpenGL.
    pub fn gl(&self) -> [f32; 16] {
        let m = &self.0;
        let mut out = [0.0f32; 16];
        for c in 0..4 {
            for r in 0..3 {
                out[c * 4 + r] = m[r][c] as f32;
            }
        }
        out[15] = 1.0;
        out
    }
}

/// The rotation `from_euler_angles(roll, pitch, yaw)` builds: about z after y after x.
fn euler(degrees: [f64; 3]) -> [[f64; 3]; 3] {
    let (sr, cr) = degrees[0].to_radians().sin_cos();
    let (sp, cp) = degrees[1].to_radians().sin_cos();
    let (sy, cy) = degrees[2].to_radians().sin_cos();
    [
        [cy * cp, cy * sp * sr - sy * cr, cy * sp * cr + sy * sr],
        [sy * cp, sy * sp * sr + cy * cr, sy * sp * cr - cy * sr],
        [-sp, cp * sr, cp * cr],
    ]
}

/// The map an `Operation::Transform` applies: its rotation, then its translation.
pub fn transform(translation: [f64; 3], rotation_deg: [f64; 3]) -> Affine {
    let r = euler(rotation_deg);
    Affine(std::array::from_fn(|i| [r[i][0], r[i][1], r[i][2], translation[i]]))
}

/// Where a placement stands a part: on the band when there is one, else on the reference crest.
pub fn seat(design: &RingDesign, surface: Option<&BandSurface>, placement: &Placement) -> Option<Affine> {
    if let Some(s) = surface {
        return s.frame(placement, design);
    }
    let f = placement.frame(design).ok()?;
    Some(Affine::frame(f.x_axis, f.y_axis, f.z_axis, f.origin))
}

/// A primitive's new size over its old along the part's own axes; `None` for an operation with no size.
fn size_ratio(old: &Operation, new: &Operation) -> Option<[f64; 3]> {
    let ratio = |a: f64, b: f64| (a > 0.0 && b.is_finite()).then(|| b / a);
    Some(match (old, new) {
        (Operation::Box { size: a }, Operation::Box { size: b }) => [ratio(a[0], b[0])?, ratio(a[1], b[1])?, ratio(a[2], b[2])?],
        (Operation::Cylinder { radius_mm: r0, height_mm: h0 }, Operation::Cylinder { radius_mm: r1, height_mm: h1 }) => {
            let r = ratio(*r0, *r1)?;
            [r, r, ratio(*h0, *h1)?]
        }
        (Operation::Sphere { radius_mm: a }, Operation::Sphere { radius_mm: b }) => [ratio(*a, *b)?; 3],
        // A torus scales by its outer reach across and by its tube along the axis.
        (Operation::Torus { major_mm: m0, minor_mm: n0 }, Operation::Torus { major_mm: m1, minor_mm: n1 }) => {
            let reach = ratio(m0 + n0, m1 + n1)?;
            [reach, reach, ratio(*n0, *n1)?]
        }
        (Operation::Extrude { height_mm: a, .. }, Operation::Extrude { height_mm: b, .. }) => [1.0, 1.0, ratio(*a, *b)?],
        _ => return None,
    })
}

/// The map carrying `target`'s placed tessellation to `preview`: a new seat, a new transform, or a new size about its seat.
pub fn placed_ghost(design: &RingDesign, surface: Option<&BandSurface>, target: &Feature, preview: &Preview) -> Option<Affine> {
    let current = seat(design, surface, &target.component.placement)?;
    if let Some(p) = &preview.placement {
        return Some(seat(design, surface, p)?.mul(&current.inverse()?));
    }
    match &preview.operation {
        Some(Operation::Transform { source, translation, rotation_deg }) => {
            let new = transform(*translation, *rotation_deg);
            match &target.operation {
                // The part's own transform edited in place: undo the old one first.
                Operation::Transform { translation: t0, rotation_deg: r0, .. } if *source != target.id => Some(new.mul(&transform(*t0, *r0).inverse()?)),
                _ => Some(new),
            }
        }
        Some(op) => {
            let s = size_ratio(&target.operation, op)?;
            Some(current.mul(&Affine::scale(s)).mul(&current.inverse()?))
        }
        None => Some(Affine::IDENTITY),
    }
}

/// The map that carries a unit primitive to the size and seat an add's preview gives it.
pub fn unit_ghost(design: &RingDesign, surface: Option<&BandSurface>, preview: &Preview) -> Option<Affine> {
    let at = seat(design, surface, preview.placement.as_ref()?)?;
    let s = match preview.operation.as_ref()? {
        Operation::Box { size } => *size,
        Operation::Cylinder { radius_mm, height_mm } => [*radius_mm, *radius_mm, *height_mm],
        Operation::Sphere { radius_mm } => [*radius_mm; 3],
        _ => return None,
    };
    Some(at.mul(&Affine::scale(s)))
}

/// A primitive at unit size, evaluated on its own: a 1 mm cube, and a cylinder and a sphere of radius 1 mm.
pub fn unit_mesh(kind: Primitive) -> Option<Mesh> {
    let operation = match kind {
        Primitive::Box => Operation::Box { size: [1.0; 3] },
        Primitive::Cylinder => Operation::Cylinder { radius_mm: 1.0, height_mm: 1.0 },
        Primitive::Sphere => Operation::Sphere { radius_mm: 1.0 },
    };
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: operation.label().into(), enabled: true, operation, component: Component::default() }).ok()?;
    let mut design = RingDesign::default();
    design.cad = Some(doc);
    let evaluated = cad::evaluate(&design, &AlphaLibrary::default(), BuildParams::default()).ok()?;
    evaluated.components.into_iter().next().map(|c| c.mesh)
}

/// How a live command reads the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reading {
    /// The surface under the cursor, in the ring frame, snapped.
    Surface,
    /// Where the camera ray crosses the plane through `at` square to the view: a size dragged on screen.
    Plane { at: [f64; 3] },
}

/// Where a camera ray crosses the plane through `at` square to it.
pub fn on_view_plane(ray: Ray, at: [f64; 3]) -> Option<[f64; 3]> {
    let dd = dot(ray.direction, ray.direction);
    if !(dd > 0.0) || !dd.is_finite() {
        return None;
    }
    let t = dot(sub(at, ray.origin), ray.direction) / dd;
    Some(std::array::from_fn(|k| ray.origin[k] + ray.direction[k] * t))
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let l = dot(v, v).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| v.map(|x| x / l))
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// How far along the line through `at` in direction `dir` its point nearest a camera ray lies; `None` when the ray runs along it.
pub fn along_line(ray: Ray, at: [f64; 3], dir: [f64; 3]) -> Option<f64> {
    let (u, v) = (unit(dir)?, unit(ray.direction)?);
    let w = sub(at, ray.origin);
    let (b, d, e) = (dot(u, v), dot(u, w), dot(v, w));
    let denom = 1.0 - b * b;
    (denom > 1e-6).then(|| (b * e - d) / denom)
}

/// Where a camera ray crosses the plane through `at` square to `normal`; `None` when it runs within a degree of the plane.
pub fn on_plane(ray: Ray, at: [f64; 3], normal: [f64; 3]) -> Option<[f64; 3]> {
    let (n, d) = (unit(normal)?, unit(ray.direction)?);
    let across = dot(d, n);
    if across.abs() < 0.0175 {
        return None;
    }
    let s = dot(sub(at, ray.origin), n) / across;
    Some(std::array::from_fn(|k| ray.origin[k] + d[k] * s))
}

/// Two unit vectors square to `axis` and to each other, with `u × v` along it.
pub fn plane_basis(axis: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let n = unit(axis).unwrap_or([0.0, 0.0, 1.0]);
    let seed = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let d = dot(seed, n);
    let u = unit(std::array::from_fn(|k| seed[k] - n[k] * d)).unwrap_or([0.0, 1.0, 0.0]);
    (u, cross(n, u))
}

/// The angle of `p` about `axis` through `centre` in degrees, counted the way a right-handed turn about the axis runs.
pub fn angle_about(p: [f64; 3], centre: [f64; 3], axis: [f64; 3]) -> f64 {
    let (u, v) = plane_basis(axis);
    let w = sub(p, centre);
    dot(w, v).atan2(dot(w, u)).to_degrees()
}

/// Everything one pointer sample is read against.
pub struct Probe<'a> {
    pub design: &'a RingDesign,
    pub surface: Option<&'a BandSurface>,
    pub snapper: Option<Snapper>,
    pub geometry: SnapGeometry<'a>,
    pub view: ViewScale,
    pub aperture_px: f32,
    /// The part the command carries, which the pointer looks through to the band.
    pub carried: Option<Id>,
}

impl Probe<'_> {
    /// Radius of the reference crest.
    fn nominal_r(&self) -> f64 {
        self.design.inner_radius_mm() + self.design.profile.thickness_mm
    }

    fn carries(&self, e: &Entity) -> bool {
        let feature = match e {
            Entity::Part { feature } | Entity::Face { feature, .. } | Entity::Edge { feature, .. } | Entity::Vertex { feature, .. } => Some(*feature),
            Entity::Band | Entity::Stone { .. } => None,
        };
        feature.is_some() && feature == self.carried
    }

    /// The metal under the cursor: the first surface pick, or the band behind the carried part or a stone.
    fn under(&self, picks: &[Pick], ray: Ray) -> Option<([f64; 3], [f64; 3])> {
        let first = picks.iter().find(|p| matches!(p.entity, Entity::Band | Entity::Part { .. } | Entity::Face { .. } | Entity::Stone { .. }));
        match first {
            Some(p) if !self.carries(&p.entity) && !matches!(p.entity, Entity::Stone { .. }) => Some((p.world, p.normal)),
            _ => self.surface?.ray(ray),
        }
    }

    /// The pointer token: the metal under the cursor in the ring frame and snapped, or the view plane through an anchor.
    pub fn token(&self, reading: Reading, picks: &[Pick], ray: Ray) -> Option<StepInput> {
        let (world, normal, snapped) = match reading {
            Reading::Plane { at } => {
                let len = dot(ray.direction, ray.direction).sqrt();
                (on_view_plane(ray, at)?, ray.direction.map(|v| -v / len), None)
            }
            Reading::Surface => {
                let (world, normal) = self.under(picks, ray)?;
                let ring = ring_point(world, self.surface, self.nominal_r());
                let snapped = self.snapper.and_then(|s| s.snap(world, ring, &self.view, self.aperture_px, &self.geometry));
                (world, normal, snapped)
            }
        };
        let ring = match &snapped {
            Some(s) => s.ring,
            None => ring_point(world, self.surface, self.nominal_r()),
        };
        Some(StepInput::Pointer {
            world: snapped.as_ref().map_or(world, |s| s.world),
            normal,
            theta_deg: ring.theta_deg,
            across_mm: ring.across_mm,
            height_mm: ring.height_mm,
            snapped,
            dragging: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::snap::{Grid, SnapKind};
    use ringdesign_core::{cad::Attach, mesh, templates};

    fn court() -> RingDesign {
        templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() }
    }
    fn close3(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }

    #[test]
    fn the_band_surface_answers_as_surface_hit_on_the_whole_mesh() {
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let band = BandSurface::new(built.mesh.clone());
        // Every quarter degree, the sweep's own columns included, where a ray lands on the edge two faces share.
        for theta in (0..1440).map(|i| f64::from(i) * 0.25) {
            for across in [-1.2, 0.0, 0.4, 1.6] {
                let whole = cad::surface_hit(&built.mesh, theta, across);
                let patched = band.hit(theta, across);
                match (whole, patched) {
                    (Some((p, n)), Some((q, m))) => assert!(close3(p, q, 0.0) && close3(n, m, 0.0), "{theta} {across}: {p:?} {n:?} vs {q:?} {m:?}"),
                    (None, None) => {}
                    other => panic!("{theta} {across}: {other:?}"),
                }
            }
        }
        // Off the band's edge every ray misses, on the patch as on the whole.
        assert!(cad::surface_hit(&built.mesh, 90.0, 9.0).is_none() && band.hit(90.0, 9.0).is_none());
        let p = Placement::Ring { theta_deg: 123.0, across_mm: 0.3, height_mm: 0.25, spin_deg: 10.0, tilt_deg: 3.0, cant_deg: -4.0 };
        let whole = p.frame_on(&d, Some(&built.mesh)).unwrap();
        let seat = band.frame(&p, &d).unwrap();
        assert_eq!(seat, Affine::frame(whole.x_axis, whole.y_axis, whole.z_axis, whole.origin));
    }

    #[test]
    fn a_point_at_the_top_of_the_ring_reads_ninety_degrees_on_the_crest_and_no_stand_off() {
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let band = BandSurface::new(built.mesh.clone());
        // A pick at the top: the band under a camera ray straight down onto the crest.
        let (top, n) = band.ray(Ray { origin: [0.0, 40.0, 0.0], direction: [0.0, -1.0, 0.0] }).unwrap();
        let p = ring_point(top, Some(&band), 0.0);
        assert!((p.theta_deg - 90.0).abs() < 1e-3 && p.across_mm.abs() < 1e-3 && p.height_mm.abs() < 1e-3, "{p:?}");
        let lifted = ring_point(std::array::from_fn(|k| top[k] + n[k] * 0.75), Some(&band), 0.0);
        assert!((lifted.height_mm - 0.75).abs() < 1e-4, "{lifted:?}");
        // With no band the stand-off is read from the reference crest.
        let r = d.inner_radius_mm() + d.profile.thickness_mm;
        let bare = ring_point([0.0, r + 0.5, 0.0], None, r);
        assert!((bare.height_mm - 0.5).abs() < 1e-12 && (bare.theta_deg - 90.0).abs() < 1e-12);
    }

    #[test]
    fn the_euler_order_matches_the_ring_frame_the_core_builds() {
        let d = RingDesign::default();
        let p = Placement::Ring { theta_deg: 33.0, across_mm: 0.0, height_mm: 0.0, spin_deg: 21.0, tilt_deg: -8.0, cant_deg: 13.0 };
        let f = p.frame(&d).unwrap();
        let seat = transform([0.0; 3], [0.0, 90.0, 33.0]);
        let lean = transform([0.0; 3], [-8.0, 13.0, 21.0]);
        let m = seat.mul(&lean);
        for (axis, col) in [(f.x_axis, 0), (f.y_axis, 1), (f.z_axis, 2)] {
            let mine = [m.0[0][col], m.0[1][col], m.0[2][col]];
            assert!(close3(axis, mine, 1e-12), "{axis:?} vs {mine:?}");
        }
    }

    #[test]
    fn an_affine_inverse_undoes_it_and_its_cofactors_carry_normals() {
        let m = transform([1.0, -2.0, 0.5], [10.0, 20.0, 30.0]).mul(&Affine::scale([2.0, 0.5, 3.0]));
        let inv = m.inverse().unwrap();
        let p = [0.3, -1.7, 2.2];
        assert!(close3(inv.apply(m.apply(p)), p, 1e-12));
        assert!(close3(m.mul(&inv).apply(p), p, 1e-12));
        // A tangent carried by the map stays square to the normal carried by the cofactors.
        let (t, n) = ([1.0, 1.0, 0.0], [1.0, -1.0, 0.0]);
        let lt: [f64; 3] = std::array::from_fn(|r| (0..3).map(|k| m.0[r][k] * t[k]).sum());
        let c = m.cofactors();
        let ln: [f64; 3] = std::array::from_fn(|r| (0..3).map(|k| c[r][k] * n[k]).sum());
        assert!(dot(lt, ln).abs() < 1e-12);
        assert!(Affine::scale([1.0, 0.0, 1.0]).inverse().is_none());
        let gl = transform([1.0, 2.0, 3.0], [0.0; 3]).gl();
        assert_eq!((gl[12], gl[13], gl[14], gl[15], gl[0]), (1.0, 2.0, 3.0, 1.0, 1.0));
    }

    #[test]
    fn a_ghost_carries_the_placed_part_to_the_preview_seat_and_scales_about_its_own() {
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let band = BandSurface::new(built.mesh.clone());
        let target = Feature {
            id: 3,
            name: "Post".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 },
            component: Component { placement: Placement::ring(90.0, 0.0), attach: Attach::Join, ..Component::default() },
        };
        let moved = Placement::ring(102.0, 0.0);
        let preview = Preview { placement: Some(moved.clone()), ..Preview::default() };
        let m = placed_ghost(&d, Some(&band), &target, &preview).unwrap();
        let from = seat(&d, Some(&band), &target.component.placement).unwrap();
        let to = seat(&d, Some(&band), &moved).unwrap();
        // The part's own origin lands on the new seat's origin, its axis on the new normal.
        assert!(close3(m.apply(from.apply([0.0; 3])), to.apply([0.0; 3]), 1e-9));
        assert!(close3(m.apply(from.apply([0.0, 0.0, 1.0])), to.apply([0.0, 0.0, 1.0]), 1e-9));
        let (hit, _) = band.hit(102.0, 0.0).unwrap();
        assert!(close3(to.apply([0.0; 3]), hit, 1e-9), "the seat stands on the band");
        // A radius of 3 doubles the part across its axis and leaves its height and seat alone.
        let preview = Preview { operation: Some(Operation::Cylinder { radius_mm: 3.0, height_mm: 2.5 }), ..Preview::default() };
        let s = placed_ghost(&d, Some(&band), &target, &preview).unwrap();
        let rim = from.apply([1.5, 0.0, 0.0]);
        let top = from.apply([0.0, 0.0, 1.25]);
        assert!(close3(s.apply(rim), from.apply([3.0, 0.0, 0.0]), 1e-9));
        assert!(close3(s.apply(top), top, 1e-9));
        // A free part's own transform edited: only the change is applied.
        let free = Feature { id: 5, operation: Operation::Transform { source: 3, translation: [1.0, 0.0, 0.0], rotation_deg: [0.0; 3] }, component: Component::default(), ..target.clone() };
        let preview = Preview { operation: Some(Operation::Transform { source: 3, translation: [1.0, 2.0, 0.0], rotation_deg: [0.0; 3] }), ..Preview::default() };
        let t = placed_ghost(&d, None, &free, &preview).unwrap();
        assert!(close3(t.apply([4.0, 0.0, 0.0]), [4.0, 2.0, 0.0], 1e-12));
        // An add's unit cylinder takes the typed size at the clicked seat.
        let add = Preview { placement: Some(Placement::ring(70.0, 0.0)), operation: Some(Operation::Cylinder { radius_mm: 1.5, height_mm: 3.0 }), ..Preview::default() };
        let u = unit_ghost(&d, Some(&band), &add).unwrap();
        let seat70 = seat(&d, Some(&band), &Placement::ring(70.0, 0.0)).unwrap();
        assert!(close3(u.apply([1.0, 0.0, 0.5]), seat70.apply([1.5, 0.0, 1.5]), 1e-9));
        let unit = unit_mesh(Primitive::Cylinder).unwrap();
        let (lo, hi) = unit.bounds().unwrap();
        assert!((hi.0 - 1.0).abs() < 1e-3 && (lo.2 + 0.5).abs() < 1e-3 && (hi.2 - 0.5).abs() < 1e-3, "{lo:?} {hi:?}");
    }

    /// The pointer on the band at (θ, across), read as the viewport reads it.
    fn on_band(band: &BandSurface, theta: f64, across: f64) -> StepInput {
        let (world, normal) = band.hit(theta, across).unwrap();
        let p = ring_point(world, Some(band), 0.0);
        StepInput::Pointer { world, normal, theta_deg: p.theta_deg, across_mm: p.across_mm, height_mm: p.height_mm, snapped: None, dragging: false }
    }

    #[test]
    fn a_move_lands_its_part_on_the_palm_rather_than_the_pointer_and_says_so() {
        use crate::command::snap::{Dofs, RingFeatures, Scene};
        use crate::command::{Effect, MoveCmd, Outcome, Session};
        let d = court();
        let band = BandSurface::new(mesh::build(&d, &AlphaLibrary::builtin(), params()).mesh);
        let features = RingFeatures::of(&d, 0.0);
        let snapper = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 }), crest: true, ..Snapper::default() };
        // Down the finger at 20 px/mm.
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 20.0 };
        let world_of = |p: RingPoint| band.world(p);
        let scene = Scene { view, aperture_px: 8.0, geometry: SnapGeometry::default(), features: &features, design: Some(&d), world_of: &world_of };
        let snap = |p: RingPoint| snapper.snap_ring(band.world(p)?, p, Dofs::ALL, &scene);
        let seat = Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.25, spin_deg: 10.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let target = Feature { id: 3, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: Component { placement: seat, ..Component::default() } };
        let mut s = Session::default();
        s.start(Box::new(MoveCmd::of(&target, 9)));
        // Taken at 100° and carried 181.2° on, the part would stand at 271.2°: 1.2° off the palm, which it takes.
        assert!(matches!(land(&mut s, on_band(&band, 100.0, 0.0), &snap), Outcome::Continue));
        land(&mut s, on_band(&band, 281.2, 0.1), &snap);
        let p = s.preview().unwrap();
        assert!(p.caption.ends_with(" · palm 270.0° · parting line"), "{}", p.caption);
        let Outcome::Commit(e) = s.enter() else { panic!() };
        let [Effect::Placement { placement, .. }] = e.as_slice() else { panic!("{e:?}") };
        assert_eq!(*placement, Placement::Ring { theta_deg: 270.0, across_mm: 0.0, height_mm: 0.25, spin_deg: 10.0, tilt_deg: 0.0, cant_deg: 0.0 }, "exactly, its stand-off kept");
        // Off every feature the grid lands it instead, on the grid's own angle rather than the pointer's step.
        s.start(Box::new(MoveCmd::of(&target, 9)));
        land(&mut s, on_band(&band, 100.0, 0.0), &snap);
        land(&mut s, on_band(&band, 123.4, 0.0), &snap);
        let Outcome::Commit(e) = s.enter() else { panic!() };
        assert!(matches!(e.as_slice(), [Effect::Placement { placement: Placement::Ring { theta_deg, across_mm, .. }, .. }] if *theta_deg == 115.0 && *across_mm == 0.0), "{e:?}");
    }

    #[test]
    fn the_dial_and_the_round_the_ring_arrow_land_on_another_parts_angle() {
        use crate::command::snap::{Dofs, RingFeatures, Scene};
        use crate::command::{Axis, MoveCmd, PlaceCmd, Session};
        let d = court();
        let band = BandSurface::new(mesh::build(&d, &AlphaLibrary::builtin(), params()).mesh);
        let mut features = RingFeatures::of(&d, 0.0);
        features.angles.push((47.0, "Post B".into()));
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 20.0 };
        let world_of = |p: RingPoint| band.world(p);
        let scene = Scene { view, aperture_px: 8.0, geometry: SnapGeometry::default(), features: &features, design: Some(&d), world_of: &world_of };
        let dial = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.0, height_mm: 0.0 }), ..Snapper::default() };
        let snap = |p: RingPoint| dial.snap_ring(band.world(p)?, p, Dofs::THETA, &scene);
        let base = Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.25, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        // The dial's pointer: its angle, on the dial at the part's across and stand-off.
        let dialled = |theta: f64| StepInput::Pointer { world: [0.0; 3], normal: [0.0, 0.0, 1.0], theta_deg: theta, across_mm: 0.0, height_mm: 0.25, snapped: None, dragging: true };
        let mut s = Session::default();
        s.start(Box::new(PlaceCmd::new(2, base.clone())));
        s.feed(StepInput::Lock(Axis::Theta));
        land(&mut s, dialled(46.6), &snap);
        let p = s.preview().unwrap();
        assert_eq!((p.placement.as_ref().and_then(Placement::theta_deg), p.caption.contains(" · Post B 47.0°")), (Some(47.0), true), "{}", p.caption);
        land(&mut s, dialled(61.3), &snap);
        assert_eq!(s.preview().unwrap().placement.as_ref().and_then(Placement::theta_deg), Some(60.0), "then the dial's 5° grid");
        // The arrow has no grid: near the other part it lands on it, anywhere else it follows the pointer.
        let arrow = Snapper { grid: None, ..dial };
        let snap = |p: RingPoint| arrow.snap_ring(band.world(p)?, p, Dofs::THETA, &scene);
        let target = Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 }, component: Component { placement: base, ..Component::default() } };
        s.start(Box::new(MoveCmd::of(&target, 9)));
        s.feed(StepInput::Lock(Axis::Theta));
        land(&mut s, on_band(&band, 90.0, 0.0), &snap);
        land(&mut s, on_band(&band, 47.4, 0.0), &snap);
        assert_eq!(s.preview().unwrap().placement.as_ref().and_then(Placement::theta_deg), Some(47.0));
        land(&mut s, on_band(&band, 61.3, 0.0), &snap);
        let free = s.preview().unwrap().placement.as_ref().and_then(Placement::theta_deg).unwrap();
        assert!((free - 61.3).abs() < 1e-3 && !s.preview().unwrap().caption.contains("Post B"), "{free}");
    }

    #[test]
    fn a_token_reads_the_band_behind_the_carried_part_and_snaps_to_the_grid() {
        let d = court();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let band = BandSurface::new(built.mesh.clone());
        let (top, n) = band.hit(90.0, 0.2).unwrap();
        let view = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 };
        let ray = Ray { origin: [top[0], 40.0, top[2]], direction: [0.0, -1.0, 0.0] };
        let on_part = Pick { entity: Entity::Face { feature: 3, face: 1 }, world: [top[0], top[1] + 2.0, top[2]], normal: n, depth: 1.0, px: 0.0 };
        let probe = Probe { design: &d, surface: Some(&band), snapper: None, geometry: SnapGeometry::default(), view, aperture_px: 8.0, carried: Some(3) };
        let StepInput::Pointer { theta_deg, across_mm, height_mm, .. } = probe.token(Reading::Surface, std::slice::from_ref(&on_part), ray).unwrap() else { panic!() };
        assert!((theta_deg - 90.0).abs() < 1e-6 && (across_mm - top[2]).abs() < 1e-6 && height_mm.abs() < 1e-4, "{theta_deg} {across_mm} {height_mm}");
        // Another part is metal to stand on: its top reads as a stand-off.
        let other = Probe { carried: Some(9), ..probe };
        let StepInput::Pointer { height_mm, .. } = other.token(Reading::Surface, std::slice::from_ref(&on_part), ray).unwrap() else { panic!() };
        assert!(height_mm > 1.9, "{height_mm}");
        // The grid rounds the crest point; the plane reading ignores the surface.
        let snapper = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 }), parting_plane: false, ..Snapper::default() };
        let snapped = Probe { snapper: Some(snapper), carried: None, ..other };
        let band_pick = Pick { entity: Entity::Band, world: top, normal: n, depth: 1.0, px: 0.0 };
        let StepInput::Pointer { snapped: Some(hit), across_mm, .. } = snapped.token(Reading::Surface, &[band_pick], ray).unwrap() else { panic!() };
        assert_eq!((hit.kind, across_mm), (SnapKind::Grid, 0.0));
        let StepInput::Pointer { world, snapped: None, .. } = snapped.token(Reading::Plane { at: [0.0, 10.0, 0.0] }, &[], ray).unwrap() else { panic!() };
        assert!(close3(world, [top[0], 10.0, top[2]], 1e-12));
        assert!(snapped.token(Reading::Surface, &[], Ray { origin: [40.0, 40.0, 40.0], direction: [1.0, 0.0, 0.0] }).is_none(), "nothing under the cursor, no token");
    }
}
