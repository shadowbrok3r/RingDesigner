//! Fit changes rebuild source geometry, preserve measured stones and heads by
//! default, and report the unavoidable rounding of a closed ornament repeat.
use crate::{Layer, LayerStack, RingDesign, RingSize, cad::Operation};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub preserve_head: bool,
    pub preserve_stones: bool,
    pub preserve_ornament_pitch: bool,
    pub regenerate_seat_counts: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            preserve_head: true,
            preserve_stones: true,
            preserve_ornament_pitch: true,
            regenerate_seat_counts: true,
        }
    }
}
pub struct Candidate {
    pub design: RingDesign,
    pub notes: Vec<String>,
}
/// Direct millimeter entry intentionally does not round to a quarter US size.
pub fn size_from_bore(mm: f64) -> Result<RingSize> {
    ensure!(mm.is_finite(), "Bore must be finite");
    let size = (mm * std::f64::consts::PI - 36.5) / 2.55;
    ensure!(
        (crate::sizing::MIN_SIZE..=crate::sizing::MAX_SIZE).contains(&size),
        "Bore is outside the supported size range"
    );
    Ok(RingSize(size))
}
pub fn candidate(d: &RingDesign, bore_mm: f64, policy: &Policy) -> Result<Candidate> {
    ensure!(
        policy.preserve_stones,
        "Measured stones cannot be resized implicitly; enter replacement stone dimensions explicitly"
    );
    let size = size_from_bore(bore_mm)?;
    let old = d.field_context();
    let mut out = d.clone();
    out.size = size;
    let next = out.field_context();
    let ratio = next.circumference_mm / old.circumference_mm;
    let mut notes = Vec::new();
    if !policy.preserve_head {
        out.shank.head.length_mm *= ratio;
        for h in &mut out.shank.extra_heads {
            h.length_mm *= ratio;
        }
    }
    fn layers(s: &mut LayerStack, ratio: f64, policy: &Policy, notes: &mut Vec<String>) {
        for entry in &mut s.layers {
            match &mut entry.layer {
                Layer::Tiling(t) if policy.preserve_ornament_pitch => {
                    let old = t.repeats_around.max(1);
                    t.repeats_around = (old as f64 * ratio).round().clamp(1.0, 4096.0) as u32;
                    notes.push(format!(
                        "{}: {} → {} closed repeats; pitch changes {:+.2}% after integer rounding",
                        entry.name,
                        old,
                        t.repeats_around,
                        (ratio * old as f64 / t.repeats_around as f64 - 1.0) * 100.0
                    ));
                }

                Layer::Group(g) => layers(&mut g.stack, ratio, policy, notes),
                _ => {}
            }
        }
    }
    layers(&mut out.layers, ratio, policy, &mut notes);
    fn check_seats(
        stack: &mut LayerStack,
        ctx: &crate::field::FieldContext,
        notes: &mut Vec<String>,
    ) -> Result<()> {
        for e in &mut stack.layers {
            match &mut e.layer {
                Layer::SeatRun(run) => {
                    let before = run.count;
                    run.solve_spacing(ctx);
                    notes.push(format!(
                        "{}: {} → {} seats with at least {:.3} mm bridge",
                        e.name, before, run.count, run.bridge_mm
                    ));
                    ensure!(
                        run.bridge_at(ctx) + 1e-6 >= run.bridge_mm,
                        "{}: stones cannot fit the new shank with the requested bridge",
                        e.name
                    );
                }
                Layer::Group(g) => check_seats(&mut g.stack, ctx, notes)?,
                _ => {}
            }
        }
        Ok(())
    }
    if policy.regenerate_seat_counts {
        check_seats(&mut out.layers, &next, &mut notes)?;
        notes.extend(crate::pave::regenerate_live(&mut out));
    }
    if let Some(doc) = &mut out.cad {
        let mut rebuilt = false;
        for f in &mut doc.features {
            match &mut f.operation {
                Operation::Band => rebuilt = true,
                Operation::Torus { major_mm, minor_mm }
                    if f.component.role == crate::cad::ComponentRole::Shank =>
                {
                    *major_mm = bore_mm / 2.0 + *minor_mm;
                    rebuilt = true;
                }
                Operation::TwistedRing {
                    major_mm,
                    radial_mm,
                    axial_mm,
                    turns,
                } if f.component.role == crate::cad::ComponentRole::Shank => {
                    *major_mm = bore_mm / 2.0
                        + if turns.abs() < 1e-9 {
                            *radial_mm / 2.0
                        } else {
                            radial_mm.max(*axial_mm) / 2.0
                        };
                    rebuilt = true;
                }
                Operation::Revolve {
                    sketch,
                    pivot,
                    axis,
                    degrees,
                } if f.component.role == crate::cad::ComponentRole::Shank => {
                    ensure!(
                        *pivot == [0.0; 3]
                            && *axis == [0.0, 0.0, 1.0]
                            && *degrees == 360.0
                            && sketch.plane == crate::sketch::Workplane::section(),
                        "Shank revolution must use the origin's Z axis and XZ section to resize automatically"
                    );
                    let minimum = sketch
                        .solved_curves()?
                        .iter()
                        .flat_map(|c| c.tessellate_within(0.001))
                        .map(|p| p[0])
                        .fold(f64::INFINITY, f64::min);
                    ensure!(minimum > 0.0, "Shank section crosses the revolution axis");
                    let delta = bore_mm / 2.0 - minimum;
                    for p in &mut sketch.points {
                        p.xy[0] += delta;
                    }
                    rebuilt = true;
                    notes.push(
                        "Revolved bore fitted to the section sampled at 0.001 mm tolerance".into(),
                    );
                }
                _ => {}
            }
        }
        ensure!(
            rebuilt,
            "Assign a torus, twisted ring, or revolution as the shank, or include a procedural shank before resizing CAD geometry"
        );
        ensure!(
            policy.preserve_head
                || !doc
                    .features
                    .iter()
                    .any(|f| f.component.role == crate::cad::ComponentRole::Head),
            "CAD heads require explicit dimension edits; enable Preserve head dimensions for shank-only resizing"
        );
        notes.push("Ring-anchored components follow the new circumference; Cartesian component placements remain explicit".into());
    }
    notes.push("Nominal bore changed without applying shrink or finishing stock. Comfort fit and wide-band preference remain separate design choices".into());
    Ok(Candidate { design: out, notes })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_bore_and_resize_keep_head_dimensions_and_round_repeat_count() {
        let mut d = RingDesign::default();
        let mut tile = crate::tiling::TilingLayer::default_for("Rope", &d.field_context());
        tile.repeats_around = 12;
        d.layers.layers.push(crate::field::LayerEntry::new(
            "Pattern",
            Layer::Tiling(tile),
        ));
        let old = d.shank.head.length_mm;
        let candidate = candidate(&d, 18.123, &Policy::default()).unwrap();
        assert!((candidate.design.size.inner_diameter_mm() - 18.123).abs() < 1e-10);
        assert_eq!(candidate.design.shank.head.length_mm, old);
        assert!(!candidate.notes.is_empty());
        assert_ne!(candidate.design.size, d.size);
    }
    #[test]
    fn cad_bore_uses_actual_shank_geometry_even_if_size_metadata_differs() {
        let mut d = crate::cad::examples::design("two-part-signet").unwrap();
        if let Operation::Torus { major_mm, .. } =
            &mut d.cad.as_mut().unwrap().features[0].operation
        {
            *major_mm = 12.0;
        }
        let head = serde_json::to_value(&d.cad.as_ref().unwrap().features[1]).unwrap();
        let resized = candidate(&d, 18.123, &Default::default()).unwrap().design;
        if let Operation::Torus { major_mm, minor_mm } =
            &resized.cad.as_ref().unwrap().features[0].operation
        {
            assert!((2.0 * (major_mm - minor_mm) - 18.123).abs() < 1e-9);
        } else {
            panic!("Expected torus");
        }
        assert_eq!(
            head,
            serde_json::to_value(&resized.cad.as_ref().unwrap().features[1]).unwrap()
        );
    }
}
