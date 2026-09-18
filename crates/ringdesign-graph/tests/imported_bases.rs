use ringdesign_core::{AlphaLibrary, BuildParams, library, mesh::try_build, stones};
use ringdesign_graph::{
    eval::{Evaluator, evaluate_design},
    file,
    registry::Registry,
    templates,
    value::Literal,
};
#[test]
fn imported_templates_travel_resize_keep_stones_and_export_their_mesh() {
    let reg = Registry::builtin();
    let lib = AlphaLibrary::builtin();
    for t in templates::IMPORTED {
        let mut g = t.load();
        let first = evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        assert_eq!(g.exposed.len(), 5);
        let first_stones = stones::stone_frames(&first.design);
        let control = g
            .exposed
            .iter()
            .find(|p| p.name == "Face length")
            .unwrap()
            .clone();
        g.set_input(
            control.node,
            control.input,
            Literal::Number(
                (first.design.shank.head.length_mm * 1.1).min(
                    first
                        .design
                        .imported_base
                        .as_ref()
                        .unwrap()
                        .source
                        .calibration
                        .face_length_mm
                        * 1.29,
                ),
            ),
        )
        .unwrap();
        let g = file::load_graph_str(&file::graph_to_string(&g).unwrap(), Some(&reg)).unwrap();
        let out = evaluate_design(&mut Evaluator::new(), &g, &reg, &lib, 0).unwrap();
        let d = &out.design;
        let next_stones = stones::stone_frames(d);
        for ((a, fa), (b, fb)) in first_stones.iter().zip(&next_stones) {
            assert_eq!(
                serde_json::to_value(a.gem).unwrap(),
                serde_json::to_value(b.gem).unwrap()
            );
            assert_eq!(fa.semi, fb.semi);
            assert!((fb.normal.iter().map(|x| x * x).sum::<f64>() - 1.0).abs() < 1e-6);
        }
        let text = library::design_json(d).unwrap();
        let reloaded = library::load_design_str(&text).unwrap();
        let params = BuildParams {
            theta_steps: 192,
            profile_steps: 96,
            ..Default::default()
        };
        let built = try_build(&reloaded, out.baked_library.as_deref().unwrap(), params).unwrap();
        assert!(built.report.validation.watertight && built.report.max_relief_mm > 0.0);
        assert_eq!(built.report.quality.degenerate_faces, 0);
        let bytes = ringdesign_core::stl::to_stl_binary(&built.mesh, &d.name);
        let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
        assert_eq!(count, built.mesh.faces.len());
        for (record, face) in bytes[84..].chunks_exact(50).zip(&built.mesh.faces) {
            for (k, &index) in face.iter().enumerate() {
                let v = built.mesh.vertices[index as usize];
                for (j, want) in [v.0, v.1, v.2].into_iter().enumerate() {
                    let offset = 12 + k * 12 + j * 4;
                    assert_eq!(
                        f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap()),
                        want
                    );
                }
            }
        }
    }
}

#[test]
fn stock_masterworks_match_their_sources_with_native_maps_and_casting_modes() {
    let reg = Registry::builtin();
    let mut names = std::collections::HashSet::new();
    for template in templates::catalog() {
        assert!(names.insert(template.name), "duplicate menu entry");
    }
    assert_eq!(templates::IMPORTED.len(), 6);
    for t in templates::IMPORTED {
        let slug = t.slug.strip_suffix("-imported").unwrap();
        let source = library::load_design(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(format!(
                    "showcase/stock-masterworks/{slug}/design.ring.json"
                )),
        )
        .unwrap();
        let mut project = t.instantiate(&reg, &AlphaLibrary::default()).unwrap();
        let graph =
            file::load_graph_str(&project.graph.take().unwrap().to_string(), Some(&reg)).unwrap();
        assert!(
            serde_json::to_value(&project).unwrap() == serde_json::to_value(&source).unwrap(),
            "{slug} lost portable source data"
        );
        let sand = matches!(slug, "solstice" | "aurelia" | "saurian" | "zenith");
        // The themed rings paint one skin from the stock's own samples, regions and all, so they carry no reserve masks.
        let one_skin = matches!(slug, "saurian" | "zenith");
        assert_eq!(
            graph.mode,
            if sand {
                ringdesign_graph::graph::Mode::SandRing
            } else {
                ringdesign_graph::graph::Mode::Free
            }
        );
        assert_eq!(project.imported_base.as_ref().unwrap().sand_envelope, sand);
        let lib =
            ringdesign_core::manufacturing::source_library(&project, &AlphaLibrary::default())
                .into_owned();
        for mask in ["Face reserve", "Cheek reserve", "Shoulder reserve"] {
            let Some(a) = lib.get(mask) else {
                assert!(one_skin || (sand && mask == "Shoulder reserve"));
                continue;
            };
            assert_eq!(
                (a.width, a.height),
                (2048, 768),
                "{slug} silently reduced a native mask"
            );
        }
        if sand {
            let params = BuildParams {
                theta_steps: 384,
                profile_steps: 192,
                ..Default::default()
            };
            let inspection = ringdesign_core::manufacturing::inspect(
                &project,
                &lib,
                project.manufacturing.as_ref().unwrap(),
                params,
            )
            .unwrap();
            assert!(
                inspection.release.obstructions.is_empty(),
                "{slug}: {:?}",
                inspection.release.obstructions
            );
            assert_eq!(inspection.release.unresolved_rays, 0);
            // A whole-ring skin is one 2048 x 768 tile, and the texture measure converts an isotropic
            // opening in texels by the finer axis at the tightest station it covers: across the palm a
            // texel is 0.008 mm where round the ring it is 0.034, so a 1 mm step reads a quarter of
            // itself. Until the measure is anisotropic, that one finding is expected of these two.
            let unexpected: Vec<_> = inspection.details.iter().filter(|d| !(one_skin && d.contains("texture's finest"))).collect();
            assert!(unexpected.is_empty(), "{slug}: {unexpected:?}");
            // Fill is a question for what is poured: a flush seat's lip is cut thin at the bench on purpose.
            let wall_mesh = ringdesign_core::mesh::try_build_pattern(
                &project,
                &lib,
                BuildParams {
                    theta_steps: 256,
                    profile_steps: 160,
                    ..params
                },
            )
            .unwrap();
            let wall = ringdesign_core::cad::measure::thickness(&wall_mesh.mesh, 0.8);
            assert!(wall.rays > 300);
            assert_eq!(wall.unresolved, 0);
            assert_eq!(
                wall.below_limit, 0,
                "{slug}: bore join leaves a thin wedge: {wall:?}"
            );
        } else {
            assert_eq!(
                stones::stone_frames(&project).len(),
                if slug == "vesper" { 15 } else { 3 }
            );
        }
    }
}
