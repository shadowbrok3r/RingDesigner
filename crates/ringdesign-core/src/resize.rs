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
                    in_plane,
                } if f.component.role == crate::cad::ComponentRole::Shank => {
                    let Some(sketch) = sketch.sketch_mut() else {
                        anyhow::bail!("Shank revolution must draw its section in the feature to resize automatically");
                    };
                    // The origin's Z axis: the section's own y axis when the line is read in its plane.
                    let along = if *in_plane { [0.0, 1.0, 0.0] } else { [0.0, 0.0, 1.0] };
                    ensure!(
                        *pivot == [0.0; 3]
                            && *axis == along
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
    fn a_shank_revolution_read_in_its_section_resizes_as_the_world_one_does_and_another_line_is_refused() {
        use crate::cad::{Component, ComponentRole, Document, Feature};
        use crate::sketch::{Geometry, Sketch, Workplane};
        let shank = |axis: [f64; 3], in_plane: bool| {
            let mut s = Sketch::default();
            let points = [[9.1, -1.0], [11.1, -1.0], [11.1, 1.0], [9.1, 1.0]].iter().map(|p| s.point(*p)).collect();
            s.entity(Geometry::Polyline { points, closed: true });
            s.plane = Workplane::section();
            let operation = Operation::Revolve { sketch: s.into(), pivot: [0.0; 3], axis, degrees: 360.0, in_plane };
            let mut doc = Document::default();
            doc.append(Feature { id: 1, name: "Shank".into(), enabled: true, operation, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
            RingDesign { cad: Some(doc), ..RingDesign::default() }
        };
        let volume = |d: &RingDesign| crate::cad::evaluate(d, &crate::AlphaLibrary::builtin(), crate::BuildParams::default()).unwrap().components[0].mesh.volume_mm3();
        let bore = |d: &RingDesign| match &d.cad.as_ref().unwrap().features[0].operation {
            Operation::Revolve { sketch: crate::cad::Profile::Inline(s), .. } => s.points.iter().map(|p| p.xy[0]).fold(f64::INFINITY, f64::min),
            other => panic!("{other:?}"),
        };
        // The section's y axis read in its own plane is the finger's axis: both lines fit the bore to 18.123 and turn the same metal.
        let (world, local) = (shank([0.0, 0.0, 1.0], false), shank([0.0, 1.0, 0.0], true));
        assert_eq!(volume(&world), volume(&local));
        let (world, local) = (candidate(&world, 18.123, &Policy::default()).unwrap().design, candidate(&local, 18.123, &Policy::default()).unwrap().design);
        assert!((bore(&world) - 9.0615).abs() < 1e-9 && bore(&local) == bore(&world), "{} {}", bore(&world), bore(&local));
        let v = volume(&local);
        assert!(v == volume(&world) && (v / (std::f64::consts::PI * (11.0615f64.powi(2) - 9.0615f64.powi(2)) * 2.0) - 1.0).abs() < 0.01, "{v}");
        // Read in the plane, the world's Z is the section's normal: a line off the finger, refused as the world's Y is.
        for (axis, in_plane) in [([0.0, 0.0, 1.0], true), ([0.0, 1.0, 0.0], false)] {
            let refused = candidate(&shank(axis, in_plane), 18.123, &Policy::default()).err().expect("refused").to_string();
            assert_eq!(refused, "Shank revolution must use the origin's Z axis and XZ section to resize automatically", "{axis:?} {in_plane}");
        }
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
