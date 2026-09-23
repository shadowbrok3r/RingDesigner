//! Raised marks where a sand pattern leaves a joined or cut part to the bench.

use serde::{Deserialize, Serialize};

use super::{AlphaLibrary, FIELD_NOISE_DEG, FaceClass, RingDesign, analyze_field, classify, draft_angle};
use crate::cad::{Attach, Document, Operation, Placement, Stage};
use crate::field::{Blend, FieldContext, Layer, LayerEntry, SeatPadLayer};
use crate::interaction::picking;
use crate::mesh::{Displacer, cross, norm, sub};
use crate::profile::ProfileLoop;
use crate::sketch::Id;

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
        if self.on_parting_line {
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
    for (id, attach) in bench {
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
        };
        (mark.foot_undercut_mm2, mark.foot_worst_deg) = added_undercut(pattern, lib, &mark, parting, min_draft);
        if mark.foot_undercut_mm2 > BLAME_MM2 && mark.foot_worst_deg < -FIELD_NOISE_DEG {
            mark.on_parting_line = true;
            mark.across_mm = parting;
            mark.v_mm = picking::v_at(pattern, lib, foot.theta_deg, PATCH_PROFILE, None, parting);
        }
        marks.push(mark);
    }
    pattern.layers.layers.extend(marks.iter().map(LocatingMark::entry));
    marks
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
