//! The Manifold kernel: primitives, booleans, frames and tubes, settings —
//! the sibling `mandrel` crate's construction set, ported — and the
//! conversions between a Manifold and the core's watertight [`Mesh`].

use manifold3d::{CrossSection, JoinType, Manifold};
use nalgebra::{Matrix3, Rotation3, Vector3};
use ringdesign_core::mesh::{Mesh, Vec3};

pub use manifold3d;

/// A solid: Manifold's own type, always a valid manifold or empty.
pub type Solid = Manifold;

/// The default circular segment count: fine enough that a 1 mm prong
/// reads round, coarse enough that a setting stays a few thousand faces.
pub const SEG: i32 = 48;

pub type V3 = Vector3<f64>;

pub fn v3(x: f64, y: f64, z: f64) -> V3 {
    Vector3::new(x, y, z)
}

pub fn cylinder(height: f64, radius: f64, segments: i32) -> Solid {
    Manifold::cylinder(height, radius, radius, segments, false)
}

pub fn cone(height: f64, radius_bottom: f64, radius_top: f64, segments: i32) -> Solid {
    Manifold::cylinder(height, radius_bottom, radius_top, segments, false)
}

pub fn sphere(radius: f64, segments: i32) -> Solid {
    Manifold::sphere(radius, segments)
}

pub fn cube(x: f64, y: f64, z: f64, center: bool) -> Solid {
    Manifold::cube(x, y, z, center)
}

/// A marquise (navette) as the intersection of two equal circles.
pub fn marquise_lens(length: f64, width: f64, segments: i32) -> CrossSection {
    let p = (length * 0.5).max(1e-3);
    let q = (width * 0.5).max(1e-3).min(p - 1e-3);
    let rc = (p * p + q * q) / (2.0 * q);
    let a = rc - q;
    let top = CrossSection::circle(rc, segments).translate(0.0, a);
    let bot = CrossSection::circle(rc, segments).translate(0.0, -a);
    top.intersection(&bot)
}

pub fn marquise_prism(length: f64, width: f64, height: f64, segments: i32) -> Solid {
    marquise_lens(length, width, segments).extrude(height)
}

pub fn marquise_rail(length: f64, width: f64, rail_thickness: f64, height: f64, segments: i32) -> Solid {
    let lens = marquise_lens(length, width, segments);
    let outer = lens.offset(rail_thickness * 0.5, JoinType::Round, 2.0, segments);
    let inner = lens.offset(-rail_thickness * 0.5, JoinType::Round, 2.0, segments);
    outer.difference(&inner).extrude(height)
}

pub fn round_rail(radius: f64, rail_thickness: f64, height: f64, segments: i32) -> Solid {
    let outer = CrossSection::circle(radius + rail_thickness * 0.5, segments);
    let inner = CrossSection::circle((radius - rail_thickness * 0.5).max(1e-3), segments);
    outer.difference(&inner).extrude(height)
}

pub fn union_all(solids: &[Solid]) -> Solid {
    if solids.is_empty() { Manifold::empty() } else { Manifold::batch_union(solids) }
}

/// A right-handed frame: an origin and three unit axes.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: V3,
    pub x: V3,
    pub y: V3,
    pub z: V3,
}

impl Frame {
    /// `z` along `z_hint`, `x` as near `x_hint` as the right angle allows.
    pub fn new(origin: V3, z_hint: V3, x_hint: V3) -> Self {
        let z = z_hint.normalize();
        let mut x = x_hint - z * z.dot(&x_hint);
        if x.norm() < 1e-9 {
            let alt = if z.x.abs() < 0.9 { v3(1.0, 0.0, 0.0) } else { v3(0.0, 1.0, 0.0) };
            x = alt - z * z.dot(&alt);
        }
        let x = x.normalize();
        let y = z.cross(&x);
        Frame { origin, x, y, z }
    }

    /// A frame on a ring of radius `r` at `theta_deg` (90° is the top), its
    /// `z` pointing out of the band, `x` along the ring, lifted `axial` mm
    /// along the finger axis and `extra` mm outward.
    pub fn on_ring(r: f64, theta_deg: f64, axial: f64, extra: f64) -> Self {
        let t = theta_deg.to_radians();
        let radial = v3(t.cos(), t.sin(), 0.0);
        let along = v3(-t.sin(), t.cos(), 0.0);
        Frame::new(radial * (r + extra) + v3(0.0, 0.0, axial), radial, along)
    }

    pub fn place(&self, solid: &Solid) -> Solid {
        let m = Matrix3::from_columns(&[self.x, self.y, self.z]);
        solid.transform(&affine12(&m, &self.origin))
    }

    pub fn point(&self, local: V3) -> V3 {
        self.origin + self.x * local.x + self.y * local.y + self.z * local.z
    }

    /// Rolled about its own `z`.
    pub fn rolled(&self, degrees: f64) -> Frame {
        let (s, c) = degrees.to_radians().sin_cos();
        Frame { origin: self.origin, x: self.x * c + self.y * s, y: self.y * c - self.x * s, z: self.z }
    }

    /// Tilted about its own `x`.
    pub fn tilted(&self, degrees: f64) -> Frame {
        let (s, c) = degrees.to_radians().sin_cos();
        Frame { origin: self.origin, x: self.x, y: self.y * c + self.z * s, z: self.z * c - self.y * s }
    }
}

fn affine12(m: &Matrix3<f64>, t: &V3) -> [f64; 12] {
    [m[(0, 0)], m[(1, 0)], m[(2, 0)], m[(0, 1)], m[(1, 1)], m[(2, 1)], m[(0, 2)], m[(1, 2)], m[(2, 2)], t.x, t.y, t.z]
}

/// A cylinder from `p0` to `p1`, or nothing for a degenerate pair.
pub fn segment(p0: V3, p1: V3, radius: f64, segments: i32) -> Option<Solid> {
    let dir = p1 - p0;
    let len = dir.norm();
    if len < 1e-6 {
        return None;
    }
    let rot = Rotation3::rotation_between(&Vector3::z(), &dir).unwrap_or_else(Rotation3::identity);
    Some(cylinder(len, radius, segments).transform(&affine12(rot.matrix(), &p0)))
}

/// A round wire along a polyline: spheres at the knots, cylinders between.
pub fn tube(path: &[V3], radius: f64, segments: i32) -> Solid {
    let mut parts: Vec<Solid> = Vec::with_capacity(path.len() * 2);
    for &p in path {
        parts.push(sphere(radius, segments).translate(p.x, p.y, p.z));
    }
    for pair in path.windows(2) {
        if let Some(seg) = segment(pair[0], pair[1], radius, segments) {
            parts.push(seg);
        }
    }
    union_all(&parts)
}

/// Solids to add and solids to cut, kept apart until the final booleans.
#[derive(Default)]
pub struct Parts {
    pub add: Vec<Solid>,
    pub cut: Vec<Solid>,
}

impl Parts {
    pub fn extend(&mut self, other: Parts) {
        self.add.extend(other.add);
        self.cut.extend(other.cut);
    }

    /// Everything added, minus everything cut.
    pub fn resolve(&self) -> Solid {
        let body = union_all(&self.add);
        if self.cut.is_empty() { body } else { body.difference(&union_all(&self.cut)) }
    }
}

fn prong(height: f64, radius: f64) -> Solid {
    cylinder(height, radius, SEG / 2).union(&sphere(radius * 1.2, SEG / 2).translate(0.0, 0.0, height))
}

/// A four-prong marquise setting: girdle rail at the seat height, a
/// smaller base rail, prongs at the points and the sides, optional cross
/// bracing.
pub fn marquise_setting(length: f64, width: f64, seat_h: f64, cross_brace: bool) -> Parts {
    let rail_t = (width * 0.16).clamp(0.35, 0.7);
    let rail_h = 0.55;
    let prong_r = (width * 0.10).clamp(0.26, 0.42);
    let p = length * 0.5;
    let q = width * 0.5;
    let girdle = marquise_rail(length, width, rail_t, rail_h, SEG).translate(0.0, 0.0, seat_h - rail_h);
    let base = marquise_rail(length * 0.58, width * 0.58, rail_t, rail_h, SEG);
    let mut add = vec![girdle, base];
    for (px, py) in [(p, 0.0), (-p, 0.0), (0.0, q), (0.0, -q)] {
        add.push(prong(seat_h + 0.25, prong_r).translate(px, py, 0.0));
    }
    if cross_brace {
        let z = rail_h * 0.5;
        let bar_r = (width * 0.08).clamp(0.18, 0.3);
        let long = cylinder(length * 0.82, bar_r, SEG / 2).rotate(0.0, 90.0, 0.0).translate(-length * 0.41, 0.0, z);
        let short = cylinder(width * 0.82, bar_r, SEG / 2).rotate(-90.0, 0.0, 0.0).translate(0.0, -width * 0.41, z);
        add.push(long);
        add.push(short);
    }
    Parts { add, cut: vec![] }
}

/// A four-prong round setting.
pub fn round_setting(dia: f64, seat_h: f64) -> Parts {
    let r = dia * 0.5;
    let rail_t = (dia * 0.2).clamp(0.25, 0.5);
    let rail_h = 0.4;
    let prong_r = (dia * 0.13).clamp(0.18, 0.32);
    let top = round_rail(r, rail_t, rail_h, SEG).translate(0.0, 0.0, seat_h - rail_h);
    let base = round_rail(r * 0.68, rail_t, rail_h, SEG);
    let mut add = vec![top, base];
    for i in 0..4 {
        let a = std::f64::consts::FRAC_PI_2 * i as f64 + std::f64::consts::FRAC_PI_4;
        add.push(prong(seat_h + 0.2, prong_r).translate(r * a.cos(), r * a.sin(), 0.0));
    }
    Parts { add, cut: vec![] }
}

/// A domed leaf with a midrib and three vein pairs cut in.
pub fn leaf(length: f64, width: f64, thickness: f64) -> Solid {
    let blade = marquise_lens(length, width, SEG).extrude(thickness * 2.0).scale(1.0, 1.0, 0.5);
    let dome_r = length * 0.95;
    let dome = sphere(dome_r, SEG).translate(0.0, 0.0, thickness - dome_r);
    let mut blade = blade.intersection(&dome);
    let rib = cylinder(length * 1.1, thickness * 0.16, SEG / 2).rotate(0.0, 90.0, 0.0).translate(-length * 0.55, 0.0, thickness * 0.92);
    blade = blade.difference(&rib);
    for k in 1..=3 {
        let off = length * 0.16 * k as f64;
        for (sx, ang) in [(off, 32.0), (-off, -32.0)] {
            let vein = cylinder(width * 0.7, thickness * 0.12, SEG / 3).rotate(0.0, 90.0, ang).translate(sx, 0.0, thickness * 0.92);
            blade = blade.difference(&vein);
        }
    }
    blade
}

/// A Manifold as the core's mesh, with area-weighted vertex normals.
pub fn to_mesh(solid: &Solid) -> Mesh {
    let (props, n_props, tris) = solid.to_mesh_f32();
    if n_props < 3 || tris.len() < 3 {
        return Mesh::default();
    }
    let vertices: Vec<Vec3> = props.chunks_exact(n_props).map(|p| Vec3(p[0], p[1], p[2])).collect();
    let faces: Vec<[u32; 3]> = tris.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
    let mut acc = vec![[0.0f32; 3]; vertices.len()];
    for f in &faces {
        let [a, b, c] = [vertices[f[0] as usize], vertices[f[1] as usize], vertices[f[2] as usize]];
        let u = [b.0 - a.0, b.1 - a.1, b.2 - a.2];
        let v = [c.0 - a.0, c.1 - a.1, c.2 - a.2];
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for i in f {
            let s = &mut acc[*i as usize];
            s[0] += n[0];
            s[1] += n[1];
            s[2] += n[2];
        }
    }
    let normals = acc
        .into_iter()
        .map(|n| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-12 { Vec3(n[0] / l, n[1] / l, n[2] / l) } else { Vec3(0.0, 0.0, 1.0) }
        })
        .collect();
    Mesh { vertices, normals, faces }
}

/// The core's mesh as a Manifold, or why Manifold would not take it.
pub fn from_mesh(mesh: &Mesh) -> anyhow::Result<Solid> {
    let props: Vec<f32> = mesh.vertices.iter().flat_map(|v| [v.0, v.1, v.2]).collect();
    let tris: Vec<u32> = mesh.faces.iter().flat_map(|f| *f).collect();
    let m = Manifold::from_mesh_f32(&props, 3, &tris).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    m.status().map_err(|e| anyhow::anyhow!("not a manifold: {e:?}"))?;
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::castability::{self, DraftSettings, FaceClass};

    #[test]
    fn two_cylinders_union_into_one_watertight_mesh() {
        let a = cylinder(4.0, 1.0, 32);
        let b = cylinder(4.0, 1.0, 32).translate(0.8, 0.0, 1.0);
        let u = a.union(&b);
        assert!(!u.is_empty());
        assert!(u.volume() > cylinder(4.0, 1.0, 32).volume(), "more than one cylinder's worth");
        let mesh = to_mesh(&u);
        let v = mesh.validate();
        assert!(v.watertight, "{v:?}");
        assert_eq!(mesh.normals.len(), mesh.vertices.len());
        // And back: a watertight core mesh is a Manifold again.
        let back = from_mesh(&mesh).unwrap();
        assert!((back.volume() - u.volume()).abs() < 1e-3 * u.volume());
    }

    #[test]
    fn a_tube_ring_reads_as_vertical_walls_not_an_undercut() {
        // A short cylindrical band: bore wall and outer wall are vertical to
        // a Z pull, the two faces are perfect draft. Nothing leans back.
        let outer = cylinder(2.0, 10.0, 96);
        let inner = cylinder(2.0, 8.65, 96);
        let band = outer.difference(&inner).translate(0.0, 0.0, -1.0);
        let mesh = to_mesh(&band);
        assert!(mesh.validate().watertight);
        let report = castability::analyze(&mesh, &DraftSettings::default(), 8.65);
        assert_eq!(report.undercut, 0, "{:?}", report.notes);
        assert!(report.vertical > 0, "the walls read as vertical");
        assert!(report.classes.iter().any(|c| *c == FaceClass::Vertical));
        assert!(report.verdict != castability::Verdict::NotCastable);
    }

    #[test]
    fn frames_tubes_and_settings_build() {
        let f = Frame::on_ring(10.0, 90.0, 0.0, 0.5);
        assert!((f.origin.y - 10.5).abs() < 1e-9 && f.origin.x.abs() < 1e-9);
        assert!((f.z.dot(&v3(0.0, 1.0, 0.0)) - 1.0).abs() < 1e-9, "z points out of the band");
        let placed = f.place(&sphere(0.5, 16));
        assert!(placed.bounding_box().unwrap().center()[1] > 10.0);
        let wire = tube(&[v3(0.0, 0.0, 0.0), v3(3.0, 0.0, 0.0), v3(3.0, 2.0, 0.0)], 0.3, 16);
        assert!(to_mesh(&wire).validate().watertight);
        let setting = marquise_setting(8.0, 4.0, 2.0, true).resolve();
        assert!(to_mesh(&setting).validate().watertight && setting.volume() > 1.0);
        let round = round_setting(5.0, 2.0).resolve();
        assert!(to_mesh(&round).validate().watertight);
        let l = leaf(6.0, 3.0, 0.8);
        assert!(to_mesh(&l).validate().watertight && l.volume() > 0.5);
        assert!(segment(v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0), 0.5, 8).is_none());
    }
}
