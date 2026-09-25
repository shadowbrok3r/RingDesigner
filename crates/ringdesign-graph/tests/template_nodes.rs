use ringdesign_core::{AlphaLibrary, BuildParams, ProfileStyle, RingDesign, imported_base::{ImportedBase, PresetSource}, library, setting::{self, RowPath, StampRow}};
use ringdesign_graph::{eval::{Evaluator, Targets, design_of}, file, graph::{Graph, Mode, NodeId}, lift, registry::Registry, templates, value::{Literal, Value}};

fn value(g: &Graph, id: NodeId, out: &str, reg: &Registry) -> Value {
    let report = Evaluator::new().evaluate(g, reg, &AlphaLibrary::default(), 0, Targets::Node(id));
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    assert!(report.notes(g).is_empty(), "{}: {:?}", g.name, report.notes(g));
    report.value(id, out).unwrap().clone()
}

fn same_source(a: &RingDesign, b: &RingDesign) {
    let mut differences = Vec::new();
    lift::diff(&serde_json::to_value(a).unwrap(), &serde_json::to_value(b).unwrap(), "", &mut differences);
    assert!(differences.is_empty(), "{:?}", differences.iter().map(|(p, _)| p).collect::<Vec<_>>());
}

#[test]
fn stock_nodes_preserve_authored_charts_and_parameters_and_their_controls_change_geometry() {
    let reg = Registry::builtin();
    for (id, sand, chart_enabled) in [("002", true, true), ("004", false, false)] {
        let source = PresetSource { preset: id.into(), sand_master: sand }.load().unwrap();
        let mut d = RingDesign::default();
        ImportedBase::attach(&mut d, source).unwrap();
        d.name = format!("Stock {id}");
        d.size.0 += 0.5;
        d.shank.head.length_mm *= 0.95;
        d.profile.width_mm *= 0.95;
        d.build.theta_steps = 96;
        d.imported_base.as_mut().unwrap().sand_envelope = sand;
        d.imported_base.as_mut().unwrap().bare = true;
        if !chart_enabled { d.imported_base.as_mut().unwrap().chart = None; }
        let mut g = lift::from_design(&d, &reg, &AlphaLibrary::default()).unwrap();
        let node = g.nodes.iter().find(|n| n.kind == "base.preset").unwrap().id;
        assert!(g.nodes.iter().all(|n| n.inputs.get("pointer") != Some(&Literal::Text("/imported_base".into()))));
        let text = file::graph_to_string(&g).unwrap();
        assert!(text.len() < 25_000, "{} bytes", text.len());
        g = file::load_graph_str(&text, Some(&reg)).unwrap();
        let (out, _) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
        same_source(&out, &d);
        let params = BuildParams { theta_steps: 96, profile_steps: 48, refine: None, ..Default::default() };
        let expected = ringdesign_core::mesh::try_build(&d, &AlphaLibrary::default(), params).unwrap().mesh;
        let actual = ringdesign_core::mesh::try_build(&out, &AlphaLibrary::default(), params).unwrap().mesh;
        assert!(expected.vertices == actual.vertices && expected.faces == actual.faces && expected.normals == actual.normals);
        g.set_input(node, "face_width_mm", Literal::Number(d.profile.width_mm * 1.05)).unwrap();
        let (edited, _) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
        assert_eq!(edited.profile.width_mm, d.profile.width_mm * 1.05);
        assert!(ringdesign_core::mesh::try_build(&edited, &AlphaLibrary::default(), params).unwrap().mesh.vertices != expected.vertices);
    }
    for (id, process) in [("002", ringdesign_core::castability::CastProcess::SandTwoPart), ("004", ringdesign_core::castability::CastProcess::LostWax)] {
        let mut g = Graph::new("Stock process", Mode::Free);
        let src = g.add("cad.source").unwrap();
        let mut d = RingDesign::default();
        ringdesign_core::castability::CastProcess::LostWax.apply(&mut d.draft);
        g.node_mut(src).unwrap().params = serde_json::to_value(&d).unwrap();
        let node = g.add("base.preset").unwrap();
        g.set_input(node, "id", Literal::Text(id.into())).unwrap();
        g.connect(src, "design", node, "design").unwrap();
        let Value::Design(d) = value(&g, node, "design", &reg) else { panic!("design") };
        assert_eq!(d.draft.process, process);
        assert_eq!(d.imported_base.as_ref().unwrap().sand_envelope, process == ringdesign_core::castability::CastProcess::SandTwoPart);
    }
}

#[test]
fn stamp_nodes_carry_shaped_tops_and_rows_and_lift_them_without_property_patches() {
    let reg = Registry::builtin();
    for kind in reg.keys().filter(|kind| kind.starts_with("stamp.outline.")) {
        let mut g = Graph::new(kind, Mode::SandRing);
        let id = g.add(kind).unwrap();
        let Value::Path(outline) = value(&g, id, "outline", &reg) else { panic!("outline") };
        ringdesign_core::outline::check(&outline).unwrap();
    }
    for shape in ["Flat", "Gable", "Ridge", "Cone", "Dome", "Taper"] {
        let mut g = Graph::new("Stamp lesson", Mode::SandRing);
        let source = g.add("cad.source").unwrap();
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::Flat);
        g.node_mut(source).unwrap().params = serde_json::to_value(&d).unwrap();
        let top = g.add("stamp.top").unwrap();
        g.set_input(top, "shape", Literal::Text(shape.into())).unwrap();
        let stamp = g.add("stamp").unwrap();
        g.set_input(stamp, "tier", Literal::Int(2)).unwrap();
        g.connect(top, "top", stamp, "top").unwrap();
        let family: setting::Stamp = serde_json::from_value(value(&g, stamp, "stamp", &reg).to_json_any().unwrap()).unwrap();
        assert_eq!(family.tier, 2);
        for (path_name, path) in [("PartingLine", RowPath::PartingLine), ("ChartV", RowPath::ChartV { v_mm: 0.5 }), ("SideFace", RowPath::SideFace { high: true, frac: 0.5 })] {
            let row = g.add("stamp.row").unwrap();
            g.connect(source, "design", row, "design").unwrap();
            g.connect(stamp, "stamp", row, "stamp").unwrap();
            for (pin, x) in [("v_mm", 0.5), ("from_deg", 32.0), ("to_deg", 74.0), ("taper", 0.3), ("fold_clear_mm", 0.1)] { g.set_input(row, pin, Literal::Number(x)).unwrap(); }
            g.set_input(row, "path", Literal::Text(path_name.into())).unwrap();
            g.set_input(row, "count", Literal::Int(4)).unwrap();
            g.set_input(row, "mirror_shoulders", Literal::Bool(true)).unwrap();
            let want = setting::stamp_row(&d, &StampRow { stamp: family.clone(), path, from_deg: 32.0, to_deg: 74.0, count: 4, taper: 0.3, fold_clear_mm: 0.1, mirror_shoulders: true });
            let got: Vec<setting::Stamp> = serde_json::from_value(value(&g, row, "stamps", &reg).to_json_any().unwrap()).unwrap();
            assert_eq!(got, want, "{shape}/{path_name}");
            let apply = g.add("design.stamps").unwrap();
            g.connect(source, "design", apply, "design").unwrap();
            g.connect(row, "stamps", apply, "stamps").unwrap();
            let Value::Design(authored) = value(&g, apply, "design", &reg) else { panic!("design") };
            assert_eq!(authored.stamps, want);
            let mut authored = (*authored).clone();
            authored.stamps.push(setting::Stamp { outline: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]], ..family.clone() });
            let lifted = lift::from_design(&authored, &reg, &AlphaLibrary::default()).unwrap();
            assert!(lifted.nodes.iter().all(|n| n.inputs.get("pointer") != Some(&Literal::Text("/stamps".into()))));
            let reloaded = file::load_graph_str(&file::graph_to_string(&lifted).unwrap(), Some(&reg)).unwrap();
            let (back, _) = design_of(&mut Evaluator::new(), &reloaded, &reg, &AlphaLibrary::default(), 0).unwrap();
            same_source(&back, &authored);
        }
    }
}

#[test]
fn shank_key_controls_rebuild_the_station_and_caiman_lifts_below_three_megabytes() {
    let reg = Registry::builtin();
    let mut d = RingDesign::default();
    d.shank.kind = ringdesign_core::ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = vec![ringdesign_core::profile::ShankKey { theta_deg: 90.0, width_scale: 1.4, thickness_scale: 1.2, crown_scale: 1.0 }, ringdesign_core::profile::ShankKey { theta_deg: 270.0, width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0 }];
    let mut g = lift::from_design(&d, &reg, &AlphaLibrary::default()).unwrap();
    let key = g.nodes.iter().find(|n| n.kind == "shank.key").unwrap().id;
    let (original, _) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
    same_source(&original, &d);
    g.set_input(key, "width_scale", Literal::Number(1.7)).unwrap();
    let (edited, _) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
    assert_eq!(edited.shank.keys[0].width_scale, 1.7);
    assert_ne!(original.section_at(90.0, 96, None, None).z_range(), edited.section_at(90.0, 96, None, None).z_range());
    let source = templates::refine_sources(&library::load_design(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../showcase/stock-masterworks/caiman/design.ring.json")).unwrap());
    let lib = ringdesign_core::manufacturing::source_library(&source, &AlphaLibrary::default()).into_owned();
    let g = lift::from_design(&source, &reg, &lib).unwrap();
    let text = file::graph_to_string(&g).unwrap();
    assert!(text.len() < 3_000_000, "Caiman graph is {} bytes", text.len());
    assert!(g.nodes.iter().all(|n| !matches!(n.inputs.get("pointer"), Some(Literal::Text(p)) if p.starts_with("/imported_base") || p.starts_with("/stamps"))));
    let g = file::load_graph_str(&text, Some(&reg)).unwrap();
    let (back, _) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::default(), 0).unwrap();
    same_source(&back, &source);
}
