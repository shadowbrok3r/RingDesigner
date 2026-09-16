//! Four authored rings using the shared casting, CAD, resizing, and package tools.
//! cargo run -p ringdesign-core --example workshop_collection -- NEW_DIR [--draft]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, Boolean, Component, ComponentRole, Document, Feature, Operation as Op},
    castability::{CastProcess, SandProcess},
    field::{Blend, FluteProfile, FlutesLayer, Layer, LayerEntry, SignetOutline},
    manufacturing::{self as mf, Setup},
    sketch::{Geometry, Sketch, Workplane},
};
use serde_json::json;
use std::path::Path;

const BORE: f64 = 18.2;
fn setup(sand: bool, alloy: &str) -> Setup {
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = alloy.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(alloy).unwrap().shrink_pct;
    s.recipe.process = if sand {
        CastProcess::SandTwoPart
    } else {
        CastProcess::LostWax
    };
    s.recipe.name = format!(
        "{} / {alloy}",
        if sand {
            "Delft clay starting recipe"
        } else {
            "Investment casting starting recipe"
        }
    );
    if !sand {
        s.recipe.sand = None;
        s.recipe.min_section_mm = 0.8;
        s.recipe.min_detail_mm = 0.2;
        s.recipe.min_draft_deg = 0.0;
    }
    s.sample_pitch_mm = 0.10;
    s.recipe.calibration_note = "Uncalibrated starting allowance. Confirm with the caster's alloy, pattern material, mold, and measured trials.".into();
    s.flask.width_mm = 70.;
    s.flask.length_mm = 70.;
    s.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0., -11., 0.],
            end: [0., -22., 0.],
            diameter_mm: 3.2,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0., -22., 0.],
            end: [0., -30., 0.],
            diameter_mm: 5.0,
        },
    ];
    s
}
fn base(name: &str, width: f64, thickness: f64, sand: bool, alloy: &str) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = name.into();
    d.size = ringdesign_core::resize::size_from_bore(BORE).unwrap();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.comfort_fit_mm = 0.15;
    d.profile.side_draft_deg = 3.5;
    d.profile.edge_round_mm = 0.22;
    d.draft.process = if sand {
        CastProcess::SandTwoPart
    } else {
        CastProcess::LostWax
    };
    d.manufacturing = Some(setup(sand, alloy));
    d
}
fn aster() -> RingDesign {
    let mut d = base("Aster — cushion seal", 10.5, 2.5, true, "Silver 925");
    d.shank.apply_signet(10.5);
    d.shank.amount = 0.66;
    d.shank.head.outline = SignetOutline::Cushion;
    d.shank.head.length_mm = 12.5;
    d.shank.head.rise_mm = 0.95;
    d.shank.head.rim_round_mm = 0.55;
    d.shank.head.shoulder_deg = 46.;
    d.shank.head.swell_deg = Some(74.);
    d.shank.head.dome = 1.0;
    d.shank.head.table_dome_mm = 0.08;
    let setup = d.manufacturing.as_mut().unwrap();
    setup.channels[0].start = [0., 12.3, 0.];
    setup.channels[0].end = [0., 22., 0.];
    setup.channels[0].diameter_mm = 3.5;
    setup.channels[1].start = [0., 22., 0.];
    setup.channels[1].end = [0., 30., 0.];
    setup.channels[1].diameter_mm = 5.5;
    setup.bench_notes="Cast as one solid cushion signet. Part on the broad Z=0 plane; lift along +Z. Preserve the bore's sand island. The preliminary gate feeds the heavier head; verify feeding with the caster. Remove the gate and seam, lightly polish the rim and shoulders, and satin the seal face. Optional initials are cut at the bench after casting; none are represented in the pattern.".into();
    d
}
fn tide() -> RingDesign {
    let mut d = base("Tide — twelve reeds", 6.2, 2.15, true, "Silver 925");
    d.profile.crown_mm = 0.9;
    d.profile.shape_a = 2.4;
    d.profile.shape_b = 2.0;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.25;
    let mut reeds = LayerEntry::new(
        "Twelve broad axial reeds",
        Layer::Flutes(FlutesLayer {
            count: 12,
            profile: FluteProfile::Round,
            width_mm: 3.6,
            height_mm: 0.22,
            lean: 0.,
            along: false,
        }),
    );
    reeds.blend = Blend::Max;
    d.layers.layers.push(reeds);
    d.manufacturing.as_mut().unwrap().bench_notes="Cast in one piece, pulling along +Z with a central parting plane. Broad straight reeds avoid diagonal hooks and fine sand fins. Gate at the palm. Remove the parting seam, retain a satin finish in the valleys, and polish the rounded reed crests. No stone setting or assembly is required.".into();
    d
}
fn add(doc: &mut Document, name: &str, operation: Op, role: ComponentRole) -> u64 {
    let id = doc.features.len() as u64 + 1;
    doc.append(Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component: Component {
            role,
            material: "Gold 14k".into(),
            ..Default::default()
        },
    })
    .unwrap();
    id
}
fn polygon(name: &str, points: &[[f64; 2]]) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    let points = points.iter().map(|p| s.point(*p)).collect();
    s.entity(Geometry::Polyline {
        points,
        closed: true,
    });
    s
}
fn lantern() -> RingDesign {
    let mut d = base(
        "Lantern — pierced octagonal signet",
        8.0,
        2.0,
        false,
        "Gold 14k",
    );
    let mut doc = Document::default();
    // An analytic cylindrical shank keeps the head union in supported surfaces.
    let mut shank = polygon(
        "Gallery shank section",
        &[[9.1, -3.4], [11.1, -3.4], [11.1, 3.4], [9.1, 3.4]],
    );
    shank.plane = Workplane::section();
    let band = add(
        &mut doc,
        "Analytic gallery shank",
        Op::Revolve {
            sketch: shank,
            pivot: [0.; 3],
            axis: [0., 0., 1.],
            degrees: 360.,
        },
        ComponentRole::Shank,
    );
    let head = polygon(
        "Octagonal seal outline",
        &[
            [-3., -6.],
            [3., -6.],
            [5., -4.],
            [5., 4.],
            [3., 6.],
            [-3., 6.],
            [-5., 4.],
            [-5., -4.],
        ],
    );
    let outer = add(
        &mut doc,
        "Octagonal seal stock",
        Op::Extrude {
            sketch: head,
            height_mm: 4.2,
            draft_deg: 0.,
        },
        ComponentRole::Head,
    );
    let pocket = add(
        &mut doc,
        "Gallery pocket tool",
        Op::Box {
            size: [6.4, 8.4, 5.8],
        },
        ComponentRole::Other,
    );
    // Box spans -2.9..2.9, leaving a 1.3 mm seal roof and an open underside.
    let hollow = add(
        &mut doc,
        "Open gallery beneath seal",
        Op::Boolean {
            a: outer,
            b: pocket,
            kind: Boolean::Subtract,
        },
        ComponentRole::Head,
    );
    let cut = add(
        &mut doc,
        "Side window tool",
        Op::Box {
            size: [14., 5.8, 1.2],
        },
        ComponentRole::Other,
    );
    let cut = add(
        &mut doc,
        "Raise window above gallery feet",
        Op::Transform {
            source: cut,
            translation: [0., 0., 1.8],
            rotation_deg: [0.; 3],
        },
        ComponentRole::Other,
    );
    let pierced = add(
        &mut doc,
        "Pierced side galleries",
        Op::Boolean {
            a: hollow,
            b: cut,
            kind: Boolean::Subtract,
        },
        ComponentRole::Head,
    );
    let placed = add(
        &mut doc,
        "Place seal over the shoulders",
        Op::Transform {
            source: pierced,
            translation: [0., 9.0, 0.],
            rotation_deg: [0., 90., 90.],
        },
        ComponentRole::Head,
    );
    doc.outputs = vec![band, placed];
    for id in [band, placed] {
        let mut recipe = d.manufacturing.clone().unwrap();
        recipe.component = Some(id);
        doc.features
            .iter_mut()
            .find(|f| f.id == id)
            .unwrap()
            .component
            .manufacturing = Some(recipe);
    }
    doc.features.last_mut().unwrap().component.bench_notes = "Cast the head separately. Its feet carry fitting stock where they meet the shank; file to full contact and solder or laser join. Do not print the overlapping nominal assembly as one manufacturing solid.".into();
    doc.joints.push(cad::Joint { a:band, b:placed, clearance_mm:0.0, method:"Fit and solder / laser join".into(), notes:"Head feet intentionally overlap the shank as fitting stock. Remove only enough material to seat the head evenly; inspect both contact patches before joining.".into() });
    d.cad = Some(doc);
    d.manufacturing.as_mut().unwrap().component = Some(placed);
    d.manufacturing.as_mut().unwrap().bench_notes="Investment cast the pierced head and shank separately. Keep the underside open for investment removal; inspect the 1.3 mm seal roof and gallery rails. Feed each part using a caster-designed tree. Fit the head feet to the shank, solder or laser join, and inspect both joints. Ease shank edges by hand, polish the octagonal rim, and satin the seal. The nominal assembly overlaps at fitting stock; use the separate component pattern files for manufacture.".into();
    d
}
fn aureole() -> RingDesign {
    let mut d = base("Aureole — half-turn ribbon", 4.6, 2.2, false, "Gold 14k");
    let mut doc = Document::default();
    add(
        &mut doc,
        "Continuous half-turn ribbon",
        Op::TwistedRing {
            major_mm: BORE / 2. + 2.3,
            radial_mm: 2.2,
            axial_mm: 4.6,
            turns: 0.5,
        },
        ComponentRole::Shank,
    );
    doc.features[0].component.manufacturing = d.manufacturing.clone();
    d.cad = Some(doc);
    d.manufacturing.as_mut().unwrap().bench_notes="Investment cast the continuous half-turn ribbon. The changing section traps a rigid sand mold in a straight pull. Feed at the thick palm segment; preserve the twist when removing the sprue. Smooth and polish the uninterrupted ribbon, ease the bore edge, and verify the minimum measured bore after finishing. No separate overlapping bands or hidden assembly joints.".into();
    d
}

fn write_ring(dir: &Path, mut d: RingDesign, draft: bool) -> Result<serde_json::Value> {
    std::fs::create_dir(dir)?;
    let lib = AlphaLibrary::builtin();
    let params = BuildParams {
        theta_steps: if draft { 256 } else { 768 },
        profile_steps: if draft { 128 } else { 256 },
        refine: None,
        ..Default::default()
    };
    d.build = params;
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let report = mf::package::report(&d, &setup, &inspection, false);
    let mesh = ringdesign_core::mesh::try_build(&d, &lib, params)?.mesh;
    let metal = ringdesign_core::metal::find(&setup.recipe.alloy).unwrap();
    render_views(dir, &d, &mesh, if draft { 640 } else { 1100 })?;
    let mut sand = setup.clone();
    sand.recipe.process = CastProcess::SandTwoPart;
    sand.recipe.min_draft_deg = 3.;
    let counterfactual = mf::release::compare_orientations(&inspection.prepared.mesh, &sand)?;
    std::fs::write(
        dir.join("sand-orientations.json"),
        serde_json::to_vec_pretty(&counterfactual)?,
    )?;
    std::fs::write(dir.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    ringdesign_core::library::save_design(dir.join("design.ring.json"), &d)?;
    ensure!(
        inspection.release.status != mf::release::Status::Invalid,
        "{} has invalid geometry",
        d.name
    );
    if setup.recipe.process == CastProcess::SandTwoPart {
        ensure!(
            inspection.release.obstructions.is_empty(),
            "{}: {} obstructions",
            d.name,
            inspection.release.obstructions.len()
        );
        ensure!(inspection.release.fits_flask, "Flask too small");
        ensure!(inspection.release.unresolved_rays == 0, "Unresolved rays");
    }
    if !draft {
        mf::package::export(
            &dir.join("pattern-package"),
            &d,
            &lib,
            &setup,
            params,
            false,
        )?;
        let assembly = d.cad.as_ref().is_some_and(|doc| doc.outputs.len() > 1);
        ringdesign_core::stl::write_stl(
            dir.join(if assembly {
                "assembly-preview.stl"
            } else {
                "nominal.stl"
            }),
            &mesh,
            &d.name,
        )?;
        ringdesign_core::threemf::write_3mf(
            dir.join("nominal.3mf"),
            &mesh,
            &d.name,
            &d.size.display(),
        )?;
        if d.cad.is_some() {
            cad::assembly::export(&dir.join("cad-assembly"), &d, &lib, params)?;
            for id in &d.cad.as_ref().unwrap().outputs {
                let mut part_setup = setup.clone();
                part_setup.component = Some(*id);
                mf::package::export(
                    &dir.join(format!("component-{id}-pattern")),
                    &d,
                    &lib,
                    &part_setup,
                    params,
                    false,
                )?;
            }
        }
    }
    let v = json!({"name":d.name,"process":setup.recipe.process,"alloy":setup.recipe.alloy,"bore_mm":BORE,"volume_mm3":mesh.volume_mm3(),"grams":mesh.volume_mm3()*metal.density/1000.0,"release":inspection.release.status,"obstructions":inspection.release.obstructions.len(),"low_draft_area_mm2":inspection.release.low_draft_area_mm2,"radial_wall_mm":inspection.field.as_ref().map(|f|f.thinnest_wall_mm),"local_wall":inspection.local_wall,"mesh":mesh.validate(),"bench":setup.bench_notes,"best_sand_orientation":counterfactual.first()});
    println!("{}", serde_json::to_string(&v)?);
    Ok(v)
}
/// Rendering-only vertex splits retain sharp seal edges while smoothly shading
/// the cylindrical shank. Exported positions and topology remain untouched.
fn crease_normals(mesh: &ringdesign_core::Mesh) -> ringdesign_core::Mesh {
    use nalgebra::Vector3;
    let mut incident = vec![Vec::new(); mesh.vertices.len()];
    let mut normals = Vec::new();
    for face in &mesh.faces {
        let p = face.map(|id| {
            let v = mesh.vertices[id as usize];
            Vector3::new(v.0 as f64, v.1 as f64, v.2 as f64)
        });
        let normal = (p[1] - p[0]).cross(&(p[2] - p[0])).normalize();
        normals.push(normal);
        for i in 0..3 {
            let a = (p[(i + 1) % 3] - p[i]).normalize();
            let b = (p[(i + 2) % 3] - p[i]).normalize();
            incident[face[i] as usize].push((normal, a.dot(&b).clamp(-1.0, 1.0).acos()));
        }
    }
    let mut out = ringdesign_core::Mesh::default();
    for (face, normal) in mesh.faces.iter().zip(normals) {
        let base = out.vertices.len() as u32;
        for id in face {
            let averaged = incident[*id as usize]
                .iter()
                .filter(|(n, _)| n.dot(&normal) > 0.82)
                .fold(Vector3::zeros(), |s, (n, w)| s + n * (*w))
                .normalize();
            out.vertices.push(mesh.vertices[*id as usize]);
            out.normals.push(ringdesign_core::Vec3(
                averaged.x as f32,
                averaged.y as f32,
                averaged.z as f32,
            ));
        }
        out.faces.push([base, base + 1, base + 2]);
    }
    out
}
fn render_views(
    dir: &Path,
    d: &RingDesign,
    mesh: &ringdesign_core::Mesh,
    edge: usize,
) -> Result<()> {
    let tint = if d
        .manufacturing
        .as_ref()
        .unwrap()
        .recipe
        .alloy
        .starts_with("Gold")
    {
        [0.88, 0.71, 0.42]
    } else {
        [0.80, 0.82, 0.85]
    };
    let shaded = d.name.starts_with("Lantern").then(|| crease_normals(mesh));
    let part = ringdesign_core::render::Part::metal(shaded.as_ref().unwrap_or(mesh), tint);
    for (name, yaw, pitch) in [
        ("hero", 0.62, 1.04),
        ("side", 0.0, 0.20),
        ("top", 0.0, std::f64::consts::FRAC_PI_2),
    ] {
        ringdesign_core::render::write_png_parts(
            dir.join(format!("{name}.png")),
            std::slice::from_ref(&part),
            yaw,
            pitch,
            edge,
        )?;
    }
    Ok(())
}
fn main() -> Result<()> {
    let out = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Choose a new directory"))?;
    let draft = std::env::args().any(|a| a == "--draft");
    let root = Path::new(&out);
    if std::env::args().any(|a| a == "--render") {
        for slug in ["aster", "tide", "lantern", "aureole"] {
            let dir = root.join(slug);
            let d = ringdesign_core::library::load_design(dir.join("design.ring.json"))?;
            let mesh =
                ringdesign_core::mesh::try_build(&d, &AlphaLibrary::builtin(), d.build)?.mesh;
            render_views(&dir, &d, &mesh, 1100)?;
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--verify") {
        return verify(root);
    }
    ensure!(!root.exists(), "Output directory already exists");
    std::fs::create_dir_all(root)?;
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (slug, d) in [
        ("aster", aster()),
        ("tide", tide()),
        ("lantern", lantern()),
        ("aureole", aureole()),
    ] {
        match write_ring(&root.join(slug), d, draft) {
            Ok(mut row) => {
                row["slug"] = slug.into();
                rows.push(row);
            }
            Err(e) => {
                eprintln!("{slug}: {e:#}");
                errors.push(format!("{slug}: {e:#}"));
            }
        }
    }
    std::fs::write(
        root.join("collection.json"),
        serde_json::to_vec_pretty(&rows)?,
    )?;
    ensure!(errors.is_empty(), "{}", errors.join("\n"));
    Ok(())
}

/// Reload exported source, compare mesh identity, and repeat sand screening at
/// a finer ray pitch. Refresh sheets only after the package fingerprint matches.
fn verify(root: &Path) -> Result<()> {
    let mut results = Vec::new();
    for slug in ["aster", "tide", "lantern", "aureole"] {
        let dir = root.join(slug);
        let d = ringdesign_core::library::load_design(dir.join("design.ring.json"))?;
        let lib = AlphaLibrary::builtin();
        let mut setup = d.manufacturing.clone().unwrap();
        setup.sample_pitch_mm = 0.075;
        let i = mf::inspect(&d, &lib, &setup, d.build)?;
        if setup.recipe.process == CastProcess::SandTwoPart {
            ensure!(
                i.release.obstructions.is_empty()
                    && i.release.unresolved_rays == 0
                    && i.release.fits_flask,
                "Finer sand check failed for {slug}"
            );
        }
        let mut packages = vec![dir.join("pattern-package")];
        if let Some(doc) = &d.cad {
            for id in &doc.outputs {
                packages.push(dir.join(format!("component-{id}-pattern")));
                packages.push(dir.join(format!("cad-assembly/component-{id}-pattern-review")));
            }
        }
        for path in &packages {
            let saved = ringdesign_core::library::load_design(path.join("design.ring.json"))?;
            let setup = saved.manufacturing.as_ref().unwrap();
            let inspection = mf::inspect(&saved, &lib, setup, saved.build)?;
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(path.join("report.json"))?)?;
            let diagnostic = report["diagnostic_only"].as_bool().unwrap();
            let rebuilt = mf::package::report(&saved, setup, &inspection, diagnostic);
            ensure!(
                rebuilt["pattern_fingerprint"] == report["pattern_fingerprint"],
                "Reload changed pattern: {}",
                path.display()
            );
            std::fs::write(
                path.join("molding-sheet.html"),
                mf::package::sheet(&saved, setup, &inspection, diagnostic),
            )?;
        }
        results.push(json!({"ring":slug,"source_packages_rebuilt":packages.len(),"finer_pitch_mm":i.release.cell_mm,"status":i.release.status,"obstructions":i.release.obstructions.len(),"unresolved_rays":i.release.unresolved_rays,"fits_flask":i.release.fits_flask}));
    }
    std::fs::write(
        root.join("verification.json"),
        serde_json::to_vec_pretty(&results)?,
    )?;
    println!("PASS: saved pattern identities and finer release checks");
    Ok(())
}
