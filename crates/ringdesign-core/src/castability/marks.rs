//! Raised marks where a sand pattern leaves a joined or cut part to the bench.

use serde::{Deserialize, Serialize};

use super::{AlphaLibrary, FIELD_NOISE_DEG, FaceClass, RingDesign, analyze_field, classify, draft_angle};
use crate::cad::{Attach, Component, Document, Feature, Operation, Placement, Profile, Stage};
use crate::field::{Blend, FieldContext, Layer, LayerEntry, SeatPadLayer};
use crate::interaction::picking;
use crate::mesh::{Displacer, cross, norm, sub};
use crate::profile::ProfileLoop;
use crate::sketch::{FaceAnchor, Id, Sketch, Workplane};

/// Undercut a dot may add where it stands, past the field's own noise lean, before the field is said to blame it, mm².
const BLAME_MM2: f64 = 0.002;
/// Rows round the ring over a dot's own patch of the field.
const PATCH_THETA: usize = 25;
/// Samples round the section on each of those rows: the reference section's own, where chart `v` is exact.
const PATCH_PROFILE: usize = crate::profile::REFERENCE_PROFILE_STEPS;
/// Surface the patch runs past the dot's rim on either side, mm.
const PATCH_MARGIN_MM: f64 = 0.3;
/// Chart rows the patch reads across the band, over the dot's own reach there.
const PATCH_ROWS_REACH: f64 = 1.5;

/// A raised dot the sand pattern carries where a joined or cut part left to the bench meets the band.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocatingMark {
    pub feature: Id,
    /// The part as the report names it: kind, name and number.
    pub label: String,
    /// `Join` stands a locating mark, `Cut` a drill mark.
    pub attach: Attach,
    pub theta_deg: f64,
    /// Where the part meets the band, along the finger from the band's mid-plane, mm.
    pub foot_across_mm: f64,
    /// Where the dot stands, along the finger from the band's mid-plane, mm.
    pub across_mm: f64,
    /// Chart `v` of the dot, mm.
    pub v_mm: f64,
    pub diameter_mm: f64,
    /// Stood on the parting line because the field blamed the dot at the foot.
    pub on_parting_line: bool,
    /// Undercut a dot at the foot added to its patch of the field, mm².
    pub foot_undercut_mm2: f64,
    /// Worst draft over that patch with the dot at the foot, degrees.
    pub foot_worst_deg: f64,
    /// The poured part whose face carries the dot, where the part left to the bench stands on it rather than on the band.
    #[serde(default)]
    pub on: Option<Id>,
    /// That part as the report names it.
    #[serde(default)]
    pub on_label: String,
}

impl LocatingMark {
    /// The layer the dot stands in the pattern as.
    pub fn layer_name(&self) -> String {
        match self.attach {
            Attach::Cut => format!("Drill mark: {}", self.label),
            _ => format!("Locating mark: {}", self.label),
        }
    }

    /// The report's line for the mark.
    pub fn note(&self) -> String {
        let (who, what, centre, at) = match self.attach {
            Attach::Cut => ("is drilled at the bench", "drill mark", "hole's centre", "at its centre"),
            _ => ("is soldered on after the pour", "locating mark", "part's foot", "at its foot"),
        };
        if self.on.is_some() {
            format!("{} {who}: the pattern carries a raised {what} on {}'s face {at}, {:.0}°, {:.1} mm across.", self.label, self.on_label, self.theta_deg, self.diameter_mm)
        } else if self.on_parting_line {
            let off = self.foot_across_mm - self.across_mm;
            format!(
                "{} {who}: {what} on the parting line at {:.0}°; the {centre} is {:.1} mm toward the {} edge (a dot there leaned to {:.0}° over {:.2} mm²).",
                self.label,
                self.theta_deg,
                off.abs(),
                if off >= 0.0 { "high" } else { "low" },
                self.foot_worst_deg,
                self.foot_undercut_mm2
            )
        } else {
            format!("{} {who}: the pattern carries a raised {what} {at}, {:.0}°, {:.1} mm across.", self.label, self.theta_deg, self.diameter_mm)
        }
    }

    /// The dot as a layer: a bare seat pad carrying only its mark, in metal millimetres.
    fn entry(&self) -> LayerEntry {
        let pad = SeatPadLayer {
            theta_deg: self.theta_deg,
            v_mm: self.v_mm,
            diameter_mm: self.diameter_mm,
            height_mm: 0.0,
            crown: 0.0,
            blend_mm: 0.0,
            mark_mm: self.diameter_mm,
            metal_true: true,
            ..SeatPadLayer::default()
        };
        let mut e = LayerEntry::new(self.layer_name(), Layer::SeatPad(pad));
        e.blend = Blend::Add;
        e
    }
}

/// A part as the report names it: `Cylinder "Post" (#3)`, by the kind of body it is.
pub(super) fn label(doc: &Document, id: Id) -> String {
    match doc.features.iter().find(|f| f.id == id) {
        Some(f) => format!("{} \"{}\" (#{id})", kind(doc, id, 0), f.name),
        None => format!("Part #{id}"),
    }
}

/// The kind of body a feature is, read through what only places, reshapes or joins it to the band.
fn kind(doc: &Document, id: Id, depth: usize) -> &'static str {
    let Some(f) = doc.features.iter().find(|f| f.id == id) else { return "Part" };
    let band = doc.band();
    let source = match &f.operation {
        Operation::Boolean { a, b, .. } if band.is_some_and(|x| x == *a || x == *b) => Some(if band == Some(*a) { *b } else { *a }),
        Operation::Transform { source, .. } | Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } => Some(*source),
        _ => None,
    };
    match source {
        Some(s) if depth < 16 => kind(doc, s, depth + 1),
        _ => f.operation.label(),
    }
}

/// Stand a dot in `pattern` for every joined or cut part of `design` staged for the bench, and say where each went.
pub(super) fn place(design: &RingDesign, pattern: &mut RingDesign, lib: &AlphaLibrary) -> Vec<LocatingMark> {
    let Some(doc) = &design.cad else { return Vec::new() };
    let bench: Vec<(Id, Attach)> = doc
        .attachments()
        .into_iter()
        .filter(|(_, a, s)| *s == Stage::Bench && *a != Attach::Separate)
        .map(|(id, a, _)| (id, a))
        .collect();
    if bench.is_empty() {
        return Vec::new();
    }
    let settings = pattern.draft;
    let parting = if settings.auto_parting {
        analyze_field(pattern, lib, &settings, 96, 64).parting_z_mm
    } else if settings.parting_z_mm.is_finite() {
        settings.parting_z_mm
    } else {
        0.0
    };
    let min_draft = settings.min_draft_deg.max(0.0);
    let mut evaluated = None;
    let mut marks = Vec::new();
    // Dots already standing on parts' faces: the part, where on its face, and how wide.
    let mut dots: Vec<(Id, [f64; 2], f64)> = Vec::new();
    for (id, attach) in bench {
        if let Some(mark) = on_part(doc, id, attach, design, pattern, lib, &mut evaluated, &mut dots) {
            marks.push(mark);
            continue;
        }
        let Some(foot) = foot_of(doc, id, design, lib, &mut evaluated) else { continue };
        let mut mark = LocatingMark {
            feature: id,
            label: label(doc, id),
            attach,
            theta_deg: foot.theta_deg,
            foot_across_mm: foot.across_mm,
            across_mm: foot.across_mm,
            v_mm: picking::v_at(pattern, lib, foot.theta_deg, PATCH_PROFILE, foot.radius_mm, foot.across_mm),
            diameter_mm: (0.4 * foot.width_mm).clamp(0.6, 1.0),
            on_parting_line: false,
            foot_undercut_mm2: 0.0,
            foot_worst_deg: 0.0,
            on: None,
            on_label: String::new(),
        };
        (mark.foot_undercut_mm2, mark.foot_worst_deg) = added_undercut(pattern, lib, &mark, parting, min_draft);
        if mark.foot_undercut_mm2 > BLAME_MM2 && mark.foot_worst_deg < -FIELD_NOISE_DEG {
            mark.on_parting_line = true;
            mark.across_mm = parting;
            mark.v_mm = picking::v_at(pattern, lib, foot.theta_deg, PATCH_PROFILE, None, parting);
        }
        marks.push(mark);
    }
    pattern.layers.layers.extend(marks.iter().filter(|m| m.on.is_none()).map(LocatingMark::entry));
    marks
}

/// Height of a dot on a part's face over the face, as a share of its width: the band's dot's.
const DOT_RISE: f64 = 0.25;
/// How far a dot on a part's face sinks into it, so the two meet in metal rather than on a plane, mm.
const DOT_SINK_MM: f64 = 0.05;
/// Draft on the wall of a dot standing on a part's face, degrees.
const DOT_DRAFT_DEG: f64 = 30.0;

/// The mark for bench part `id` when its stone stands on the face of a part the pattern pours joined: a dot extruded from that
/// face under the stone's axis, added to `pattern`'s parts once for every part standing there; `None` for a part on the band.
#[allow(clippy::too_many_arguments)]
fn on_part(
    doc: &Document,
    id: Id,
    attach: Attach,
    design: &RingDesign,
    pattern: &mut RingDesign,
    lib: &AlphaLibrary,
    evaluated: &mut Option<Option<crate::cad::Evaluated>>,
    dots: &mut Vec<(Id, [f64; 2], f64)>,
) -> Option<LocatingMark> {
    let (_, part, seat) = crate::cad::face_stone(doc, id)?;
    let poured = pattern.cad.as_ref()?.attachments().into_iter().any(|(p, a, s)| p == part && a == Attach::Join && s == Stage::Cast);
    if !poured {
        return None;
    }
    let e = evaluated.get_or_insert_with(|| evaluate(design, lib)).as_ref()?;
    let host = e.components.iter().find(|c| c.id == part)?;
    let face = seat.face_of(host).ok()?;
    let foot = seat.foot(&face, &host.frame);
    let on_face = |p: [f64; 3]| {
        let d = sub(p, face.origin);
        let along = |a: [f64; 3]| d[0] * a[0] + d[1] * a[1] + d[2] * a[2];
        [along(face.x), along(face.y)]
    };
    let at = on_face(foot);
    // The bench part's reach across the face, in the face's own plane.
    let width = e.components.iter().find(|c| c.id == id).map_or(2.0, |c| {
        let (lo, hi) = c.mesh.vertices.iter().map(|v| on_face([f64::from(v.0), f64::from(v.1), f64::from(v.2)])).fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| {
            ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
        });
        (hi[0] - lo[0]).min(hi[1] - lo[1]).max(0.0)
    });
    // Two parts at one stone share its dot.
    let shared = dots.iter().find(|(p, q, d)| *p == part && (q[0] - at[0]).hypot(q[1] - at[1]) < 0.5 * d).map(|(_, _, d)| *d);
    let diameter_mm = shared.unwrap_or((0.4 * width).clamp(0.6, 1.0));
    let theta_deg = foot[1].atan2(foot[0]).to_degrees().rem_euclid(360.0);
    let mark = LocatingMark {
        feature: id,
        label: label(doc, id),
        attach,
        theta_deg,
        foot_across_mm: foot[2],
        across_mm: foot[2],
        v_mm: picking::v_at(pattern, lib, theta_deg, PATCH_PROFILE, None, foot[2]),
        diameter_mm,
        on_parting_line: false,
        foot_undercut_mm2: 0.0,
        foot_worst_deg: 0.0,
        on: Some(part),
        on_label: label(doc, part),
    };
    if shared.is_some() {
        return Some(mark);
    }
    let plane =Workplane { origin: [at[0], at[1], -DOT_SINK_MM], on_face: Some(FaceAnchor { feature: part, face: seat.face.clone() }), ..Workplane::default() };
    let sketch = Sketch { name: "Mark".into(), plane, ..Sketch::circle(0.5 * diameter_mm) };
    let operation = Operation::Extrude { sketch: Profile::Inline(sketch), height_mm: DOT_SINK_MM + DOT_RISE * diameter_mm, draft_deg: DOT_DRAFT_DEG };
    let doc = pattern.cad.as_mut()?;
    let component = Component { attach: Attach::Join, stage: Stage::Cast, ..Component::default() };
    doc.append(Feature { id: doc.fresh_id(), name: mark.layer_name(), enabled: true, operation, component }).ok()?;
    dots.push((part, at, diameter_mm));
    Some(mark)
}

/// Where a part meets the band and how wide it stands there.
struct Foot {
    theta_deg: f64,
    across_mm: f64,
    /// The radius to find the surface nearest; `None` drops radially onto it at `across_mm`.
    radius_mm: Option<f64>,
    width_mm: f64,
}

/// Where a part meets the band: its ring placement, else its evaluated body's centroid.
fn foot_of(doc: &Document, id: Id, design: &RingDesign, lib: &AlphaLibrary, evaluated: &mut Option<Option<crate::cad::Evaluated>>) -> Option<Foot> {
    let body = |evaluated: &mut Option<Option<crate::cad::Evaluated>>| -> Option<([f64; 3], f64, f64)> {
        let e = evaluated.get_or_insert_with(|| evaluate(design, lib)).as_ref()?;
        let c = e.components.iter().find(|c| c.id == id)?;
        let centroid = centroid(&c.mesh)?;
        let theta = centroid[1].atan2(centroid[0]);
        let (sin, cos) = theta.sin_cos();
        let (mut t, mut z) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
        for v in &c.mesh.vertices {
            let along = -sin * v.0 as f64 + cos * v.1 as f64;
            t = (t.0.min(along), t.1.max(along));
            z = (z.0.min(v.2 as f64), z.1.max(v.2 as f64));
        }
        Some((centroid, theta.to_degrees().rem_euclid(360.0), (t.1 - t.0).min(z.1 - z.0).max(0.0)))
    };
    if let Some((theta_deg, across_mm, width)) = ring_seat(doc, id, 0) {
        let width_mm = width.or_else(|| body(evaluated).map(|b| b.2)).unwrap_or(2.0);
        return Some(Foot { theta_deg, across_mm, radius_mm: None, width_mm });
    }
    let (c, theta_deg, width_mm) = body(evaluated)?;
    Some(Foot { theta_deg, across_mm: c[2], radius_mm: Some(c[0].hypot(c[1])), width_mm })
}

/// A part's ring placement, followed through what reshapes it or joins it to the band, and its primitive's foot width.
fn ring_seat(doc: &Document, id: Id, depth: usize) -> Option<(f64, f64, Option<f64>)> {
    if depth > 16 {
        return None;
    }
    let f = doc.features.iter().find(|f| f.id == id)?;
    if let Placement::Ring { theta_deg, across_mm, .. } = f.component.placement {
        return Some((theta_deg, across_mm, primitive_width(&f.operation)));
    }
    let band = doc.band();
    match &f.operation {
        Operation::Boolean { a, b, .. } if band.is_some_and(|x| x == *a || x == *b) => {
            ring_seat(doc, if band == Some(*a) { *b } else { *a }, depth + 1)
        }
        Operation::Fillet { source, .. } | Operation::Chamfer { source, .. } | Operation::Shell { source, .. } => {
            ring_seat(doc, *source, depth + 1)
        }
        _ => None,
    }
}

/// Width of a primitive's foot in its own xy plane, the plane a ring placement lays on the surface.
fn primitive_width(op: &Operation) -> Option<f64> {
    match *op {
        Operation::Cylinder { radius_mm, .. } | Operation::Sphere { radius_mm } => Some(2.0 * radius_mm),
        Operation::Box { size } => Some(size[0].min(size[1])),
        Operation::Torus { major_mm, minor_mm } => Some(2.0 * (major_mm + minor_mm)),
        _ => None,
    }
}

/// The design's CAD parts, placed without a built surface: enough to find where each stands.
fn evaluate(design: &RingDesign, lib: &AlphaLibrary) -> Option<crate::cad::Evaluated> {
    let never = std::sync::atomic::AtomicBool::new(false);
    let params = crate::BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..crate::BuildParams::default() };
    crate::cad::evaluate_with(design, lib, params, &crate::cad::BuildCtx::new(&never)).ok()
}

/// Area-weighted centroid of a mesh's surface.
fn centroid(mesh: &crate::Mesh) -> Option<[f64; 3]> {
    let (mut sum, mut total) = ([0.0; 3], 0.0);
    for f in &mesh.faces {
        let Some((a, b, c)) = mesh.triangle(f) else { continue };
        let area = norm(cross(sub(b, a), sub(c, a))) * 0.5;
        if !area.is_finite() {
            continue;
        }
        for k in 0..3 {
            sum[k] += area * (a[k] + b[k] + c[k]) / 3.0;
        }
        total += area;
    }
    (total > 1e-12).then(|| sum.map(|s| s / total))
}

/// Undercut a dot adds to its patch of the field, sampled with it and muted, and the worst draft over the patch with it.
fn added_undercut(pattern: &mut RingDesign, lib: &AlphaLibrary, mark: &LocatingMark, parting_z: f64, min_draft: f64) -> (f64, f64) {
    pattern.layers.layers.push(mark.entry());
    let ctx = pattern.field_context();
    let reference = pattern.reference_loop();
    let reach_mm = 0.5 * mark.diameter_mm + PATCH_MARGIN_MM;
    let step = 2.0 * (reach_mm / ctx.crest_radius_mm.max(1.0)).to_degrees() / (PATCH_THETA - 3) as f64;
    let mid = 0.5 * (PATCH_THETA - 1) as f64;
    let loops: Vec<(f64, ProfileLoop)> = (0..PATCH_THETA)
        .map(|i| {
            let theta = mark.theta_deg + (i as f64 - mid) * step;
            (theta, pattern.section_at(theta, PATCH_PROFILE, None, Some(&reference)))
        })
        .collect();
    let n = loops[0].1.pts.len();
    let mut out = (0.0, 0.0);
    if n >= 3 && loops.iter().all(|(_, l)| l.pts.len() == n) {
        // The rows within reach of the dot across the band, in chart millimetres at its station.
        let reach_v = PATCH_ROWS_REACH * reach_mm / ctx.station_stretch(mark.theta_deg).max(0.25);
        let rows: Vec<usize> = (0..n)
            .filter(|&j| {
                loops.iter().any(|(_, l)| {
                    let p = &l.pts[j];
                    p.surface && (p.v_mm / l.surface_len_mm.max(1e-9) * ctx.band_v_len_mm - mark.v_mm).abs() <= reach_v
                })
            })
            .collect();
        let with = patch_undercut(pattern, lib, &ctx, &loops, &rows, parting_z, min_draft);
        if let Some(e) = pattern.layers.layers.last_mut() {
            e.enabled = false;
        }
        let without = patch_undercut(pattern, lib, &ctx, &loops, &rows, parting_z, min_draft);
        out = ((with.0 - without.0).max(0.0), with.1);
    }
    pattern.layers.layers.pop();
    out
}

/// Undercut area and worst draft over `rows` of the inner sections of `loops`, displaced through `d`'s stack.
fn patch_undercut(
    d: &RingDesign,
    lib: &AlphaLibrary,
    ctx: &FieldContext,
    loops: &[(f64, ProfileLoop)],
    rows: &[usize],
    parting_z: f64,
    min_draft: f64,
) -> (f64, f64) {
    let n = loops[0].1.pts.len();
    let displacer = Displacer { stack: &d.layers, ctx, lib, soften_mm: 0.0, inner_r: d.inner_radius_mm(), min_wall: d.build.min_wall_mm.max(0.05) };
    // Every row a normal reads: the patch's and one either side of it.
    let mut need: Vec<usize> = rows.iter().flat_map(|&j| [(j + n - 1) % n, j, (j + 1) % n]).collect();
    need.sort_unstable();
    need.dedup();
    let pts: Vec<Vec<[f64; 3]>> = loops
        .iter()
        .map(|(theta, l)| {
            let u = ctx.u_of_theta(*theta);
            let (sin, cos) = theta.to_radians().sin_cos();
            let mut row = vec![[f64::NAN; 3]; n];
            for &j in &need {
                let q = displacer.at(&l.pts[j], l.surface_len_mm.max(1e-9), u);
                row[j] = [q.r * cos, q.r * sin, q.z];
            }
            row
        })
        .collect();
    let (mut area, mut worst) = (0.0, 0.0f64);
    for i in 1..loops.len() - 1 {
        for &j in rows {
            if !loops[i].1.pts[j].surface {
                continue;
            }
            let x = cross(sub(pts[i + 1][j], pts[i - 1][j]), sub(pts[i][(j + 1) % n], pts[i][(j + n - 1) % n]));
            let len = norm(x);
            let z = pts[i][j][2];
            if !(len > 1e-12) || !z.is_finite() {
                continue;
            }
            let nz = x[2] / len;
            let draft = draft_angle([0.0, (1.0 - nz * nz).max(0.0).sqrt(), nz], z, parting_z);
            if classify(draft, min_draft) == FaceClass::Undercut {
                area += len * 0.25;
                worst = worst.min(draft);
            }
        }
    }
    (area, worst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::Component;
    use crate::templates;

    /// A 2 mm post on the Court band at 6 mm, `across` along the finger at the top, staged for the bench.
    fn bench_post(across: f64) -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        d.profile.width_mm = 6.0;
        let mut doc = Document::default();
        let feature = |id, name: &str, operation, component| crate::cad::Feature { id, name: name.into(), enabled: true, operation, component };
        doc.append(feature(0, "Procedural shank", Operation::Band, Component::default())).unwrap();
        let placement = Placement::Ring { theta_deg: 90.0, across_mm: across, height_mm: 0.6, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let post = Component { attach: Attach::Join, stage: Stage::Bench, placement, ..Default::default() };
        doc.append(feature(3, "Post", Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, post)).unwrap();
        d.cad = Some(doc);
        d
    }

    /// The Court band with a 4 x 6 x 1.5 mm plate joined on its top, staged `plate`, a 3 mm stone on the plate's top, and its four-claw head and seat left to the bench.
    fn plated_solitaire(plate: Stage) -> RingDesign {
        use crate::cad::{FaceSeat, builders, face_signature, stone_on_face};
        let lib = AlphaLibrary::builtin();
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        let feature = |id, name: &str, operation, component| crate::cad::Feature { id, name: name.into(), enabled: true, operation, component };
        doc.append(feature(1, "Procedural shank", Operation::Band, Component::default())).unwrap();
        let on = Component { attach: Attach::Join, stage: plate, placement: Placement::ring(90.0, 0.65), ..Default::default() };
        doc.append(feature(2, "Plate", Operation::Box { size: [4.0, 6.0, 1.5] }, on)).unwrap();
        d.cad = Some(doc.clone());
        let e = crate::cad::evaluate(&d, &lib, crate::BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() }).unwrap();
        let host = e.components.iter().find(|c| c.id == 2).unwrap();
        let top = (0..host.body.faces.len()).find(|i| face_signature(&host.body, *i, &host.frame).is_some_and(|s| s.normal[2] > 0.99)).unwrap() as u32;
        let gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 3.0);
        let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
        doc.append(stone_on_face(3, gem, 2, &seat)).unwrap();
        let mut next = 3;
        for f in builders::setting_features("claw4", 3, gem, true, &mut || {
            next += 1;
            next
        })
        .unwrap()
        {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }

    #[test]
    fn a_bench_head_on_a_stone_on_a_plate_leaves_its_mark_on_the_plates_top_and_not_in_the_band_under_it() {
        use crate::cad::FaceSeat;
        let lib = AlphaLibrary::builtin();
        let params = crate::BuildParams { theta_steps: 256, profile_steps: 128, ..Default::default() };
        let d = plated_solitaire(Stage::Cast);
        let p = crate::castability::pattern_parts(&d, &lib);
        assert_eq!(p.parts, vec!["Four-claw head".to_string(), "Seat bur".to_string()]);
        // Both marks stand on the plate, and the band carries none.
        assert_eq!(p.marks.iter().map(|m| (m.feature, m.attach, m.on)).collect::<Vec<_>>(), [(4, Attach::Join, Some(2)), (5, Attach::Cut, Some(2))]);
        assert!(p.marks.iter().all(|m| m.diameter_mm == 1.0 && !m.on_parting_line && m.theta_deg == 90.0), "the seat shares its head's dot: {:?}", p.marks);
        assert!(!p.design.layers.layers.iter().any(|e| e.name.contains(" mark: ")), "no dot in the band under the plate");
        let note = p.marks[0].note();
        assert!(note.starts_with("Claw head \"Four-claw head\" (#4) is soldered on after the pour: the pattern carries a raised locating mark on Box \"Plate\" (#2)'s face at its foot, 90°"), "{note}");
        // One dot for the head and its seat alike, extruded from the plate's top face.
        let dots: Vec<&crate::cad::Feature> = p.design.cad.as_ref().unwrap().features.iter().filter(|f| f.name.contains(" mark: ")).collect();
        assert_eq!(dots.len(), 1, "{:?}", dots.iter().map(|f| &f.name).collect::<Vec<_>>());
        let dot_id = dots[0].id;
        assert!(matches!(&dots[0].operation, Operation::Extrude { sketch: crate::cad::Profile::Inline(s), .. } if s.plane.on_face.as_ref().is_some_and(|a| a.feature == 2)));
        // The pattern pours the band, the plate and the dot on it; the stone stands where the finished ring stands it.
        let pattern = crate::mesh::try_build_pattern(&d, &lib, params).unwrap();
        let finished = crate::mesh::try_build(&d, &lib, params).unwrap();
        assert!(pattern.parts.notes.is_empty() && pattern.report.validation.watertight, "{:?} {:?}", pattern.parts.notes, pattern.report.validation);
        assert_eq!((pattern.parts.joined, pattern.parts.cut, pattern.parts.features.clone()), (2, 0, vec![2, dot_id]));
        let e = pattern.parts.evaluated.as_ref().unwrap();
        let part = |b: &crate::mesh::BuildResult, id| b.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == id).unwrap().clone();
        assert_eq!(part(&pattern, 3).frame, part(&finished, 3).frame);
        let Operation::Builder { params: stone, .. } = &d.cad.as_ref().unwrap().feature(3).unwrap().operation else { unreachable!() };
        let seat = FaceSeat::of(stone).unwrap().unwrap();
        let host = part(&pattern, 2);
        let face = seat.face_of(&host).unwrap();
        let foot = seat.foot(&face, &host.frame);
        let dot = e.components.iter().find(|c| c.id == dot_id).unwrap();
        let up = |v: &crate::Vec3| (f64::from(v.0) - face.origin[0]) * face.normal[0] + (f64::from(v.1) - face.origin[1]) * face.normal[1] + (f64::from(v.2) - face.origin[2]) * face.normal[2];
        let d_mm = p.marks[0].diameter_mm;
        let (low, high) = dot.mesh.vertices.iter().map(up).fold((f64::MAX, f64::MIN), |(a, b), h| (a.min(h), b.max(h)));
        assert!((low + DOT_SINK_MM).abs() < 1e-6 && (high - DOT_RISE * d_mm).abs() < 1e-6, "the dot runs {low:.4} to {high:.4} mm off the plate's top");
        let centre = dot.mesh.vertices.iter().fold([0.0; 3], |s, v| [s[0] + f64::from(v.0), s[1] + f64::from(v.1), s[2] + f64::from(v.2)]).map(|s| s / dot.mesh.vertices.len() as f64);
        let off = sub(sub(centre, foot), face.normal.map(|n| n * up(&crate::Vec3(centre[0] as f32, centre[1] as f32, centre[2] as f32))));
        assert!(norm(off) < 1e-3, "the dot stands {:.5} mm off the stone's axis", norm(off));
        // What it adds is the frustum over the plate's top: 1 mm across at its foot, drafted 30° to 0.25 mm high.
        let mut plain = p.design.into_owned();
        plain.cad.as_mut().unwrap().features.retain(|f| f.id != dot_id);
        plain.cad.as_mut().unwrap().outputs.retain(|id| *id != dot_id);
        let bare = crate::mesh::try_build_poured(&plain, &d, &lib, params).unwrap();
        let (t, rise) = (DOT_DRAFT_DEG.to_radians().tan(), DOT_RISE * d_mm);
        let (r0, r1) = (0.5 * d_mm - DOT_SINK_MM * t, 0.5 * d_mm - (DOT_SINK_MM + rise) * t);
        let frustum = std::f64::consts::PI * rise / 3.0 * (r0 * r0 + r0 * r1 + r1 * r1);
        let added = pattern.report.volume_mm3 - bare.report.volume_mm3;
        eprintln!("the dot on the plate adds {added:.4} mm³ against a {frustum:.4} mm³ frustum; the band under the plate is untouched");
        assert!((added / frustum - 1.0).abs() < 0.05, "{added} against {frustum}");
        assert_eq!(crate::cad::surface_epoch(pattern.band.as_deref().unwrap()), crate::cad::surface_epoch(finished.band.as_deref().unwrap()));
        // A plate soldered on after the pour carries nothing in the pattern: its own mark and its head's stand on the band.
        let soldered = plated_solitaire(Stage::Bench);
        let p = crate::castability::pattern_parts(&soldered, &lib);
        assert!(p.marks.len() == 3 && p.marks.iter().all(|m| m.on.is_none()), "{:?}", p.marks);
        assert!(!p.design.cad.as_ref().unwrap().features.iter().any(|f| f.name.contains(" mark: ")));
    }

    /// Relief elsewhere in the dot's own sections is not the dot's lean: the patch is read over the rows the dot reaches, and costs a patch.
    #[test]
    fn a_dots_lean_is_read_where_it_stands_not_across_its_whole_section() {
        let lib = AlphaLibrary::builtin();
        let d = bench_post(0.0);
        let mut far = d.clone();
        let boss = SeatPadLayer { theta_deg: 90.0, v_mm: d.field_context().crest_v_mm - 2.3, diameter_mm: 1.0, height_mm: 0.5, crown: 0.0, blend_mm: 0.0, ..SeatPadLayer::default() };
        far.layers.layers.push(LayerEntry::new("Low boss", Layer::SeatPad(boss)));
        let (near, with_far) = (crate::castability::pattern_parts(&d, &lib).marks, crate::castability::pattern_parts(&far, &lib).marks);
        // Read over its whole section the dot's patch finds the boss leaning back.
        let mut pattern = far.clone();
        pattern.cad = None;
        pattern.layers.layers.push(with_far[0].entry());
        let (ctx, reference) = (pattern.field_context(), pattern.reference_loop());
        let loops: Vec<(f64, ProfileLoop)> = [89.9, 90.0, 90.1].iter().map(|&t| (t, pattern.section_at(t, PATCH_PROFILE, None, Some(&reference)))).collect();
        let all: Vec<usize> = (0..loops[0].1.pts.len()).collect();
        let (boss_area, boss_worst) = patch_undercut(&pattern, &lib, &ctx, &loops, &all, 0.0, pattern.draft.min_draft_deg);
        eprintln!("the dot's sections read whole: {boss_area:.4} mm² at {boss_worst:.1}°; the dot's own patch: {:.1}° bare, {:.1}° beside the boss", near[0].foot_worst_deg, with_far[0].foot_worst_deg);
        assert!(boss_area > 0.0 && boss_worst < -FIELD_NOISE_DEG, "the boss leans in the dot's sections: {boss_worst}");
        // The dot's own patch reads the same with the boss or without it, and the dot stays at the foot.
        assert_eq!((near[0].foot_undercut_mm2, near[0].foot_worst_deg), (with_far[0].foot_undercut_mm2, with_far[0].foot_worst_deg));
        assert!(!near[0].on_parting_line && !with_far[0].on_parting_line);
        // Twice twenty-five sections round the whole profile is what the patch used to read.
        let mut pattern = d.clone();
        pattern.cad = None;
        let mark = near[0].clone();
        let time = |f: &mut dyn FnMut()| {
            f();
            let started = std::time::Instant::now();
            for _ in 0..3 {
                f();
            }
            started.elapsed().as_secs_f64() * 1e3 / 3.0
        };
        let patch_ms = time(&mut || {
            let _ = added_undercut(&mut pattern, &lib, &mark, 0.0, 1.0);
        });
        let whole_ms = time(&mut || {
            for i in 0..2 * PATCH_THETA {
                let _ = crate::castability::section_at_spaced(&d, &lib, 89.0 + i as f64 * 0.04, PATCH_PROFILE, None);
            }
        });
        eprintln!("the dot's patch {patch_ms:.2} ms, its sections whole {whole_ms:.2} ms");
        assert!(patch_ms < 0.5 * whole_ms, "{patch_ms} ms against {whole_ms}");
    }
}
