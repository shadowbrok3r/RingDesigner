use ringdesign_core::{AlphaLibrary, BuildParams, RingDesign, build, library};
use ringdesign_graph::{eval::{Evaluator, evaluate_design}, file, lift, registry::Registry, templates, value::Literal};

/// The showcase source as its template carries it: only the artwork that reaches a layer or mask.
fn source(slug: &str) -> RingDesign {
    let dir = if matches!(slug, "nocturne" | "solstice") {
        format!("showcase/masterwork-signets/{slug}")
    } else {
        format!("showcase/{slug}")
    };
    templates::refine_sources(&library::load_design(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(dir).join("design.ring.json")).unwrap())
}

#[test]
fn showcase_graphs_reproduce_source_and_geometry_without_a_user_library() {
    let reg = Registry::builtin();
    for template in templates::SHOWCASE {
        let original = source(template.slug);
        let graph = file::load_graph_str(&template.json(), Some(&reg)).unwrap();
        assert!(graph.validate(Some(&reg)).is_empty(), "{}", template.name);
        // Load/reload is part of the contract, including nested Json inputs.
        let graph = file::load_graph_str(&file::graph_to_string(&graph).unwrap(), Some(&reg)).unwrap();
        let cold = AlphaLibrary::builtin();
        let out = evaluate_design(&mut Evaluator::new(), &graph, &reg, &cold, 0).unwrap();
        assert!(out.notes.is_empty(), "{}: {:?}", template.name, out.notes);
        let mut project = template.instantiate(&reg, &cold).unwrap();
        assert!(project.graph.take().is_some(), "{} opens with an editable graph", template.name);
        assert!(serde_json::to_value(&project).unwrap() == serde_json::to_value(&original).unwrap(), "{} template project lost source metadata", template.name);
        let mut differences = Vec::new();
        lift::diff(&serde_json::to_value(&*out.design).unwrap(), &serde_json::to_value(&original).unwrap(), "", &mut differences);
        assert!(differences.is_empty(), "{} source differs at {:?}", template.name, differences.iter().map(|(p, _)| p).collect::<Vec<_>>());
        let mut a = cold.clone();
        original.unpack_embedded(&mut a);
        original.bake_all(&mut a);
        let mut b = cold.clone();
        out.design.unpack_embedded(&mut b);
        out.design.bake_all(&mut b);
        for name in out.design.layers.referenced_alphas() {
            assert!(b.get(name).is_some(), "{} is missing {name}", template.name);
        }
        let params = BuildParams { theta_steps: 192, profile_steps: 96, refine: None, ..Default::default() };
        let expected = build(&original, &a, params).mesh;
        let actual = build(&out.design, &b, params).mesh;
        assert!(actual.vertices == expected.vertices, "{} vertices", template.name);
        assert!(actual.faces == expected.faces, "{} faces", template.name);
        assert!(actual.normals == expected.normals, "{} normals", template.name);
        let rebuilt = lift::from_design(&original, &reg, &a).unwrap();
        assert!(file::graph_to_string(&graph).unwrap() == file::graph_to_string(&rebuilt).unwrap(), "{}: rerun the showcase_templates example", template.name);
        assert!(graph.nodes.iter().filter(|n| n.kind == "design.set").all(|n| !matches!(n.inputs.get("pointer"), Some(Literal::Text(p)) if p.starts_with("/layers"))), "layer properties must not be patched by stack index");
    }
}

#[test]
fn showcase_face_size_and_layer_edits_survive_evaluation_and_reload() {
    let reg = Registry::builtin();
    let lib = AlphaLibrary::builtin();
    for template in templates::SHOWCASE {
        let mut graph = template.load();
        let original = source(template.slug);
        // A size the design does not already have: one of the showcases is a 9.
        let size = if original.size.0 == 9.0 { 7.5 } else { 9.0 };
        for (control, value) in [("US size", size), ("Face length", original.shank.head.length_mm * 1.1), ("Band width", original.profile.width_mm * 1.05)] {
            let e = graph.exposed.iter().find(|e| e.name == control).unwrap().clone();
            graph.set_input(e.node, e.input, Literal::Number(value)).unwrap();
        }
        let entry = graph.nodes.iter().find(|n| n.kind == "entry").unwrap().id;
        graph.set_input(entry, "enabled", Literal::Bool(false)).unwrap();
        graph.set_input(entry, "bench_only", Literal::Bool(true)).unwrap();
        let graph = file::load_graph_str(&file::graph_to_string(&graph).unwrap(), Some(&reg)).unwrap();
        let out = evaluate_design(&mut Evaluator::new(), &graph, &reg, &lib, 0).unwrap();
        assert_eq!(out.design.size.0, size);
        assert_eq!(out.design.shank.head.length_mm, original.shank.head.length_mm * 1.1);
        assert_eq!(out.design.profile.width_mm, original.profile.width_mm * 1.05);
        assert!(!out.design.layers.layers[0].enabled);
        assert!(out.design.layers.layers[0].bench_only);
        assert!(serde_json::to_value(&out.design.embedded).unwrap() == serde_json::to_value(&original.embedded).unwrap());
        // An edit must affect the actual base surface, not just metadata.
        assert_ne!(out.design.inner_radius_mm(), original.inner_radius_mm());
        assert_ne!(out.design.reference_loop().z_range(), original.reference_loop().z_range());
    }
}

#[test]
fn an_embedded_raster_alone_is_included_in_the_graph_field_verdict() {
    let reg = Registry::builtin();
    let mut d = RingDesign::default();
    let mut lib = AlphaLibrary::default();
    lib.insert(ringdesign_core::Alpha::new("Imported stock ornament", 16, 16, (0..256).map(|i| if i % 16 < 8 { 1.0 } else { 0.0 }).collect()));
    let mut tiling = ringdesign_core::tiling::TilingLayer::default_for("Imported stock ornament", &d.field_context());
    tiling.height_mm = 0.8;
    d.layers.layers.push(ringdesign_core::LayerEntry::new("Imported ornament", ringdesign_core::Layer::Tiling(tiling)));
    d.embed_alphas(&lib);
    let g = lift::from_design(&d, &reg, &lib).unwrap();
    let mut g = file::load_graph_str(&file::graph_to_string(&g).unwrap(), Some(&reg)).unwrap();
    let mut evaluator = Evaluator::new();
    let out = evaluate_design(&mut evaluator, &g, &reg, &AlphaLibrary::default(), 0).unwrap();
    let expected = ringdesign_core::castability::attributed_field_report(&d, &lib, &d.draft, ringdesign_graph::eval::FIELD_THETA_STEPS, ringdesign_graph::eval::FIELD_PROFILE_STEPS);
    let absent = ringdesign_core::castability::attributed_field_report(&d, &AlphaLibrary::default(), &d.draft, ringdesign_graph::eval::FIELD_THETA_STEPS, ringdesign_graph::eval::FIELD_PROFILE_STEPS);
    assert_ne!(expected.total_area_mm2, absent.total_area_mm2, "fixture must detect omitted raster relief");
    assert!(serde_json::to_value(&out.field).unwrap() == serde_json::to_value(&expected).unwrap(), "cold graph evaluation omitted its embedded PNG");
    let baked = out.baked_library.as_ref().expect("the preview receives evaluated artwork");
    let params = BuildParams { theta_steps: 96, profile_steps: 48, refine: None, ..Default::default() };
    let before = build(&out.design, baked, params).mesh;
    assert!(before.vertices == build(&d, &lib, params).mesh.vertices);

    // Editing an image node must replace the same-named raster from the
    // previous preview, even with the evaluator and host library reused.
    lib.insert(ringdesign_core::Alpha::new("Imported stock ornament", 16, 16, vec![0.0; 256]));
    d.embed_alphas(&lib);
    let image = g.nodes.iter().find(|n| n.kind == "alpha.png").unwrap().id;
    g.set_input(image, "png_base64", Literal::Text(d.embedded[0].png.clone())).unwrap();
    let edited = evaluate_design(&mut evaluator, &g, &reg, baked, 1).unwrap();
    let actual = build(&edited.design, edited.baked_library.as_ref().unwrap(), params).mesh;
    assert!(actual.vertices != before.vertices, "artwork edits must reach the rendered surface");
    assert!(actual.vertices == build(&d, &lib, params).mesh.vertices, "preview retained the previous raster");
}
