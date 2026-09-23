//! Small inspectable source projects: each component and operation stays named.
use super::*;
pub const NAMES: &[&str] = &[
    "twisted-band",
    "two-part-signet",
    "solitaire",
    "inlay-band",
    "gallery",
    "claw-solitaire",
];
pub fn design(name: &str) -> Result<RingDesign> {
    let mut d = RingDesign::default();
    d.name = name.into();
    let mut doc = Document::default();
    fn add(doc: &mut Document, name: &str, op: Operation, role: ComponentRole) -> Result<Id> {
        let id = doc.features.len() as u64 + 1;
        doc.append(Feature {
            id,
            name: name.into(),
            enabled: true,
            operation: op,
            component: Component {
                role,
                ..Default::default()
            },
        })?;
        Ok(id)
    }
    match name {
        "twisted-band" => {
            add(
                &mut doc,
                "Half-turn oval shank",
                Operation::TwistedRing {
                    major_mm: d.inner_radius_mm() + 2.0,
                    radial_mm: 2.0,
                    axial_mm: 4.0,
                    turns: 0.5,
                },
                ComponentRole::Shank,
            )?;
        }
        "two-part-signet" => {
            add(
                &mut doc,
                "Shank",
                Operation::Torus {
                    major_mm: d.inner_radius_mm() + 1.2,
                    minor_mm: 1.2,
                },
                ComponentRole::Shank,
            )?;
            let id = add(
                &mut doc,
                "Separate signet head",
                Operation::Extrude {
                    sketch: Sketch::rectangle(9.0, 7.0).into(),
                    height_mm: 2.5,
                    draft_deg: 3.0,
                },
                ComponentRole::Head,
            )?;
            let head = doc.features.last_mut().unwrap();
            head.component.placement = Placement::ring(90.0, 0.2);
            head.component.bench_notes="Fit the head to the shank and solder; review the joint's contact surfaces before manufacture".into();
            doc.joints.push(Joint {
                a: 1,
                b: id,
                clearance_mm: 0.0,
                method: "Solder".into(),
                notes: "Prepare mating contact faces".into(),
            });
        }
        "solitaire" => {
            add(
                &mut doc,
                "Shank",
                Operation::Torus {
                    major_mm: d.inner_radius_mm() + 1.1,
                    minor_mm: 1.1,
                },
                ComponentRole::Shank,
            )?;
            let outer = add(
                &mut doc,
                "Setting stock",
                Operation::Cylinder {
                    radius_mm: 3.3,
                    height_mm: 3.0,
                },
                ComponentRole::Setting,
            )?;
            let cavity = add(
                &mut doc,
                "Setting cavity tool",
                Operation::Cylinder {
                    radius_mm: 2.8,
                    height_mm: 5.0,
                },
                ComponentRole::Other,
            )?;
            let setting = add(
                &mut doc,
                "Open bezel stock",
                Operation::Boolean {
                    a: outer,
                    b: cavity,
                    kind: Boolean::Subtract,
                },
                ComponentRole::Setting,
            )?;
            let f = doc.features.last_mut().unwrap();
            f.component.placement = Placement::ring(90.0, 1.5);
            f.component.bench_notes="Cut the final bearing to the measured stone; bezel stock includes metal for setting".into();
            let stone = add(
                &mut doc,
                "Measured round stone envelope",
                Operation::Cylinder {
                    radius_mm: 3.0,
                    height_mm: 2.0,
                },
                ComponentRole::Stone,
            )?;
            let f = doc.features.last_mut().unwrap();
            f.component.reference = true;
            f.component.stone_id = Some("stone-001-6mm-round".into());
            f.component.placement = Placement::ring(90.0, 2.0);
            doc.joints.push(Joint {a:setting,b:stone,clearance_mm:0.05,method:"Cut bearing / set".into(),notes:"The cylinder is a measured envelope; refine pavilion and crown geometry before cutting".into()});
        }
        "inlay-band" => {
            add(
                &mut doc,
                "Outer shank",
                Operation::Torus {
                    major_mm: d.inner_radius_mm() + 1.6,
                    minor_mm: 1.6,
                },
                ComponentRole::Shank,
            )?;
            add(
                &mut doc,
                "Inlay reference",
                Operation::Torus {
                    major_mm: d.inner_radius_mm() + 2.4,
                    minor_mm: 0.5,
                },
                ComponentRole::Inlay,
            )?;
            doc.features.last_mut().unwrap().component.material = "Gold 18k".into();
            doc.joints.push(Joint {
                a: 1,
                b: 2,
                clearance_mm: 0.05,
                method: "Cut groove / inlay".into(),
                notes: "Overlapping stock intentionally shows the groove that still needs cutting"
                    .into(),
            });
        }
        "gallery" => {
            add(
                &mut doc,
                "Gallery lower ring",
                Operation::Torus {
                    major_mm: 4.0,
                    minor_mm: 0.6,
                },
                ComponentRole::Setting,
            )?;
            let top = add(
                &mut doc,
                "Gallery upper ring",
                Operation::Torus {
                    major_mm: 5.0,
                    minor_mm: 0.6,
                },
                ComponentRole::Setting,
            )?;
            add(
                &mut doc,
                "Raise upper ring",
                Operation::Transform {
                    source: top,
                    translation: [0.0, 0.0, 3.0],
                    rotation_deg: [0.0; 3],
                },
                ComponentRole::Setting,
            )?;
            for k in 0..4 {
                let a = k as f64 * std::f64::consts::FRAC_PI_2;
                add(
                    &mut doc,
                    &format!("Gallery strut {}", k + 1),
                    Operation::Sweep {
                        sketch: Sketch::circle(0.4).into(),
                        path: vec![
                            [4.0 * a.cos(), 4.0 * a.sin(), 0.0],
                            [5.0 * a.cos(), 5.0 * a.sin(), 3.0],
                        ],
                    },
                    ComponentRole::Setting,
                )?;
            }
        }
        "claw-solitaire" => {
            // A carat round brilliant in a four-claw head joined to the band, its seat bur staged Bench.
            add(&mut doc, "Procedural shank", Operation::Band, ComponentRole::Shank)?;
            let gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 6.5);
            let at = Placement::ring(90.0, builders::stand_off_mm("claw4", gem));
            doc.append(builders::stone_feature(2, gem, at))?;
            doc.append(builders::feature_on(3, "Four-claw head", builders::CLAW, 2, serde_json::json!({ "prongs": 4 })))?;
            let mut bur = builders::feature_on(4, "Seat bur", builders::BUR, 2, serde_json::json!({ "through": true }));
            bur.component.stage = Stage::Bench;
            doc.append(bur)?;
        }
        _ => anyhow::bail!("Unknown CAD example {name}; choose {}", NAMES.iter().copied().collect::<Vec<_>>().join(", ")),
    }
    d.cad = Some(doc);
    Ok(d)
}
#[cfg(test)]
mod tests {
    use super::*;
    /// The claw solitaire's gallery preview, as `examples/cad_thumbnails.rs` draws the rest, into the
    /// directory `RD_CAD_THUMBS` names: the band with its head joined and its seat cut, and the stone.
    #[test]
    #[ignore]
    fn the_claw_solitaire_draws_its_gallery_preview() {
        let Some(dest) = std::env::var_os("RD_CAD_THUMBS") else { return };
        let d = design("claw-solitaire").unwrap();
        let built = crate::mesh::try_build(&d, &AlphaLibrary::builtin(), BuildParams::default()).unwrap();
        let mut mesh = built.mesh.clone();
        for c in built.parts.evaluated.iter().flat_map(|e| &e.components).filter(|c| c.settings.reference) {
            let offset = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&c.mesh.vertices);
            mesh.normals.extend_from_slice(&c.mesh.normals);
            mesh.faces.extend(c.mesh.faces.iter().map(|f| f.map(|v| v + offset)));
        }
        mesh.corner_normals.clear();
        mesh.origin.clear();
        crate::render::write_png(std::path::Path::new(&dest).join("claw-solitaire.png"), &mesh, 0.55, 1.12, 160, [0.76, 0.80, 0.87]).unwrap();
    }
    #[test]
    fn sample_projects_are_evaluable_and_keep_reference_identity() {
        for name in NAMES {
            let d = design(name).unwrap();
            let e = evaluate(
                &d,
                &AlphaLibrary::builtin(),
                BuildParams {
                    theta_steps: 128,
                    profile_steps: 64,
                    refine: None,
                    ..Default::default()
                },
            )
            .unwrap_or_else(|e| panic!("{name}: {e:#}"));
            assert!(e.components.iter().all(|c| c.mesh.validate().watertight));
            if *name == "solitaire" {
                assert!(
                    e.components
                        .iter()
                        .any(|c| c.settings.reference && c.settings.stone_id.is_some())
                );
            }
        }
    }
}
