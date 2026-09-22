//! Generates STEP fixtures plus independently checkable source volumes.
//! cargo run -p ringdesign-core --example cad_exchange_probe -- /tmp/cad-probe
use ringdesign_core::cad::{self, Document, Feature, Operation as Op, EdgeRef, FaceRef};
fn main() -> anyhow::Result<()> {
    run()
}
fn run() -> anyhow::Result<()> {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Choose a new output directory"))?,
    );
    anyhow::ensure!(!dir.exists(), "Output exists");
    std::fs::create_dir_all(&dir)?;
    let mut cases = vec![
        ("box", vec![Op::Box { size: [8.0; 3] }]),
        (
            "cylinder",
            vec![Op::Cylinder {
                radius_mm: 3.0,
                height_mm: 4.0,
            }],
        ),
        ("sphere", vec![Op::Sphere { radius_mm: 3.0 }]),
        (
            "torus",
            vec![Op::Torus {
                major_mm: 10.0,
                minor_mm: 1.0,
            }],
        ),
        (
            "shell",
            vec![
                Op::Box { size: [8.0; 3] },
                Op::Shell {
                    source: 1,
                    open_faces: vec![],
                    thickness_mm: 1.0,
                },
            ],
        ),
        (
            "fillet",
            vec![
                Op::Box { size: [8.0; 3] },
                Op::Fillet {
                    source: 1,
                    edges: vec![EdgeRef::bare(0)],
                    radius_mm: 0.5,
                },
            ],
        ),
        (
            "chamfer",
            vec![
                Op::Box { size: [8.0; 3] },
                Op::Chamfer {
                    source: 1,
                    edges: vec![EdgeRef::bare(0)],
                    base_face: FaceRef::bare(4),
                    distance_mm: 0.5,
                },
            ],
        ),
        (
            "sweep",
            vec![Op::Sweep {
                sketch: ringdesign_core::sketch::Sketch::circle(1.0).into(),
                path: vec![[0.0; 3], [0.0, 0.0, 5.0]],
            }],
        ),
    ];
    let mut top = ringdesign_core::sketch::Sketch::rectangle(6.0, 4.0);
    top.plane.origin[2] = 5.0;
    cases.push((
        "loft",
        vec![Op::Loft {
            sections: vec![ringdesign_core::sketch::Sketch::rectangle(8.0.into(), 6.0).into(), top.into()],
        }],
    ));
    cases.push((
        "tapered-circle",
        vec![Op::Extrude {
            sketch: ringdesign_core::sketch::Sketch::circle(3.0).into(),
            height_mm: 4.0,
            draft_deg: 5.0,
        }],
    ));
    let mut curved = ringdesign_core::sketch::Sketch::default();
    let points = [
        [-3.0, -2.0],
        [3.0, -2.0],
        [3.0, 2.0],
        [1.0, 4.0],
        [-1.0, 4.0],
        [-3.0, 2.0],
    ]
    .map(|p| curved.point(p));
    for (a, b) in [(points[0], points[1]), (points[1], points[2])] {
        curved.entity(ringdesign_core::sketch::Geometry::Line { a, b });
    }
    curved.entity(ringdesign_core::sketch::Geometry::Bezier {
        points: [points[2], points[3], points[4], points[5]],
    });
    curved.entity(ringdesign_core::sketch::Geometry::Line {
        a: points[5],
        b: points[0],
    });
    cases.push((
        "cubic-extrusion",
        vec![Op::Extrude {
            sketch: curved.into(),
            height_mm: 3.0,
            draft_deg: 0.0,
        }],
    ));
    let lib = ringdesign_core::AlphaLibrary::builtin();
    let params = ringdesign_core::BuildParams {
        theta_steps: 128,
        profile_steps: 64,
        refine: None,
        ..Default::default()
    };
    let mut volumes = serde_json::Map::new();
    let mut designs = Vec::new();
    for (name, ops) in cases {
        let mut d = ringdesign_core::RingDesign::default();
        d.name = name.into();
        let mut doc = Document::default();
        for (i, operation) in ops.into_iter().enumerate() {
            doc.append(Feature {
                id: i as u64 + 1,
                name: operation.label().into(),
                enabled: true,
                operation,
                component: Default::default(),
            })?;
        }
        d.cad = Some(doc);
        designs.push(d);
    }
    for name in cad::examples::NAMES {
        designs.push(cad::examples::design(name)?);
    }
    for d in designs {
        let e = cad::evaluate(&d, &lib, params)?;
        let volume = cad::combined(&e, false).volume_mm3();
        std::fs::write(
            dir.join(format!("{}.step", d.name)),
            cad::step::export(&e, &d.name)?,
        )?;
        volumes.insert(d.name,serde_json::json!({"mesh_volume_mm3":volume,"solid_components":e.components.iter().filter(|c|!c.settings.reference).count()}));
    }
    for (name, sketch) in [
        (
            "rectangle",
            ringdesign_core::sketch::Sketch::rectangle(8.0, 6.0),
        ),
        ("circle", ringdesign_core::sketch::Sketch::circle(3.0)),
    ] {
        std::fs::write(
            dir.join(format!("{name}.dxf")),
            ringdesign_core::sketch::exchange::dxf(&sketch)?,
        )?;
        std::fs::write(
            dir.join(format!("{name}.svg")),
            ringdesign_core::sketch::exchange::svg(&sketch)?,
        )?;
    }
    std::fs::write(
        dir.join("expected.json"),
        serde_json::to_vec_pretty(&volumes)?,
    )?;
    Ok(())
}
