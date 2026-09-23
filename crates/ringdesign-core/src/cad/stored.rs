//! A closed mesh another kernel made, kept in the design file so every build renders, joins and judges it without that kernel.
use super::{BuildCtx, Built, Document, Feature, Placement, SurfaceKind, Value, builders};
use crate::{RingDesign, csg, setting::Named, sketch::Id};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine;
use cadkernel::brep;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The key a stored mesh carries as a mesh value.
pub const STORED: &str = "stored";
/// The grid a stored position snaps to, mm: half of `csg::clean`'s 20 nm.
pub const QUANTUM_MM: f64 = 1e-5;
/// Most vertices a stored mesh holds.
pub const MAX_VERTICES: usize = 4_000_000;
/// Most triangles a stored mesh holds.
pub const MAX_TRIANGLES: usize = 8_000_000;
/// Most faces the triangles may name.
pub const MAX_FACES: usize = 65_536;
/// Most parts a stored mesh reads and replaces.
pub const MAX_SOURCES: usize = 8;
/// The packed stream's layout.
const VERSION: u8 = 1;
/// Largest grid coordinate a stored position may carry: about 11 m either way.
const MAX_GRID: i64 = 1 << 40;

/// What made a stored mesh, kept for a rerun where that kernel is.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    /// The kernel that made it: `occt` for OpenCascade.
    pub kernel: String,
    /// Its operation: `fillet`, `shell`, `junction` or `import`.
    pub op: String,
    /// The operation's parameters as the kernel read them.
    #[serde(default)]
    pub params: serde_json::Value,
    /// [`digest`] of the sources when it ran; empty for a mesh with none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub digest: String,
}

impl Recipe {
    /// The kernel as a person names it.
    pub fn kernel_name(&self) -> &str {
        match self.kernel.as_str() {
            "occt" => "OpenCascade",
            other => other,
        }
    }
    /// What the feature is called by default.
    pub fn label(&self) -> &'static str {
        match (self.kernel.as_str(), self.op.as_str()) {
            ("occt", "fillet") => "Fillet (OpenCascade)",
            ("occt", "shell") => "Shell (OpenCascade)",
            ("occt", "junction") => "Junction (OpenCascade)",
            (_, "import") => "Imported solid",
            _ => "Stored mesh",
        }
    }
}

/// A mesh packed for the file: positions on a 10 nm grid as zigzag varint deltas, triangles as corner deltas, faces as runs, under base64.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Packed {
    pub vertices: u32,
    pub triangles: u32,
    /// The surface each face the triangles name reads as.
    pub faces: Vec<SurfaceKind>,
    /// Base64 of the packed stream.
    pub data: Arc<str>,
}

/// A packed mesh read back: positions on the grid, triangles, and the face behind each.
#[derive(Clone, Debug, PartialEq)]
pub struct Unpacked {
    pub positions: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub face_of: Vec<u32>,
    pub kinds: Vec<SurfaceKind>,
}

fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}
fn zig(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}
fn unzig(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}
/// A cursor over the packed stream.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn next(&mut self) -> Result<u64> {
        let mut v = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = *self.bytes.get(self.at).context("Stored mesh ends early")?;
            self.at += 1;
            ensure!(shift <= 63, "Stored mesh holds an overlong number");
            v |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
        }
    }
    fn signed(&mut self) -> Result<i64> {
        self.next().map(unzig)
    }
}

impl Packed {
    /// `positions`, `triangles` and the face behind each triangle, of `kinds.len()` faces, packed.
    pub fn encode(positions: &[[f64; 3]], triangles: &[[u32; 3]], face_of: &[u32], kinds: &[SurfaceKind]) -> Result<Self> {
        ensure!(positions.len() <= MAX_VERTICES, "A stored mesh holds at most {MAX_VERTICES} vertices, not {}", positions.len());
        ensure!(triangles.len() <= MAX_TRIANGLES, "A stored mesh holds at most {MAX_TRIANGLES} triangles, not {}", triangles.len());
        ensure!(kinds.len() <= MAX_FACES, "A stored mesh names at most {MAX_FACES} faces, not {}", kinds.len());
        ensure!(face_of.len() == triangles.len(), "{} faces named for {} triangles", face_of.len(), triangles.len());
        let mut out = vec![VERSION];
        let mut last = [0i64; 3];
        for p in positions {
            for k in 0..3 {
                ensure!(p[k].is_finite(), "A stored position is not a finite number");
                let q = (p[k] / QUANTUM_MM).round();
                ensure!(q.abs() < MAX_GRID as f64, "A stored position lies past {} m", MAX_GRID as f64 * QUANTUM_MM / 1000.0);
                let q = q as i64;
                put(&mut out, zig(q - last[k]));
                last[k] = q;
            }
        }
        let mut first = 0i64;
        for t in triangles {
            ensure!(t.iter().all(|i| (*i as usize) < positions.len()), "A stored triangle names a missing vertex");
            let a = i64::from(t[0]);
            put(&mut out, zig(a - first));
            put(&mut out, zig(i64::from(t[1]) - a));
            put(&mut out, zig(i64::from(t[2]) - a));
            first = a;
        }
        let mut k = 0;
        while k < face_of.len() {
            let face = face_of[k];
            ensure!((face as usize) < kinds.len(), "A stored triangle names face {face} of {}", kinds.len());
            let run = face_of[k..].iter().take_while(|f| **f == face).count();
            put(&mut out, u64::from(face));
            put(&mut out, run as u64);
            k += run;
        }
        Ok(Self {
            vertices: positions.len() as u32,
            triangles: triangles.len() as u32,
            faces: kinds.to_vec(),
            data: base64::engine::general_purpose::STANDARD.encode(out).into(),
        })
    }

    /// The mesh as packed, positions on the grid; refused by name when the stream does not read.
    pub fn decode(&self) -> Result<Unpacked> {
        let (nv, nt) = (self.vertices as usize, self.triangles as usize);
        ensure!(nv <= MAX_VERTICES && nt <= MAX_TRIANGLES && self.faces.len() <= MAX_FACES, "A stored mesh past the caps: {nv} vertices, {nt} triangles, {} faces", self.faces.len());
        let bytes = base64::engine::general_purpose::STANDARD.decode(self.data.as_bytes()).context("Stored mesh is not base64")?;
        ensure!(bytes.first() == Some(&VERSION), "Stored mesh is packed as version {:?}; this build reads {VERSION}", bytes.first());
        let mut r = Reader { bytes: &bytes, at: 1 };
        let mut positions = Vec::with_capacity(nv);
        let mut last = [0i64; 3];
        for _ in 0..nv {
            let mut p = [0.0; 3];
            for k in 0..3 {
                last[k] = last[k].checked_add(r.signed()?).context("Stored position overflows")?;
                ensure!(last[k].abs() < MAX_GRID, "A stored position lies past the grid");
                p[k] = last[k] as f64 * QUANTUM_MM;
            }
            positions.push(p);
        }
        let mut triangles = Vec::with_capacity(nt);
        let mut first = 0i64;
        for _ in 0..nt {
            let a = first.checked_add(r.signed()?).context("Stored triangle overflows")?;
            let t = [a, a.saturating_add(r.signed()?), a.saturating_add(r.signed()?)];
            ensure!(t.iter().all(|i| (0..nv as i64).contains(i)), "A stored triangle names a missing vertex");
            triangles.push(t.map(|i| i as u32));
            first = a;
        }
        let mut face_of = Vec::with_capacity(nt);
        while face_of.len() < nt {
            let face = r.next()?;
            let run = r.next()? as usize;
            ensure!((face as usize) < self.faces.len(), "A stored triangle names face {face} of {}", self.faces.len());
            ensure!(run > 0 && run <= nt - face_of.len(), "A stored face run overruns the triangles");
            face_of.extend(std::iter::repeat_n(face as u32, run));
        }
        ensure!(r.at == bytes.len(), "Stored mesh carries {} bytes past its end", bytes.len() - r.at);
        Ok(Unpacked { positions, triangles, face_of, kinds: self.faces.clone() })
    }

    /// The mesh as a closed named part in its own frame: a patch per face, creases read off it.
    pub fn made(&self) -> Result<builders::Made> {
        let u = self.decode()?;
        let named = Named {
            solid: csg::Solid { v: u.positions, f: u.triangles },
            patch: u.face_of,
            names: (0..u.kinds.len()).map(|k| format!("Face {k}")).collect(),
        };
        let (open, repeated) = named.solid.open_edges();
        ensure!(open == 0 && repeated == 0 && !named.solid.is_empty(), "The stored mesh does not close ({open} open edges, {repeated} repeated)");
        let creases = builders::creases(&named, builders::CREASE_DEG);
        Ok(builders::Made { key: STORED.to_string(), named, kinds: u.kinds, creases, gem: None, seat: None, stations: Vec::new() })
    }
}

/// Whether `design` carries a stored mesh: in its document, or anywhere in its graph, clusters included.
pub fn carried_by(design: &RingDesign) -> bool {
    let in_document = design.cad.as_ref().is_some_and(|doc| doc.features.iter().any(|f| matches!(f.operation, super::Operation::Stored { .. })));
    in_document || design.graph.as_ref().is_some_and(in_json)
}

/// Whether `v` holds a stored mesh anywhere: an object keyed `Stored` over a recipe and the mesh.
fn in_json(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(map) => map.get("Stored").is_some_and(|s| s.get("recipe").is_some() && s.get("mesh").is_some()) || map.values().any(in_json),
        serde_json::Value::Array(items) => items.iter().any(in_json),
        _ => false,
    }
}

/// `v` with every object's keys in order, so a digest does not hang on how a map was built.
fn canonical(v: &serde_json::Value, out: &mut String) {
    match v {
        serde_json::Value::Object(map) => {
            let keys: BTreeSet<&String> = map.keys().collect();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::Value::String(k.clone()).to_string());
                out.push(':');
                canonical(&map[k], out);
            }
            out.push('}');
        }
        serde_json::Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// 16 hex digits of FNV-1a over every upstream feature's switch and operation and every source's placement but the first's.
pub fn digest(doc: &Document, sources: &[Id]) -> String {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<Id> = sources.to_vec();
    while let Some(id) = stack.pop() {
        if seen.len() >= 256 || !seen.insert(id) {
            continue;
        }
        if let Some(f) = doc.feature(id) {
            stack.extend(f.operation.sources());
        }
    }
    let mut text = String::new();
    for id in &seen {
        let Some(f) = doc.feature(*id) else {
            text.push_str(&format!("#{id}:missing;"));
            continue;
        };
        let placement = (sources.first() != Some(id) && sources.contains(id)).then_some(&f.component.placement);
        let v = serde_json::to_value((f.enabled, &f.operation, placement)).unwrap_or_default();
        text.push_str(&format!("#{id}:"));
        canonical(&v, &mut text);
        text.push(';');
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// A stored mesh as built: in its first source's seated frame, or placed by its own component when it has no source.
#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    f: &Feature,
    recipe: &Recipe,
    sources: &[Id],
    mesh: &Packed,
    design: &RingDesign,
    ctx: &BuildCtx,
    values: &BTreeMap<Id, Value>,
    frames: &BTreeMap<Id, brep::Placement>,
    who: &dyn Fn(Id) -> String,
    doc: &Document,
) -> Result<Built> {
    ensure!(sources.len() <= MAX_SOURCES, "A stored mesh reads at most {MAX_SOURCES} parts, not {}", sources.len());
    for s in sources {
        ensure!(values.contains_key(s), "Source feature #{s} is unavailable or suppressed");
    }
    let frame = match sources.first() {
        Some(first) => {
            if f.component.placement != Placement::Free {
                bail!("{} stands where {} stands; move that to move it", recipe.label(), who(*first));
            }
            frames.get(first).copied().unwrap_or(brep::Placement::IDENTITY)
        }
        None => match &f.component.placement {
            Placement::Free => brep::Placement::IDENTITY,
            p => p.frame_on(design, ctx.surface)?,
        },
    };
    let made = mesh.made()?;
    let mut notes = Vec::new();
    if !recipe.digest.is_empty() && recipe.digest != digest(doc, sources) {
        let names = sources.iter().map(|s| who(*s)).collect::<Vec<_>>().join(", ");
        notes.push(format!("{names} changed after {} made this mesh; it stands as made until it runs again where {} is", recipe.kernel_name(), recipe.kernel_name()));
    }
    Ok(Built { value: Value::Mesh(Arc::new(made.placed(&frame))), frame: Some(frame), attach: None, notes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Attach, Component, EvaluatedComponent, Operation, evaluate};
    use crate::{AlphaLibrary, BuildParams};

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..BuildParams::default() }
    }
    /// The Court band with a procedural shank and whatever `more` adds.
    fn court(more: Vec<Feature>) -> RingDesign {
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        for f in more {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }
    fn plate(theta: f64) -> Feature {
        let component = Component { attach: Attach::Join, placement: Placement::ring(theta, 0.0), ..Component::default() };
        Feature { id: 2, name: "Plate".into(), enabled: true, operation: Operation::Box { size: [4.0, 6.0, 2.0] }, component }
    }
    fn part(d: &RingDesign, id: Id) -> EvaluatedComponent {
        let b = crate::mesh::try_build(d, &AlphaLibrary::builtin(), params()).unwrap();
        b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).cloned().unwrap_or_else(|| panic!("#{id}: {:?}", b.parts.notes))
    }
    /// A kernel part's tessellation taken back into its seated frame and packed, as a kernel would hand one over.
    fn packed_from(c: &EvaluatedComponent) -> Packed {
        let f = c.frame;
        let local: Vec<[f64; 3]> = c
            .trace
            .positions
            .iter()
            .map(|p| {
                let d: [f64; 3] = std::array::from_fn(|k| p[k] - f.origin[k]);
                let on = |a: [f64; 3]| (0..3).map(|k| d[k] * a[k]).sum::<f64>();
                [on(f.x_axis), on(f.y_axis), on(f.z_axis)]
            })
            .collect();
        let faces = c.trace.face_kind.len() as u32;
        let face_of: Vec<u32> = (0..c.mesh.faces.len()).map(|t| c.trace.face_of(t).unwrap_or(faces)).collect();
        let mut kinds = c.trace.face_kind.clone();
        kinds.push(SurfaceKind::Freeform);
        Packed::encode(&local, &c.mesh.faces, &face_of, &kinds).unwrap()
    }
    fn stored(id: Id, recipe: Recipe, sources: Vec<Id>, mesh: Packed, attach: Attach) -> Feature {
        let name = recipe.label().to_string();
        Feature { id, name, enabled: true, operation: Operation::Stored { recipe, sources, mesh }, component: Component { attach, ..Component::default() } }
    }

    #[test]
    fn a_packed_mesh_reads_back_on_its_grid_and_packs_again_to_the_same_text() {
        let positions = vec![[0.0, 0.0, 0.0], [1.000_004, 0.0, 0.0], [0.0, -2.345_678_9, 0.0], [0.0, 0.0, 3.141_592_65]];
        let triangles = vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
        let kinds = [SurfaceKind::Plane, SurfaceKind::Freeform];
        let face_of = vec![0, 0, 1, 1];
        let p = Packed::encode(&positions, &triangles, &face_of, &kinds).unwrap();
        let u = p.decode().unwrap();
        assert_eq!((u.triangles.as_slice(), u.face_of.as_slice(), u.kinds.as_slice()), (triangles.as_slice(), face_of.as_slice(), kinds.as_slice()));
        for (a, b) in positions.iter().zip(&u.positions) {
            for k in 0..3 {
                assert!((a[k] - b[k]).abs() <= QUANTUM_MM / 2.0 + 1e-12, "{a:?} read back as {b:?}");
                assert_eq!(b[k], (a[k] / QUANTUM_MM).round() * QUANTUM_MM, "the grid point itself");
            }
        }
        assert_eq!(Packed::encode(&u.positions, &u.triangles, &u.face_of, &u.kinds).unwrap(), p, "a read mesh packs to the same text");
        // Four vertices, four triangles and two face runs in 39 bytes of stream, 52 characters of base64.
        assert_eq!(p.data.len(), 52, "{}", p.data);
        let back: Packed = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn a_damaged_pack_is_refused_by_name() {
        let good = Packed::encode(&[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], &[[0, 1, 2]], &[0], &[SurfaceKind::Plane]).unwrap();
        let with = |f: &dyn Fn(&mut Packed)| {
            let mut p = good.clone();
            f(&mut p);
            p.decode().unwrap_err().to_string()
        };
        assert_eq!(with(&|p| p.data = "!!".into()), "Stored mesh is not base64");
        assert_eq!(with(&|p| p.triangles = 2), "Stored mesh ends early");
        assert_eq!(with(&|p| p.vertices = 2), "A stored triangle names a missing vertex");
        assert_eq!(with(&|p| p.faces.clear()), "A stored triangle names face 0 of 0");
        let bytes = base64::engine::general_purpose::STANDARD.decode(good.data.as_bytes()).unwrap();
        let mut newer = bytes.clone();
        newer[0] = 2;
        assert_eq!(with(&|p| p.data = base64::engine::general_purpose::STANDARD.encode(&newer).into()), "Stored mesh is packed as version Some(2); this build reads 1");
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(with(&|p| p.data = base64::engine::general_purpose::STANDARD.encode(&longer).into()), "Stored mesh carries 1 bytes past its end");
        assert!(Packed::encode(&[[f64::NAN; 3]], &[], &[], &[]).is_err());
        // An open mesh packs, and is refused as a part.
        assert_eq!(good.made().unwrap_err().to_string(), "The stored mesh does not close (3 open edges, 0 repeated)");
    }

    #[test]
    fn a_stored_part_stands_on_its_source_joins_the_band_and_is_judged_without_a_kernel() {
        let lib = AlphaLibrary::builtin();
        let source = part(&court(vec![plate(90.0)]), 2);
        let mesh = packed_from(&source);
        let recipe = Recipe { kernel: "occt".into(), op: "fillet".into(), params: serde_json::json!({ "radius_mm": 0.3 }), digest: String::new() };
        let mut d = court(vec![plate(90.0)]);
        let digest_now = digest(d.cad.as_ref().unwrap(), &[2]);
        let recipe = Recipe { digest: digest_now, ..recipe };
        d.cad.as_mut().unwrap().append(stored(3, recipe.clone(), vec![2], mesh.clone(), Attach::Join)).unwrap();
        assert_eq!(d.cad.as_ref().unwrap().outputs, vec![1, 3], "the stored mesh replaces the part it was made from");
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?} {:?}", built.report.validation, built.parts.notes);
        assert_eq!(built.parts.joined, 1);
        let e = built.parts.evaluated.as_ref().unwrap();
        let c = e.components.iter().find(|c| c.id == 3).unwrap();
        let made = c.made.as_ref().unwrap();
        assert_eq!((made.key.as_str(), c.body.faces.len()), (STORED, 0), "a mesh part, no kernel body");
        assert!(e.features.iter().all(|r| r.notes.is_empty()), "{:?}", e.features);
        // It stands where its source stood, to the grid.
        let moved = source.trace.positions.iter().zip(&c.trace.positions).map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()).fold(0.0, f64::max);
        assert!(moved < 2e-5, "{moved}");
        assert!((c.mesh.volume_mm3() - source.mesh.volume_mm3()).abs() < 1e-3, "{} against {}", c.mesh.volume_mm3(), source.mesh.volume_mm3());
        assert_eq!(c.trace.face_kind[..6], source.trace.face_kind[..6], "faces keep their kinds");
        // The verdict reads it like any joined part.
        let f = crate::castability::judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&built));
        let judged = f.parts.iter().find(|p| p.feature == 3).expect("the stored part is judged");
        assert!(judged.judged && judged.total_area_mm2 > 50.0, "{judged:?}");
        // The source moved 25° round the ring carries the mesh with it, and nothing reads as stale.
        let mut turned = d.clone();
        turned.cad.as_mut().unwrap().features[1].component.placement = Placement::ring(65.0, 0.0);
        let c1 = part(&turned, 3);
        let (sin, cos) = (-25f64).to_radians().sin_cos();
        let turn = |p: &[f64; 3]| [p[0] * cos - p[1] * sin, p[0] * sin + p[1] * cos, p[2]];
        // Measured 0.00055 mm: the swept band's facets at the two angles.
        let rode = source.trace.positions.iter().zip(&c1.trace.positions).map(|(a, b)| (0..3).map(|k| (turn(a)[k] - b[k]).powi(2)).sum::<f64>().sqrt()).fold(0.0, f64::max);
        assert!(rode < 2e-3, "{rode}");
        let e1 = evaluate(&turned, &lib, params()).unwrap();
        assert!(e1.features.iter().all(|r| r.notes.is_empty()), "a move is not a change: {:?}", e1.features);
        // The file reopens bit for bit, stored mesh and all, at the version an older build refuses by name.
        let text = serde_json::to_string(&d).unwrap();
        let back: RingDesign = serde_json::from_str(&text).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), text);
        let saved = crate::library::design_json(&d).unwrap();
        assert!(carried_by(&d) && saved.contains(&format!("\"format_version\": {}", crate::library::FORMAT_VERSION)), "{}", &saved[..60]);
        assert_eq!(serde_json::to_string(&crate::library::load_design_str(&saved).unwrap()).unwrap(), text);
    }

    #[test]
    fn a_stored_mesh_whose_source_changed_says_so_and_keeps_standing() {
        let lib = AlphaLibrary::builtin();
        let source = part(&court(vec![plate(90.0)]), 2);
        let mut d = court(vec![plate(90.0)]);
        let recipe = Recipe { kernel: "occt".into(), op: "fillet".into(), params: serde_json::Value::Null, digest: digest(d.cad.as_ref().unwrap(), &[2]) };
        d.cad.as_mut().unwrap().append(stored(3, recipe, vec![2], packed_from(&source), Attach::Join)).unwrap();
        d.cad.as_mut().unwrap().features[1].operation = Operation::Box { size: [4.0, 6.0, 3.0] };
        let e = evaluate(&d, &lib, params()).unwrap();
        let r = e.features.iter().find(|r| r.id == 3).unwrap();
        assert!(r.status.is_ok(), "{:?}", r.status);
        assert_eq!(r.notes, ["#2 Plate changed after OpenCascade made this mesh; it stands as made until it runs again where OpenCascade is"]);
        assert_eq!(e.components.len(), 1);
    }

    #[test]
    fn an_import_with_no_source_stands_by_its_own_placement() {
        let source = part(&court(vec![plate(90.0)]), 2);
        let mut import = stored(3, Recipe { kernel: "occt".into(), op: "import".into(), ..Recipe::default() }, vec![], packed_from(&source), Attach::Separate);
        import.component.placement = Placement::ring(90.0, 0.0);
        let c = part(&court(vec![import.clone()]), 3);
        assert_eq!(c.frame, source.frame, "seated like the plate the mesh was taken from");
        let d = court(vec![import]);
        assert_eq!(d.cad.as_ref().unwrap().feature(3).unwrap().operation.label(), "Imported solid");
    }
}
