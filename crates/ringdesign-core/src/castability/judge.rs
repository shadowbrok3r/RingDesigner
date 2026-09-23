//! The CAD parts of a built ring, judged against the field verdict's parting plane.

use serde::{Deserialize, Serialize};

use super::{
    AlphaLibrary, BORE_TOL_MM, BoreTrace, CastProcess, DRAG_FRACTION, DraftSettings, FaceClass, FieldReport,
    NOT_CASTABLE_FRACTION, NOT_JUDGED_HERE, RingDesign, VERTICAL_TOL_DEG, Verdict, attributed_field_report, draft_angle, marks,
    read_face,
};
use crate::cad::{Attach, Stage};
use crate::mesh::{BuildResult, Vec3};
use crate::sketch::Id;

/// Undercut on a part under this is below anything sand holds, mm²: reported, never gating.
pub const PART_NOISE_MM2: f64 = 0.005;
/// A part facet reaching this close to the parting plane spans it, mm.
pub(super) const SILHOUETTE_MM: f64 = 0.005;
/// Lean a facet spanning the parting plane may show as its own chord, past half the preview chord's 8.6° step, degrees.
pub(super) const SILHOUETTE_DEG: f64 = 5.0;

/// Whether a facet's lean is its chord's alone: every corner off the parting plane faces into its own mould half.
pub(super) fn chord_lean(corners: [[f64; 3]; 3], normals: [Vec3; 3], parting: f64) -> bool {
    corners.iter().zip(normals).all(|(p, n)| {
        (p[2] - parting).abs() <= SILHOUETTE_MM || draft_angle([f64::from(n.0), f64::from(n.1), f64::from(n.2)], p[2], parting) >= -VERTICAL_TOL_DEG
    })
}

/// One CAD part as the verdict read it off a built ring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PartVerdict {
    pub feature: Id,
    /// Kind, name and number, as the notes name the part.
    pub label: String,
    pub name: String,
    pub attach: Attach,
    pub stage: Stage,
    /// Read against the ring's parting plane; a separate part, and a bench part under sand, are not.
    pub judged: bool,
    /// Every face of the undercut class, as the draft colours paint them.
    pub undercut_area_mm2: f64,
    /// Of that, facets spanning the parting plane that lean less than their own chord: reported, never gating.
    pub silhouette_mm2: f64,
    pub marginal_area_mm2: f64,
    /// Parallel to the pull, outside the bore.
    pub vertical_area_mm2: f64,
    pub total_area_mm2: f64,
    /// Most negative draft on the part outside the bore, degrees; 0 when nothing was read.
    pub worst_draft_deg: f64,
    /// Where the undercut stands, when there is any past the noise.
    pub undercut_at: Option<PartSpan>,
    /// The part's line in the report.
    pub note: String,
}

/// Where on the ring a stretch of a part's faces stands.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartSpan {
    /// Round the ring, degrees; `from > to` runs through 0°.
    pub theta_from_deg: f64,
    pub theta_to_deg: f64,
    /// Along the finger from the band's mid-plane, mm; positive toward the high edge.
    pub across_from_mm: f64,
    pub across_to_mm: f64,
    pub side: PartingSide,
}

/// Which side of the parting plane faces stand on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartingSide {
    Above,
    Below,
    Both,
}

impl PartSpan {
    /// `88–92°`.
    fn theta(&self) -> String {
        format!("{:.0}–{:.0}°", self.theta_from_deg, self.theta_to_deg)
    }

    /// `0.5–2.5 mm toward the high edge`.
    fn across(&self) -> String {
        let (lo, hi) = (self.across_from_mm, self.across_to_mm);
        if lo >= -0.05 {
            format!("{:.1}–{:.1} mm toward the high edge", lo.max(0.0), hi)
        } else if hi <= 0.05 {
            format!("{:.1}–{:.1} mm toward the low edge", (-hi).max(0.0), -lo)
        } else {
            format!("{:.1} mm toward the low edge to {:.1} mm toward the high edge", -lo, hi)
        }
    }
}

/// What one part's faces added up to.
struct Tally {
    judged: bool,
    undercut: f64,
    /// Undercut on facets spanning the parting plane that lean less than their own chord.
    silhouette: f64,
    marginal: f64,
    vertical: f64,
    total: f64,
    worst: f64,
    /// Worst draft over the undercut that locks.
    worst_lock: f64,
    thetas: Vec<f64>,
    across: (f64, f64),
    above: bool,
    below: bool,
}

/// Read the faces of `built`'s poured CAD parts at `field`'s parting plane and fold them into its areas, notes and verdict.
pub fn judge_parts(field: &mut FieldReport, design: &RingDesign, built: &BuildResult) {
    judge_with(field, design, built, design.draft.min_draft_deg);
}

/// [`super::attributed_field_report`], with the CAD parts of `built` judged into it when one is given.
pub fn judged_field_report(
    design: &RingDesign,
    lib: &AlphaLibrary,
    settings: &DraftSettings,
    theta_steps: usize,
    profile_steps: usize,
    built: Option<&BuildResult>,
) -> FieldReport {
    let mut f = attributed_field_report(design, lib, settings, theta_steps, profile_steps);
    if let Some(b) = built {
        judge_with(&mut f, design, b, settings.min_draft_deg);
    }
    f
}

fn severity(v: Verdict) -> u8 {
    match v {
        Verdict::Castable => 0,
        Verdict::Marginal => 1,
        Verdict::NotCastable => 2,
    }
}

fn judge_with(field: &mut FieldReport, design: &RingDesign, built: &BuildResult, min_draft: f64) {
    if !design.band_is_procedural() || !field.parts.is_empty() {
        return;
    }
    let (Some(e), resolved) = (&built.parts.evaluated, &built.parts) else { return };
    if resolved.features.is_empty() {
        return;
    }
    let lost_wax = field.process == CastProcess::LostWax;
    let min_draft = min_draft.max(0.0);
    let parting = field.parting_z_mm;
    let parts: Vec<(Id, String, String, Attach, Stage)> = resolved
        .features
        .iter()
        .map(|&id| {
            let c = e.components.iter().find(|c| c.id == id);
            let name = c.map_or_else(String::new, |c| c.name.clone());
            let label = design.cad.as_ref().map_or_else(|| format!("\"{name}\" (#{id})"), |doc| marks::label(doc, id));
            let (attach, stage) = c.map_or((Attach::Separate, Stage::Cast), |c| (c.attach, c.stage));
            (id, label, name, attach, stage)
        })
        .collect();
    let mut tallies: Vec<Tally> = parts
        .iter()
        .map(|p| Tally {
            judged: p.3 != Attach::Separate && (lost_wax || p.4 == Stage::Cast),
            undercut: 0.0,
            silhouette: 0.0,
            marginal: 0.0,
            vertical: 0.0,
            total: 0.0,
            worst: f64::MAX,
            worst_lock: 0.0,
            thetas: Vec::new(),
            across: (f64::MAX, f64::MIN),
            above: false,
            below: false,
        })
        .collect();

    if tallies.iter().any(|t| t.judged) {
        let mesh = &built.mesh;
        let owners = crate::interaction::pick::part_owners(built);
        let trace = BoreTrace::of(mesh);
        let bore_limit = design.inner_radius_mm().max(0.0) + BORE_TOL_MM;
        let mut cursor = 0;
        for (i, (f, owner)) in mesh.faces.iter().zip(&owners).enumerate() {
            let Some(t) = owner.and_then(|p| tallies.get_mut(p as usize)).filter(|t| t.judged) else { continue };
            let Some(r) = read_face(mesh, f, parting, min_draft, bore_limit, &trace) else { continue };
            t.total += r.area;
            if !r.bore {
                t.worst = t.worst.min(r.draft);
            }
            match r.class {
                FaceClass::Undercut => {
                    t.undercut += r.area;
                    let Some((a, b, c)) = mesh.triangle(f) else { continue };
                    let (lo, hi) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
                    if lo <= parting + SILHOUETTE_MM
                        && hi >= parting - SILHOUETTE_MM
                        && r.draft > -SILHOUETTE_DEG
                        && chord_lean([a, b, c], mesh.face_normals(i, &mut cursor), parting)
                    {
                        t.silhouette += r.area;
                        continue;
                    }
                    t.worst_lock = t.worst_lock.min(r.draft);
                    t.thetas.push(r.centroid[1].atan2(r.centroid[0]).to_degrees().rem_euclid(360.0));
                    t.across = (t.across.0.min(lo), t.across.1.max(hi));
                    if r.centroid[2] >= parting {
                        t.above = true;
                    } else {
                        t.below = true;
                    }
                }
                FaceClass::Marginal => t.marginal += r.area,
                FaceClass::Vertical if !r.bore => t.vertical += r.area,
                _ => {}
            }
        }
    }

    let mut verdicts = Vec::with_capacity(parts.len());
    let mut notes = Vec::new();
    for ((id, label, name, attach, stage), t) in parts.into_iter().zip(tallies) {
        let worst = if t.worst == f64::MAX { 0.0 } else { t.worst };
        let locking = t.undercut - t.silhouette;
        let span = (t.judged && locking >= PART_NOISE_MM2).then(|| {
            let (from, to) = theta_span(&t.thetas);
            let side = match (t.above, t.below) {
                (true, true) => PartingSide::Both,
                (false, true) => PartingSide::Below,
                _ => PartingSide::Above,
            };
            PartSpan { theta_from_deg: from, theta_to_deg: to, across_from_mm: t.across.0, across_to_mm: t.across.1, side }
        });
        let note = if !t.judged && attach == Attach::Separate {
            let n = format!("{label} stands apart from the band: it is cast on its own and not judged against the ring's parting plane.");
            notes.push(n.clone());
            n
        } else if !t.judged {
            format!("{label} is left to the bench: not in the pattern, and not judged.")
        } else if let Some(span) = &span {
            let lean = t.worst_lock;
            let n = if lost_wax {
                format!(
                    "Lost wax: {label} would undercut a two-part pull over {locking:.1} mm² (worst {lean:.0}°) — fine for investment, but this pattern cannot move to sand as-is."
                )
            } else if attach == Attach::Cut {
                let walls = match span.side {
                    PartingSide::Above => "walls facing down above the parting plane",
                    PartingSide::Below => "walls facing up below the parting plane",
                    PartingSide::Both => "walls either side of the parting plane",
                };
                format!(
                    "{label}: {locking:.1} mm² of undercut leaning to {lean:.0}° on the hole's {walls}, {}, {} — drill it at the bench: stage it Bench and the pattern keeps a raised drill mark at its centre.",
                    span.theta(),
                    span.across()
                )
            } else {
                let flank = match span.side {
                    PartingSide::Above => "its drag-facing flank above the parting plane",
                    PartingSide::Below => "its cope-facing flank below the parting plane",
                    PartingSide::Both => "flanks facing back across the parting plane",
                };
                format!(
                    "{label}: {locking:.1} mm² of undercut leaning to {lean:.0}° on {flank}, {}, {} — stage it Bench to solder it on after the pour (the pattern keeps a locating mark), or move it onto the parting line.",
                    span.theta(),
                    span.across()
                )
            };
            notes.push(n.clone());
            n
        } else if t.undercut > 0.0 {
            format!("{label}: {:.1} mm² judged, clean but for {:.4} mm² of facet noise at {worst:.1}°.", t.total, t.undercut)
        } else {
            format!("{label}: {:.1} mm² judged, no undercut.", t.total)
        };
        verdicts.push(PartVerdict {
            feature: id,
            label,
            name,
            attach,
            stage,
            judged: t.judged,
            undercut_area_mm2: t.undercut,
            silhouette_mm2: t.silhouette,
            marginal_area_mm2: t.marginal,
            vertical_area_mm2: t.vertical,
            total_area_mm2: t.total,
            worst_draft_deg: worst,
            undercut_at: span,
            note,
        });
    }

    let judged: Vec<&PartVerdict> = verdicts.iter().filter(|p| p.judged).collect();
    if !judged.is_empty() {
        let sum = |f: fn(&PartVerdict) -> f64| judged.iter().map(|p| f(p)).sum::<f64>();
        let (undercut, marginal, vertical, total) =
            (sum(|p| p.undercut_area_mm2), sum(|p| p.marginal_area_mm2), sum(|p| p.vertical_area_mm2), sum(|p| p.total_area_mm2));
        let band_drag = (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9);
        field.undercut_area_mm2 += undercut;
        field.marginal_area_mm2 += marginal;
        field.vertical_area_mm2 += vertical;
        field.total_area_mm2 += total;
        let worst = judged.iter().map(|p| p.worst_draft_deg).fold(f64::MAX, f64::min);
        field.worst_draft_deg = field.worst_draft_deg.min(worst);
        let count = |a: Attach| judged.iter().filter(|p| p.attach == a).count();
        let n = judged.len();
        let s = if n == 1 { "" } else { "s" };
        if lost_wax {
            notes.insert(0, format!(
                "Measured with the ring as built: {n} CAD part{s} ({} joined, {} cut), {total:.1} mm² of part surface; under lost wax they are reported, never gating.",
                count(Attach::Join),
                count(Attach::Cut)
            ));
        } else {
            let locks = judged.iter().any(|p| p.undercut_at.is_some());
            let locking: f64 = judged.iter().filter(|p| p.undercut_at.is_some()).map(|p| p.undercut_area_mm2 - p.silhouette_mm2).sum();
            let drag = (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9);
            let from_parts = if field.undercut_fraction() > NOT_CASTABLE_FRACTION {
                Verdict::NotCastable
            } else if locks || drag > DRAG_FRACTION {
                Verdict::Marginal
            } else {
                Verdict::Castable
            };
            if severity(from_parts) > severity(field.verdict) {
                field.verdict = from_parts;
            }
            notes.insert(0, format!(
                "Judged with the ring as built: {n} CAD part{s} ({} joined, {} cut) at the parting plane z = {parting:+.2} mm — {total:.1} mm² of part surface, {}.",
                count(Attach::Join),
                count(Attach::Cut),
                if locks { format!("{locking:.1} mm² of it undercut") } else { "none of it locking".to_string() }
            ));
            if drag > DRAG_FRACTION && band_drag <= DRAG_FRACTION {
                notes.push(format!(
                    "With its CAD parts {:.1}% of the ring carries less than {min_draft:.1} deg of draft or stands parallel to the pull — it releases, but drags on the sand.",
                    drag * 100.0
                ));
            }
        }
    }
    field.notes.retain(|n| !n.contains(NOT_JUDGED_HERE));
    field.notes.extend(notes);
    field.parts = verdicts;
}

/// The shortest arc round the ring holding every angle: `(from, to)`, degrees, `from > to` through 0°.
fn theta_span(thetas: &[f64]) -> (f64, f64) {
    let mut t: Vec<f64> = thetas.iter().copied().filter(|v| v.is_finite()).collect();
    if t.is_empty() {
        return (0.0, 0.0);
    }
    t.sort_by(f64::total_cmp);
    let (mut from, mut to) = (t[0], t[t.len() - 1]);
    let mut gap = t[0] + 360.0 - t[t.len() - 1];
    for w in t.windows(2) {
        if w[1] - w[0] > gap {
            gap = w[1] - w[0];
            (from, to) = (w[1], w[0]);
        }
    }
    (from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Component, Document, Feature, Operation, Placement};
    use crate::interaction::bvh::Bvh;
    use crate::mesh::SOLID_VERTEX;
    use crate::{BuildParams, Mesh, templates};

    fn preview() -> BuildParams {
        BuildParams { theta_steps: 384, profile_steps: 144, ..BuildParams::default() }
    }
    /// The Court band at 6 mm, so a 2 mm post 1.5 mm off the mid-plane stands on the crown and not over its edge.
    fn band() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        d.profile.width_mm = 6.0;
        d
    }
    fn with_part(mut d: RingDesign, name: &str, operation: Operation, attach: Attach, stage: Stage, placement: Placement) -> RingDesign {
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 3, name: name.into(), enabled: true, operation, component: Component { attach, stage, placement, ..Default::default() } }).unwrap();
        d.cad = Some(doc);
        d
    }
    const POST_R: f64 = 1.0;
    const POST_H: f64 = 2.0;
    const SINK: f64 = 0.4;
    /// A post standing on the top of the ring `across` along the finger, its foot sunk `SINK` so it seats.
    fn post_at(across: f64) -> Placement {
        Placement::Ring { theta_deg: 90.0, across_mm: across, height_mm: 0.5 * POST_H - SINK, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 }
    }
    fn post(across: f64, stage: Stage) -> RingDesign {
        with_part(band(), "Post", Operation::Cylinder { radius_mm: POST_R, height_mm: POST_H }, Attach::Join, stage, post_at(across))
    }
    /// The same post on the top of the Court band as it ships, 4 mm wide.
    fn court_post() -> RingDesign {
        let court = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        with_part(court, "Post", Operation::Cylinder { radius_mm: POST_R, height_mm: POST_H }, Attach::Join, Stage::Cast, post_at(0.0))
    }
    /// Wall area of a cylinder on `bare` facing as `keep` says, over 720 wall lines, `outside` above the band else below it.
    fn wall_area(d: &RingDesign, bare: &Mesh, placement: &Placement, r: f64, h: f64, outside: bool, keep: impl Fn([f64; 3], [f64; 3]) -> bool) -> f64 {
        let f = placement.frame_on(d, Some(bare)).unwrap();
        let bvh = Bvh::build(bare);
        let n = 720;
        let mut area = 0.0;
        for k in 0..n {
            let phi = (k as f64 + 0.5) / n as f64 * std::f64::consts::TAU;
            let (s, c) = phi.sin_cos();
            let out: [f64; 3] = std::array::from_fn(|i| c * f.x_axis[i] + s * f.y_axis[i]);
            let top: [f64; 3] = std::array::from_fn(|i| f.origin[i] + r * out[i] + 0.5 * h * f.z_axis[i]);
            let down = f.z_axis.map(|v| -v);
            let hit = bvh.ray(bare, top, down).map_or(h, |(_, t)| t.min(h));
            let length = if outside { hit } else { h - hit };
            let mid: [f64; 3] = std::array::from_fn(|i| top[i] + down[i] * (if outside { 0.5 * hit } else { hit + 0.5 * length }));
            let normal = if outside { out } else { out.map(|v| -v) };
            if keep(normal, mid) {
                area += r * std::f64::consts::TAU / n as f64 * length;
            }
        }
        area
    }
    /// Whether a face turned this way at this point leans back over the parting plane at `z`.
    fn locks(normal: [f64; 3], at: [f64; 3], parting: f64) -> bool {
        let pull = if at[2] >= parting { 1.0 } else { -1.0 };
        normal[2] * pull < -super::super::VERTICAL_TOL_DEG.to_radians().sin()
    }

    fn bare() -> (RingDesign, crate::mesh::BuildResult, FieldReport) {
        let lib = AlphaLibrary::builtin();
        let d = band();
        let built = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let field = attributed_field_report(&d, &lib, &d.draft, 192, 128);
        (d, built, field)
    }
    fn judged(d: &RingDesign) -> (crate::mesh::BuildResult, FieldReport) {
        let lib = AlphaLibrary::builtin();
        let built = crate::mesh::try_build(d, &lib, preview()).unwrap();
        let field = judged_field_report(d, &lib, &d.draft, 192, 128, Some(&built));
        (built, field)
    }

    /// (a) Straddling the parting plane a radial post's wall faces away from it on both halves.
    #[test]
    fn a_post_on_the_parting_line_pulls_clean_and_leaves_the_verdict_alone() {
        let lib = AlphaLibrary::builtin();
        let (_, bare_built, bare) = bare();
        assert_eq!((bare.verdict, bare.undercut_area_mm2, bare.parting_z_mm), (Verdict::Castable, 0.0, 0.0));
        let d = post(0.0, Stage::Cast);
        let design_only = attributed_field_report(&d, &lib, &d.draft, 192, 128);
        assert!(design_only.notes.iter().any(|n| n.contains("1 CAD part stands on the band (1 joined") && n.contains(NOT_JUDGED_HERE)), "{:?}", design_only.notes);
        let (built, f) = judged(&d);
        assert_eq!(f.parts.len(), 1);
        let p = &f.parts[0];
        eprintln!("post on the parting line: {:.4} mm² undercut at {:.2}°, {:.3} marginal, {:.3} vertical, {:.3} total", p.undercut_area_mm2, p.worst_draft_deg, p.marginal_area_mm2, p.vertical_area_mm2, p.total_area_mm2);
        assert!(p.judged && p.label == "Cylinder \"Post\" (#3)" && (p.attach, p.stage) == (Attach::Join, Stage::Cast));
        // 0.000 mm²: what is left is a CDT sliver along the plane, 0.00016 mm² at -2.8°, under the noise floor.
        assert!(p.undercut_area_mm2 < 5e-4 && p.undercut_at.is_none(), "{p:?}");
        assert_eq!(f.verdict, bare.verdict);
        assert!((f.undercut_area_mm2 - bare.undercut_area_mm2 - p.undercut_area_mm2).abs() < 1e-12);
        assert!((f.total_area_mm2 - bare.total_area_mm2 - p.total_area_mm2).abs() < 1e-9);
        // The part's surface is its exposed wall and its top, as the geometry says: the wall read along 720 lines.
        let wall = wall_area(&d, &bare_built.mesh, &post_at(0.0), POST_R, POST_H, true, |_, _| true);
        let top = std::f64::consts::PI * POST_R * POST_R;
        assert!((p.total_area_mm2 - (wall + top)).abs() < 0.02 * (wall + top), "{:.3} of {:.3}", p.total_area_mm2, wall + top);
        // The top disc stands square to the ring's radius: a vertical wall, a polygon of it.
        assert!((p.vertical_area_mm2 - top).abs() < 0.05, "{}", p.vertical_area_mm2);
        assert!(!f.notes.iter().any(|n| n.contains(NOT_JUDGED_HERE)), "{:?}", f.notes);
        assert!(f.notes.iter().any(|n| n.starts_with("Judged with the ring as built: 1 CAD part (1 joined, 0 cut)") && n.contains("none of it locking")), "{:?}", f.notes);
        // Judged once: a second pass adds nothing.
        let mut again = f.clone();
        judge_parts(&mut again, &d, &built);
        assert_eq!((again.undercut_area_mm2, again.total_area_mm2, again.notes.len()), (f.undercut_area_mm2, f.total_area_mm2, f.notes.len()));
    }

    /// (b) 1.5 mm off the plane the post's axis leans with the dome and its lower flank faces the drag.
    #[test]
    fn a_post_off_the_parting_line_locks_on_its_drag_flank_and_says_bench_it() {
        let (_, bare_built, bare) = bare();
        let d = post(1.5, Stage::Cast);
        let (_, f) = judged(&d);
        let p = &f.parts[0];
        let expected = wall_area(&d, &bare_built.mesh, &post_at(1.5), POST_R, POST_H, true, |n, at| locks(n, at, f.parting_z_mm));
        let (_, normal) = crate::cad::surface_hit(&bare_built.mesh, 90.0, 1.5).unwrap();
        let lean = normal[2].asin().to_degrees();
        eprintln!("post 1.5 mm off: {:.4} mm² undercut against {expected:.4} from the geometry, worst {:.2}° with the dome leaning {lean:.2}°, {:.2}% of the ring", p.undercut_area_mm2, p.worst_draft_deg, f.undercut_fraction() * 100.0);
        assert!((p.undercut_area_mm2 - expected).abs() < 0.10 * expected, "{} of {expected}", p.undercut_area_mm2);
        assert!(expected > 5.0 && expected < 5.5, "the drag-facing half of a 1 mm post standing 1.6 mm proud: {expected}");
        // The flank's steepest facet faces down the post's own lean off the finger's axis.
        assert!((p.worst_draft_deg + 90.0 - lean).abs() < 5.0, "worst {} with the axis leaning {lean}", p.worst_draft_deg);
        let span = p.undercut_at.unwrap();
        assert_eq!(span.side, PartingSide::Above);
        assert!(span.theta_from_deg < 90.0 && span.theta_to_deg > 90.0 && span.theta_to_deg - span.theta_from_deg < 12.0, "{span:?}");
        assert!((span.across_from_mm - 0.5).abs() < 0.05 && span.across_to_mm > 1.5 && span.across_to_mm < 2.5, "{span:?}");
        // Same thresholds as the face analyzer: past 1% of the ring it will not release, under it it is with care.
        let want = if f.undercut_fraction() > NOT_CASTABLE_FRACTION { Verdict::NotCastable } else { Verdict::Marginal };
        assert_eq!((bare.verdict, f.verdict), (Verdict::Castable, want));
        assert_eq!(f.verdict, Verdict::Marginal, "5.2 mm² is 0.62% of an 845 mm² ring");
        assert!(f.worst_draft_deg <= p.worst_draft_deg);
        let note = f.notes.iter().find(|n| n.starts_with("Cylinder \"Post\" (#3):")).unwrap_or_else(|| panic!("{:?}", f.notes));
        for words in ["mm² of undercut leaning to -", "on its drag-facing flank above the parting plane", "toward the high edge", "stage it Bench to solder it on after the pour", "locating mark", "or move it onto the parting line"] {
            assert!(note.contains(words), "{note:?} lacks {words:?}");
        }
        assert_eq!(&p.note, note);
    }

    /// (c) A bench post is not judged; the pattern marks its foot, or the parting line where a dot at the foot leans.
    #[test]
    fn a_bench_post_leaves_a_locating_mark_where_the_field_lets_it_stand() {
        let lib = AlphaLibrary::builtin();
        let (bare_design, bare_built, bare) = bare();
        let bare_pattern = crate::mesh::try_build_pattern(&bare_design, &lib, preview()).unwrap();
        // On the parting line the dot straddles the plane: each flank faces its own half, nothing leans.
        let d = post(0.0, Stage::Bench);
        let p = crate::castability::pattern_parts(&d, &lib);
        let at_foot = &p.marks[0];
        assert!(!at_foot.on_parting_line && at_foot.foot_undercut_mm2 < 1e-9 && at_foot.across_mm == 0.0, "{at_foot:?}");
        assert!((at_foot.diameter_mm - 0.8).abs() < 1e-12, "0.4 of the 2 mm foot");
        assert!((at_foot.v_mm - bare_design.field_context().crest_v_mm).abs() < 1e-9, "on the crest line");
        // 1.5 mm off it the dot's near flank faces back across the plane on a dome with 16° of draft.
        let d = post(1.5, Stage::Bench);
        let p = crate::castability::pattern_parts(&d, &lib);
        assert_eq!(p.parts, vec!["Post".to_string()]);
        let moved = &p.marks[0];
        eprintln!("mark at the foot 1.5 mm off: {:.4} mm² at {:.2}° — moved to the parting line", moved.foot_undercut_mm2, moved.foot_worst_deg);
        assert!(moved.on_parting_line && moved.foot_undercut_mm2 > 0.1 && moved.foot_worst_deg < -20.0, "{moved:?}");
        assert!((moved.foot_across_mm, moved.across_mm, moved.theta_deg) == (1.5, 0.0, 90.0) && moved.v_mm == at_foot.v_mm, "{moved:?}");
        let layer = p.design.layers.layers.last().unwrap();
        assert_eq!(layer.name, "Locating mark: Cylinder \"Post\" (#3)");
        // The verdict is the bare band's with the dot on it: no part judged, nothing leaning.
        let f = attributed_field_report(&d, &lib, &d.draft, 192, 128);
        assert_eq!((f.verdict, f.undercut_area_mm2), (bare.verdict, bare.undercut_area_mm2));
        assert!((f.total_area_mm2 - bare.total_area_mm2).abs() < 0.5, "{} against {}", f.total_area_mm2, bare.total_area_mm2);
        let note = f.notes.iter().find(|n| n.contains("soldered on after the pour")).unwrap_or_else(|| panic!("{:?}", f.notes));
        assert!(note.contains("locating mark on the parting line at 90°") && note.contains("1.5 mm toward the high edge"), "{note}");
        let finished = crate::mesh::try_build(&d, &lib, preview()).unwrap();
        let judged = judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&finished));
        assert!(!judged.parts[0].judged && judged.parts[0].note.contains("left to the bench"), "{:?}", judged.parts);
        assert_eq!((judged.verdict, judged.undercut_area_mm2, judged.total_area_mm2), (f.verdict, f.undercut_area_mm2, f.total_area_mm2));
        // The sand pattern is the band and the dot: no post, and the dot's metal raised on the crest.
        let pattern = crate::mesh::try_build_pattern(&d, &lib, preview()).unwrap();
        assert!(pattern.parts.features.is_empty() && pattern.mesh.origin.iter().all(|o| *o < SOLID_VERTEX));
        let dot = pattern.report.volume_mm3 - bare_pattern.report.volume_mm3;
        let rise = |m: &Mesh| m.vertices.iter().filter(|v| v.0.abs() < 0.5 && v.2.abs() < 0.5).map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(0.0, f64::max);
        let peak = rise(&pattern.mesh) - rise(&bare_built.mesh);
        eprintln!("the dot in the pattern: {dot:.4} mm³ raised {peak:.4} mm; a raised cosine 0.8 across and 0.2 high is 0.0299 mm³");
        assert!((peak - 0.2).abs() < 0.01, "{peak}");
        assert!(dot > 0.015 && dot < 0.06, "{dot}");
        let dir = std::env::temp_dir().join(format!("rd-parts-verdict-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bytes = crate::stl::write_stl(dir.join("pattern.stl"), &pattern.mesh, "pattern").unwrap();
        assert_eq!(bytes, 84 + 50 * pattern.mesh.faces.len());
        let _ = std::fs::remove_dir_all(&dir);
        // The finished ring shows the post and never the dot.
        assert_eq!(finished.parts.joined, 1);
        assert!(d.layers.layers.is_empty() && finished.report.volume_mm3 > bare_built.report.volume_mm3 + 3.0);
        // A drill mark for a bench cut stands at the hole's centre, on the parting line here.
        let pilot = with_part(band(), "Pilot", Operation::Cylinder { radius_mm: 0.5, height_mm: 1.0 }, Attach::Cut, Stage::Bench, Placement::ring(90.0, 0.0));
        let p = crate::castability::pattern_parts(&pilot, &lib);
        assert!(p.marks.len() == 1 && p.marks[0].attach == Attach::Cut && !p.marks[0].on_parting_line && p.marks[0].diameter_mm == 0.6, "{:?}", p.marks);
        assert!(p.marks[0].note().contains("is drilled at the bench: the pattern carries a raised drill mark at its centre, 90°"), "{}", p.marks[0].note());
    }

    /// A part placed by a transform is found by its body.
    #[test]
    fn a_free_bench_part_is_marked_where_its_body_meets_the_band() {
        let lib = AlphaLibrary::builtin();
        let mut d = band();
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 3, name: "Stock".into(), enabled: true, operation: Operation::Cylinder { radius_mm: POST_R, height_mm: POST_H }, component: Component::default() }).unwrap();
        let placed = Operation::Transform { source: 3, translation: [0.0, d.inner_radius_mm() + d.profile.thickness_mm + 0.5 * POST_H - SINK, 0.0], rotation_deg: [-90.0, 0.0, 0.0] };
        doc.append(Feature { id: 4, name: "Post".into(), enabled: true, operation: placed, component: Component { attach: Attach::Join, stage: Stage::Bench, ..Default::default() } }).unwrap();
        d.cad = Some(doc);
        let p = crate::castability::pattern_parts(&d, &lib);
        let m = &p.marks[0];
        assert!((m.theta_deg - 90.0).abs() < 1e-6 && m.foot_across_mm.abs() < 1e-6 && (m.diameter_mm - 0.8).abs() < 1e-9, "{m:?}");
        assert!(!m.on_parting_line && m.label == "Cylinder \"Post\" (#4)", "{m:?}");
    }

    /// (d) A radial hole straddling the parting plane locks on both walls.
    #[test]
    fn a_pilot_hole_locks_both_walls_and_says_drill_it_at_the_bench() {
        let (_, bare_built, bare) = bare();
        let pilot = Placement::ring(90.0, 0.0);
        let d = with_part(band(), "Pilot", Operation::Cylinder { radius_mm: 0.5, height_mm: 1.0 }, Attach::Cut, Stage::Cast, pilot.clone());
        let (_, f) = judged(&d);
        let p = &f.parts[0];
        let expected = wall_area(&d, &bare_built.mesh, &pilot, 0.5, 1.0, false, |n, at| locks(n, at, f.parting_z_mm));
        let walls = wall_area(&d, &bare_built.mesh, &pilot, 0.5, 1.0, false, |_, _| true);
        let depth = walls / (std::f64::consts::TAU * 0.5);
        eprintln!("pilot: {:.4} mm² undercut against {expected:.4} from the geometry, 2π·r·depth = {walls:.4} at a mean depth of {depth:.3} mm", p.undercut_area_mm2);
        assert!((p.undercut_area_mm2 - expected).abs() < 0.05 * expected, "{} of {expected}", p.undercut_area_mm2);
        // All of 2π·r·depth but the two strips square to the plane, which are vertical walls.
        assert!(expected > 0.98 * walls && expected < walls && depth > 0.45 && depth < 0.5, "{expected} of {walls} at {depth}");
        assert_eq!(p.undercut_at.unwrap().side, PartingSide::Both);
        assert_eq!((bare.verdict, f.verdict), (Verdict::Castable, Verdict::Marginal));
        assert!(p.note.contains("on the hole's walls either side of the parting plane") && p.note.contains("drill it at the bench: stage it Bench and the pattern keeps a raised drill mark at its centre"), "{}", p.note);
        assert!(f.notes.contains(&p.note));
    }

    /// (e) Lost wax burns out of any closed surface: the post is measured, reported, and never gates.
    #[test]
    fn under_lost_wax_the_post_is_reported_and_never_gates() {
        let lib = AlphaLibrary::builtin();
        let (_, sand) = judged(&post(1.5, Stage::Cast));
        let mut d = post(1.5, Stage::Cast);
        CastProcess::LostWax.apply(&mut d.draft);
        let bare = {
            let mut b = band();
            CastProcess::LostWax.apply(&mut b.draft);
            attributed_field_report(&b, &lib, &b.draft, 192, 128)
        };
        let (_, f) = judged(&d);
        let p = &f.parts[0];
        assert!(p.judged && (p.undercut_area_mm2 - sand.parts[0].undercut_area_mm2).abs() < 1e-9, "{p:?}");
        assert_eq!((bare.verdict, f.verdict), (Verdict::Castable, Verdict::Castable));
        assert!(p.note.starts_with("Lost wax: Cylinder \"Post\" (#3) would undercut a two-part pull over 5.2 mm²") && p.note.contains("cannot move to sand as-is"), "{}", p.note);
        assert!(f.notes.iter().any(|n| n.starts_with("Measured with the ring as built")), "{:?}", f.notes);
        // A bench part is cast in place under lost wax: in the pattern, measured, and no mark.
        let mut bench = post(1.5, Stage::Bench);
        CastProcess::LostWax.apply(&mut bench.draft);
        assert!(crate::castability::pattern_parts(&bench, &lib).marks.is_empty());
        let (_, f) = judged(&bench);
        assert!(f.parts[0].judged && f.parts[0].undercut_area_mm2 > 5.0 && f.verdict == Verdict::Castable);
    }

    /// The part's own faces' areas per class, undercut, marginal, vertical and good, as `classes` paints them.
    fn painted(built: &crate::mesh::BuildResult, classes: &[FaceClass]) -> [f64; 4] {
        let own = crate::interaction::pick::part_owners(built);
        let mut areas = [0.0; 4];
        for ((face, owner), class) in built.mesh.faces.iter().zip(&own).zip(classes) {
            if *owner != Some(0) {
                continue;
            }
            let (a, b, c) = built.mesh.triangle(face).unwrap();
            let area = crate::mesh::norm(crate::mesh::cross(crate::mesh::sub(b, a), crate::mesh::sub(c, a))) * 0.5;
            areas[match class { FaceClass::Undercut => 0, FaceClass::Marginal => 1, FaceClass::Vertical => 2, FaceClass::Good => 3 }] += area;
        }
        areas
    }

    /// (f) The draft colours over a part are the classes the verdict judged it by: one parting plane, one rule.
    #[test]
    fn the_draft_colours_on_a_part_are_the_classes_the_verdict_judged() {
        for d in [post(1.5, Stage::Cast), court_post()] {
            let (built, f) = judged(&d);
            let cast = super::super::analyze_at(&built.mesh, &d.draft, d.inner_radius_mm(), f.parting_z_mm);
            assert_eq!(cast.parting_z_mm, f.parting_z_mm);
            let areas = painted(&built, &cast.classes);
            let p = &f.parts[0];
            assert!((areas[0] - p.undercut_area_mm2).abs() < 1e-9 && (areas[1] - p.marginal_area_mm2).abs() < 1e-9, "{areas:?} {p:?}");
            assert!((areas[2] - p.vertical_area_mm2).abs() < 1e-9 && (areas.iter().sum::<f64>() - p.total_area_mm2).abs() < 1e-9, "{areas:?} {p:?}");
            // The mesh's own widest silhouette is not the field's plane: painted from it, some part faces change class.
            let auto = super::super::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
            let own = crate::interaction::pick::part_owners(&built);
            let differ = own.iter().zip(auto.classes.iter().zip(&cast.classes)).filter(|(o, (x, y))| o.is_some() && x != y).count();
            eprintln!("{}: parting plane field {:+.4}, the mesh's own {:+.4}; {differ} part faces painted differently from it", d.name, f.parting_z_mm, auto.parting_z_mm);
            if (auto.parting_z_mm - f.parting_z_mm).abs() > 0.01 {
                assert!(differ > 0);
            }
            // The pick scene names the same owner for every fused face.
            let scene = crate::interaction::pick::PickScene::build(&built, &d);
            assert!((0..built.mesh.faces.len()).all(|i| scene.feature_of_face(i) == own[i].map(|k| built.parts.features[k as usize])));
        }
        // On a heart signet's asymmetric head the two planes are measurably apart; a post seated square on a court band leaves them together.
        let heart = templates::all().iter().find(|t| t.name == "Heart signet").unwrap().design();
        let d = with_part(heart, "Post", Operation::Cylinder { radius_mm: POST_R, height_mm: POST_H }, Attach::Join, Stage::Cast, post_at(0.0));
        let (built, f) = judged(&d);
        let auto = super::super::analyze(&built.mesh, &d.draft, d.inner_radius_mm());
        assert!((auto.parting_z_mm - f.parting_z_mm).abs() > 0.01, "{} {}", auto.parting_z_mm, f.parting_z_mm);
        let (built, f) = judged(&court_post());
        let auto = super::super::analyze(&built.mesh, &court_post().draft, court_post().inner_radius_mm());
        assert!((auto.parting_z_mm - f.parting_z_mm).abs() < 0.01, "{} {}", auto.parting_z_mm, f.parting_z_mm);
    }

    /// Area of the part of a triangle above the plane at `z`, or below it.
    fn area_beyond(tri: [[f64; 3]; 3], z: f64, above: bool) -> f64 {
        let inside = |p: &[f64; 3]| if above { p[2] >= z } else { p[2] <= z };
        let mut poly: Vec<[f64; 3]> = Vec::new();
        for k in 0..3 {
            let (p, q) = (tri[k], tri[(k + 1) % 3]);
            if inside(&p) {
                poly.push(p);
            }
            if inside(&p) != inside(&q) {
                let t = (z - p[2]) / (q[2] - p[2]);
                poly.push(std::array::from_fn(|i| p[i] + (q[i] - p[i]) * t));
            }
        }
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (1..poly.len().saturating_sub(1)).map(|k| 0.5 * crate::mesh::norm(crate::mesh::cross(sub(poly[k], poly[0]), sub(poly[k + 1], poly[0])))).sum()
    }

    /// A flat wall leaning back across the parting plane locks over its far half; only a curved wall's chord is forgiven there.
    #[test]
    fn a_flat_wall_leaning_back_across_the_parting_plane_locks_where_a_curved_walls_chord_does_not() {
        const SPIN: f64 = 3.0;
        let block = |spin: f64| {
            let place = Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: 1.0 - SINK, spin_deg: spin, tilt_deg: 0.0, cant_deg: 0.0 };
            with_part(band(), "Block", Operation::Box { size: [2.0, 2.0, 2.0] }, Attach::Join, Stage::Cast, place)
        };
        // Square to the ring its walls round the ring stand along the pull.
        let (_, square) = judged(&block(0.0));
        assert!(square.parts[0].undercut_area_mm2 < 5e-4, "{:?}", square.parts[0]);
        // Turned about the radius those walls lean 3°, each facing its own mould half on one side of the plane only.
        let (built, f) = judged(&block(SPIN));
        let p = &f.parts[0];
        let owners = crate::interaction::pick::part_owners(&built);
        let (mut truth, mut spanning) = (0.0, 0.0);
        for (face, owner) in built.mesh.faces.iter().zip(&owners) {
            let (Some(_), Some(n), Some((a, b, c))) = (owner, built.mesh.face_normal(face), built.mesh.triangle(face)) else { continue };
            if (n[2].abs() - SPIN.to_radians().sin()).abs() > 0.01 {
                continue;
            }
            // A wall facing up locks below the plane, one facing down above it.
            truth += area_beyond([a, b, c], f.parting_z_mm, n[2] < 0.0);
            if a[2].min(b[2]).min(c[2]) < f.parting_z_mm && a[2].max(b[2]).max(c[2]) > f.parting_z_mm {
                spanning += area_beyond([a, b, c], f64::NEG_INFINITY, true);
            }
        }
        let locking = p.undercut_area_mm2 - p.silhouette_mm2;
        eprintln!("block turned {SPIN}°: {locking:.4} mm² locking, {:.4} forgiven, against {truth:.4} leaning back ({spanning:.4} on facets across the plane)", p.silhouette_mm2);
        assert!(truth > 2.0, "each wall stands about 1.6 mm proud: {truth}");
        assert!(p.silhouette_mm2 < PART_NOISE_MM2, "a flat wall's lean is its own, not its chord's: {p:?}");
        // Each facet is judged whole on its centroid's side, so a facet across the plane is all or none of its share.
        assert!((locking - truth).abs() <= spanning + 1e-6, "{locking} against {truth}");
        assert!(p.undercut_at.is_some_and(|s| s.side == PartingSide::Both) && f.notes.iter().any(|n| n.contains("Block")), "{p:?}");
    }

    /// A seam bead is the part's own metal: the verdict judges it with the part, and the pick names it.
    #[test]
    fn a_bead_is_judged_with_its_part() {
        let mut d = post(0.0, Stage::Cast);
        let (plain, f0) = judged(&d);
        d.cad.as_mut().unwrap().features[1].component.blend_mm = 0.3;
        let (built, f) = judged(&d);
        assert_eq!(built.parts.beads, 1, "{:?}", built.parts.notes);
        let owned = |b: &crate::mesh::BuildResult| crate::interaction::pick::part_owners(b).iter().filter(|o| o.is_some()).count();
        let (p0, p) = (&f0.parts[0], &f.parts[0]);
        eprintln!("beaded post: {} faces and {:.3} mm² judged, against {} and {:.3} bare", owned(&built), p.total_area_mm2, owned(&plain), p0.total_area_mm2);
        assert!(owned(&built) > owned(&plain) + 500 && p.total_area_mm2 > p0.total_area_mm2, "{} {}", p.total_area_mm2, p0.total_area_mm2);
        // Where the bead's sweep crosses the parting plane its facets tilt by their own chord: painted, never locking.
        eprintln!("beaded post: {:.4} mm² of the undercut class, {:.4} of it on facets spanning the plane, worst {:.2}°", p.undercut_area_mm2, p.silhouette_mm2, p.worst_draft_deg);
        assert!(p.undercut_area_mm2 - p.silhouette_mm2 < PART_NOISE_MM2 && p.worst_draft_deg > -SILHOUETTE_DEG, "{p:?}");
        assert!(p.undercut_at.is_none() && f.verdict == Verdict::Castable && p.note.contains("clean but for"), "{p:?}");
        // The hover names the bead's faces as the part's too.
        let scene = crate::interaction::pick::PickScene::build(&built, &d);
        let own = crate::interaction::pick::part_owners(&built);
        assert!((0..built.mesh.faces.len()).all(|i| scene.feature_of_face(i) == own[i].map(|k| built.parts.features[k as usize])));
    }

    /// A separate part is cast on its own sprue: noted, never judged against the ring's plane.
    #[test]
    fn a_separate_part_is_noted_and_not_judged() {
        let (_, _, bare) = bare();
        let d = with_part(band(), "Bead", Operation::Sphere { radius_mm: 1.0 }, Attach::Separate, Stage::Cast, Placement::ring(90.0, 3.0));
        let (_, f) = judged(&d);
        assert!(!f.parts[0].judged && f.parts[0].total_area_mm2 == 0.0);
        assert_eq!((f.verdict, f.undercut_area_mm2, f.total_area_mm2), (bare.verdict, bare.undercut_area_mm2, bare.total_area_mm2));
        assert!(f.notes.iter().any(|n| n == "Sphere \"Bead\" (#3) stands apart from the band: it is cast on its own and not judged against the ring's parting plane."), "{:?}", f.notes);
        assert!(!f.notes.iter().any(|n| n.contains(NOT_JUDGED_HERE)));
    }

    /// (g) What judging costs next to the build it reads, at preview and at export resolution.
    #[test]
    fn judging_parts_costs_little_next_to_the_build() {
        let lib = AlphaLibrary::builtin();
        let d = post(1.5, Stage::Cast);
        let field = attributed_field_report(&d, &lib, &d.draft, 192, 128);
        for (label, params) in [("preview 384x144", preview()), ("export 1024x320", BuildParams { theta_steps: 1024, profile_steps: 320, ..BuildParams::default() })] {
            let started = std::time::Instant::now();
            let built = crate::mesh::try_build(&d, &lib, params).unwrap();
            let build_ms = started.elapsed().as_secs_f64() * 1e3;
            let mut f = field.clone();
            let started = std::time::Instant::now();
            judge_parts(&mut f, &d, &built);
            let ms = started.elapsed().as_secs_f64() * 1e3;
            eprintln!("{label}: {} faces, build {build_ms:.1} ms, judge_parts {ms:.1} ms, {:.4} mm² undercut", built.mesh.faces.len(), f.parts[0].undercut_area_mm2);
            assert!((f.parts[0].undercut_area_mm2 - 5.2).abs() < 0.3);
            assert!(ms < build_ms.max(50.0), "{ms} ms against a {build_ms} ms build");
        }
        // What a bench part's mark adds to the pattern every design-only verdict judges.
        let bench = post(1.5, Stage::Bench);
        let started = std::time::Instant::now();
        let p = crate::castability::pattern_parts(&bench, &lib);
        let marks_ms = started.elapsed().as_secs_f64() * 1e3;
        let bare = band();
        let started = std::time::Instant::now();
        let plain = crate::castability::pattern_parts(&bare, &lib);
        let plain_ms = started.elapsed().as_secs_f64() * 1e3;
        eprintln!("pattern with one bench part's mark {marks_ms:.1} ms, without {plain_ms:.3} ms");
        assert!(p.marks.len() == 1 && plain.marks.is_empty());
    }
}
