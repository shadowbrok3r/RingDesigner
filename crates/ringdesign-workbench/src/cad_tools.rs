//! Shared, inspectable CAD creation tools. Solid evaluation remains in core.
use crate::icons::Icon;
use ringdesign_core::{
    RingDesign,
    cad::{Attach, Boolean, Component, EdgeRef, FaceRef, FaceSeat, MirrorPlane, Operation, PatternKind, Placement, PlaneBase, Stage},
    sketch::{Geometry, Sketch, Workplane},
};
pub use ringdesign_core::interaction::surface::PARTS_ONLY;
/// True when the CAD parts stand in for the band: features, none of them `Operation::Band`.
pub fn replaces_band(d: &RingDesign) -> bool {
    ringdesign_core::interaction::surface::replaces_band(d)
}
/// The design with every part kept beside the band, so a preview skips the joins and cuts.
pub fn without_joins(d: &RingDesign) -> RingDesign {
    let mut d = d.clone();
    if let Some(doc) = d.cad.as_mut() {
        for f in &mut doc.features {
            f.component.attach = Attach::Separate;
        }
    }
    d
}
pub fn icon(op: &Operation) -> Icon {
    use Operation::*;
    match op {
        Band => Icon::CadBand,
        Box { .. } => Icon::CadBox,
        Cylinder { .. } => Icon::CadCylinder,
        Sphere { .. } => Icon::CadSphere,
        Torus { .. } => Icon::CadTorus,
        TwistedRing { .. } => Icon::CadTwistedRing,
        Sketch { .. } => Icon::CadSketch,
        Extrude { .. } => Icon::CadExtrude,
        Revolve { .. } => Icon::CadRevolve,
        Sweep { .. } => Icon::CadSweep,
        Twist { .. } => Icon::CadTwist,
        Loft { .. } => Icon::CadLoft,
        Boolean { kind, .. } => match kind {
            ringdesign_core::cad::Boolean::Union => Icon::CadUnion,
            ringdesign_core::cad::Boolean::Subtract => Icon::CadSubtract,
            ringdesign_core::cad::Boolean::Intersect => Icon::CadIntersect,
        },
        Fillet { .. } => Icon::CadFillet,
        Chamfer { .. } => Icon::CadChamfer,
        Shell { .. } => Icon::CadShell,
        Transform { .. } => Icon::CadPlace,
        Builder { key, .. } => if key == ringdesign_core::cad::builders::STONE { Icon::Stones } else { Icon::NodeHead },
        Pattern { kind: PatternKind::Mirror { .. }, .. } => Icon::Mirror,
        Pattern { .. } => Icon::Pattern,
        Plane { .. } => Icon::Section,
        PressPull { .. } => Icon::Raise,
        Stored { recipe, .. } => match recipe.op.as_str() {
            "fillet" => Icon::CadFillet,
            "shell" => Icon::CadShell,
            "junction" => Icon::CadUnion,
            _ => Icon::Files,
        },
    }
}
pub fn hint(op: &Operation) -> &'static str {
    use Operation::*;
    match op {
        Band => "Build the current ring size, profile and ornament as a solid shank.",
        Box { .. } => "Solid rectangular stock. Set X, Y and Z dimensions.",
        Cylinder { .. } => "Round stock along Z. Set radius and height.",
        Sphere { .. } => "A complete spherical solid defined by its radius.",
        Torus { .. } => "A circular tube around the finger opening.",
        TwistedRing { .. } => {
            "An oval section twisted around a closed ring. Whole and half turns join cleanly."
        }
        Sketch { .. } => "A closed profile of its own, for other features to extrude, revolve, sweep or loft.",
        Extrude { .. } => "Give a closed sketch depth, optionally tapering the walls.",
        Revolve { .. } => "Rotate a closed sketch about an axis. Edit the profile in Sketch.",
        Sweep { .. } => "Carry a closed section along a 3D path. Edit the stations in Properties.",
        Twist { .. } => "Twist a closed section along a planar path, optionally scaling its end. The section stands square to the path at its start.",
        Loft { .. } => "Join matching closed sections. Move each station to shape the transition.",
        Boolean { .. } => {
            "Combine two different earlier solids. Consumes the source components; Preview checks the intersection."
        }
        Fillet { .. } => {
            "Round a selected analytic edge. Pick an edge in the viewport, then set its radius."
        }
        Chamfer { .. } => "Bevel a selected edge. Pick an edge and choose the adjoining base face.",
        Shell { .. } => {
            "Hollow a box, cylinder or sphere. Select opening face indices, or leave them empty for a sealed cavity."
        }
        Transform { .. } => "Move or rotate an existing solid without modifying its source recipe.",
        Builder { key, .. } => ringdesign_core::cad::builders::spec(key).map_or("A part built round a stone.", |s| s.hint),
        Pattern { kind: PatternKind::Ring { .. }, .. } => "Copies of the part round the finger, each dropped onto the band at its own angle; the part stays beside them.",
        Pattern { kind: PatternKind::About { .. }, .. } => "Copies of the part round a stone's axis or another part's: six prongs from one.",
        Pattern { kind: PatternKind::Mirror { .. }, .. } => "The part reflected across the band, through the head, or across a work plane, as a part of its own.",
        Plane { .. } => "A plane with no body: through the finger's axis, square to the band, the parting plane or a part's face. Sketches lie on it; mirrors reflect across it.",
        PressPull { .. } => "Push or pull a planar face of a part along its normal; its neighbours follow it.",
        Stored { .. } => "A mesh another kernel made, kept in the file so every build shows and judges it; run it again where that kernel is to change it.",
    }
}
pub fn modify(op: &Operation) -> bool {
    !op.sources().is_empty()
}
pub fn starters(source: u64, second: u64) -> Vec<Operation> {
    let mut section = Sketch::rectangle(2.0, 5.0);
    section.plane = ringdesign_core::sketch::Workplane::section();
    for p in &mut section.points {
        p.xy[0] += 10.0;
    }
    let mut top = Sketch::rectangle(8.0, 6.0);
    top.plane.origin[2] = 5.0;
    vec![
        Operation::Band,
        Operation::TwistedRing {
            major_mm: 10.0,
            radial_mm: 2.0,
            axial_mm: 4.0,
            turns: 1.0,
        },
        Operation::Box {
            size: [8.0, 6.0, 3.0],
        },
        Operation::Cylinder {
            radius_mm: 4.0,
            height_mm: 3.0,
        },
        Operation::Sphere { radius_mm: 3.0 },
        Operation::Torus {
            major_mm: 10.0,
            minor_mm: 1.5,
        },
        Operation::Sketch {
            sketch: Sketch::rectangle(8.0, 6.0),
        },
        Operation::Plane {
            base: PlaneBase::Section { theta_deg: 90.0 },
            offset_mm: 0.0,
        },
        Operation::Extrude {
            sketch: Sketch::rectangle(8.0, 6.0).into(),
            height_mm: 3.0,
            draft_deg: 0.0,
        },
        Operation::Revolve {
            sketch: section.into(),
            pivot: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            degrees: 360.0,
        },
        Operation::Sweep {
            sketch: Sketch::circle(1.0).into(),
            path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [2.0, 0.0, 8.0]],
        },
        Operation::Loft {
            sections: vec![Sketch::rectangle(10.0, 8.0).into(), top.into()],
        },
        Operation::Boolean {
            a: source,
            b: second,
            kind: Boolean::Union,
        },
        Operation::Boolean {
            a: source,
            b: second,
            kind: Boolean::Subtract,
        },
        Operation::Boolean {
            a: source,
            b: second,
            kind: Boolean::Intersect,
        },
        Operation::Twist {
            sketch: Sketch::rectangle(2.0, 1.5).into(),
            path: twist_path(),
            degrees: 180.0,
            end_scale: 1.0,
        },
        Operation::Fillet {
            source,
            edges: vec![EdgeRef::bare(0)],
            radius_mm: 0.5,
        },
        Operation::Chamfer {
            source,
            edges: vec![EdgeRef::bare(0)],
            base_face: FaceRef::bare(4),
            distance_mm: 0.3,
        },
        Operation::Shell {
            source,
            open_faces: vec![],
            thickness_mm: 0.8,
        },
        Operation::Transform {
            source,
            translation: [0.0, 0.0, 5.0],
            rotation_deg: [0.0; 3],
        },
        Operation::Pattern {
            source,
            kind: PatternKind::Ring { count: 6, span_deg: 360.0 },
        },
        Operation::Pattern {
            source,
            kind: PatternKind::Mirror { plane: MirrorPlane::Band },
        },
    ]
}

fn twist_path() -> Sketch {
    let mut path = Sketch::default();
    path.plane = Workplane::section();
    let a = path.point([0.0, 0.0]);
    let b = path.point([0.0, 8.0]);
    path.entity(Geometry::Line { a, b });
    path
}

/// Cached thumbnails rendered from the actual bundled CAD documents.
pub fn example_button(ui: &mut egui::Ui, name: &str) -> egui::Response {
    let (label, bytes): (&str, &[u8]) = match name {
        "twisted-band" => (
            "Twisted oval band",
            include_bytes!("../assets/cad/twisted-band.png"),
        ),
        "two-part-signet" => (
            "Two-part signet",
            include_bytes!("../assets/cad/two-part-signet.png"),
        ),
        "solitaire" => (
            "Solitaire & bezel",
            include_bytes!("../assets/cad/solitaire.png"),
        ),
        "inlay-band" => ("Inlay band", include_bytes!("../assets/cad/inlay-band.png")),
        "claw-solitaire" => ("Claw solitaire", include_bytes!("../assets/cad/claw-solitaire.png")),
        "gallery" => ("Open gallery", include_bytes!("../assets/cad/gallery.png")),
        _ => return ui.button(name),
    };
    let id = egui::Id::new(("cad-example", name));
    let tex = ui
        .data_mut(|d| d.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let rgba = image::load_from_memory(bytes)
                .expect("bundled CAD preview")
                .to_rgba8();
            let tex = ui.ctx().load_texture(
                name,
                egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            );
            ui.data_mut(|d| d.insert_temp(id, tex.clone()));
            tex
        });
    ui.add(egui::Button::new((
        egui::Image::new((tex.id(), egui::vec2(48., 48.))),
        label,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parts_replace_the_band_only_without_a_procedural_shank_and_a_preview_can_unjoin_them() {
        use ringdesign_core::{RingDesign, cad::{Document, Feature}};
        let feature = |id, operation: Operation, attach| Feature {
            id,
            name: operation.label().into(),
            enabled: true,
            operation,
            component: Component { attach, ..Default::default() },
        };
        let cylinder = || Operation::Cylinder { radius_mm: 2.0, height_mm: 3.0 };
        let mut d = RingDesign::default();
        assert!(!replaces_band(&d), "no parts, so the band stands");
        let mut doc = Document::default();
        doc.append(feature(1, cylinder(), Attach::Join)).unwrap();
        d.cad = Some(doc.clone());
        assert!(replaces_band(&d), "a cylinder alone is the whole ring");
        doc.append(feature(2, Operation::Band, Attach::Separate)).unwrap();
        d.cad = Some(doc);
        assert!(!replaces_band(&d), "a procedural shank keeps the band");
        let stock = without_joins(&d);
        let attaches: Vec<_> = stock.cad.as_ref().unwrap().features.iter().map(|f| f.component.attach).collect();
        assert_eq!(attaches, [Attach::Separate, Attach::Separate]);
        assert_eq!(d.cad.as_ref().unwrap().features[0].component.attach, Attach::Join, "the design itself keeps its joins");
    }
    #[test]
    fn every_create_menu_starter_builds_a_closed_positive_volume_solid() {
        use ringdesign_core::{
            AlphaLibrary, BuildParams, RingDesign,
            cad::{self, Document, Feature},
        };
        let lib = AlphaLibrary::builtin();
        for op in starters(0, 0).into_iter().filter(|op| !modify(op)) {
            let label = op.label();
            let mut d = RingDesign::default();
            let mut doc = Document::default();
            let (sketch, plane) = (matches!(op, Operation::Sketch { .. }), matches!(op, Operation::Plane { .. }));
            doc.append(Feature {
                id: 1,
                name: label.into(),
                enabled: true,
                operation: op,
                component: Default::default(),
            })
            .unwrap();
            // A sketch or a work plane has no body of its own; the starter is judged by what extrudes from it or off it.
            let profile = if plane {
                let mut on = Sketch::rectangle(2.0, 2.0);
                on.plane.on_face = Some(ringdesign_core::sketch::FaceAnchor { feature: 1, face: FaceRef::bare(0) });
                Some(ringdesign_core::cad::Profile::Inline(on))
            } else {
                sketch.then_some(ringdesign_core::cad::Profile::Feature { feature: 1 })
            };
            if let Some(sketch) = profile {
                doc.append(Feature {
                    id: 2,
                    name: "Extrude".into(),
                    enabled: true,
                    operation: Operation::Extrude { sketch, height_mm: 2.0, draft_deg: 0.0 },
                    component: Default::default(),
                })
                .unwrap();
            }
            let band = matches!(doc.features[0].operation, Operation::Band);
            d.cad = Some(doc);
            let params = BuildParams {
                theta_steps: 64,
                profile_steps: 48,
                ..Default::default()
            };
            // The procedural shank is the band itself, an anchor with no body of its own: it is
            // judged by the ring it builds.
            if band {
                let built = ringdesign_core::mesh::try_build(&d, &lib, params).unwrap_or_else(|e| panic!("{label}: {e:#}"));
                assert!(built.report.validation.watertight && built.mesh.volume_mm3() > 1.0, "{label}: no band");
                continue;
            }
            let evaluated = cad::evaluate(&d, &lib, params).unwrap_or_else(|e| panic!("{label}: {e:#}"));
            assert!(!evaluated.components.is_empty(), "{label} has no output");
            for c in evaluated.components {
                assert!(c.mesh.volume_mm3() > 0.001, "{label}: empty solid");
            }
        }
    }
    #[test]
    fn modify_tools_accept_valid_source_solids_and_keep_the_source_document() {
        use ringdesign_core::{
            AlphaLibrary, BuildParams, RingDesign,
            cad::{self, Document, Feature},
        };
        let lib = AlphaLibrary::builtin();
        for op in starters(1, 2).into_iter().filter(modify) {
            let mut d = RingDesign::default();
            let mut doc = Document::default();
            for (id, operation) in [
                (1, Operation::Box { size: [8., 6., 3.] }),
                (2, Operation::Box { size: [4., 4., 5.] }),
                (3, op.clone()),
            ] {
                doc.append(Feature {
                    id,
                    name: operation.label().into(),
                    enabled: true,
                    operation,
                    component: Default::default(),
                })
                .unwrap();
            }
            let original = serde_json::to_value(&doc).unwrap();
            d.cad = Some(doc);
            let evaluated = cad::evaluate(&d, &lib, BuildParams::default())
                .unwrap_or_else(|e| panic!("{}: {e:#}", op.label()));
            assert!(
                evaluated
                    .components
                    .iter()
                    .any(|c| c.id == 3 && c.mesh.volume_mm3() > 0.001),
                "{}",
                op.label()
            );
            assert_eq!(
                serde_json::to_value(d.cad.as_ref().unwrap()).unwrap(),
                original
            );
        }
    }
}

/// The seat of a part on the ring: on or off, then the six numbers.
pub fn placement(ui: &mut egui::Ui, p: &mut Placement) {
    let mut seated = matches!(p, Placement::Ring { .. });
    if ui.checkbox(&mut seated, "Seat on the ring").on_hover_text("Stand the part on the ring's outer surface at an angle; it follows resizing").changed() {
        *p = if seated { Placement::ring(90.0, 0.0) } else { Placement::Free };
    }
    if let Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } = p {
        crate::controls::named(ui, "Ring angle °", "Ring angle", |ui| ui.add(egui::DragValue::new(theta_deg).speed(0.5).max_decimals(2)));
        crate::controls::named(ui, "Across the band mm", "Across the band", |ui| ui.add(egui::DragValue::new(across_mm).speed(0.05).max_decimals(3)));
        crate::controls::named(ui, "Stand-off mm", "Stand-off", |ui| ui.add(egui::DragValue::new(height_mm).speed(0.05).max_decimals(3)));
        crate::controls::named(ui, "Spin °", "Spin", |ui| ui.add(egui::DragValue::new(spin_deg).speed(0.5).max_decimals(2)));
        crate::controls::named(ui, "Tilt along the ring °", "Tilt", |ui| ui.add(egui::DragValue::new(tilt_deg).speed(0.5).max_decimals(2)));
        crate::controls::named(ui, "Cant across the band °", "Cant", |ui| ui.add(egui::DragValue::new(cant_deg).speed(0.5).max_decimals(2)));
    }
}

/// Where a part stands: a stone on a part's face by its seat there, anything else by [`placement`]; `true` when the face seat moved.
pub fn seat(ui: &mut egui::Ui, p: &mut Placement, op: &mut Operation) -> bool {
    match op {
        Operation::Builder { on: Some(on), params, .. } if FaceSeat::of(params).is_ok_and(|s| s.is_some()) => face_seat(ui, *on, params),
        _ => {
            placement(ui, p);
            false
        }
    }
}

/// A stone's seat on a face of part `on`: offsets along and across the face, stand-off and spin; `true` when a value moved.
pub fn face_seat(ui: &mut egui::Ui, on: u64, params: &mut serde_json::Value) -> bool {
    let Ok(Some(mut seat)) = FaceSeat::of(params) else { return false };
    let before = seat.clone();
    ui.label(format!("On face {} of #{on}, riding it when the part moves or grows", seat.face.ordinal)).on_hover_text("Along runs with the part's own axis that lies on the face; across is square to it on the face");
    crate::controls::named(ui, "Along the face mm", "Along the face", |ui| ui.add(egui::DragValue::new(&mut seat.u_mm).speed(0.05).max_decimals(3)));
    crate::controls::named(ui, "Across the face mm", "Across the face", |ui| ui.add(egui::DragValue::new(&mut seat.v_mm).speed(0.05).max_decimals(3)));
    crate::controls::named(ui, "Stand-off mm", "Stand-off", |ui| ui.add(egui::DragValue::new(&mut seat.height_mm).speed(0.05).max_decimals(3)));
    crate::controls::named(ui, "Spin °", "Spin", |ui| ui.add(egui::DragValue::new(&mut seat.spin_deg).speed(0.5).max_decimals(2)));
    let moved = seat != before;
    if moved {
        seat.write(params);
    }
    moved
}

#[cfg(test)]
mod seat_tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};
    use ringdesign_core::cad::builders;
    use ringdesign_core::gem::{Gem, GemCut};

    #[test]
    fn a_stone_on_a_face_shows_its_seat_and_a_ring_part_its_placement() {
        let gem = Gem::calibrated(GemCut::Round, 5.0);
        let on = FaceSeat { face: FaceRef::bare(4), u_mm: 0.5, v_mm: -0.25, height_mm: 2.4, spin_deg: 0.0 };
        let stone = ringdesign_core::cad::stone_on_face(3, gem, 2, &on);
        let state = (stone.component.placement.clone(), stone.operation.clone(), false);
        let mut h = Harness::builder().with_size([400.0, 300.0]).build_ui_state(
            |ui, (p, op, moved): &mut (Placement, Operation, bool)| {
                *moved |= seat(ui, p, op);
            },
            state,
        );
        h.run_steps(2);
        let shown = |h: &Harness<'_, (Placement, Operation, bool)>, label: &str| h.get_by_label(label).value().and_then(|v| v.parse::<f64>().ok());
        assert_eq!(shown(&h, "Along the face"), Some(0.5));
        assert_eq!(shown(&h, "Across the face"), Some(-0.25));
        assert_eq!(shown(&h, "Stand-off"), Some(2.4));
        assert!(h.query_by_label("Seat on the ring").is_none(), "a face stone's placement stays free");
        assert!(!h.state().2, "drawing moves nothing");
        let ring = builders::stone_feature(3, gem, Placement::ring(90.0, 2.4));
        let mut h = Harness::builder().with_size([400.0, 300.0]).build_ui_state(
            |ui, (p, op, moved): &mut (Placement, Operation, bool)| {
                *moved |= seat(ui, p, op);
            },
            (ring.component.placement.clone(), ring.operation.clone(), false),
        );
        h.run_steps(2);
        assert!(h.query_by_label("Seat on the ring").is_some() && h.query_by_label("Along the face").is_none());
    }
}

/// How a part meets the band and when it is added; the seam bead only where it joins or cuts.
pub fn attachment(ui: &mut egui::Ui, c: &mut Component) {
    const STONE: &str = "A reference stone is never metal";
    if c.reference {
        ui.label("Reference stone: never metal, so it stands beside the band unjoined.");
    }
    ui.add_enabled_ui(!c.reference, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("Attach");
            for (attach, label, hint) in [
                (Attach::Separate, "Separate", "Kept beside the band as its own solid"),
                (Attach::Join, "Join", "United into the band; the pattern and the verdict see one solid. A part seated on a procedural shank starts joined"),
                (Attach::Cut, "Cut", "Subtracted from the band"),
            ] {
                ui.selectable_value(&mut c.attach, attach, label).on_hover_text(hint).on_disabled_hover_text(STONE);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Stage");
            for (stage, label, hint) in [
                (Stage::Cast, "Cast", "Cast in the pattern"),
                (Stage::Bench, "Bench", "Added at the bench after the pour; never in a sand pattern"),
            ] {
                ui.selectable_value(&mut c.stage, stage, label).on_hover_text(hint).on_disabled_hover_text(STONE);
            }
        });
        if c.attaches() {
            crate::controls::named(ui, "Seam blend mm", "Seam blend", |ui| {
                ui.add(egui::DragValue::new(&mut c.blend_mm).range(0.0..=1.5).speed(0.01).max_decimals(2).suffix(" mm"))
                    .on_hover_text("Radius of the bead laid along the seam where the part meets the band; 0 lays none")
            });
        }
    });
}
