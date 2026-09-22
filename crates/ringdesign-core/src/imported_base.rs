//! Portable, immutable signet stock with smooth, bore-preserving deformation.
//! Source triangles are retained, including where relief is zero. Subdivision
//! adds detail samples on those triangles; it never reconstructs the shoulders.
use crate::mesh::{BuildClock, BuildResult, Report, smooth_normals};
use crate::{AlphaLibrary, BuildParams, Mesh, ProfileLoop, ProfileSample, RingDesign, Vec3};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};
mod pull;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calibration {
    pub bore_radius_mm: f64,
    pub face_length_mm: f64,
    pub face_width_mm: f64,
    pub palm_thickness_mm: f64,
    /// Height above the bore at the centre of the face.
    pub head_height_mm: f64,
    /// C2 falloff bounds, in the source's Y coordinate.
    pub shoulder_start_mm: f64,
    pub shoulder_end_mm: f64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    pub version: u32,
    pub name: String,
    /// Provenance only; the embedded geometry is the source of truth.
    #[serde(default)]
    pub source_sha256: String,
    pub calibration: Calibration,
    /// Millimetres, finger axis Z, face pointing +Y, centred bore.
    pub vertices: Vec<[f64; 3]>,
    pub faces: Vec<[u32; 3]>,
    #[serde(skip)]
    cache: OnceLock<Mutex<Vec<(Target, Arc<Surface>)>>>,
    #[serde(skip)]
    fingerprint: OnceLock<u64>,
    #[serde(skip)]
    verified: OnceLock<std::result::Result<(), String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SurfaceChart {
    pub profile: crate::BandProfile,
    pub bore_radius_mm: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedBase {
    pub source: Arc<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<SurfaceChart>,
    /// Inspection switch. It hides relief without deleting the layer stack.
    #[serde(default)]
    pub bare: bool,
    /// Fill radial relief undercuts toward a Z=0 mold parting line. The
    /// original stock remains embedded and the layer stack stays editable.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sand_envelope: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct Target {
    bore: f64,
    length: f64,
    width: f64,
    thickness: f64,
    height: f64,
}
#[derive(Debug)]
struct Surface {
    mesh: Mesh,
    sections: Vec<Vec<[f64; 2]>>,
    paths: Vec<OuterPath>,
    field: OnceLock<Arc<FieldSurface>>,
}
#[derive(Debug)]
struct OuterPath {
    points: Vec<[f64; 2]>,
    arc: Vec<f64>,
    length: f64,
}
impl OuterPath {
    fn new(raw: &[[f64; 2]], bore: f64) -> Self {
        let bottom = raw
            .iter()
            .enumerate()
            .filter(|(_, p)| p[0] <= bore + 0.140001)
            .min_by(|a, b| a.1[1].total_cmp(&b.1[1]))
            .map(|(i, _)| i)
            .unwrap_or(1)
            .clamp(1, raw.len() - 2);
        let mut points = raw[bottom..].to_vec();
        points.push(raw[0]);
        let mut arc = vec![0.];
        for w in points.windows(2) {
            arc.push(arc.last().unwrap() + distance(w[0], w[1]));
        }
        let length = *arc.last().unwrap();
        Self {
            points,
            arc,
            length,
        }
    }
    fn at(&self, fraction: f64) -> [f64; 2] {
        let at = fraction.clamp(0., 1.) * self.length;
        let k = self
            .arc
            .partition_point(|v| *v < at)
            .clamp(1, self.arc.len() - 1);
        let t = ((at - self.arc[k - 1]) / (self.arc[k] - self.arc[k - 1]).max(1e-12)).clamp(0., 1.);
        std::array::from_fn(|i| {
            self.points[k - 1][i] + (self.points[k][i] - self.points[k - 1][i]) * t
        })
    }
}

/// Constant-time native chart lookup for millimetre-true settings. A tangent
/// plane footprint stays rigid on the stock; section arc fractions do not.
#[derive(Debug)]
pub struct FieldSurface {
    points: Vec<[f32; 3]>,
}
const FIELD_ACROSS: usize = 384;
#[derive(Clone, Copy, Debug)]
pub struct TangentFrame {
    pub point: [f64; 3],
    pub normal: [f64; 3],
    pub along: [f64; 3],
    pub across: [f64; 3],
}
impl FieldSurface {
    pub fn point(&self, theta: f64, fraction: f64) -> [f64; 3] {
        let u = theta.rem_euclid(360.) / 360. * STATIONS as f64;
        let v = fraction.clamp(0., 1.) * (FIELD_ACROSS - 1) as f64;
        let x = u.floor() as usize;
        let y = (v.floor() as usize).min(FIELD_ACROSS - 2);
        let (fu, fv) = (u - x as f64, v - y as f64);
        std::array::from_fn(|k| {
            let a = self.points[x * FIELD_ACROSS + y][k] as f64;
            let b = self.points[x * FIELD_ACROSS + y + 1][k] as f64;
            let c = self.points[((x + 1) % STATIONS) * FIELD_ACROSS + y][k] as f64;
            let d = self.points[((x + 1) % STATIONS) * FIELD_ACROSS + y + 1][k] as f64;
            (a + (b - a) * fv) * (1. - fu) + (c + (d - c) * fv) * fu
        })
    }
    pub fn frame(&self, theta: f64, fraction: f64) -> TangentFrame {
        let unit = |v: [f64; 3]| {
            let l = crate::mesh::norm(v).max(1e-9);
            v.map(|x| x / l)
        };
        let point = self.point(theta, fraction);
        let u = crate::mesh::sub(
            self.point(theta + 0.1, fraction),
            self.point(theta - 0.1, fraction),
        );
        let v = crate::mesh::sub(
            self.point(theta, fraction + 0.001),
            self.point(theta, fraction - 0.001),
        );
        let normal = unit(crate::mesh::cross(u, v));
        let (s, c) = theta.to_radians().sin_cos();
        let t = [-s, c, 0.];
        let dot = t.iter().zip(normal).map(|(a, b)| a * b).sum::<f64>();
        let along = unit(std::array::from_fn(|i| t[i] - normal[i] * dot));
        TangentFrame {
            point,
            normal,
            along,
            across: crate::mesh::cross(normal, along),
        }
    }
}
const STATIONS: usize = 720;
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * t * (10.0 + t * (-15.0 + 6.0 * t))
}
fn radius(p: [f64; 3]) -> f64 {
    p[0].hypot(p[1])
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
impl Source {
    /// Import triangle/quad OBJ stock. Calibration is explicit: file units and
    /// axes are never guessed. UV/normal seam duplicates are welded at 1e-6 mm.
    pub fn from_obj(
        text: &str,
        name: String,
        calibration: Calibration,
        scale: f64,
        crossgems_axes: bool,
    ) -> Result<Arc<Self>> {
        ensure!(
            text.len() <= 64 * 1024 * 1024 && scale.is_finite() && scale > 0.0,
            "Invalid OBJ scale or file size"
        );
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut faces = Vec::new();
        let mut weld = HashMap::new();
        for (line_no, line) in text.lines().enumerate() {
            let row: Vec<_> = line.split_whitespace().collect();
            if row.first() == Some(&"v") {
                ensure!(
                    row.len() >= 4,
                    "OBJ vertex is incomplete on line {}",
                    line_no + 1
                );
                let mut p = [0.0; 3];
                for k in 0..3 {
                    p[k] = row[k + 1].parse::<f64>()? * scale;
                }
                if crossgems_axes {
                    p = [p[0], p[2], -p[1]];
                }
                ensure!(
                    p.iter().all(|x| x.is_finite() && x.abs() < 100.0),
                    "OBJ must be centred and calibrated to millimetres"
                );
                let key = p.map(|x| (x * 1e6).round() as i64);
                let i = *weld.entry(key).or_insert_with(|| {
                    let i = vertices.len() as u32;
                    vertices.push(p);
                    i
                });
                indices.push(i);
            } else if row.first() == Some(&"f") {
                ensure!(
                    (4..=5).contains(&row.len()),
                    "OBJ importer accepts triangles and quads only"
                );
                let mut polygon = Vec::new();
                for token in &row[1..] {
                    let i = token.split('/').next().unwrap().parse::<isize>()?;
                    let i = if i < 0 {
                        indices.len() as isize + i
                    } else {
                        i - 1
                    };
                    ensure!(i >= 0 && (i as usize) < indices.len(), "Invalid OBJ index");
                    polygon.push(indices[i as usize]);
                }
                for k in 1..polygon.len() - 1 {
                    faces.push([polygon[0], polygon[k], polygon[k + 1]]);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        faces.retain(|f| {
            let [a, b, c] = f.map(|i| vertices[i as usize]);
            let n = crate::mesh::cross(crate::mesh::sub(b, a), crate::mesh::sub(c, a));
            let mut key = *f;
            key.sort();
            crate::mesh::norm(n) > 1e-8 && seen.insert(key)
        });
        let source = Self {
            version: 1,
            name,
            source_sha256: String::new(),
            calibration,
            vertices,
            faces,
            cache: OnceLock::new(),
            fingerprint: OnceLock::new(),
            verified: OnceLock::new(),
        };
        source.validate()?;
        Ok(Arc::new(source))
    }
    pub fn from_json(text: &str) -> Result<Arc<Self>> {
        ensure!(text.len() <= 64 * 1024 * 1024, "Base file exceeds 64 MiB");
        let s: Self = serde_json::from_str(text)?;
        s.validate()?;
        Ok(Arc::new(s))
    }
    pub(crate) fn fingerprint(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        *self.fingerprint.get_or_init(|| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for p in &self.vertices {
                for x in p {
                    x.to_bits().hash(&mut h);
                }
            }
            self.faces.hash(&mut h);
            serde_json::to_vec(&self.calibration)
                .unwrap_or_default()
                .hash(&mut h);
            h.finish()
        })
    }
    pub fn validate(&self) -> Result<()> {
        self.verified
            .get_or_init(|| self.check().map_err(|e| e.to_string()))
            .clone()
            .map_err(anyhow::Error::msg)
    }
    fn check(&self) -> Result<()> {
        ensure!(self.version == 1, "Unsupported imported-base version");
        ensure!(
            (16..=200_000).contains(&self.vertices.len())
                && (32..=400_000).contains(&self.faces.len()),
            "Base must contain 16–200,000 vertices and 32–400,000 triangles"
        );
        ensure!(
            self.vertices
                .iter()
                .flatten()
                .all(|x| x.is_finite() && x.abs() < 100.0),
            "Base must be finite, centred and in millimetres"
        );
        let c = &self.calibration;
        ensure!(
            [
                c.bore_radius_mm,
                c.face_length_mm,
                c.face_width_mm,
                c.palm_thickness_mm,
                c.head_height_mm,
                c.shoulder_start_mm,
                c.shoulder_end_mm
            ]
            .iter()
            .all(|x| x.is_finite()),
            "Invalid base calibration"
        );
        ensure!(
            (5.0..=16.0).contains(&c.bore_radius_mm)
                && (3.0..=40.0).contains(&c.face_length_mm)
                && (3.0..=40.0).contains(&c.face_width_mm)
                && (0.5..=6.0).contains(&c.palm_thickness_mm)
                && (1.0..=15.0).contains(&c.head_height_mm)
                && c.shoulder_end_mm - c.shoulder_start_mm > 2.0,
            "Base calibration is outside supported dimensions"
        );
        let size = (c.bore_radius_mm * std::f64::consts::TAU - 36.5) / 2.55;
        ensure!(
            (crate::sizing::MIN_SIZE..=crate::sizing::MAX_SIZE).contains(&size),
            "Calibrated bore is outside the app's supported ring sizes"
        );
        ensure!(
            self.faces
                .iter()
                .flatten()
                .all(|&i| (i as usize) < self.vertices.len()),
            "Base has invalid triangle indices"
        );
        let signed_volume: f64 = self
            .faces
            .iter()
            .map(|f| {
                let [a, b, c] = f.map(|i| self.vertices[i as usize]);
                let n = crate::mesh::cross(b, c);
                a.iter().zip(n).map(|(x, y)| x * y).sum::<f64>()
            })
            .sum();
        ensure!(
            signed_volume > 0.0,
            "Base triangles face inward; reverse their winding before import"
        );
        let mesh = self.mesh(Target::rest(c));
        ensure!(
            mesh.validate().watertight,
            "Base must be closed and manifold; repair it before importing"
        );
        ensure!(
            mesh.quality().degenerate_faces == 0,
            "Base has collapsed triangles"
        );
        let mut edges = HashMap::<(u32, u32), i32>::new();
        for f in &self.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                *edges.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
            }
        }
        ensure!(
            edges.values().all(|&n| n == 0),
            "Base has inconsistent triangle winding"
        );
        let min = self
            .vertices
            .iter()
            .copied()
            .map(radius)
            .fold(f64::INFINITY, f64::min);
        ensure!(
            (min - c.bore_radius_mm).abs() < 0.12,
            "Bore calibration disagrees with the mesh; check units and orientation"
        );
        // Reject disconnected stock rather than treating decorations as shoulders.
        let mut adj = vec![Vec::new(); self.vertices.len()];
        for f in &self.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                adj[a as usize].push(b);
                adj[b as usize].push(a);
            }
        }
        let mut seen = vec![false; adj.len()];
        let mut todo = vec![0];
        seen[0] = true;
        while let Some(i) = todo.pop() {
            for &j in &adj[i] {
                if !seen[j as usize] {
                    seen[j as usize] = true;
                    todo.push(j as usize);
                }
            }
        }
        ensure!(seen.iter().all(|x| *x), "Base must be one connected solid");
        Ok(())
    }
    fn deform(&self, p: [f64; 3], t: Target) -> [f64; 3] {
        let c = &self.calibration;
        let r = radius(p);
        let depth = (r - c.bore_radius_mm).max(0.0);
        // Protect the bore and comfort edge; the entire palm participates in
        // radial fit changes. All region boundaries have zero first/second slope.
        let skin = smooth(depth / c.palm_thickness_mm.max(0.5));
        let head =
            smooth((p[1] - c.shoulder_start_mm) / (c.shoulder_end_mm - c.shoulder_start_mm)) * skin;
        let dr =
            (t.bore - c.bore_radius_mm + (t.thickness - c.palm_thickness_mm) * skin) * (1.0 - head);
        [
            p[0] + dr * p[0] / r + (t.length / c.face_length_mm - 1.0) * p[0] * head,
            p[1] + dr * p[1] / r + (t.height - c.head_height_mm + t.bore - c.bore_radius_mm) * head,
            p[2] * (1.0 + (t.width / c.face_width_mm - 1.0) * head),
        ]
    }
    fn mesh(&self, t: Target) -> Mesh {
        let vertices = self
            .vertices
            .iter()
            .map(|&p| {
                let p = self.deform(p, t);
                Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
            })
            .collect::<Vec<_>>();
        let normals = smooth_normals(&vertices, &self.faces);
        Mesh {
            vertices,
            normals,
            faces: self.faces.clone(),
            ..Default::default()
        }
    }
}
impl Target {
    fn rest(c: &Calibration) -> Self {
        Self {
            bore: c.bore_radius_mm,
            length: c.face_length_mm,
            width: c.face_width_mm,
            thickness: c.palm_thickness_mm,
            height: c.head_height_mm,
        }
    }
    fn design(d: &RingDesign) -> Self {
        Self {
            bore: d.inner_radius_mm(),
            length: d.shank.head.length_mm,
            width: d.profile.width_mm,
            thickness: d.profile.thickness_mm,
            height: d.profile.thickness_mm + d.shank.head.rise_mm,
        }
    }
}
impl ImportedBase {
    pub fn attach(d: &mut RingDesign, source: Arc<Source>) -> Result<()> {
        source.validate()?;
        ensure!(
            d.band_is_procedural(),
            "A ring of CAD parts only has no band to change; add a Procedural shank first"
        );
        let previous = d.clone();
        let chart = d
            .imported_base
            .as_ref()
            .and_then(|b| b.chart.clone())
            .unwrap_or_else(|| SurfaceChart {
                profile: d.profile.clone(),
                bore_radius_mm: d.inner_radius_mm(),
            });
        d.imported_base = Some(Self {
            source,
            chart: Some(chart),
            bare: false,
            sand_envelope: false,
        });
        Self::reset(d);
        // Graph provenance must be regenerated from the new source.
        d.graph = None;
        d.build.refine = None;
        d.build.adaptive = false;
        if let Err(error) = d.imported_base.as_ref().unwrap().validate_shape(d) {
            *d = previous;
            return Err(error);
        }
        Ok(())
    }
    pub fn reset(d: &mut RingDesign) {
        if let Some(b) = &d.imported_base {
            let c = &b.source.calibration;
            d.size = crate::RingSize((c.bore_radius_mm * 2.0 * std::f64::consts::PI - 36.5) / 2.55);
            d.profile.width_mm = c.face_width_mm;
            d.profile.thickness_mm = c.palm_thickness_mm;
            d.shank.kind = crate::ShankKind::Signet;
            d.shank.head.length_mm = c.face_length_mm;
            d.shank.head.rise_mm = c.head_height_mm - c.palm_thickness_mm;
        }
    }
    pub fn validate_design(&self, d: &RingDesign) -> Result<()> {
        self.source.validate()?;
        let c = &self.source.calibration;
        let t = Target::design(d);
        ensure!(
            [t.bore, t.length, t.width, t.thickness, t.height]
                .iter()
                .all(|x| x.is_finite()),
            "Base dimensions must be finite"
        );
        ensure!(
            (t.bore - c.bore_radius_mm).abs() <= 1.5,
            "Imported base: bore change is limited to ±3 mm diameter"
        );
        ensure!(
            (0.7..=1.3).contains(&(t.length / c.face_length_mm))
                && (0.7..=1.3).contains(&(t.width / c.face_width_mm)),
            "Imported base: face dimensions must stay within 70–130% of the master"
        );
        ensure!(
            (t.thickness - c.palm_thickness_mm).abs() <= 0.5 && t.thickness >= 0.8,
            "Imported base: palm thickness change is limited to ±0.5 mm, minimum 0.8 mm"
        );
        ensure!(
            (t.height - c.head_height_mm).abs() <= 1.5,
            "Imported base: head height change is limited to ±1.5 mm"
        );
        Ok(())
    }
    fn surface(&self, d: &RingDesign) -> Result<Arc<Surface>> {
        self.validate_design(d)?;
        let t = Target::design(d);
        let cache = self.source.cache.get_or_init(|| Mutex::new(Vec::new()));
        {
            let cache = cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((_, s)) = cache.iter().find(|(k, _)| *k == t) {
                return Ok(s.clone());
            }
        }
        let mesh = self.source.mesh(t);
        // Numerical Jacobian checks throughout material vertices catch folds
        // from combined edits, including between the editable regions.
        for &p in &self.source.vertices {
            let mut j = [[0.0; 3]; 3];
            let e = 1e-4;
            for k in 0..3 {
                let mut a = p;
                let mut b = p;
                a[k] += e;
                b[k] -= e;
                let a = self.source.deform(a, t);
                let b = self.source.deform(b, t);
                for i in 0..3 {
                    j[k][i] = (a[i] - b[i]) / (2.0 * e);
                }
            }
            let det = j[0][0] * (j[1][1] * j[2][2] - j[1][2] * j[2][1])
                - j[0][1] * (j[1][0] * j[2][2] - j[1][2] * j[2][0])
                + j[0][2] * (j[1][0] * j[2][1] - j[1][1] * j[2][0]);
            ensure!(
                det > 0.12,
                "This combination folds the shoulder. Reduce the face or height change."
            );
        }
        #[cfg(feature = "parallel")]
        use rayon::prelude::*;
        #[cfg(feature = "parallel")]
        let iter = (0..STATIONS).into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let iter = 0..STATIONS;
        let sections = iter
            .map(|i| slice(&mesh, i as f64 * 360.0 / STATIONS as f64, t.bore))
            .collect::<Result<Vec<_>>>()?;
        let paths = sections.iter().map(|s| OuterPath::new(s, t.bore)).collect();
        let s = Arc::new(Surface {
            mesh,
            sections,
            paths,
            field: OnceLock::new(),
        });
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= 3 {
            cache.remove(0);
        }
        cache.push((t, s.clone()));
        Ok(s)
    }
    pub fn validate_shape(&self, d: &RingDesign) -> Result<()> {
        self.surface(d).map(|_| ())
    }
    pub fn field_surface(&self, d: &RingDesign) -> Result<Arc<FieldSurface>> {
        let surface = self.surface(d)?;
        Ok(surface
            .field
            .get_or_init(|| {
                let mut points = Vec::with_capacity(STATIONS * FIELD_ACROSS);
                for (i, path) in surface.paths.iter().enumerate() {
                    let (sin, cos) = (i as f64 * std::f64::consts::TAU / STATIONS as f64).sin_cos();
                    for j in 0..FIELD_ACROSS {
                        let [r, z] = path.at(j as f64 / (FIELD_ACROSS - 1) as f64);
                        points.push([(r * cos) as f32, (r * sin) as f32, z as f32]);
                    }
                }
                Arc::new(FieldSurface { points })
            })
            .clone())
    }
    /// Point and outward normal on the deformed stock, in the persistent UV chart.
    pub fn point_normal(&self, d: &RingDesign, theta: f64, v: f64) -> Result<([f64; 3], [f64; 3])> {
        let frame = self
            .field_surface(d)?
            .frame(theta, v / d.reference_loop().surface_len_mm.max(1e-9));
        Ok((frame.point, frame.normal))
    }
    pub fn section(&self, d: &RingDesign, theta: f64, n: usize) -> Result<ProfileLoop> {
        let s = self.surface(d)?;
        let f = theta.rem_euclid(360.0) * STATIONS as f64 / 360.0;
        let i = f.floor() as usize;
        let t = f.fract();
        let a = sample_loop(&s.sections[i], n, d.inner_radius_mm());
        let b = sample_loop(&s.sections[(i + 1) % STATIONS], n, d.inner_radius_mm());
        let pts = a
            .pts
            .iter()
            .zip(&b.pts)
            .map(|(a, b)| ProfileSample {
                r: a.r + (b.r - a.r) * t,
                z: a.z + (b.z - a.z) * t,
                ..*a
            })
            .collect();
        Ok(crate::profile::finish_loop(pts, Vec::new()))
    }
}

/// Slice a triangle solid with a radial half-plane, connect the segments,
/// and keep the closed CCW material boundary. The tiny deterministic angular
/// offset avoids ambiguities at source vertices lying exactly on the plane.
fn slice(mesh: &Mesh, theta: f64, bore: f64) -> Result<Vec<[f64; 2]>> {
    let (s, c) = (theta.to_radians() + 1e-7).sin_cos();
    let mut pts = Vec::<[f64; 2]>::new();
    let mut map = HashMap::new();
    let mut edges = Vec::new();
    for f in &mesh.faces {
        let v = f.map(|i| mesh.vertices[i as usize]);
        let d = v.map(|v| -v.0 as f64 * s + v.1 as f64 * c);
        if d.iter().all(|x| *x >= 0.0) || d.iter().all(|x| *x < 0.0) {
            continue;
        }
        let mut hit = Vec::new();
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if (d[a] < 0.0) != (d[b] < 0.0) {
                let t = d[a] / (d[a] - d[b]);
                let r = (v[a].0 as f64 + (v[b].0 as f64 - v[a].0 as f64) * t) * c
                    + (v[a].1 as f64 + (v[b].1 as f64 - v[a].1 as f64) * t) * s;
                let z = v[a].2 as f64 + (v[b].2 as f64 - v[a].2 as f64) * t;
                if r > 0.0 {
                    hit.push(((f[a].min(f[b]), f[a].max(f[b])), [r, z]));
                }
            }
        }
        if hit.len() != 2 {
            continue;
        }
        let mut ids = [0; 2];
        for k in 0..2 {
            let (key, p) = hit[k];
            ids[k] = *map.entry(key).or_insert_with(|| {
                let i = pts.len();
                pts.push(p);
                i
            });
        }
        if ids[0] != ids[1] {
            edges.push(ids);
        }
    }
    let mut adj = vec![Vec::new(); pts.len()];
    for [a, b] in edges {
        if !adj[a].contains(&b) {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    ensure!(
        pts.len() > 8 && adj.iter().all(|a| a.len() == 2),
        "Base section at {theta:.1}° is open or branched; repair the master mesh"
    );
    let mut order = vec![0];
    let mut prev = usize::MAX;
    let mut i = 0;
    loop {
        let next = *adj[i].iter().find(|&&x| x != prev).unwrap();
        if next == 0 {
            break;
        }
        ensure!(order.len() < pts.len(), "Invalid base section");
        order.push(next);
        prev = i;
        i = next;
    }
    ensure!(
        order.len() == pts.len(),
        "Base has multiple section loops at {theta:.1}°; hollow or separate stock is unsupported"
    );
    let ordered: Vec<_> = order.into_iter().map(|i| pts[i]).collect();
    // Insert exact intersections with the protected bore cylinder. Picking
    // the nearest source vertex made the UV seam jump as a section rotated.
    let cutoff = bore + 0.14;
    let mut out = Vec::new();
    for i in 0..ordered.len() {
        let a = ordered[i];
        let b = ordered[(i + 1) % ordered.len()];
        out.push(a);
        if (a[0] < cutoff) != (b[0] < cutoff) {
            let t = (cutoff - a[0]) / (b[0] - a[0]);
            out.push([cutoff, a[1] + (b[1] - a[1]) * t]);
        }
    }
    let area = (0..out.len())
        .map(|i| {
            let a = out[i];
            let b = out[(i + 1) % out.len()];
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>();
    if area < 0.0 {
        out.reverse();
    }
    // Start at the upper bore edge. Sorting is deliberately avoided: a real
    // concave shoulder is retained as a connected polyline.
    let start = out
        .iter()
        .enumerate()
        .filter(|(_, p)| p[0] <= bore + 0.140001)
        .max_by(|a, b| a.1[1].total_cmp(&b.1[1]))
        .map(|(i, _)| i)
        .ok_or_else(|| anyhow::anyhow!("No calibrated bore in section"))?;
    out.rotate_left(start);
    Ok(out)
}
fn sample_path(p: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let mut cum = vec![0.0];
    for w in p.windows(2) {
        cum.push(cum.last().unwrap() + distance(w[0], w[1]));
    }
    let len = *cum.last().unwrap();
    let mut k = 0;
    (0..n)
        .map(|i| {
            let at = len * i as f64 / n as f64;
            while k + 2 < p.len() && cum[k + 1] < at {
                k += 1;
            }
            let t = (at - cum[k]) / (cum[k + 1] - cum[k]).max(1e-12);
            [
                p[k][0] + (p[k + 1][0] - p[k][0]) * t,
                p[k][1] + (p[k + 1][1] - p[k][1]) * t,
            ]
        })
        .collect()
}
fn sample_loop(raw: &[[f64; 2]], n: usize, bore: f64) -> ProfileLoop {
    let n = n.max(24);
    let bottom = raw
        .iter()
        .enumerate()
        .filter(|(_, p)| p[0] <= bore + 0.140001)
        .min_by(|a, b| a.1[1].total_cmp(&b.1[1]))
        .map(|(i, _)| i)
        .unwrap_or(1)
        .clamp(1, raw.len() - 2);
    let nb = (n / 4).max(4);
    let mut outer = raw[bottom..].to_vec();
    outer.push(raw[0]);
    let pts = sample_path(&raw[..=bottom], nb)
        .into_iter()
        .map(|p| (p, false))
        .chain(sample_path(&outer, n - nb).into_iter().map(|p| (p, true)))
        .map(|(p, surface)| ProfileSample {
            r: p[0],
            z: p[1],
            nr: 0.0,
            nz: 0.0,
            v_mm: 0.0,
            surface,
            weight: 0.0,
        })
        .collect();
    crate::profile::finish_loop(pts, Vec::new())
}

/// Split marked edges conformingly, preserving the exact piecewise-linear
/// master surface. A centre fan handles mixed edge lengths without T junctions.
fn subdivide(mut mesh: Mesh, edge: f64, detail: impl Fn(Vec3) -> bool) -> Result<Mesh> {
    let mut detailed: Vec<bool> = mesh.vertices.iter().map(|&p| detail(p)).collect();
    for _ in 0..10 {
        let mut mids = HashMap::new();
        for f in &mesh.faces {
            for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
                let key = (a.min(b), a.max(b));
                if mids.contains_key(&key) {
                    continue;
                }
                let va = mesh.vertices[a as usize];
                let vb = mesh.vertices[b as usize];
                let d = ((va.0 - vb.0) as f64)
                    .hypot((va.1 - vb.1) as f64)
                    .hypot((va.2 - vb.2) as f64);
                let midpoint = Vec3(
                    (va.0 + vb.0) * 0.5,
                    (va.1 + vb.1) * 0.5,
                    (va.2 + vb.2) * 0.5,
                );
                // First sample every region at <=0.4 mm, then spend the fine
                // budget on ornament footprints. Quiet stock and the bore do
                // not need the same density as the face's beading.
                if d > edge
                    && (d > 0.4 || detailed[a as usize] || detailed[b as usize] || detail(midpoint))
                {
                    let i = mesh.vertices.len() as u32;
                    mesh.vertices.push(Vec3(
                        (va.0 + vb.0) * 0.5,
                        (va.1 + vb.1) * 0.5,
                        (va.2 + vb.2) * 0.5,
                    ));
                    detailed.push(detail(midpoint));
                    let na = mesh.normals[a as usize];
                    let nb = mesh.normals[b as usize];
                    mesh.normals
                        .push(unit(Vec3(na.0 + nb.0, na.1 + nb.1, na.2 + nb.2)));
                    mids.insert(key, i);
                }
            }
        }
        if mids.is_empty() {
            break;
        }
        let mut faces = Vec::new();
        for [a, b, c] in mesh.faces {
            let get = |a: u32, b: u32| mids.get(&(a.min(b), a.max(b))).copied();
            match (get(a, b), get(b, c), get(c, a)) {
                (None, None, None) => faces.push([a, b, c]),
                (Some(d), None, None) => faces.extend([[a, d, c], [d, b, c]]),
                (None, Some(e), None) => faces.extend([[b, e, a], [e, c, a]]),
                (None, None, Some(f)) => faces.extend([[c, f, b], [f, a, b]]),
                (Some(d), Some(e), None) => faces.extend([[d, b, e], [a, d, c], [d, e, c]]),
                (None, Some(e), Some(f)) => faces.extend([[e, c, f], [b, e, a], [e, f, a]]),
                (Some(d), None, Some(f)) => faces.extend([[f, a, d], [c, f, b], [f, d, b]]),
                (Some(d), Some(e), Some(f)) => {
                    faces.extend([[a, d, f], [d, b, e], [f, e, c], [d, e, f]])
                }
            }
        }
        ensure!(
            faces.len() < 2_000_000,
            "Imported relief exceeds the 2 million triangle budget; lower mesh detail"
        );
        mesh.faces = faces;
    }
    Ok(mesh)
}

fn unit(v: Vec3) -> Vec3 {
    let l = v.0.hypot(v.1).hypot(v.2).max(1e-12);
    Vec3(v.0 / l, v.1 / l, v.2 / l)
}

fn project_uv(
    p: Vec3,
    sections: &[OuterPath],
    ctx: &crate::field::FieldContext,
) -> (crate::Uv, f64) {
    let theta = (p.1 as f64)
        .atan2(p.0 as f64)
        .rem_euclid(std::f64::consts::TAU);
    let r = (p.0 as f64).hypot(p.1 as f64);
    let z = p.2 as f64;
    let f = theta / std::f64::consts::TAU * STATIONS as f64;
    let i = f.floor() as usize;
    let t = f.fract();
    let project = |sec: &OuterPath| {
        let mut best = f64::INFINITY;
        let mut result = (0., 0.);
        for (k, w) in sec.points.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let dr = b[0] - a[0];
            let dz = b[1] - a[1];
            let q = (((r - a[0]) * dr + (z - a[1]) * dz) / (dr * dr + dz * dz).max(1e-12))
                .clamp(0., 1.);
            let dist = (r - a[0] - q * dr).powi(2) + (z - a[1] - q * dz).powi(2);
            if dist < best {
                best = dist;
                let at = sec.arc[k] + (sec.arc[k + 1] - sec.arc[k]) * q;
                result = (
                    at / sec.length,
                    (at.min(sec.length - at) / crate::profile::EDGE_FADE_MM).clamp(0., 1.),
                );
            }
        }
        result
    };
    let a = project(&sections[i]);
    let b = project(&sections[(i + 1) % STATIONS]);
    (
        crate::Uv {
            u: theta / std::f64::consts::TAU * ctx.circumference_mm,
            v: (a.0 + (b.0 - a.0) * t) * ctx.band_v_len_mm,
        },
        a.1 + (b.1 - a.1) * t,
    )
}

pub fn build(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<BuildResult> {
    ensure!(
        params.refine.is_none(),
        "Imported bases use fixed relief sampling; choose a mesh quality tier instead of a procedural refinement tolerance"
    );
    let clock = BuildClock::start();
    let b = d.imported_base.as_ref().unwrap();
    let surface = b.surface(d)?;
    let mut mesh = surface.mesh.clone();
    let mut hi = 0.0f64;
    let mut lo = 0.0f64;
    if !b.bare && b.sand_envelope {
        (mesh, hi, lo) = pull::build(d, lib, params, &surface)?;
    } else if !b.bare && d.layers.layers.iter().any(|l| l.enabled) {
        // Same source at every quality tier; only the relief sampling changes.
        let edge = (2.0 * std::f64::consts::TAU * d.inner_radius_mm()
            / params.theta_steps.clamp(24, 4096) as f64)
            .clamp(0.06, 0.4);
        let ctx = d.field_context();
        let sections = &surface.paths;
        let footprints: Vec<_> = d
            .layers
            .layers
            .iter()
            .filter(|e| e.enabled && e.opacity > 0.)
            .map(|e| (e, e.layer.feature_footprints(&ctx)))
            .collect();
        mesh = subdivide(mesh, edge, |p| {
            if (p.0 as f64).hypot(p.1 as f64) < d.inner_radius_mm() + 0.15 {
                return false;
            }
            let (uv, _) = project_uv(p, &sections, &ctx);
            footprints.iter().any(|(e, feet)| {
                e.mask_at(uv, &ctx, lib) > 0.
                    && feet.iter().any(|f| {
                        let v = uv.v >= f.v_mm.0 - 0.4 && uv.v <= f.v_mm.1 + 0.4;
                        let u = f.u_mm.is_none_or(|(a, b)| {
                            crate::field::wrap_delta(uv.u - (a + b) * 0.5, ctx.circumference_mm)
                                .abs()
                                <= (b - a).abs() * 0.5 + 0.4
                        });
                        u && v
                    })
            })
        })?;
        let geometric_normals = smooth_normals(&mesh.vertices, &mesh.faces);
        #[cfg(feature = "parallel")]
        use rayon::prelude::*;
        #[cfg(feature = "parallel")]
        let iter = mesh.vertices.par_iter().zip(mesh.normals.par_iter());
        #[cfg(not(feature = "parallel"))]
        let iter = mesh.vertices.iter().zip(mesh.normals.iter());
        let displaced: Vec<_> = iter
            .map(|(p, normal)| {
                let r = (p.0 as f64).hypot(p.1 as f64);
                if r < d.inner_radius_mm() + 0.15 {
                    return (*p, 0.0);
                }
                let (uv, weight) = project_uv(*p, &sections, &ctx);
                let h =
                    crate::mesh::soft_height(&d.layers, uv, &ctx, lib, params.soften_mm) * weight;
                let h = if h.is_finite() { h } else { 0.0 };
                let mut out = Vec3(
                    p.0 + (h * normal.0 as f64) as f32,
                    p.1 + (h * normal.1 as f64) as f32,
                    p.2 + (h * normal.2 as f64) as f32,
                );
                let new_r = (out.0 as f64).hypot(out.1 as f64);
                let floor = (d.inner_radius_mm() + params.min_wall_mm.max(0.05)).min(r);
                if new_r < floor {
                    let k = (floor / new_r.max(1e-9)) as f32;
                    out.0 *= k;
                    out.1 *= k;
                }
                (out, h)
            })
            .collect();
        for (i, (p, h)) in displaced.into_iter().enumerate() {
            mesh.vertices[i] = p;
            hi = hi.max(h);
            lo = lo.min(h);
        }
        let detailed = smooth_normals(&mesh.vertices, &mesh.faces);
        for i in 0..mesh.normals.len() {
            let n = mesh.normals[i];
            let a = geometric_normals[i];
            let b = detailed[i];
            mesh.normals[i] = unit(Vec3(n.0 + b.0 - a.0, n.1 + b.1 - a.1, n.2 + b.2 - a.2));
        }
    }
    let (a, z) = mesh.bounds().unwrap();
    let bounds = [(z.0 - a.0) as f64, (z.1 - a.1) as f64, (z.2 - a.2) as f64];
    let volume = mesh.volume_mm3();
    let report = Report {
        validation: mesh.validate(),
        volume_mm3: volume,
        surface_area_mm2: mesh.surface_area_mm2(),
        bounds_mm: bounds,
        inner_diameter_mm: mesh
            .vertices
            .iter()
            .map(|p| (p.0 as f64).hypot(p.1 as f64) * 2.0)
            .fold(f64::INFINITY, f64::min),
        outer_diameter_mm: bounds[0].max(bounds[1]),
        band_width_mm: bounds[2],
        max_relief_mm: hi,
        min_relief_mm: lo,
        metals: crate::metal::metal_table(volume),
        build_ms: clock.ms(),
        refine: None,
        quality: mesh.quality(),
    };
    ensure!(
        report.validation.watertight && report.quality.degenerate_faces == 0,
        "Deformed mesh failed topology/triangle validation"
    );
    Ok(BuildResult {
        mesh,
        report,
        reference: d.reference_loop(),
        spacing: crate::adaptive::Spacing::uniform(params.theta_steps.clamp(24, 4096)),
        solids: Default::default(),
        parts: Default::default(),
    })
}

/// One bundled stock master: the factory's own head, under the name of the
/// shape its table draws. The number is the stock's provenance and is what a
/// design or an example names it by; the name is what a picker shows.
pub struct Preset {
    /// The decoded factory preset's number, "001" through "020".
    pub id: &'static str,
    /// The table plan's shape — cushion, heart, octagon, and the rest.
    pub name: &'static str,
    /// Face length and width in mm, off the master's own calibration.
    pub face_mm: (f32, f32),
    /// The table's outline as radii at 48 even bearings, largest 1.0, so a
    /// picker can draw the head without loading its mesh.
    pub plan: &'static [f32],
    pub json: &'static str,
}
impl Preset {
    pub fn load(&self) -> Result<Arc<Source>> {
        Source::from_json(self.json)
    }

    /// The name inside the master file, which is how a design's attached
    /// base is matched back to the stock it was taken from.
    pub fn stock_name(&self) -> String {
        format!("Signet {}", self.id)
    }

    /// "Cushion · 20 × 20 mm · 001".
    pub fn label(&self) -> String {
        format!("{} · {:.0} × {:.0} mm · {}", self.name, self.face_mm.0, self.face_mm.1, self.id)
    }
}
macro_rules! presets {($($id:literal => $name:literal, $l:literal, $w:literal, [$($r:literal),*]);* $(;)?)=>{pub static PRESETS:&[Preset]=&[$(Preset{id:$id,name:$name,face_mm:($l,$w),plan:&[$($r),*],json:include_str!(concat!("../../../bases/signets/",$id,".ringbase.json"))}),*];};}
presets! {
    "001" => "Cushion", 20.0, 20.0, [0.822,0.840,0.859,0.902,0.955,1.000,0.998,0.964,0.914,0.868,0.837,0.820,0.819,0.837,0.865,0.910,0.963,0.996,1.000,0.958,0.906,0.862,0.841,0.823,0.821,0.830,0.859,0.903,0.958,0.991,0.993,0.950,0.897,0.856,0.836,0.822,0.823,0.835,0.865,0.909,0.963,0.992,0.991,0.944,0.897,0.861,0.832,0.819];
    "002" => "Kite", 14.0, 25.0, [0.571,0.577,0.588,0.603,0.622,0.656,0.698,0.729,0.796,0.850,0.924,0.987,0.985,0.921,0.847,0.794,0.743,0.696,0.655,0.631,0.601,0.587,0.576,0.570,0.571,0.578,0.590,0.605,0.635,0.659,0.702,0.749,0.800,0.854,0.928,0.996,1.000,0.926,0.870,0.798,0.746,0.699,0.658,0.633,0.604,0.589,0.578,0.571];
    "003" => "Clover", 18.0, 18.0, [0.676,0.892,0.989,0.997,0.957,0.904,0.906,0.969,0.991,0.978,0.868,0.640,0.692,0.865,0.975,0.988,0.965,0.901,0.898,0.971,0.990,0.982,0.885,0.670,0.669,0.852,0.974,0.993,0.965,0.902,0.902,0.960,0.993,0.981,0.872,0.701,0.644,0.873,0.984,0.996,0.965,0.907,0.908,0.971,1.000,0.982,0.861,0.679];
    "004" => "Shield", 18.0, 18.0, [0.762,0.744,0.737,0.734,0.743,0.761,0.778,0.807,0.854,0.891,0.943,1.000,0.998,0.943,0.891,0.854,0.808,0.778,0.762,0.743,0.734,0.737,0.744,0.762,0.788,0.820,0.874,0.923,0.956,0.928,0.705,0.615,0.573,0.560,0.577,0.602,0.601,0.577,0.560,0.573,0.615,0.706,0.928,0.956,0.922,0.874,0.821,0.788];
    "005" => "Rosette", 18.0, 21.0, [0.811,0.878,0.911,0.940,0.948,0.946,0.924,0.874,0.850,0.927,0.977,1.000,1.000,0.978,0.927,0.850,0.874,0.924,0.946,0.948,0.940,0.911,0.878,0.811,0.823,0.868,0.917,0.943,0.948,0.944,0.930,0.884,0.864,0.937,0.983,1.000,1.000,0.983,0.938,0.864,0.884,0.929,0.944,0.948,0.943,0.917,0.869,0.824];
    "006" => "Square", 16.0, 21.0, [0.620,0.631,0.661,0.706,0.764,0.854,0.994,0.999,0.929,0.868,0.835,0.815,0.815,0.835,0.868,0.929,0.999,0.994,0.854,0.764,0.707,0.661,0.631,0.620,0.622,0.635,0.655,0.699,0.775,0.868,0.995,1.000,0.923,0.863,0.832,0.814,0.814,0.832,0.863,0.922,1.000,0.995,0.868,0.775,0.699,0.656,0.635,0.622];
    "007" => "Quatrefoil", 16.0, 18.5, [0.405,0.573,0.730,0.839,0.907,0.976,0.995,0.994,0.953,0.896,0.732,0.522,0.531,0.753,0.896,0.963,0.993,0.993,0.966,0.919,0.824,0.727,0.564,0.421,0.406,0.568,0.714,0.837,0.905,0.977,0.999,0.998,0.964,0.902,0.751,0.531,0.531,0.738,0.903,0.974,0.998,1.000,0.969,0.921,0.830,0.692,0.570,0.407];
    "008" => "Heart", 19.0, 19.0, [0.885,0.862,0.843,0.817,0.812,0.811,0.823,0.842,0.866,0.906,0.946,0.987,0.984,0.919,0.876,0.843,0.816,0.794,0.781,0.778,0.786,0.802,0.820,0.839,0.868,0.896,0.924,0.951,0.969,0.970,0.963,0.940,0.899,0.815,0.753,0.658,0.616,0.726,0.809,0.887,0.952,0.988,0.997,1.000,0.985,0.967,0.931,0.908];
    "009" => "Drop", 17.0, 22.3, [0.728,0.724,0.725,0.729,0.741,0.754,0.773,0.797,0.836,0.882,0.936,0.994,1.000,0.936,0.882,0.835,0.796,0.773,0.754,0.741,0.729,0.725,0.724,0.728,0.736,0.747,0.760,0.775,0.796,0.813,0.837,0.855,0.877,0.893,0.901,0.905,0.905,0.901,0.893,0.877,0.855,0.837,0.813,0.796,0.775,0.760,0.747,0.736];
    "010" => "Trillion", 18.0, 20.0, [0.722,0.692,0.681,0.678,0.690,0.706,0.728,0.771,0.823,0.882,0.964,1.000,0.999,0.971,0.891,0.831,0.777,0.733,0.700,0.685,0.677,0.678,0.695,0.715,0.756,0.790,0.848,0.896,0.922,0.924,0.910,0.860,0.826,0.800,0.781,0.771,0.773,0.784,0.797,0.823,0.868,0.907,0.925,0.924,0.903,0.857,0.800,0.749];
    "011" => "Badge", 18.0, 20.0, [0.784,0.847,0.888,0.909,0.910,0.900,0.879,0.864,0.930,0.939,0.950,0.997,1.000,0.951,0.939,0.930,0.864,0.879,0.900,0.910,0.909,0.888,0.847,0.784,0.737,0.709,0.703,0.759,0.929,0.929,0.931,0.940,0.843,0.824,0.840,0.838,0.838,0.840,0.824,0.843,0.940,0.931,0.929,0.929,0.759,0.703,0.709,0.737];
    "012" => "Cushion", 10.0, 10.0, [0.829,0.843,0.876,0.919,0.970,0.999,0.997,0.963,0.903,0.864,0.830,0.814,0.814,0.832,0.864,0.904,0.961,0.998,1.000,0.971,0.920,0.872,0.844,0.829,0.826,0.838,0.867,0.903,0.964,0.991,0.991,0.958,0.903,0.858,0.831,0.815,0.816,0.831,0.858,0.903,0.958,0.993,0.992,0.957,0.907,0.868,0.838,0.826];
    "013" => "Round", 10.0, 10.0, [1.000,1.000,0.999,0.997,0.995,0.993,0.990,0.988,0.986,0.984,0.983,0.982,0.982,0.983,0.984,0.986,0.988,0.991,0.993,0.995,0.997,0.999,1.000,1.000,1.000,1.000,0.999,0.997,0.995,0.993,0.990,0.988,0.986,0.984,0.983,0.982,0.982,0.983,0.984,0.986,0.988,0.991,0.993,0.995,0.997,0.999,1.000,1.000];
    "014" => "Heater", 15.0, 18.0, [0.595,0.582,0.577,0.583,0.596,0.616,0.641,0.688,0.743,0.804,0.889,1.000,0.994,0.883,0.797,0.736,0.681,0.634,0.608,0.589,0.576,0.571,0.577,0.591,0.612,0.640,0.691,0.752,0.820,0.881,0.845,0.758,0.686,0.644,0.624,0.606,0.608,0.619,0.649,0.692,0.746,0.832,0.886,0.843,0.752,0.692,0.658,0.616];
    "015" => "Octagon", 16.0, 16.0, [0.831,0.854,0.881,0.938,1.000,0.996,0.991,0.995,0.933,0.872,0.836,0.822,0.819,0.838,0.876,0.929,0.995,0.990,0.992,0.999,0.955,0.894,0.851,0.835,0.833,0.848,0.889,0.949,1.000,0.993,0.993,0.994,0.934,0.879,0.840,0.819,0.820,0.843,0.873,0.938,0.995,0.991,0.994,1.000,0.943,0.885,0.857,0.832];
    "016" => "Star", 19.0, 20.0, [0.934,0.852,0.753,0.729,0.822,0.955,0.969,0.932,0.815,0.764,0.897,0.997,1.000,0.902,0.792,0.787,0.920,0.977,0.966,0.835,0.739,0.762,0.860,0.946,0.940,0.842,0.742,0.729,0.836,0.956,0.959,0.885,0.752,0.748,0.855,0.975,0.965,0.856,0.747,0.774,0.875,0.951,0.948,0.830,0.722,0.728,0.827,0.932];
    "017" => "Tonneau", 16.0, 12.0, [0.922,0.946,0.987,1.000,0.969,0.906,0.851,0.796,0.746,0.726,0.700,0.695,0.694,0.710,0.725,0.744,0.793,0.849,0.903,0.968,1.000,0.987,0.947,0.922,0.920,0.938,0.985,0.984,0.935,0.872,0.824,0.778,0.737,0.703,0.691,0.677,0.677,0.690,0.702,0.735,0.776,0.822,0.870,0.934,0.984,0.985,0.939,0.921];
    "018" => "Butterfly", 20.0, 17.0, [0.653,0.594,0.570,0.690,0.822,0.895,0.905,0.891,0.855,0.798,0.706,0.673,0.673,0.704,0.795,0.853,0.890,0.905,0.895,0.825,0.693,0.571,0.594,0.650,0.735,0.864,0.929,0.975,1.000,0.995,0.967,0.883,0.809,0.731,0.620,0.531,0.529,0.617,0.728,0.806,0.913,0.965,1.000,0.999,0.977,0.932,0.867,0.739];
    "019" => "Jewel", 19.0, 19.0, [0.732,0.674,0.644,0.615,0.606,0.612,0.627,0.651,0.701,0.765,0.867,1.000,0.997,0.875,0.771,0.705,0.652,0.626,0.604,0.603,0.614,0.635,0.680,0.719,0.813,0.857,0.854,0.825,0.817,0.827,0.849,0.831,0.750,0.704,0.681,0.662,0.662,0.680,0.702,0.747,0.827,0.853,0.833,0.821,0.829,0.860,0.863,0.827];
    "020" => "Escutcheon", 18.0, 19.0, [0.741,0.779,0.820,0.859,0.881,0.885,0.883,0.868,0.842,0.824,0.858,0.955,0.956,0.839,0.823,0.839,0.865,0.882,0.885,0.878,0.861,0.812,0.769,0.743,0.729,0.749,0.796,0.875,1.000,0.980,0.962,0.876,0.786,0.762,0.780,0.797,0.796,0.783,0.759,0.788,0.863,0.961,0.977,1.000,0.891,0.806,0.739,0.727];
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_large_setting_keeps_its_physical_footprint_when_the_imported_face_changes() {
        use crate::field::{SeatPadLayer, SeatStyle};
        let mut d = RingDesign::default();
        ImportedBase::attach(
            &mut d,
            PRESETS
                .iter()
                .find(|p| p.id == "019")
                .unwrap()
                .load()
                .unwrap(),
        )
        .unwrap();
        let chart_span = d.reference_loop().surface_len_mm;
        for ratio in [0.8, 1.0, 1.2] {
            d.shank.head.length_mm = 19.0 * ratio;
            d.profile.width_mm = 19.0 * ratio;
            let ctx = d.field_context();
            let native = ctx.imported_surface.as_ref().unwrap();
            let f = (0..2000)
                .map(|i| i as f64 / 1999.)
                .min_by(|a, b| {
                    native.point(90., *a)[2]
                        .abs()
                        .total_cmp(&native.point(90., *b)[2].abs())
                })
                .unwrap();
            let frame = native.frame(90., f);
            let mut seat = SeatPadLayer {
                theta_deg: 90.,
                v_mm: chart_span * f,
                metal_true: true,
                style: SeatStyle::Bezel,
                bezel_wall_mm: 0.48,
                ..Default::default()
            };
            seat.fit_stone(crate::gem::Gem::calibrated(
                crate::gem::GemCut::Asscher,
                4.4,
            ));
            let plane = SeatPadLayer {
                theta_deg: 0.,
                v_mm: 0.,
                metal_true: false,
                ..seat.clone()
            };
            let mut hits = 0;
            for theta in (68..=112).step_by(2) {
                for k in -20..=20 {
                    let v = seat.v_mm + k as f64 * 0.12;
                    let p = native.point(theta as f64, v / chart_span);
                    let delta = crate::mesh::sub(p, frame.point);
                    let dot = |v: [f64; 3]| delta.iter().zip(v).map(|(a, b)| a * b).sum::<f64>();
                    let expected = plane.height(
                        crate::Uv {
                            u: dot(frame.along),
                            v: dot(frame.across),
                        },
                        &ctx,
                    );
                    let actual = seat.height(
                        crate::Uv {
                            u: ctx.u_of_theta(theta as f64),
                            v,
                        },
                        &ctx,
                    );
                    assert!(
                        (actual - expected).abs() < 1e-9,
                        "setting stretched at {ratio}"
                    );
                    if actual > 0.05 {
                        hits += 1;
                    }
                }
            }
            assert!(hits > 30);
            assert_eq!(
                seat.height(
                    crate::Uv {
                        u: ctx.u_of_theta(270.),
                        v: seat.v_mm
                    },
                    &ctx
                ),
                0.0
            );
        }
    }

    #[test]
    fn sand_support_has_a_real_parting_loop_and_a_releasing_nominal_bore() {
        let mut d = RingDesign::default();
        ImportedBase::attach(&mut d, PRESETS[1].load().unwrap()).unwrap();
        d.imported_base.as_mut().unwrap().sand_envelope = true;
        let ctx = d.field_context();
        let mut layer = crate::tiling::TilingLayer::default_for("Beads", &ctx);
        layer.height_mm = 0.12;
        d.layers.layers.push(crate::LayerEntry::new(
            "relief to support",
            crate::Layer::Tiling(layer),
        ));
        let lib = AlphaLibrary::builtin();
        let params = BuildParams {
            theta_steps: 384,
            profile_steps: 192,
            ..Default::default()
        };
        let built = build(&d, &lib, params).unwrap();
        assert!(built.report.validation.watertight);
        assert_eq!(built.report.quality.degenerate_faces, 0);
        assert!(built.report.quality.worst_aspect < 1000.);
        assert!((built.report.inner_diameter_mm - d.inner_radius_mm() * 2.).abs() < 1e-4);
        for p in &built.mesh.normals {
            assert!(p.0.is_finite() && p.1.is_finite() && p.2.is_finite());
        }
        let no = (params.profile_steps * 3 / 4 / 2) * 2;
        for row in built.mesh.vertices.chunks_exact(params.profile_steps) {
            assert_eq!(row[no / 2].2, 0.);
            let radial = |p: Vec3| (p.0 as f64).hypot(p.1 as f64);
            for j in 1..=no / 2 {
                assert!(radial(row[j]) + 2e-6 >= radial(row[j - 1]));
            }
            for j in no / 2..no {
                assert!(radial(row[j]) + 2e-6 >= radial(row[j + 1]));
            }
        }
        let setup = crate::manufacturing::Setup {
            auto_parting: false,
            parting_mm: 0.,
            sample_pitch_mm: 0.1,
            ..Default::default()
        };
        let release = crate::manufacturing::release::analyze(&built.mesh, &setup).unwrap();
        assert!(
            release.obstructions.is_empty(),
            "{:?}",
            release.obstructions
        );
        assert_eq!(release.unresolved_rays, 0);
        let loaded: RingDesign = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
        assert_eq!(
            build(&loaded, &lib, params).unwrap().mesh.vertices,
            built.mesh.vertices
        );
        d.imported_base.as_mut().unwrap().bare = true;
        assert_eq!(
            build(&d, &lib, params).unwrap().mesh.faces,
            d.imported_base.as_ref().unwrap().source.faces
        );
    }
    #[test]
    fn stock_rest_resize_reset_and_roundtrip() {
        for preset in PRESETS {
            let mut d = RingDesign::default();
            ImportedBase::attach(&mut d, preset.load().unwrap()).unwrap();
            let b = d.imported_base.as_ref().unwrap().clone();
            let rest = build(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
            assert_eq!(rest.mesh.faces, b.source.faces);
            for (v, p) in rest.mesh.vertices.iter().zip(&b.source.vertices) {
                assert_eq!(*v, Vec3(p[0] as f32, p[1] as f32, p[2] as f32));
            }
            for ratio in [0.8, 1.0, 1.2] {
                d.profile.width_mm = b.source.calibration.face_width_mm * ratio;
                d.shank.head.length_mm = b.source.calibration.face_length_mm * ratio;
                let r = build(&d, &AlphaLibrary::builtin(), BuildParams::default())
                    .unwrap_or_else(|e| panic!("{} at {ratio}: {e}", preset.name));
                assert!(r.report.validation.watertight);
                assert!((r.report.inner_diameter_mm - rest.report.inner_diameter_mm).abs() < 0.005);
                for angle in [0.0, 45.0, 90.0, 135.0, 270.0] {
                    assert_eq!(d.section_at(angle, 192, None, None).len(), 192);
                }
            }
            ImportedBase::reset(&mut d);
            let reset = build(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
            assert_eq!(reset.mesh.vertices, rest.mesh.vertices);
            let loaded: RingDesign =
                serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
            assert_eq!(
                build(&loaded, &AlphaLibrary::builtin(), BuildParams::default())
                    .unwrap()
                    .mesh
                    .vertices,
                rest.mesh.vertices
            );
        }
    }
    #[test]
    fn obj_seam_welding_orientation_validation_and_fit_are_real_geometry() {
        use std::fmt::Write;
        let source = PRESETS[0].load().unwrap();
        let mut obj = String::new();
        // OBJ deliberately duplicates every face corner and uses source Y as
        // finger axis. Import must weld and rotate without smoothing the stock.
        for f in &source.faces {
            for &i in f {
                let [x, y, z] = source.vertices[i as usize];
                writeln!(obj, "v {x} {} {y}", -z).unwrap();
            }
        }
        for i in 0..source.faces.len() {
            writeln!(obj, "f {} {} {}", i * 3 + 1, i * 3 + 2, i * 3 + 3).unwrap();
        }
        let imported = Source::from_obj(
            &obj,
            "OBJ test".into(),
            source.calibration.clone(),
            1.0,
            true,
        )
        .unwrap();
        assert_eq!(imported.vertices.len(), source.vertices.len());
        assert_eq!(imported.faces.len(), source.faces.len());
        let mut d = RingDesign::default();
        ImportedBase::attach(&mut d, imported.clone()).unwrap();
        let base = d.imported_base.clone().unwrap();
        base.validate_shape(&d).unwrap();
        let t = Target::design(&d);
        let mut resized = t;
        resized.bore += 0.75;
        let face: Vec<_> = imported
            .vertices
            .iter()
            .copied()
            .filter(|p| {
                p[1] > source.calibration.shoulder_end_mm
                    && radius(*p)
                        > source.calibration.bore_radius_mm + source.calibration.palm_thickness_mm
            })
            .collect();
        assert!(!face.is_empty());
        for p in face {
            let a = imported.deform(p, t);
            let b = imported.deform(p, resized);
            assert!((a[0] - b[0]).abs() < 1e-8);
            assert!((a[2] - b[2]).abs() < 1e-8);
            assert!((b[1] - a[1] - 0.75).abs() < 1e-8);
        }
        let mut bad = serde_json::to_value(&*source).unwrap();
        bad["faces"].as_array_mut().unwrap().pop();
        assert!(Source::from_json(&bad.to_string()).is_err());
        let mut bad = serde_json::to_value(&*source).unwrap();
        bad["version"] = serde_json::json!(2);
        assert!(Source::from_json(&bad.to_string()).is_err());
    }

    #[test]
    fn relief_is_attached_to_stock_and_invalid_edits_fail() {
        let mut d = RingDesign::default();
        ImportedBase::attach(&mut d, PRESETS[0].load().unwrap()).unwrap();
        let lib = AlphaLibrary::builtin();
        let ctx = d.field_context();
        d.layers.layers.push(crate::LayerEntry::new(
            "detail",
            crate::Layer::Tiling(crate::tiling::TilingLayer::default_for("Beads", &ctx)),
        ));
        let p = BuildParams {
            theta_steps: 192,
            profile_steps: 96,
            ..Default::default()
        };
        let out = build(&d, &lib, p).unwrap();
        assert!(out.report.validation.watertight);
        assert!(out.report.max_relief_mm > 0.0);
        d.profile.width_mm = 100.0;
        assert!(crate::mesh::try_build(&d, &lib, p).is_err());
    }
}
