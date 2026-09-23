//! A sketch anchored to a face of a part as the build seats it: the frame the part's references are
//! signed in, a picked face signed there, and the plane an anchored sketch lies on.
use super::{FaceAnchor, Sketch, Workplane};
use crate::cad::{self, FaceRef, Operation, Placement};
use crate::{Mesh, RingDesign};
use anyhow::Result;
use cadkernel::brep::{self, Body};

/// The frame the build seats `feature` by: its ring placement on `surface`, an attached part's, a suppressed feature's source's, else none.
pub fn seat_frame(design: &RingDesign, surface: Option<&Mesh>, feature: u64) -> brep::Placement {
    let Some(doc) = &design.cad else { return brep::Placement::IDENTITY };
    let band = doc.features.iter().find(|f| f.enabled && matches!(f.operation, Operation::Band)).map(|f| f.id);
    let mut id = feature;
    for _ in 0..=doc.features.len() {
        let Some(f) = doc.feature(id) else { break };
        let next = if !f.enabled {
            f.operation.consumes().first().copied()
        } else if f.component.placement != Placement::Free {
            return f.component.placement.frame_on(design, surface).unwrap_or(brep::Placement::IDENTITY);
        } else {
            match f.operation {
                Operation::Boolean { a, b, .. } if band.is_some_and(|x| x == a || x == b) => Some(if band == Some(a) { b } else { a }),
                _ => None,
            }
        };
        match next {
            Some(n) => id = n,
            None => break,
        }
    }
    brep::Placement::IDENTITY
}

/// Face `face` of `feature`'s built `body` signed in that frame; refused in the core's words when it will not carry a sketch.
pub fn on_face(design: &RingDesign, surface: Option<&Mesh>, feature: u64, body: &Body, face: usize) -> Result<FaceAnchor> {
    let frame = seat_frame(design, surface, feature);
    let anchor = FaceAnchor { feature, face: FaceRef::signed(body, face, &frame) };
    let probe = Sketch { plane: Workplane { on_face: Some(anchor.clone()), ..Workplane::default() }, ..Sketch::default() };
    cad::sketch_plane(&probe, body, &frame, &mut Vec::new())?;
    Ok(anchor)
}

/// The world plane `sketch` lies on, read off the built `body` it is anchored to, with any refinding said in `notes`.
pub fn plane(design: &RingDesign, surface: Option<&Mesh>, sketch: &Sketch, body: &Body, notes: &mut Vec<String>) -> Result<cadkernel::space::Plane> {
    let frame = sketch.plane.on_face.as_ref().map_or(brep::Placement::IDENTITY, |a| seat_frame(design, surface, a.feature));
    cad::sketch_plane(sketch, body, &frame, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Attach, Boolean, Component, Document, Feature, Profile, evaluate_with, BuildCtx};
    use crate::{AlphaLibrary, BuildParams, ProfileStyle};

    fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    #[test]
    fn a_face_is_signed_in_the_frame_the_build_seats_its_part_by() {
        let lib = AlphaLibrary::builtin();
        let params = BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() };
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HalfRound);
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 3.0;
        let band = crate::mesh::try_build(&d, &lib, params).unwrap();
        // A block on the flank, where the surface leans well off the radial the reference crest assumes.
        let seat = Placement::Ring { theta_deg: 90.0, across_mm: 2.2, height_mm: 0.0, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
        let (reference, built) = (seat.frame(&d).unwrap(), seat.frame_on(&d, Some(&band.mesh)).unwrap());
        let lean = dot(reference.z_axis, built.z_axis).clamp(-1.0, 1.0).acos().to_degrees();
        assert!(lean > 30.0, "the flank leans {lean:.1}° off the radial");
        let feature = |id, operation, component| Feature { id, name: format!("#{id}"), enabled: true, operation, component };
        let mut doc = Document::default();
        doc.append(feature(1, Operation::Band, Component::default())).unwrap();
        doc.append(feature(2, Operation::Box { size: [2.0, 2.0, 1.0] }, Component { attach: Attach::Join, placement: seat.clone(), ..Component::default() })).unwrap();
        d.cad = Some(doc);
        let never = std::sync::atomic::AtomicBool::new(false);
        let evaluate = |d: &RingDesign| evaluate_with(d, &lib, params, &BuildCtx::new(&never).with_surface(&band.mesh)).unwrap();
        let body = evaluate(&d).components.iter().find(|c| c.id == 2).unwrap().body.clone();
        let faces: Vec<_> = body.faces.iter().map(|(k, _)| k).collect();
        let outward = |i: usize| brep::planar_face_profile(&body, faces[i]).map(|p| p.outward);
        let top = (0..faces.len()).max_by(|i, j| {
            let along = |i: usize| outward(i).map_or(-2.0, |n| dot(n, built.z_axis));
            along(*i).total_cmp(&along(*j))
        });
        let top = top.unwrap();
        let n = outward(top).unwrap();
        assert!(dot(n, built.z_axis) > 1.0 - 1e-9, "the top looks along the seat's normal");
        // The frame is the build's own, so the ordinal holds and nothing is found again.
        assert!(seat_frame(&d, Some(&band.mesh), 2) == built);
        let anchor = on_face(&d, Some(&band.mesh), 2, &body, top).unwrap();
        let mut notes = Vec::new();
        let mut square = Sketch::rectangle(1.0, 1.0);
        square.plane.on_face = Some(anchor);
        let p = plane(&d, Some(&band.mesh), &square, &body, &mut notes).unwrap();
        assert!(notes.is_empty() && dot(p.normal().unwrap(), n) > 1.0 - 1e-9, "{notes:?}");
        // A square on it, extruded half a millimetre, stands on the top.
        let stand = |square: &Sketch| {
            let mut d = d.clone();
            let doc = d.cad.as_mut().unwrap();
            doc.append(feature(3, Operation::Sketch { sketch: square.clone() }, Component::default())).unwrap();
            let post = Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: 0.5, draft_deg: 0.0 };
            doc.append(feature(4, post, Component { attach: Attach::Join, ..Component::default() })).unwrap();
            evaluate(&d)
        };
        let e = stand(&square);
        assert!(e.failures().is_empty(), "{:?}", e.failures());
        let post = e.components.iter().find(|c| c.id == 4).unwrap();
        let heights: Vec<f64> = post.trace.positions.iter().map(|q| dot(std::array::from_fn(|k| q[k] - p.origin[k]), n)).collect();
        let (low, high) = heights.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
        assert!(low.abs() < 1e-6 && (high - 0.5).abs() < 1e-6, "{low} .. {high}");
        // Signed against the reference crest instead, the same face reads as gone and the post is not built.
        let mut stale = square.clone();
        stale.plane.on_face = Some(FaceAnchor { feature: 2, face: FaceRef::signed(&body, top, &reference) });
        let e = stand(&stale);
        assert!(e.failures().iter().any(|(id, why)| *id == 3 && why.contains("no longer")), "{:?}", e.failures());
        assert!(!e.components.iter().any(|c| c.id == 4));
        // A boolean against the band seats as the part it attaches; a feature made from a part keeps no seat.
        let doc = d.cad.as_mut().unwrap();
        doc.append(feature(5, Operation::Boolean { a: 1, b: 2, kind: Boolean::Union }, Component::default())).unwrap();
        doc.append(feature(6, Operation::Fillet { source: 2, edges: Vec::new(), radius_mm: 0.1 }, Component::default())).unwrap();
        assert!(seat_frame(&d, Some(&band.mesh), 5) == built);
        assert!(seat_frame(&d, Some(&band.mesh), 6) == brep::Placement::IDENTITY);
        d.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == 6).unwrap().enabled = false;
        assert!(seat_frame(&d, Some(&band.mesh), 6) == built, "a suppressed feature passes its source's seat through");
    }
}
