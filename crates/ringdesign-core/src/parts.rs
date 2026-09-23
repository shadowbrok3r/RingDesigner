//! CAD parts resolved into the height-field band, in the stage the seats' solids and the stamps
//! use: the band never enters the kernel. Each output component of the design's CAD document is
//! the kernel's own tessellation, seated on the built surface, and then joined to the band, cut
//! from it, or set beside it through `csg`. Reference parts are stones and never metal. A part
//! asking for a `blend_mm` gets a rolling-ball bead along every seam it shares with the band,
//! joined after the part is, so the fillet is the part's own metal and names it.
use crate::{
    AlphaLibrary, BuildParams, Mesh, RingDesign, Vec3, blend,
    cad::{self, Attach, BuildCtx, FeatureStatus, Memo},
    csg::{self, Op, Parent, Snag, Solid, Traced, P3},
    mesh::{BuildResult, SOLID_VERTEX},
    sketch::Id,
};
use anyhow::{Result, ensure};
use std::sync::atomic::{AtomicBool, Ordering};

/// Boxes closer than this are one cluster of joined parts, united into one tool before the band sees it.
pub const CLUSTER_PAD_MM: f64 = 0.1;
/// Sliver tolerance after each bead is laid, the same the finished mesh is cleaned at.
const CLEAN_MM: f64 = 2e-5;

/// What resolving the CAD parts did to a build.
#[derive(Clone, Debug, Default)]
pub struct Resolved {
    /// Parts united into the band.
    pub joined: usize,
    /// Parts subtracted from the band.
    pub cut: usize,
    /// Parts appended as closed shells of their own.
    pub separate: usize,
    /// Reference parts passed over.
    pub references: usize,
    /// Seam loops beaded with a part's fillet.
    pub beads: usize,
    /// Stations along every bead laid.
    pub bead_stations: usize,
    /// Stations whose radius a guard shrank.
    pub bead_clamped: usize,
    /// Faces the parts account for.
    pub faces: usize,
    pub ms: u128,
    /// What could not be resolved, by part.
    pub notes: Vec<String>,
    /// The first part index a [`Mesh::origin`] value names, after the stones and stamps.
    pub first: u32,
    /// The feature id behind each part index, in the order the origin counts them.
    pub features: Vec<Id>,
    /// The evaluation the parts came from, placed on the built surface, for inspectors that need
    /// the bodies, edges and traces without evaluating again.
    pub evaluated: Option<cad::Evaluated>,
}

impl Resolved {
    /// The feature an origin value names, when it is a part's.
    pub fn feature_of(&self, origin: u32) -> Option<Id> {
        let i = origin.checked_sub(SOLID_VERTEX.checked_add(self.first)?)? as usize;
        self.features.get(i).copied()
    }
    /// The origin value the part at `index` writes.
    pub fn origin_of(&self, index: u32) -> u32 {
        SOLID_VERTEX + self.first + index
    }
}

struct Part {
    index: u32,
    name: String,
    solid: Solid,
    blend_mm: f64,
}


/// The running solid with every vertex named, chained through booleans that pay the band's census once.
struct Chain<'a> {
    solid: Solid,
    origin: Vec<u32>,
    /// Whether `solid` is a combine's own closed output.
    vouched: bool,
    cancel: &'a AtomicBool,
}

impl Chain<'_> {
    fn combine(&self, tool: &Solid, op: Op) -> Result<Traced, Snag> {
        if self.vouched { csg::combine_unchecked(&self.solid, tool, op, Some(self.cancel)) } else { csg::combine_traced(&self.solid, tool, op, Some(self.cancel)) }
    }

    /// Take `t` as the running solid, its new vertices named by the part behind each tool face.
    fn take(&mut self, t: Traced, face_part: &[u32], fallback: u32, base: u32) {
        extend_origin(&mut self.origin, &t, self.solid.v.len(), face_part, fallback, base);
        self.solid = t.solid;
        self.vouched = true;
    }

    /// Every seam loop of `t`, the boolean of the running solid with `tool`, whose part asks for a
    /// fillet, beaded against `t`'s faces; `parts` are the tool's parts and `face_part` the part index
    /// behind each tool face. A bead that fails, or a part whose blend finds no seam long enough to
    /// follow, is a note; a raised flag is the error.
    fn beads<'p>(&self, t: &Traced, tool: &Solid, concave: bool, parts: &[&'p Part], face_part: &[u32], notes: &mut Vec<String>) -> Result<Vec<(&'p Part, blend::Bead)>> {
        let mut out = Vec::new();
        let mut touched = Vec::new();
        for seam in blend::seams(t, &self.solid, tool, concave) {
            let Some(&bf) = seam.b_faces.first() else { continue };
            let index = face_part.get(bf as usize).copied().unwrap_or(parts[0].index);
            let part = parts.iter().copied().find(|p| p.index == index).unwrap_or(parts[0]);
            if part.blend_mm <= 0.0 {
                continue;
            }
            match blend::bead_seam(t, &seam, part.blend_mm, Some(self.cancel)) {
                None => {}
                Some(Ok(bead)) => {
                    touched.push(part.index);
                    out.push((part, bead));
                }
                Some(Err(e)) => {
                    check(self.cancel)?;
                    touched.push(part.index);
                    notes.push(format!("{}: its fillet could not be laid ({e})", part.name));
                }
            }
        }
        for p in parts.iter().filter(|p| p.blend_mm > 0.0 && !touched.contains(&p.index)) {
            notes.push(format!("{}: its fillet found no seam long enough to follow", p.name));
        }
        Ok(out)
    }

    /// Lay each bead into the running solid, a union into a concave junction or a subtraction off a
    /// convex rim, every bead vertex named for its part.
    fn lay(&mut self, beads: Vec<(&Part, blend::Bead)>, op: Op, base: u32, out: &mut Resolved) -> Result<()> {
        for (part, bead) in beads {
            match self.combine(&bead.solid, op) {
                Ok(t) => {
                    let na = self.solid.v.len();
                    self.origin.truncate(na);
                    self.origin.resize(t.solid.v.len(), SOLID_VERTEX + base + part.index);
                    self.solid = t.solid;
                    csg::clean(&mut self.solid, CLEAN_MM);
                    out.beads += 1;
                    out.bead_stations += bead.stations;
                    out.bead_clamped += bead.clamped;
                    if bead.min_radius_mm <= blend::RADIUS_MIN_MM + 1e-9 {
                        out.notes.push(format!("{}: its fillet pinches to {:.2} mm at {} of {} stations", part.name, bead.min_radius_mm, bead.clamped, bead.stations));
                    }
                }
                Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
                Err(e) => out.notes.push(format!("{}: its fillet could not be laid ({e})", part.name)),
            }
        }
        Ok(())
    }
}

/// [`resolve_with`] remembering nothing between builds.
pub fn resolve(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams, ctx: &BuildCtx, built: &mut BuildResult) -> Result<Resolved> {
    resolve_with(design, lib, params, ctx, Memo::default(), built)
}

/// Every feature that failed or was skipped, as a note naming it.
fn status_notes(e: &cad::Evaluated) -> Vec<String> {
    e.features
        .iter()
        .filter_map(|r| match &r.status {
            FeatureStatus::Failed(message) => Some(format!("Feature #{} — {}: {message}", r.id, r.name)),
            FeatureStatus::Skipped(why) => Some(format!("Feature #{} — {}: skipped, {why}", r.id, r.name)),
            _ => None,
        })
        .collect()
}

/// Evaluate the design's CAD parts against the built band and resolve them into it: joins first,
/// clustered so touching parts become one tool, then cuts, then separate shells appended. A part
/// that will not resolve is left out and said; the flag is read between parts and inside each
/// boolean. Nothing happens when the document is the whole ring or the band is empty. A memo's
/// cache is keyed on the built band's [`cad::surface_epoch`].
pub fn resolve_with(design: &RingDesign, lib: &AlphaLibrary, params: BuildParams, ctx: &BuildCtx, memo: Memo, built: &mut BuildResult) -> Result<Resolved> {
    let mut out = Resolved::default();
    let Some(doc) = &design.cad else { return Ok(out) };
    if doc.replaces_band() || built.mesh.faces.is_empty() {
        return Ok(out);
    }
    let clock = crate::mesh::BuildClock::start();
    // Parts are seated on the band as swept, before the seats' solids and stamps.
    let band = built.band.clone();
    let surface: &Mesh = band.as_deref().unwrap_or(&built.mesh);
    let memo = if memo.cache.is_some() { memo.with_epoch(cad::surface_epoch(surface)) } else { memo };
    let evaluated = cad::evaluate_memo(design, lib, params, &BuildCtx::new(ctx.cancel).with_surface(surface), memo)?;
    out.notes = status_notes(&evaluated);
    out.first = (built.solids.paths.len() + design.stamps.len()) as u32;
    let (mut joins, mut cuts, mut separates) = (Vec::new(), Vec::new(), Vec::new());
    for c in &evaluated.components {
        if c.settings.reference {
            out.references += 1;
            continue;
        }
        let part = Part {
            index: out.features.len() as u32,
            name: c.name.clone(),
            solid: Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() },
            blend_mm: if c.settings.blend_mm.is_finite() { c.settings.blend_mm.max(0.0) } else { 0.0 },
        };
        out.features.push(c.id);
        match c.attach {
            Attach::Join => joins.push(part),
            Attach::Cut => cuts.push(part),
            Attach::Separate => separates.push(part),
        }
    }
    out.evaluated = Some(evaluated);
    if joins.is_empty() && cuts.is_empty() && separates.is_empty() {
        out.ms = clock.ms();
        return Ok(out);
    }
    let cancel = Some(ctx.cancel);
    let mesh = &built.mesh;
    let n0 = mesh.vertices.len();
    let mut chain = Chain {
        solid: Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() },
        origin: if mesh.origin.len() == n0 { mesh.origin.clone() } else { (0..n0 as u32).collect() },
        vouched: false,
        cancel: ctx.cancel,
    };
    let base = out.first;
    let refs: Vec<&Solid> = joins.iter().map(|p| &p.solid).collect();
    for group in csg::cluster(&refs, CLUSTER_PAD_MM) {
        check(ctx.cancel)?;
        let names = || group.iter().map(|g| joins[*g].name.as_str()).collect::<Vec<_>>().join(", ");
        match tool_of(&joins, &group, cancel) {
            Ok((tool, face_part)) => match chain.combine(&tool, Op::Union) {
                Ok(t) => {
                    let parts: Vec<&Part> = group.iter().map(|g| &joins[*g]).collect();
                    let beads = chain.beads(&t, &tool, true, &parts, &face_part, &mut out.notes)?;
                    chain.take(t, &face_part, joins[group[0]].index, base);
                    chain.lay(beads, Op::Union, base, &mut out)?;
                    out.joined += group.len();
                }
                Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
                Err(e) => out.notes.push(format!("{}: could not be joined to the band ({e})", names())),
            },
            Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
            Err(e) => {
                // The cluster would not unite; each part meets the band on its own.
                out.notes.push(format!("{}: could not be united with each other ({e}); joined one by one", names()));
                for g in group {
                    check(ctx.cancel)?;
                    let p = &joins[g];
                    match chain.combine(&p.solid, Op::Union) {
                        Ok(t) => {
                            let own = vec![p.index; p.solid.f.len()];
                            let beads = chain.beads(&t, &p.solid, true, &[p], &own, &mut out.notes)?;
                            chain.take(t, &own, p.index, base);
                            chain.lay(beads, Op::Union, base, &mut out)?;
                            out.joined += 1;
                        }
                        Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
                        Err(e) => out.notes.push(format!("{}: could not be joined to the band ({e})", p.name)),
                    }
                }
            }
        }
    }
    for p in &cuts {
        check(ctx.cancel)?;
        match chain.combine(&p.solid, Op::Subtract) {
            Ok(t) => {
                let own = vec![p.index; p.solid.f.len()];
                let beads = chain.beads(&t, &p.solid, false, &[p], &own, &mut out.notes)?;
                chain.take(t, &own, p.index, base);
                chain.lay(beads, Op::Subtract, base, &mut out)?;
                out.cut += 1;
            }
            Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
            Err(e) => out.notes.push(format!("{}: could not be cut from the band ({e})", p.name)),
        }
    }
    for p in &separates {
        check(ctx.cancel)?;
        let (open, repeated) = p.solid.open_edges();
        if open > 0 || repeated > 0 {
            out.notes.push(format!("{}: left out, {}", p.name, Snag::Unclosed { open, repeated }));
            continue;
        }
        chain.solid.push(&p.solid);
        chain.origin.extend(std::iter::repeat_n(SOLID_VERTEX + base + p.index, p.solid.v.len()));
        out.separate += 1;
    }
    if out.joined + out.cut + out.separate == 0 {
        out.ms = clock.ms();
        return Ok(out);
    }
    let band_normals = swept_normals(mesh);
    let band_faces = mesh.faces.len();
    built.mesh = into_mesh(chain.solid, &band_normals, chain.origin);
    out.faces = built.mesh.faces.len().saturating_sub(band_faces);
    let mesh = &built.mesh;
    let bounds = mesh.bounds().unwrap_or_default();
    let volume = mesh.volume_mm3();
    let r = &mut built.report;
    r.bounds_mm = [(bounds.1.0 - bounds.0.0) as f64, (bounds.1.1 - bounds.0.1) as f64, (bounds.1.2 - bounds.0.2) as f64];
    r.validation = mesh.validate();
    r.volume_mm3 = volume;
    r.surface_area_mm2 = mesh.surface_area_mm2();
    r.metals = crate::metal::metal_table(volume);
    r.quality = mesh.quality();
    out.ms = clock.ms();
    Ok(out)
}

fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), cad::CANCELLED);
    Ok(())
}

/// One cluster of joined parts united into a tool, with the part index behind each tool face.
fn tool_of(parts: &[Part], group: &[usize], cancel: Option<&AtomicBool>) -> Result<(Solid, Vec<u32>), Snag> {
    let first = &parts[group[0]];
    let mut tool = first.solid.clone();
    let mut face_part = vec![first.index; tool.f.len()];
    for &g in &group[1..] {
        let p = &parts[g];
        let t = csg::combine_traced(&tool, &p.solid, Op::Union, cancel)?;
        face_part = t.parent.iter().map(|q| match q { Parent::A(f) => face_part[*f as usize], Parent::B(_) => p.index }).collect();
        tool = t.solid;
    }
    Ok((tool, face_part))
}

/// Every vertex the boolean added after the first `na` names the part whose face uses it; a vertex
/// no tool face reaches names `fallback`.
fn extend_origin(origin: &mut Vec<u32>, t: &Traced, na: usize, face_part: &[u32], fallback: u32, base: u32) {
    let n = t.solid.v.len();
    let mut owner = vec![u32::MAX; n];
    for (f, p) in t.solid.f.iter().zip(&t.parent) {
        if let Parent::B(bf) = p {
            let part = face_part.get(*bf as usize).copied().unwrap_or(fallback);
            for &v in f {
                if v as usize >= na && owner[v as usize] == u32::MAX {
                    owner[v as usize] = part;
                }
            }
        }
    }
    origin.truncate(na);
    origin.extend((na..n).map(|v| SOLID_VERTEX + base + if owner[v] == u32::MAX { fallback } else { owner[v] }));
}

/// The band's normals indexed by swept vertex, which is what a band vertex's origin names.
fn swept_normals(mesh: &Mesh) -> Vec<Vec3> {
    if mesh.origin.len() != mesh.vertices.len() {
        return mesh.normals.clone();
    }
    let n = mesh.origin.iter().filter(|o| **o < SOLID_VERTEX).map(|o| *o as usize + 1).max().unwrap_or(0);
    let mut table = vec![Vec3(0.0, 0.0, 1.0); n];
    for (v, o) in mesh.origin.iter().enumerate() {
        if *o < SOLID_VERTEX {
            if let Some(normal) = mesh.normals.get(v) {
                table[*o as usize] = *normal;
            }
        }
    }
    table
}

/// Turn between neighbouring faces of a part past which its corner normals hold a crease, degrees.
pub(crate) const CREASE_DEG: f64 = 38.0;

/// The resolved solid as a mesh: band vertices keep the normals they were swept with, every
/// vertex a solid made shades from its faces, and every face a solid touches gets corner normals
/// that hold a crease wherever its neighbours turn more than `CREASE_DEG`.
pub(crate) fn into_mesh(mut solid: Solid, band_normals: &[Vec3], origin: Vec<u32>) -> Mesh {
    // Twenty nanometres: a face with no edge and no height under it keeps an area f32 can still hold.
    csg::clean(&mut solid, 2e-5);
    let map = solid.compact();
    let mut from = vec![u32::MAX; solid.v.len()];
    for (old, new) in map.iter().enumerate() {
        if *new != u32::MAX {
            from[*new as usize] = origin[old];
        }
    }
    let n = solid.v.len();
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let unit = |a: P3| {
        let l = dot(a, a).sqrt();
        if l > 1e-300 { [a[0] / l, a[1] / l, a[2] / l] } else { [0.0, 0.0, 1.0] }
    };
    let face_n: Vec<P3> = solid
        .f
        .iter()
        .map(|f| {
            let [a, b, c] = f.map(|i| solid.v[i as usize]);
            unit(cross(sub(b, a), sub(c, a)))
        })
        .collect();
    let angle_at = |f: &[u32; 3], k: usize| {
        let (p, q, r) = (solid.v[f[k] as usize], solid.v[f[(k + 1) % 3] as usize], solid.v[f[(k + 2) % 3] as usize]);
        dot(unit(sub(q, p)), unit(sub(r, p))).clamp(-1.0, 1.0).acos()
    };
    let is_band = |v: u32| from[v as usize] < SOLID_VERTEX;
    // Faces round each vertex a solid made.
    let mut start = vec![0u32; n + 1];
    for f in &solid.f {
        for &v in f {
            if !is_band(v) {
                start[v as usize + 1] += 1;
            }
        }
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut around = vec![0u32; start[n] as usize];
    for (fi, f) in solid.f.iter().enumerate() {
        for &v in f {
            if !is_band(v) {
                around[fill[v as usize] as usize] = fi as u32;
                fill[v as usize] += 1;
            }
        }
    }
    let cos_crease = CREASE_DEG.to_radians().cos();
    let to_v3 = |p: P3| Vec3(p[0] as f32, p[1] as f32, p[2] as f32);
    let mut normals = vec![Vec3(0.0, 0.0, 1.0); n];
    for v in 0..n {
        if is_band(v as u32) {
            normals[v] = band_normals.get(from[v] as usize).copied().unwrap_or(Vec3(0.0, 0.0, 1.0));
        } else {
            let mut sum = [0.0; 3];
            for &fi in &around[start[v] as usize..start[v + 1] as usize] {
                let f = &solid.f[fi as usize];
                let k = f.iter().position(|x| *x as usize == v).unwrap_or(0);
                let w = angle_at(f, k);
                sum = [sum[0] + face_n[fi as usize][0] * w, sum[1] + face_n[fi as usize][1] * w, sum[2] + face_n[fi as usize][2] * w];
            }
            normals[v] = to_v3(unit(sum));
        }
    }
    let mut corner_normals = Vec::new();
    for (fi, f) in solid.f.iter().enumerate() {
        if f.iter().all(|v| is_band(*v)) {
            continue;
        }
        let mine = face_n[fi];
        let corners = std::array::from_fn(|k| {
            let v = f[k];
            if is_band(v) {
                return normals[v as usize];
            }
            let mut sum = [0.0; 3];
            for &gi in &around[start[v as usize] as usize..start[v as usize + 1] as usize] {
                let theirs = face_n[gi as usize];
                if dot(mine, theirs) < cos_crease {
                    continue;
                }
                let g = &solid.f[gi as usize];
                let w = angle_at(g, g.iter().position(|x| *x == v).unwrap_or(0));
                sum = [sum[0] + theirs[0] * w, sum[1] + theirs[1] * w, sum[2] + theirs[2] * w];
            }
            to_v3(unit(if dot(sum, sum) > 1e-20 { sum } else { mine }))
        });
        corner_normals.push((fi as u32, corners));
    }
    Mesh { vertices: solid.v.iter().map(|p| to_v3(*p)).collect(), normals, faces: solid.f, corner_normals, origin: from }
}

/// A CAD-only ring's components as one mesh: parts whose boxes meet are united so a shank and the
/// head standing in it are counted once, parts that touch nothing ride along as their own shells,
/// and a union that will not resolve leaves both shells as they were, said in the notes. Each
/// vertex's origin names its part from index 0, and the evaluation rides in the [`Resolved`].
pub fn assembled(e: cad::Evaluated, cancel: Option<&AtomicBool>) -> Result<(Mesh, Resolved)> {
    let mut out = Resolved { notes: status_notes(&e), ..Default::default() };
    let metal: Vec<&cad::EvaluatedComponent> = e.components.iter().filter(|c| !c.settings.reference).collect();
    out.references = e.components.len() - metal.len();
    if metal.is_empty() {
        anyhow::bail!(e.first_error().unwrap_or_else(|| "The design contains only reference components".into()));
    }
    let solids: Vec<Solid> = metal.iter().map(|c| Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }).collect();
    let refs: Vec<&Solid> = solids.iter().collect();
    let name_of = |i: usize| SOLID_VERTEX + i as u32;
    let mut solid = Solid::default();
    let mut origin: Vec<u32> = Vec::new();
    for group in csg::cluster(&refs, 0.0) {
        let mut tool = solids[group[0]].clone();
        let mut named = vec![name_of(group[0]); tool.v.len()];
        for &g in &group[1..] {
            match csg::combine_traced(&tool, &solids[g], Op::Union, cancel) {
                Ok(t) => {
                    extend_origin(&mut named, &t, tool.v.len(), &vec![g as u32; solids[g].f.len()], g as u32, 0);
                    tool = t.solid;
                }
                Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
                Err(err) => {
                    out.notes.push(format!("{} and {}: overlap counted twice, their union did not resolve ({err})", metal[group[0]].name, metal[g].name));
                    tool.push(&solids[g]);
                    named.extend(std::iter::repeat_n(name_of(g), solids[g].v.len()));
                }
            }
        }
        solid.push(&tool);
        origin.extend(named);
    }
    let mesh = into_mesh(solid, &[], origin);
    out.first = 0;
    out.features = metal.iter().map(|c| c.id).collect();
    out.separate = metal.len();
    out.faces = mesh.faces.len();
    out.evaluated = Some(e);
    Ok((mesh, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Boolean, Component, Document, Feature, Operation, Placement, Stage};
    use crate::templates;

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
    }
    fn template(name: &str) -> RingDesign {
        templates::all().iter().find(|t| t.name == name).unwrap().design()
    }
    fn part(id: Id, name: &str, operation: Operation, attach: Attach, stage: Stage, placement: Placement) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation, component: Component { attach, stage, placement, ..Default::default() } }
    }
    fn cylinder(r: f64, h: f64) -> Operation {
        Operation::Cylinder { radius_mm: r, height_mm: h }
    }
    /// The design's parts on its band: the `Band` anchor first, as the GUI seeds it, then the parts.
    fn with_parts(mut d: RingDesign, features: Vec<Feature>) -> RingDesign {
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        for f in features {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }
    /// A cylinder of height `h` standing on the surface at `theta`: its base is the hit point.
    fn seated(theta: f64, h: f64) -> Placement {
        Placement::ring(theta, h / 2.0)
    }

    #[test]
    fn a_memo_build_is_the_plain_build_and_a_second_one_reuses_every_part() {
        let lib = AlphaLibrary::builtin();
        let d = with_parts(template("Court band"), vec![
            part(1, "bezel", cylinder(3.0, 2.5), Attach::Join, Stage::Cast, Placement::ring(90.0, 1.25)),
            part(2, "pilot", cylinder(1.2, 12.0), Attach::Cut, Stage::Cast, Placement::ring(90.0, 0.0)),
        ]);
        let never = AtomicBool::new(false);
        let plain = crate::mesh::try_build_with(&d, &lib, params(), &never).unwrap();
        let cache = std::sync::Mutex::new(cad::Cache::default());
        let first = crate::mesh::try_build_memo(&d, &lib, params(), &never, Memo::new(&cache)).unwrap();
        assert!(first.mesh.vertices == plain.mesh.vertices && first.mesh.faces == plain.mesh.faces);
        let (hits, misses) = { let c = cache.lock().unwrap(); (c.hits(), c.misses()) };
        assert_eq!(hits, 0);
        assert!(misses > 0);
        let again = crate::mesh::try_build_memo(&d, &lib, params(), &never, Memo::new(&cache)).unwrap();
        assert!(again.mesh.vertices == plain.mesh.vertices && again.mesh.faces == plain.mesh.faces);
        let c = cache.lock().unwrap();
        assert_eq!(c.misses(), misses, "an unchanged design builds nothing twice");
        assert!(c.hits() >= misses, "{} hits for {misses} entries", c.hits());
    }

    #[test]
    fn a_joined_cylinder_is_one_watertight_solid_with_the_band_and_names_its_feature() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let bare = crate::mesh::try_build(&court, &lib, params()).unwrap();
        let d = with_parts(court.clone(), vec![part(1, "bezel", cylinder(3.0, 2.5), Attach::Join, Stage::Cast, seated(90.0, 2.5))]);
        let started = std::time::Instant::now();
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let ms = started.elapsed().as_millis();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.cut, built.parts.separate), (1, 0, 0));
        assert_eq!(built.parts.features, vec![1]);
        assert_eq!(built.parts.first, 0);
        // The band gained the cylinder less what it overlaps: on a low crown at this size under 1 mm³.
        let added = built.report.volume_mm3 - bare.report.volume_mm3;
        let whole = std::f64::consts::PI * 9.0 * 2.5;
        assert!(added > whole - 1.0 && added < whole + 0.5, "added {added:.3} of {whole:.3}");
        // Every vertex names the band or the part, and the part's vertices are where the cylinder stands.
        assert_eq!(built.mesh.origin.len(), built.mesh.vertices.len());
        let own: Vec<usize> = built.mesh.origin.iter().enumerate().filter(|(_, o)| **o >= SOLID_VERTEX).map(|(v, _)| v).collect();
        assert!(!own.is_empty() && own.iter().all(|v| built.mesh.origin[*v] == built.parts.origin_of(0)));
        assert!(own.iter().all(|v| built.mesh.vertices[*v].1 > bare.mesh.bounds().unwrap().1.1 - 3.5), "the part stands over the top");
        assert_eq!(built.parts.feature_of(built.mesh.origin[own[0]]), Some(1));
        assert_eq!(built.parts.feature_of(0), None);
        assert!(built.mesh.origin.iter().any(|o| *o < SOLID_VERTEX));
        assert!(!built.mesh.corner_normals.is_empty());
        assert!(built.parts.faces > 0 && built.parts.ms <= ms);
        // The design still says it is the band, and the field judges it.
        assert!(d.band_is_procedural());
    }

    #[test]
    fn a_cut_pilot_removes_metal_and_a_separate_part_is_its_own_shell() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let bare = crate::mesh::try_build(&court, &lib, params()).unwrap();
        let d = with_parts(court.clone(), vec![part(1, "pilot", cylinder(1.0, 12.0), Attach::Cut, Stage::Cast, Placement::ring(90.0, 0.0))]);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!(built.parts.cut, 1);
        let removed = bare.report.volume_mm3 - built.report.volume_mm3;
        // A 1 mm pilot through 2 mm of band, give or take the crown's curve.
        assert!(removed > 4.0 && removed < 9.0, "removed {removed:.3}");
        assert!(built.mesh.origin.iter().any(|o| *o == built.parts.origin_of(0)));
        // A separate part rides along as a closed shell in the same mesh, counted in its volume.
        let d = with_parts(court.clone(), vec![part(1, "bead", Operation::Sphere { radius_mm: 1.0 }, Attach::Separate, Stage::Cast, Placement::ring(90.0, 3.0))]);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight);
        assert_eq!(built.parts.separate, 1);
        let ball = 4.0 / 3.0 * std::f64::consts::PI;
        let added = built.report.volume_mm3 - bare.report.volume_mm3;
        assert!((added - ball).abs() < 0.1, "added {added:.3} of {ball:.3}");
        let shell: Vec<usize> = built.mesh.origin.iter().enumerate().filter(|(_, o)| **o >= SOLID_VERTEX).map(|(v, _)| v).collect();
        assert!(built.mesh.faces.iter().all(|f| f.iter().all(|v| shell.contains(&(*v as usize))) || f.iter().all(|v| !shell.contains(&(*v as usize)))), "no face bridges the shells");
        // A reference stone is never metal.
        let mut d = d;
        d.cad.as_mut().unwrap().features[1].component.reference = true;
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert_eq!((built.parts.references, built.parts.separate), (1, 0));
        assert!((built.report.volume_mm3 - bare.report.volume_mm3).abs() < 1e-6);
    }

    #[test]
    fn a_bench_part_is_shown_finished_and_left_out_of_a_sand_pattern() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let mut d = with_parts(court, vec![
            part(1, "cast collar", cylinder(2.0, 1.0), Attach::Join, Stage::Cast, Placement::ring(90.0, 0.5)),
            part(2, "soldered post", cylinder(0.8, 3.0), Attach::Join, Stage::Bench, Placement::ring(270.0, 1.5)),
        ]);
        let finished = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert_eq!((finished.parts.joined, finished.parts.features.clone()), (2, vec![1, 2]));
        let p = crate::castability::pattern_parts(&d, &lib);
        assert!(p.layers.is_empty() && p.seats.is_empty() && p.parts == vec!["soldered post".to_string()], "{:?} {:?}", p.layers, p.parts);
        assert_eq!(p.design.cad.as_ref().unwrap().outputs, vec![0, 1], "the anchor and the cast part stay");
        let sand = crate::mesh::try_build_pattern(&d, &lib, params()).unwrap();
        assert_eq!((sand.parts.joined, sand.parts.features.clone()), (1, vec![1]));
        assert!(sand.report.volume_mm3 < finished.report.volume_mm3 - 3.0);
        d.draft.process = crate::castability::CastProcess::LostWax;
        let wax = crate::mesh::try_build_pattern(&d, &lib, params()).unwrap();
        assert_eq!(wax.parts.joined, 2);
        assert!((wax.report.volume_mm3 - finished.report.volume_mm3).abs() < 1e-6);
        // The field verdict judges the band and says what stands on it.
        let report = crate::castability::attributed_field_report(&d, &lib, &d.draft, 96, 64);
        assert!(report.notes.iter().any(|n| n.contains("2 CAD parts") && n.contains("2 joined")), "{:?}", report.notes);
        d.draft.process = crate::castability::CastProcess::SandTwoPart;
        let report = crate::castability::attributed_field_report(&d, &lib, &d.draft, 96, 64);
        assert!(report.notes.iter().any(|n| n.contains("1 CAD part ") && n.contains("1 joined")), "{:?}", report.notes);
        assert_ne!(report.verdict, crate::castability::Verdict::Marginal);
    }

    #[test]
    fn a_union_against_the_band_in_the_document_joins_the_part_and_a_raised_flag_stops_the_build() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let bare = crate::mesh::try_build(&court, &lib, params()).unwrap();
        let mut d = with_parts(court, vec![
            part(2, "bezel", cylinder(3.0, 2.5), Attach::Separate, Stage::Cast, Placement::ring(90.0, 1.25)),
            Feature { id: 3, name: "Union".into(), enabled: true, operation: Operation::Boolean { a: 0, b: 2, kind: Boolean::Union }, component: Component::default() },
        ]);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight);
        assert_eq!((built.parts.joined, built.parts.features.clone()), (1, vec![3]));
        assert!(built.report.volume_mm3 > bare.report.volume_mm3 + 60.0);
        // Joins cluster: two touching collars are one tool, each vertex still names its own feature.
        d.cad.as_mut().unwrap().append(part(4, "collar", cylinder(3.0, 1.0), Attach::Join, Stage::Cast, Placement::ring(90.0, 3.0))).unwrap();
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.features.clone()), (2, vec![3, 4]));
        let named: std::collections::BTreeSet<u32> = built.mesh.origin.iter().filter(|o| **o >= SOLID_VERTEX).copied().collect();
        assert_eq!(named, [built.parts.origin_of(0), built.parts.origin_of(1)].into_iter().collect());
        let stop = AtomicBool::new(true);
        let started = std::time::Instant::now();
        let error = crate::mesh::try_build_with(&d, &lib, params(), &stop).err().unwrap();
        assert_eq!(error.root_cause().to_string(), cad::CANCELLED);
        assert!(started.elapsed().as_millis() < 200, "{:?}", started.elapsed());
    }

    #[test]
    fn a_part_on_a_signet_shoulder_stands_on_the_surface() {
        let lib = AlphaLibrary::builtin();
        let heart = template("Heart signet");
        let d = with_parts(heart, vec![part(1, "boss", cylinder(1.0, 1.0), Attach::Join, Stage::Cast, Placement::ring(45.0, 0.5))]);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight && built.parts.joined == 1, "{:?}", built.parts.notes);
        // The part stands on the shoulder's surface along its normal, its top one cylinder up: the
        // reference-crest anchor buried it 2 mm deep here and the union would have added nothing.
        let mut plain = d.clone();
        plain.cad = None;
        let bare = crate::mesh::try_build(&plain, &lib, params()).unwrap();
        let (hit, normal) = cad::surface_hit(&bare.mesh, 45.0, 0.0).unwrap();
        let along = |v: usize| { let p = built.mesh.vertices[v]; (0..3).map(|k| ([p.0 as f64, p.1 as f64, p.2 as f64][k] - hit[k]) * normal[k]).sum::<f64>() };
        let own: Vec<usize> = built.mesh.origin.iter().enumerate().filter(|(_, o)| **o >= SOLID_VERTEX).map(|(v, _)| v).collect();
        let reach = own.iter().map(|v| along(*v)).fold(f64::MIN, f64::max);
        let foot = own.iter().map(|v| along(*v)).fold(f64::MAX, f64::min);
        assert!((reach - 1.0).abs() < 0.05 && foot > -0.3, "reach {reach:.3}, foot {foot:.3}");
        let added = built.report.volume_mm3 - bare.report.volume_mm3;
        assert!((added - std::f64::consts::PI).abs() < 0.4, "added {added:.3}");
    }

    #[test]
    fn cad_only_rings_build_as_before_and_overlapping_parts_count_once() {
        let lib = AlphaLibrary::builtin();
        for name in cad::examples::NAMES.iter().filter(|n| !cad::examples::design(n).unwrap().band_is_procedural()) {
            let d = cad::examples::design(name).unwrap();
            assert!(!d.band_is_procedural(), "{name}");
            let built = crate::mesh::try_build(&d, &lib, params()).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(built.report.validation.watertight, "{name}: {:?}", built.report.validation);
            assert!(built.report.volume_mm3 > 1.0, "{name}");
            // Every vertex names its part from the first index, and the evaluation rides along.
            let e = built.parts.evaluated.as_ref().unwrap_or_else(|| panic!("{name}: no evaluation"));
            let metal: Vec<Id> = e.components.iter().filter(|c| !c.settings.reference).map(|c| c.id).collect();
            assert_eq!((built.parts.first, &built.parts.features, built.parts.separate), (0, &metal, metal.len()), "{name}");
            assert_eq!(built.parts.references, e.components.len() - metal.len(), "{name}");
            assert_eq!(built.mesh.origin.len(), built.mesh.vertices.len(), "{name}");
            // A part wholly inside another, as the inlay-band's inlay is in its shank, leaves no vertex.
            let named: std::collections::BTreeSet<Id> = built.mesh.origin.iter().filter_map(|o| built.parts.feature_of(*o)).collect();
            assert!(!named.is_empty() && named.iter().all(|id| metal.contains(id)), "{name}: {named:?} of {metal:?}");
            if *name == "inlay-band" {
                assert_eq!(named, [1].into_iter().collect(), "the inlay is swallowed by the shank");
            }
            assert!(built.mesh.origin.iter().all(|o| *o >= SOLID_VERTEX), "{name}: nothing here is band");
            assert!(!built.mesh.corner_normals.is_empty(), "{name}");
        }
        // A ring of parts whose only body fails says which feature, not "reference components".
        let mut d = cad::examples::design("solitaire").unwrap();
        d.cad.as_mut().unwrap().features.retain(|f| f.id == 5);
        d.cad.as_mut().unwrap().outputs.retain(|id| *id == 5);
        d.cad.as_mut().unwrap().joints.clear();
        d.cad.as_mut().unwrap().features[0].component.reference = false;
        d.cad.as_mut().unwrap().features[0].operation = Operation::Fillet { source: 1, edges: vec![cad::EdgeRef::bare(0)], radius_mm: 0.2 };
        let error = crate::mesh::try_build(&d, &lib, params()).err().unwrap().to_string();
        assert!(error.starts_with("Feature #5 — ") && error.contains("#1 is unavailable"), "{error}");
        // Two overlapping boxes as separate components are one solid, not two counted twice.
        let mut doc = Document::default();
        for f in [
            part(1, "a", Operation::Box { size: [4.0; 3] }, Attach::Separate, Stage::Cast, Placement::Free),
            part(2, "b", Operation::Box { size: [4.0; 3] }, Attach::Separate, Stage::Cast, Placement::Free),
            part(3, "b moved", Operation::Transform { source: 2, translation: [2.0, 0.0, 0.0], rotation_deg: [0.0; 3] }, Attach::Separate, Stage::Cast, Placement::Free),
        ] {
            doc.append(f).unwrap();
        }
        let mut d = RingDesign::default();
        d.cad = Some(doc);
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        assert!(built.report.validation.watertight);
        assert!((built.report.volume_mm3 - 96.0).abs() < 0.01, "two 64 mm³ boxes overlapping by 32: {}", built.report.volume_mm3);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
    }

    /// A bezel of radius `r` standing 2.5 mm on the top of the ring, its foot sunk `sink` into the
    /// crown, with a fillet of `blend_mm` at the junction.
    fn bezel(r: f64, sink: f64, blend_mm: f64) -> Feature {
        let mut f = part(1, "bezel", cylinder(r, 2.5), Attach::Join, Stage::Cast, Placement::ring(90.0, 1.25 - sink));
        f.component.blend_mm = blend_mm;
        f
    }
    /// The parts of `d` evaluated on the built band, as csg solids in build order.
    fn placed(d: &RingDesign, lib: &AlphaLibrary, surface: &Mesh) -> Vec<Solid> {
        let never = AtomicBool::new(false);
        let e = cad::evaluate_with(d, lib, params(), &BuildCtx { cancel: &never, surface: Some(surface) }).unwrap();
        e.components.iter().map(|c| Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }).collect()
    }
    fn as_solid(mesh: &Mesh) -> Solid {
        Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() }
    }

    #[test]
    fn a_blend_lays_the_parts_fillet_along_its_seam_and_the_fillet_names_the_part() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        // The bezel the bead spike beads clean: 1.5 mm, its foot 0.5 mm into the crown, one seam loop.
        let plain = crate::mesh::try_build(&with_parts(court.clone(), vec![bezel(1.5, 0.5, 0.0)]), &lib, params()).unwrap();
        // No blend takes no bead path: the join is the part's own, vertex for vertex.
        assert_eq!((plain.parts.beads, plain.parts.bead_stations, plain.parts.bead_clamped), (0, 0, 0));
        let default = crate::mesh::try_build(&with_parts(court.clone(), vec![part(1, "bezel", cylinder(1.5, 2.5), Attach::Join, Stage::Cast, Placement::ring(90.0, 0.75))]), &lib, params()).unwrap();
        assert!(plain.mesh.vertices == default.mesh.vertices && plain.mesh.faces == default.mesh.faces && plain.mesh.origin == default.mesh.origin);
        let started = std::time::Instant::now();
        let built = crate::mesh::try_build(&with_parts(court.clone(), vec![bezel(1.5, 0.5, 0.3)]), &lib, params()).unwrap();
        let ms = started.elapsed().as_millis();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.beads, built.parts.bead_clamped), (1, 1, 0));
        assert!(built.parts.bead_stations > 150, "{} stations", built.parts.bead_stations);
        // Under the plane's torus: the crown falls away from the foot, so the junction is blunter on the flanks.
        let fillet = blend::torus_fillet_volume(1.5, 0.3);
        let added = built.report.volume_mm3 - plain.report.volume_mm3;
        eprintln!("beaded bezel on the Court band: {ms} ms, parts {} ms, {} stations, added {added:.4} mm³ of the plane's {fillet:.4}, {} faces", built.parts.ms, built.parts.bead_stations, built.mesh.faces.len());
        assert!(added > 0.25 * fillet && added < fillet, "added {added:.4} of {fillet:.4}");
        // Every vertex is the band's or the bezel's, and the fillet's own vertices are the bezel's, at its foot.
        assert_eq!(built.mesh.origin.len(), built.mesh.vertices.len());
        let named: std::collections::BTreeSet<u32> = built.mesh.origin.iter().filter(|o| **o >= SOLID_VERTEX).copied().collect();
        assert_eq!(named, [built.parts.origin_of(0)].into_iter().collect());
        let bare = crate::mesh::try_build(&court, &lib, params()).unwrap();
        let (hit, normal) = cad::surface_hit(&bare.mesh, 90.0, 0.0).unwrap();
        let up = |v: usize| { let p = built.mesh.vertices[v]; (0..3).map(|k| ([p.0 as f64, p.1 as f64, p.2 as f64][k] - hit[k]) * normal[k]).sum::<f64>() };
        let foot = built.mesh.origin.iter().enumerate().filter(|(v, o)| **o >= SOLID_VERTEX && up(*v) < 0.0).count();
        let plain_foot = plain.mesh.origin.iter().enumerate().filter(|(v, o)| **o >= SOLID_VERTEX && { let p = plain.mesh.vertices[*v]; (0..3).map(|k| ([p.0 as f64, p.1 as f64, p.2 as f64][k] - hit[k]) * normal[k]).sum::<f64>() } < 0.0).count();
        assert!(foot > plain_foot + 500, "the fillet's vertices sit below the crest round the foot: {foot} against {plain_foot}");
        assert!(built.parts.faces > plain.parts.faces);
    }

    #[test]
    fn an_overhanging_bezel_beads_round_its_corners_and_a_tangent_foot_has_no_seam() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        // A 3 mm bezel overhangs the 4 mm band: its seam runs under the disc across each side face and turns four corners.
        let plain = crate::mesh::try_build(&with_parts(court.clone(), vec![bezel(3.0, 0.5, 0.0)]), &lib, params()).unwrap();
        let built = crate::mesh::try_build(&with_parts(court.clone(), vec![bezel(3.0, 0.5, 0.3)]), &lib, params()).unwrap();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.joined, built.parts.beads), (1, 1));
        assert!(built.parts.bead_clamped > 0 && built.parts.bead_clamped < built.parts.bead_stations, "{} of {} clamped", built.parts.bead_clamped, built.parts.bead_stations);
        let added = built.report.volume_mm3 - plain.report.volume_mm3;
        eprintln!("overhanging bezel: {} of {} stations clamped, added {added:.4} mm³", built.parts.bead_clamped, built.parts.bead_stations);
        assert!(added > blend::torus_fillet_volume(3.0, 0.3), "the run under the disc adds more than a torus: {added:.4}");
        let named: std::collections::BTreeSet<u32> = built.mesh.origin.iter().filter(|o| **o >= SOLID_VERTEX).copied().collect();
        assert_eq!(named, [built.parts.origin_of(0)].into_iter().collect());
        // Standing on the crest unsunk, the same bezel touches the band in a point: a 0.0006 mm seam, nothing to fillet.
        let tangent = crate::mesh::try_build(&with_parts(court.clone(), vec![bezel(3.0, 0.0, 0.3)]), &lib, params()).unwrap();
        assert!(tangent.report.validation.watertight);
        assert_eq!((tangent.parts.joined, tangent.parts.beads), (1, 0));
        assert!(tangent.parts.notes.iter().any(|n| n.contains("no seam")), "{:?}", tangent.parts.notes);
    }

    #[test]
    fn a_cut_with_a_blend_rounds_both_rims_of_the_hole() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let pilot = || part(1, "pilot", cylinder(1.0, 12.0), Attach::Cut, Stage::Cast, Placement::ring(90.0, 0.0));
        let plain = crate::mesh::try_build(&with_parts(court.clone(), vec![pilot()]), &lib, params()).unwrap();
        let mut rounded = pilot();
        rounded.component.blend_mm = 0.2;
        let built = crate::mesh::try_build(&with_parts(court.clone(), vec![rounded]), &lib, params()).unwrap();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert!(built.parts.notes.is_empty(), "{:?}", built.parts.notes);
        assert_eq!((built.parts.cut, built.parts.beads), (1, 2), "a rim on the crown and one in the bore");
        // Both rims are sharper than a plane's: the crown falls away from the hole and the comfort bore opens toward the edges.
        let removed = plain.report.volume_mm3 - built.report.volume_mm3;
        let fillet = 2.0 * blend::torus_fillet_volume(1.0, 0.2);
        eprintln!("rounded pilot: removed {removed:.4} mm³ of two plane rims' {fillet:.4}, {} stations, {} clamped", built.parts.bead_stations, built.parts.bead_clamped);
        assert!(removed > fillet && removed < 1.8 * fillet, "removed {removed:.4} of {fillet:.4}");
        assert_eq!(built.parts.bead_clamped, 0);
        let named: std::collections::BTreeSet<u32> = built.mesh.origin.iter().filter(|o| **o >= SOLID_VERTEX).copied().collect();
        assert_eq!(named, [built.parts.origin_of(0)].into_iter().collect());
    }

    #[test]
    fn a_wire_lying_on_the_dome_beads_or_says_it_pinches_and_the_build_stands() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        // A 0.8 mm wire along the ring, sunk 0.05 mm at the top: the wedge under it closes toward both tips.
        let mut wire = part(1, "wire", cylinder(0.4, 3.0), Attach::Join, Stage::Cast, Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 0.35, spin_deg: 0.0, tilt_deg: 90.0, cant_deg: 0.0 });
        wire.component.blend_mm = 0.3;
        let built = crate::mesh::try_build(&with_parts(court, vec![wire]), &lib, params()).unwrap();
        assert!(built.report.validation.watertight, "{:?}", built.report.validation);
        assert_eq!(built.parts.joined, 1);
        eprintln!("lying wire: {} beads, {} of {} clamped, notes {:?}", built.parts.beads, built.parts.bead_clamped, built.parts.bead_stations, built.parts.notes);
        let pinched = built.parts.beads == 1 && built.parts.notes.iter().any(|n| n.contains("pinches"));
        let refused = built.parts.beads == 0 && built.parts.notes.iter().any(|n| n.contains("fillet could not be laid"));
        assert!(pinched || refused, "{} beads, {:?}", built.parts.beads, built.parts.notes);
    }

    #[test]
    fn combine_unchecked_is_combine_with_along_the_join_chain() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let bare = crate::mesh::try_build(&court, &lib, params()).unwrap();
        let band = as_solid(&bare.mesh);
        let tools = placed(&with_parts(court.clone(), vec![bezel(1.5, 0.5, 0.0), part(2, "pilot", cylinder(1.0, 12.0), Attach::Cut, Stage::Cast, Placement::ring(90.0, 0.0))]), &lib, &bare.mesh);
        let (bezel, pilot) = (&tools[0], &tools[1]);
        let checked = csg::combine_with(&band, bezel, Op::Union, None).unwrap();
        let vouched = csg::combine_unchecked(&band, bezel, Op::Union, None).unwrap().solid;
        assert!(checked.v == vouched.v && checked.f == vouched.f);
        let checked2 = csg::combine_with(&checked, pilot, Op::Subtract, None).unwrap();
        let vouched2 = csg::combine_unchecked(&vouched, pilot, Op::Subtract, None).unwrap().solid;
        assert!(checked2.v == vouched2.v && checked2.f == vouched2.f);
        assert_eq!(vouched2.open_edges(), (0, 0));
        // The tool is still checked.
        let mut open = bezel.clone();
        open.f.pop();
        assert!(matches!(csg::combine_unchecked(&vouched, &open, Op::Union, None), Err(Snag::Unclosed { open: 3, .. })));
        let started = std::time::Instant::now();
        let census = band.open_edges();
        eprintln!("census of {} faces: {:?} in {:.2} ms", band.f.len(), census, started.elapsed().as_secs_f64() * 1e3);
        assert_eq!(census, (0, 0));
    }

    #[test]
    fn a_raised_flag_stops_the_build_with_a_bead_in_flight() {
        let lib = AlphaLibrary::builtin();
        // The overhanging bezel: its bead spends most of the build settling and re-sweeping its corners.
        let d = with_parts(template("Court band"), vec![bezel(3.0, 0.5, 0.3)]);
        crate::mesh::try_build(&d, &lib, params()).unwrap();
        let started = std::time::Instant::now();
        crate::mesh::try_build(&d, &lib, params()).unwrap();
        let full = started.elapsed();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let raise = std::thread::spawn({
            let stop = stop.clone();
            move || {
                std::thread::sleep(full / 2);
                stop.store(true, Ordering::Relaxed);
            }
        });
        let started = std::time::Instant::now();
        let result = crate::mesh::try_build_with(&d, &lib, params(), &stop);
        let elapsed = started.elapsed();
        raise.join().unwrap();
        let error = result.err().unwrap_or_else(|| panic!("the build finished in {elapsed:?} with the flag raised at {:?}", full / 2));
        assert_eq!(error.root_cause().to_string(), cad::CANCELLED);
        eprintln!("bead in flight: full build {full:?}, flag at {:?}, stopped {:?} after it", full / 2, elapsed.saturating_sub(full / 2));
        assert!(elapsed < full / 2 + std::time::Duration::from_millis(200), "stopped {:?} after the flag, full build {full:?}", elapsed.saturating_sub(full / 2));
    }

    #[test]
    fn profile_stock_needs_the_band_anchor_and_a_failed_part_is_a_note_on_the_build() {
        use crate::manufacturing::{Setup, prepare};
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let d = with_parts(court.clone(), vec![part(1, "bezel", cylinder(3.0, 2.5), Attach::Join, Stage::Cast, seated(90.0, 2.5))]);
        let stock = Setup { radial_stock_mm: 0.1, component: Some(1), ..Setup::default() };
        // The anchor is the band, so the pattern carries profile stock and the part stays joined.
        let p = prepare(&d, &lib, &stock, params()).unwrap();
        assert!((p.design.profile.thickness_mm - court.profile.thickness_mm - 0.1).abs() < 1e-9);
        assert_eq!(p.design.cad.as_ref().unwrap().outputs, vec![1]);
        let bare = prepare(&court, &lib, &Setup { radial_stock_mm: 0.1, ..Setup::default() }, params()).unwrap();
        assert!(p.build.volume_mm3 > bare.build.volume_mm3 + 60.0, "{} vs {}", p.build.volume_mm3, bare.build.volume_mm3);
        // A ring of parts only has no band to carry stock: refused with stock, prepared without.
        let solo = cad::examples::design("twisted-band").unwrap();
        let error = prepare(&solo, &lib, &Setup { radial_stock_mm: 0.1, ..Setup::default() }, params()).err().unwrap().to_string();
        assert!(error.contains("Profile stock requires a procedural shank"), "{error}");
        assert!(prepare(&solo, &lib, &Setup { radial_stock_mm: 0.0, axial_stock_mm: 0.0, bore_stock_mm: 0.0, ..Setup::default() }, params()).is_ok());
        // A part that fails to build is a note on the resolve, and the band still builds.
        let broken = with_parts(court.clone(), vec![
            part(1, "bezel", cylinder(3.0, 2.5), Attach::Join, Stage::Cast, seated(90.0, 2.5)),
            Feature { id: 2, name: "Fillet".into(), enabled: true, operation: Operation::Fillet { source: 1, edges: vec![cad::EdgeRef::bare(999)], radius_mm: 0.3 }, component: Component::default() },
        ]);
        let built = crate::mesh::try_build(&broken, &lib, params()).unwrap();
        assert_eq!((built.parts.joined, built.parts.features.clone()), (0, vec![]));
        assert_eq!(built.parts.notes, vec!["Feature #2 — Fillet: Edge 999 is unavailable; reselect after changing the source".to_string()]);
        assert!(matches!(built.parts.evaluated.as_ref().unwrap().status_of(2), Some(FeatureStatus::Failed(_))));
        assert!(built.report.validation.watertight);
    }

    #[test]
    fn a_cache_carried_across_builds_answers_the_parts_the_band_did_not_move() {
        let lib = AlphaLibrary::builtin();
        let court = template("Court band");
        let d = with_parts(court.clone(), vec![
            part(1, "bezel", cylinder(3.0, 2.5), Attach::Join, Stage::Cast, seated(90.0, 2.5)),
            part(2, "pilot", cylinder(1.0, 12.0), Attach::Cut, Stage::Cast, Placement::ring(90.0, 0.0)),
        ]);
        let cache = std::sync::Mutex::new(cad::Cache::default());
        let never = AtomicBool::new(false);
        let counts = || { let c = cache.lock().unwrap(); (c.hits(), c.misses()) };
        let build = |d: &RingDesign| {
            let mut built = crate::mesh::try_build(&court, &lib, params()).unwrap();
            let ctx = BuildCtx::new(&never);
            let r = resolve_with(d, &lib, params(), &ctx, Memo::new(&cache), &mut built).unwrap();
            (built, r)
        };
        let (cold, r) = build(&d);
        assert_eq!((r.joined, r.cut), (1, 1));
        assert_eq!(counts(), (0, 4));
        let (warm, r) = build(&d);
        assert_eq!((r.joined, r.cut), (1, 1));
        assert_eq!(counts(), (4, 4));
        assert!(cold.mesh.vertices == warm.mesh.vertices && cold.mesh.origin == warm.mesh.origin);
        // A wider band is a new surface: every ring-placed part is re-seated.
        let mut wider = d.clone();
        wider.profile.width_mm += 0.5;
        let mut plain = wider.clone();
        plain.cad = None;
        let mut built = crate::mesh::try_build(&plain, &lib, params()).unwrap();
        resolve_with(&wider, &lib, params(), &BuildCtx::new(&never), Memo::new(&cache), &mut built).unwrap();
        assert_eq!(counts(), (4, 8));
    }

    #[test]
    fn a_builders_mesh_joins_cuts_and_beads_like_a_kernel_part_and_names_its_feature() {
        use crate::cad::builders;
        use crate::gem::{Gem, GemCut};
        let lib = AlphaLibrary::builtin();
        // A stone whose bezel stands wholly on the 4 mm crown, so its seam is one loop.
        let gem = Gem::calibrated(GemCut::Round, 2.5);
        let mut d = with_parts(template("Court band"), vec![builders::stone_feature(1, gem, Placement::ring(90.0, builders::stand_off_mm("bezel", gem)))]);
        let doc = d.cad.as_mut().unwrap();
        let mut next = 1;
        for f in builders::setting_features("bezel", 1, gem, true, &mut || { next += 1; next }).unwrap() {
            doc.append(f).unwrap();
        }
        let plain = crate::mesh::try_build(&d, &lib, params()).unwrap();
        d.cad.as_mut().unwrap().feature_mut(2).unwrap().component.blend_mm = 0.2;
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        for b in [&plain, &built] {
            assert!(b.report.validation.watertight, "{:?}", b.report.validation);
            assert_eq!((b.parts.joined, b.parts.cut, b.parts.references), (1, 1, 1));
            assert_eq!(b.parts.features, vec![2, 3], "the bezel and its bur, in the order the origin counts them; the stone is never metal");
        }
        // The bezel's seam with the band is beaded, and the bead is the bezel's own metal.
        assert!(built.parts.beads >= 1 && built.report.volume_mm3 > plain.report.volume_mm3, "{} beads, {:?}", built.parts.beads, built.parts.notes);
        let named: std::collections::BTreeSet<Id> = built.mesh.origin.iter().filter_map(|o| built.parts.feature_of(*o)).collect();
        assert_eq!(named, [2, 3].into_iter().collect());
        let bezel = built.mesh.origin.iter().filter(|o| built.parts.feature_of(**o) == Some(2)).count();
        let plain_bezel = plain.mesh.origin.iter().filter(|o| plain.parts.feature_of(**o) == Some(2)).count();
        assert!(bezel > plain_bezel, "the bead's vertices name the bezel: {bezel} against {plain_bezel}");
    }

    /// Timings for the report: `cargo test -p ringdesign-core measured_junctions -- --ignored --nocapture`.
    #[test]
    #[ignore = "timings only"]
    fn measured_junctions() {
        let lib = AlphaLibrary::builtin();
        let sizes = [("preview 256x128", BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }), ("export 1024x384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() })];
        for (label, p) in &sizes {
            for name in ["Court band", "Heart signet"] {
                for (r, blend) in [(1.5, 0.0), (1.5, 0.3), (3.0, 0.0), (3.0, 0.3)] {
                    let d = with_parts(template(name), vec![bezel(r, 0.5, blend)]);
                    let started = std::time::Instant::now();
                    let built = crate::mesh::try_build(&d, &lib, *p).unwrap();
                    let ms = started.elapsed().as_millis();
                    eprintln!("{label:<16} {name:<13} r {r:.1} blend {blend:.1}: build {ms:>5} ms, parts {:>5} ms, {} faces, beads {} ({} stations, {} clamped), open {} non-manifold {}, notes {:?}", built.parts.ms, built.mesh.faces.len(), built.parts.beads, built.parts.bead_stations, built.parts.bead_clamped, built.report.validation.boundary_edges, built.report.validation.non_manifold_edges, built.parts.notes);
                }
            }
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/oriel/design.ring.json");
        let Ok(oriel) = crate::library::load_design(&path) else { eprintln!("Oriel not found at {}", path.display()); return };
        let mut lib = AlphaLibrary::builtin();
        oriel.unpack_embedded(&mut lib);
        oriel.bake_all(&mut lib);
        for (label, p) in &sizes {
            let started = std::time::Instant::now();
            let built = crate::mesh::try_build(&oriel, &lib, *p).unwrap();
            let ms = started.elapsed().as_millis();
            eprintln!("{label:<16} Oriel: build {ms:>5} ms, setting::apply {:>5} ms, {} seats resolved, {} faces, notes {:?}", built.solids.ms, built.solids.resolved, built.mesh.faces.len(), built.solids.notes);
        }
    }

}
