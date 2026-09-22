//! Shared, inspectable CAD creation tools. Solid evaluation remains in core.
use crate::icons::Icon;
use ringdesign_core::{
    cad::{Boolean, Operation},
    sketch::{Geometry, Sketch, Workplane},
};
pub fn icon(op: &Operation) -> Icon {
    use Operation::*;
    match op {
        Band => Icon::CadBand,
        Box { .. } => Icon::CadBox,
        Cylinder { .. } => Icon::CadCylinder,
        Sphere { .. } => Icon::CadSphere,
        Torus { .. } => Icon::CadTorus,
        TwistedRing { .. } => Icon::CadTwistedRing,
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
        Extrude { .. } => "Give a closed sketch depth, optionally tapering the walls.",
        Revolve { .. } => "Rotate a closed sketch about an axis. Edit the profile in Sketch.",
        Sweep { .. } => "Carry a closed section along a 3D path. Edit the stations in Properties.",
        Twist { .. } => "Twist a polygon section along a planar path, optionally scaling its end.",
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
    }
}
/// Keep tools with a known invalid default out of the creation path.
/// Existing source remains inspectable and Preview reports the kernel error.
pub fn unavailable(op: &Operation) -> Option<&'static str> {
    matches!(op,Operation::Twist{..}).then_some("Twisted sweep is unavailable: the solid kernel currently leaves open tessellation edges. Twisted ring is supported.")
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
        Operation::Extrude {
            sketch: Sketch::rectangle(8.0, 6.0),
            height_mm: 3.0,
            draft_deg: 0.0,
        },
        Operation::Revolve {
            sketch: section,
            pivot: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            degrees: 360.0,
        },
        Operation::Sweep {
            sketch: Sketch::circle(1.0),
            path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 5.0], [2.0, 0.0, 8.0]],
        },
        Operation::Loft {
            sections: vec![Sketch::rectangle(10.0, 8.0), top],
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
            sketch: Sketch::rectangle(2.0, 1.5),
            path: twist_path(),
            degrees: 180.0,
            end_scale: 1.0,
        },
        Operation::Fillet {
            source,
            edges: vec![0],
            radius_mm: 0.5,
        },
        Operation::Chamfer {
            source,
            edges: vec![0],
            base_face: 4,
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
    fn every_create_menu_starter_builds_a_closed_positive_volume_solid() {
        use ringdesign_core::{
            AlphaLibrary, BuildParams, RingDesign,
            cad::{self, Document, Feature},
        };
        let lib = AlphaLibrary::builtin();
        for op in starters(0, 0)
            .into_iter()
            .filter(|op| !modify(op) && unavailable(op).is_none())
        {
            let label = op.label();
            let mut d = RingDesign::default();
            let mut doc = Document::default();
            doc.append(Feature {
                id: 1,
                name: label.into(),
                enabled: true,
                operation: op,
                component: Default::default(),
            })
            .unwrap();
            d.cad = Some(doc);
            let evaluated = cad::evaluate(
                &d,
                &lib,
                BuildParams {
                    theta_steps: 64,
                    profile_steps: 48,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{label}: {e:#}"));
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
