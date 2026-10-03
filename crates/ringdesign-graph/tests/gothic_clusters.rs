//! Each Gothic cluster wired into a CAD feature history the way a packaged template wires it, built, and judged.

use ringdesign_core::cad::{Attach, Component, ComponentRole, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, PlaneBase, Profile, Stage, pattern::Sources};
use ringdesign_core::castability::{self, CastProcess, SandProcess, Verdict, judged_field_report};
use ringdesign_core::csg::{self, Solid};
use ringdesign_core::mesh::BuildResult;
use ringdesign_core::sketch::{FaceAnchor, Sketch, gothic};
use ringdesign_core::{AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind, dfm, mesh};
use ringdesign_graph::eval::{Evaluator, design_of};
use ringdesign_graph::graph::{Graph, NodeId};
use ringdesign_graph::nodes::{cad::from_document, cluster::add_cluster};
use ringdesign_graph::registry::Registry;
use ringdesign_graph::templates::{build_gothic_cluster, tenebrae_size};
use ringdesign_graph::value::Literal;

/// Undercut a part may show as facet noise, mm².
const PART_NOISE_MM2: f64 = 0.005;

fn params() -> BuildParams {
    BuildParams { theta_steps: 256, profile_steps: 128, refine: None, ..BuildParams::default() }
}

fn feature(id: u64, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}

fn cut() -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, stage: Stage::Cast, ..Component::default() }
}

fn placeholder() -> Operation {
    Operation::Sketch { sketch: Sketch::default() }
}

/// A band of `style` with square side faces at Tenebrae's bore.
fn band(style: ProfileStyle, width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Gothic cluster trial".into();
    d.size = ringdesign_core::sizing::RingSize(tenebrae_size());
    d.profile.apply_style(style);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.25;
    d.shank.kind = ShankKind::Uniform;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d
}

fn delft(d: &mut RingDesign) {
    d.draft.process = CastProcess::SandTwoPart;
    SandProcess::DelftClay.apply(&mut d.draft);
}

fn lost_wax(d: &mut RingDesign) {
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = 0.8;
}

/// `d`'s feature history lifted to a graph, the cluster's sketch wired into feature `sketch`, evaluated.
fn driven(d: &RingDesign, cluster: &str, values: &[(&str, Literal)], sketch: u64) -> RingDesign {
    let reg = Registry::builtin();
    let mut g: Graph = from_document(d).unwrap();
    let node = add_cluster(&mut g, &build_gothic_cluster(cluster).unwrap()).unwrap();
    for (pin, v) in values {
        g.set_input(node, *pin, v.clone()).unwrap();
    }
    g.connect(node, "sketch_op", NodeId(sketch), "operation").unwrap();
    assert!(g.validate(Some(&reg)).is_empty(), "{:?}", g.validate(Some(&reg)));
    let (out, report) = design_of(&mut Evaluator::new(), &g, &reg, &AlphaLibrary::builtin(), 0).unwrap();
    assert!(report.notes(&g).is_empty(), "{cluster}: {:?}", report.notes(&g));
    let mut out = (*out).clone();
    out.graph = None;
    out
}

fn sketch_of(d: &RingDesign, id: u64) -> Sketch {
    match &d.cad.as_ref().unwrap().feature(id).unwrap().operation {
        Operation::Sketch { sketch } => sketch.clone(),
        other => panic!("#{id} is {other:?}"),
    }
}

/// Watertight, no degenerate face, no crossing, and every feature resolved.
fn sound(built: &BuildResult, what: &str) {
    let v = &built.report.validation;
    assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{what}: {v:?}");
    assert!(built.parts.notes.is_empty(), "{what}: {:?}", built.parts.notes);
    let solid = Solid { v: built.mesh.vertices.iter().map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)]).collect(), f: built.mesh.faces.clone() };
    let check = solid.check(false);
    assert_eq!(check.zero_area_faces, 0, "{what}: {check:?}");
    assert_eq!(csg::self_crossings(&solid), 0, "{what}");
}

/// The sand verdict on the ring as built, its parts read against the parting plane.
fn castable(d: &RingDesign, built: &BuildResult, what: &str) {
    let f = judged_field_report(d, &AlphaLibrary::builtin(), &d.draft, 256, 128, Some(built));
    assert_eq!(f.verdict, Verdict::Castable, "{what}: {:?}", f.notes);
    for p in &f.parts {
        assert!(p.judged && p.undercut_area_mm2 - p.silhouette_mm2 < PART_NOISE_MM2, "{what}: {p:?}");
    }
}

/// The wheel on a work plane over the high side face at `offset`, cut `depth` with `draft`, and mirrored to the low face when `mirror`.
fn wheel_on(mut d: RingDesign, offset: f64, depth: f64, draft: f64, mirror: bool) -> RingDesign {
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Component::default())).unwrap();
    doc.append(feature(2, "High side face", Operation::Plane { base: PlaneBase::Parting, offset_mm: offset }, Component::default())).unwrap();
    doc.append(feature(3, "The wheel of lancet lights", placeholder(), Component::default())).unwrap();
    let cut_in = Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: -depth, draft_deg: draft };
    doc.append(feature(4, "Cut the lights", cut_in, cut())).unwrap();
    if mirror {
        let low = Operation::Pattern { sources: Sources(vec![4]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } };
        doc.append(feature(5, "Cut them in the low side face", low, cut())).unwrap();
    }
    d.cad = Some(doc);
    let out = driven(&d, "Wheel window", &[("Plane", Literal::Int(2))], 3);
    let bore = out.inner_radius_mm();
    let mut want = gothic::lights(&gothic::Lights { sill_r_mm: bore + 1.0, apex_r_mm: bore + 3.1 - 0.35, ..gothic::Lights::default() }).unwrap();
    want.name = "Wheel window".into();
    want.plane.on_face = Some(FaceAnchor { feature: 2, face: FaceRef::bare(0) });
    assert_eq!(serde_json::to_string(&sketch_of(&out, 3)).unwrap(), serde_json::to_string(&want).unwrap());
    out
}

#[test]
fn a_wheel_window_sunk_along_the_pull_into_both_side_faces_pours_in_delft_sand() {
    // A low dome with square sides pours Castable bare; the lights are blind drafted pockets from each side face.
    let mut d = band(ProfileStyle::LowDome, 7.0, 4.6);
    delft(&mut d);
    let out = wheel_on(d.clone(), 3.5, 1.5, 5.0, true);
    let built = mesh::try_build(&out, &AlphaLibrary::builtin(), params()).unwrap();
    sound(&built, "blind wheel");
    castable(&out, &built, "blind wheel");
    let plain = mesh::try_build(&RingDesign { cad: None, ..d }, &AlphaLibrary::builtin(), params()).unwrap();
    assert!(built.report.volume_mm3 < plain.report.volume_mm3 - 48.0, "{} against {}", built.report.volume_mm3, plain.report.volume_mm3);
}

#[test]
fn a_wheel_window_pierced_through_the_band_casts_in_lost_wax() {
    let mut d = band(ProfileStyle::LowDome, 7.0, 4.6);
    lost_wax(&mut d);
    let out = wheel_on(d.clone(), 3.8, 7.6, 0.0, false);
    let built = mesh::try_build(&out, &AlphaLibrary::builtin(), params()).unwrap();
    sound(&built, "pierced wheel");
    let f = judged_field_report(&out, &AlphaLibrary::builtin(), &out.draft, 256, 128, Some(&built));
    assert_eq!(f.verdict, Verdict::Castable, "{:?}", f.notes);
    let lands = dfm::cut_lands(&out, &built, 0.79);
    assert!(lands.is_empty(), "{lands:?}");
}

#[test]
fn a_revolved_pointed_arch_section_is_a_ring_that_pulls_both_ways() {
    let mut d = RingDesign::default();
    d.name = "Pointed-arch ring".into();
    d.size = ringdesign_core::sizing::RingSize(tenebrae_size());
    delft(&mut d);
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    let mut doc = Document::default();
    doc.append(feature(1, "The pointed-arch section", placeholder(), Component::default())).unwrap();
    let revolve = Operation::Revolve { sketch: Profile::Feature { feature: 1 }, pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false };
    doc.append(feature(2, "Revolve the ring", revolve, Component { role: ComponentRole::Shank, ..Component::default() })).unwrap();
    d.cad = Some(doc);
    let out = driven(&d, "Pointed-arch section", &[], 1);
    let mut want = gothic::arch_section(&gothic::ArchSection { bore_r_mm: out.inner_radius_mm(), ..gothic::ArchSection::default() }).unwrap();
    want.name = "Pointed-arch section".into();
    assert_eq!(serde_json::to_string(&sketch_of(&out, 1)).unwrap(), serde_json::to_string(&want).unwrap());
    let built = mesh::try_build(&out, &AlphaLibrary::builtin(), params()).unwrap();
    sound(&built, "pointed-arch ring");
    let report = castability::analyze(&built.mesh, &out.draft, out.inner_radius_mm());
    assert_eq!(report.verdict, Verdict::Castable, "{:?}", report.notes);
    assert!(report.undercut_area_mm2 < PART_NOISE_MM2, "{} mm² at {:.1}°", report.undercut_area_mm2, report.worst_draft_deg);
    // The tightest bore is the size's, and the keel stands on the parting plane at the section's thickness.
    let (lo, hi) = built.mesh.vertices.iter().map(|v| f64::from(v.0).hypot(f64::from(v.1))).fold((f64::MAX, 0.0f64), |(lo, hi), r| (lo.min(r), hi.max(r)));
    assert!(lo > out.inner_radius_mm() - 0.01 && hi < out.inner_radius_mm() + 3.6 + 0.01, "{lo} {hi}");
}

#[test]
fn a_rose_pierced_through_a_crown_keeps_its_bar_between_lights_in_lost_wax() {
    let mut d = band(ProfileStyle::Flat, 11.0, 2.0);
    lost_wax(&mut d);
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Component::default())).unwrap();
    doc.append(feature(2, "Crown", Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: 0.0 }, Component::default())).unwrap();
    doc.append(feature(3, "The rose", placeholder(), Component::default())).unwrap();
    let pierce = Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: -3.5, draft_deg: 0.0 };
    doc.append(feature(4, "Pierce the rose to the bore", pierce, cut())).unwrap();
    d.cad = Some(doc);
    let values = [("Plane", Literal::Int(2)), ("Inner radius", Literal::Number(1.2)), ("Outer radius", Literal::Number(4.6))];
    for head in ["Trefoil", "Pointed", "Mouchette"] {
        let mut values = values.to_vec();
        values.push(("Head", Literal::Text(head.into())));
        let out = driven(&d, "Rose tracery", &values, 3);
        let built = mesh::try_build(&out, &AlphaLibrary::builtin(), params()).unwrap();
        sound(&built, head);
        let f = judged_field_report(&out, &AlphaLibrary::builtin(), &out.draft, 256, 128, Some(&built));
        assert_eq!(f.verdict, Verdict::Castable, "{head}: {:?}", f.notes);
        let lands = dfm::cut_lands(&out, &built, 0.79);
        assert!(lands.is_empty(), "{head}: {lands:?}");
    }
}

#[test]
fn an_arcade_of_niches_sunk_along_the_pull_into_both_side_faces_pours_in_delft_sand() {
    let mut d = band(ProfileStyle::LowDome, 7.0, 4.6);
    delft(&mut d);
    let middle = d.inner_radius_mm() + 1.2;
    // Pointed and round niches are drafted; a trefoil's cusps take no draft and sink straight.
    for (head, draft) in [("Pointed", 5.0), ("Round", 5.0), ("Trefoil", 0.0)] {
        let mut doc = Document::default();
        doc.append(feature(1, "Procedural shank", Operation::Band, Component::default())).unwrap();
        doc.append(feature(2, "High side face", Operation::Plane { base: PlaneBase::Parting, offset_mm: 3.5 }, Component::default())).unwrap();
        doc.append(feature(3, "The arcade", placeholder(), Component::default())).unwrap();
        let sink = Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: -0.35, draft_deg: draft };
        doc.append(feature(4, "Sink the niches", sink, cut())).unwrap();
        let mirror = Operation::Pattern { sources: Sources(vec![4]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } };
        doc.append(feature(5, "Sink them in the low side face", mirror, cut())).unwrap();
        d.cad = Some(doc);
        let values = [("Plane", Literal::Int(2)), ("Y", Literal::Number(middle)), ("Height", Literal::Number(2.0)), ("Head", Literal::Text(head.into()))];
        let out = driven(&d, "Lancet arcade", &values, 3);
        let built = mesh::try_build(&out, &AlphaLibrary::builtin(), params()).unwrap();
        sound(&built, head);
        castable(&out, &built, head);
    }
}
