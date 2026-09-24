//! A viewport's part gestures as edits for the funnel: parts, stones, settings, mirrors, fillets, chamfers and command effects.
use crate::cad_tools;
use crate::command::Effect;
use crate::command::pattern::pattern_feature;
use ringdesign_core::{
    Mesh, RingDesign, Vec3,
    cad::{Attach, Component, ComponentRole, EdgeRef, Evaluated, EvaluatedComponent, Feature, MirrorPlane, Operation, PatternKind, Placement, builders, edit::CadEdit, pattern},
    gem::Gem,
    sketch::Id,
};

/// How far from a part a stone may stand and still be the one an array turns about, mm.
pub const STONE_REACH_MM: f64 = 8.0;
/// Closer than this to the plane it would be mirrored across, a part is its own mirror, mm or degrees.
const ON_PLANE: f64 = 0.05;

/// Ids no feature and no graph node holds yet, in order.
pub fn fresh_ids(design: &RingDesign) -> impl FnMut() -> Id + use<> {
    let doc = design.cad.as_ref().map_or(1, |d| d.fresh_id());
    let graph = design
        .graph
        .as_ref()
        .and_then(|g| serde_json::from_value::<ringdesign_graph::graph::Graph>(g.clone()).ok())
        .map_or(0, |g| g.nodes.iter().map(|n| n.id.0 + 1).max().unwrap_or(0).max(g.next_id));
    let mut next = doc.max(graph).max(1);
    move || {
        next += 1;
        next - 1
    }
}

/// Whether a feature builds a body of its own, as a plain ring's first part does: a sketch, a work plane and the band do not.
pub fn is_body(f: &Feature) -> bool {
    f.operation.sources().is_empty() && f.operation.has_body() && !matches!(f.operation, Operation::Band)
}

/// The procedural shank a plain ring's first part brings with it, so the part stands beside the band.
pub fn band_first(design: &RingDesign, next: &mut impl FnMut() -> Id) -> Option<CadEdit> {
    design.cad.as_ref().is_none_or(|d| d.features.is_empty()).then(|| {
        let component = Component { role: ComponentRole::Shank, ..Component::default() };
        CadEdit::Add { feature: Feature { id: next(), name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component }, after: None }
    })
}

/// The starter part called `label` seated at `theta_deg`, `height_mm` off the band and joined to it: its edits and its id.
pub fn part_here(design: &RingDesign, label: &str, theta_deg: f64, height_mm: f64) -> Result<(Vec<CadEdit>, Id), String> {
    let operation = cad_tools::starters(0, 0).into_iter().find(|op| op.label() == label).ok_or_else(|| format!("No part called {label}"))?;
    let mut next = fresh_ids(design);
    let mut edits: Vec<CadEdit> = band_first(design, &mut next).into_iter().collect();
    let shank = !edits.is_empty() || design.cad.as_ref().is_some_and(|d| d.band().is_some());
    let mut component = Component { placement: Placement::ring(theta_deg, height_mm), ..Component::default() };
    if matches!(operation, Operation::Torus { .. } | Operation::TwistedRing { .. } | Operation::Revolve { .. }) {
        component.role = ComponentRole::Shank;
    }
    if shank {
        component.attach = Attach::Join;
    }
    let id = next();
    edits.push(CadEdit::Add { feature: Feature { id, name: operation.label().into(), enabled: true, operation, component }, after: None });
    Ok((edits, id))
}

/// A reference stone named by `key` seated at `theta_deg`, its culet clear of the metal: its edits and its id.
pub fn stone_here(design: &RingDesign, theta_deg: f64, key: &str) -> Result<(Vec<CadEdit>, Id), String> {
    let preset = builders::stone_preset(key).ok_or_else(|| format!("No stone called {key}"))?;
    let gem = preset.gem();
    let mut next = fresh_ids(design);
    let mut edits: Vec<CadEdit> = band_first(design, &mut next).into_iter().collect();
    let id = next();
    edits.push(CadEdit::Add { feature: builders::stone_feature(id, gem, Placement::ring(theta_deg, builders::stand_off_mm("claw4", gem))), after: None });
    Ok((edits, id))
}

/// Stone `key` seated on planar face `face` of part `feature` of the ring as built, where `at` falls on it or at its centroid: edits, stone id.
pub fn stone_on_face(design: &RingDesign, built: Option<&Evaluated>, feature: Id, face: u32, at: Option<[f64; 3]>, key: &str) -> Result<(Vec<CadEdit>, Id), String> {
    let preset = builders::stone_preset(key).ok_or_else(|| format!("No stone called {key}"))?;
    let gem = preset.gem();
    let part = built.and_then(|e| e.components.iter().find(|c| c.id == feature)).ok_or_else(|| format!("Part #{feature} is not in the ring as built yet"))?;
    let seat = ringdesign_core::cad::FaceSeat::on(part, face, at, builders::stand_off_mm("claw4", gem)).map_err(|e| format!("{e:#}"))?;
    let id = fresh_ids(design)();
    Ok((vec![CadEdit::Add { feature: ringdesign_core::cad::stone_on_face(id, gem, feature, &seat), after: None }], id))
}

/// Setting `key` round stone part `part`, or round height-field stone `stone` first seated as a part on `mesh`: edits, head id.
pub fn setting(design: &RingDesign, mesh: Option<&Mesh>, part: Option<Id>, stone: Option<&[usize]>, key: &str) -> Result<(Vec<CadEdit>, Option<Id>), String> {
    builders::setting_preset(key).ok_or_else(|| format!("No setting called {key}"))?;
    let mut next = fresh_ids(design);
    let mut edits = Vec::new();
    let (stone_id, gem) = match (part, stone) {
        (Some(id), _) => {
            let (gem, placement) = stone_part(design, id, key)?;
            edits.extend(placement.map(|placement| CadEdit::Placement { id, placement }));
            (id, gem)
        }
        (None, Some(path)) => {
            let (placement, gem) = seat_stone(design, mesh, path).ok_or("That stone is no longer in the design")?;
            edits.extend(band_first(design, &mut next));
            let id = next();
            edits.push(CadEdit::Add { feature: builders::stone_feature(id, gem, placement), after: None });
            (id, gem)
        }
        (None, None) => return Err("Choose a stone to set".into()),
    };
    let features = builders::setting_features(key, stone_id, gem, builders::sand(design), &mut next).map_err(|e| format!("{e:#}"))?;
    let head = features.first().map(|f| f.id);
    edits.extend(features.into_iter().map(|feature| CadEdit::Add { feature, after: None }));
    Ok((edits, head))
}

/// The gem of stone part `id`, and the placement that stands it where setting `key` holds it when it stands on the ring.
fn stone_part(design: &RingDesign, id: Id, key: &str) -> Result<(Gem, Option<Placement>), String> {
    let f = design.cad.as_ref().and_then(|d| d.feature(id)).ok_or_else(|| format!("No part #{id}"))?;
    let Operation::Builder { key: builder, params, .. } = &f.operation else {
        return Err(format!("#{id} {} is not a stone", f.name));
    };
    if builder != builders::STONE {
        return Err(format!("#{id} {} is not a stone", f.name));
    }
    let gem = builders::gem_of(params).map_err(|e| format!("{e:#}"))?;
    let placement = match f.component.placement.clone() {
        Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } => {
            let held = builders::stand_off_mm(key, gem);
            ((held - height_mm).abs() > 1e-9).then_some(Placement::Ring { theta_deg, across_mm, height_mm: held, spin_deg, tilt_deg, cant_deg })
        }
        Placement::Free => None,
    };
    Ok((gem, placement))
}

/// A height-field stone as a ring placement at its own girdle frame, read against `mesh` when there is one, and its gem.
fn seat_stone(design: &RingDesign, mesh: Option<&Mesh>, path: &[usize]) -> Option<(Placement, Gem)> {
    let (st, frame) = ringdesign_core::stones::stone_frames(design).into_iter().find(|(s, _)| s.path == path)?;
    let stand_off = st.stand_off_mm();
    let across_mm = frame.girdle[2] - frame.normal[2] * stand_off;
    let height_mm = mesh
        .and_then(|m| ringdesign_core::cad::surface_hit(m, st.theta_deg, across_mm))
        .map_or(stand_off, |(hit, n)| (0..3).map(|k| (frame.girdle[k] - hit[k]) * n[k]).sum());
    Some((Placement::Ring { theta_deg: st.theta_deg, across_mm, height_mm, spin_deg: st.rot_deg(), tilt_deg: 0.0, cant_deg: 0.0 }, st.gem))
}

/// The mirror of part `f` across `plane`, meeting the band by `attach`; refused when the part stands on the plane.
pub fn mirror(design: &RingDesign, f: &Feature, attach: Attach, plane: MirrorPlane) -> Result<CadEdit, String> {
    if let Some(doc) = design.cad.as_ref()
        && let Some((_, Placement::Ring { theta_deg, across_mm, .. })) = pattern::seat_of(doc, f.id)
    {
        let on = match &plane {
            MirrorPlane::Band => across_mm.abs() < ON_PLANE,
            MirrorPlane::Section { theta_deg: through } => ((theta_deg - through + 180.0).rem_euclid(360.0) - 180.0).abs() < ON_PLANE,
            MirrorPlane::Plane { .. } => false,
        };
        if on {
            return Err(format!("#{} {} stands on that plane already; its mirror would be itself", f.id, f.name));
        }
    }
    Ok(CadEdit::Add { feature: pattern_feature(0, f, attach, PatternKind::Mirror { plane }), after: None })
}

/// A new Fillet or Chamfer, named by `label`, on edge `edge` of part `part`, meeting the band as the part does.
pub fn modifier(design: &RingDesign, label: &str, part: Id, edge: u32) -> Result<CadEdit, String> {
    let source = design.cad.as_ref().and_then(|d| d.feature(part)).ok_or_else(|| format!("No part #{part}"))?;
    let mut operation = cad_tools::starters(part, 0).into_iter().find(|op| op.label() == label && cad_tools::modify(op)).ok_or_else(|| format!("No modifier called {label}"))?;
    match &mut operation {
        Operation::Fillet { edges, .. } | Operation::Chamfer { edges, .. } => *edges = vec![EdgeRef::bare(edge as usize)],
        _ => return Err(format!("{label} does not take an edge")),
    }
    let component = Component { placement: Placement::Free, ..source.component.clone() };
    Ok(CadEdit::Add { feature: Feature { id: 0, name: label.into(), enabled: true, operation, component }, after: None })
}

/// The stone `f` is built round or seated by, else the nearest stone within `STONE_REACH_MM` of the part as built.
pub fn stone_by(design: &RingDesign, evaluated: &Evaluated, f: &Feature, c: &EvaluatedComponent) -> Option<Id> {
    let doc = design.cad.as_ref()?;
    let is_stone = |id: Id| doc.feature(id).is_some_and(|s| matches!(&s.operation, Operation::Builder { key, .. } if key == builders::STONE));
    if let Some((seat, _)) = pattern::seat_of(doc, f.id).filter(|(seat, _)| is_stone(*seat)) {
        return Some(seat);
    }
    let (lo, hi) = c.mesh.bounds()?;
    let centre = [(lo.0 + hi.0) as f64 * 0.5, (lo.1 + hi.1) as f64 * 0.5, (lo.2 + hi.2) as f64 * 0.5];
    let gap = |p: [f64; 3]| (0..3).map(|k| (p[k] - centre[k]).powi(2)).sum::<f64>().sqrt();
    evaluated.components.iter().filter(|s| is_stone(s.id)).map(|s| (s.id, gap(s.frame.origin))).filter(|(_, d)| *d <= STONE_REACH_MM).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
}

/// A pattern's copies as its ghost shows them: the source's mesh carried onto each, which triangles belong to a copy left out, and what is said of those.
#[derive(Clone, Debug, Default)]
pub struct Copies {
    pub mesh: Mesh,
    /// Per face: part of a copy the pattern leaves out for standing off its face.
    pub refused: Vec<bool>,
    pub note: Option<String>,
}

/// Every copy a pattern `kind` of `source_id` would stand, as the evaluation places them on the parts `evaluated` on `surface`: `source`, the part's mesh,
/// carried onto each, the copies it would leave out for standing off their face refused and named by their angle.
pub fn copies(design: &RingDesign, surface: Option<&Mesh>, evaluated: &Evaluated, source_id: Id, source: &Mesh, kind: &PatternKind) -> Option<Copies> {
    let instances = pattern::copy_instances(design, surface, evaluated, source_id, kind).ok()?;
    let mut out = Copies::default();
    for i in &instances {
        let (m, base) = (i.motion, out.mesh.vertices.len() as u32);
        out.mesh.vertices.extend(source.vertices.iter().map(|v| {
            let p = m.point([f64::from(v.0), f64::from(v.1), f64::from(v.2)]);
            Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
        }));
        let flip = m.reflects();
        out.mesh.faces.extend(source.faces.iter().map(|f| if flip { [f[0] + base, f[2] + base, f[1] + base] } else { f.map(|k| k + base) }));
        out.refused.extend(std::iter::repeat_n(i.off_face, source.faces.len()));
    }
    let off: Vec<String> = instances.iter().filter(|i| i.off_face).map(|i| format!("{:.0}°", i.angle_deg)).collect();
    if !off.is_empty() {
        let host = design.cad.as_ref().and_then(|d| d.feature(ringdesign_core::cad::face_stone(d, source_id)?.1)).map_or_else(|| "its part".to_string(), |f| format!("#{} {}", f.id, f.name));
        let stand = if off.len() == 1 { "1 copy stands".to_string() } else { format!("{} copies stand", off.len()) };
        out.note = Some(format!("{stand} off the face of {host}, left out: {}", off.join(", ")));
    }
    Some(out)
}

/// A command's effects as funnel edits and whether one adds a part; a plain ring's first feature, a work plane or a sketch included, brings its shank, a ring of parts keeps a body apart.
pub fn effect_edits(design: &RingDesign, effects: Vec<Effect>) -> (Vec<CadEdit>, bool) {
    let mut edits = Vec::new();
    let mut added = false;
    for effect in effects {
        match effect {
            Effect::Placement { feature, placement } => edits.push(CadEdit::Placement { id: feature, placement }),
            Effect::Operation { feature, operation } => edits.push(CadEdit::Operation { id: feature, operation }),
            Effect::Attach { feature, attach } => edits.push(CadEdit::Attach { id: feature, attach }),
            Effect::Stage { feature, stage } => edits.push(CadEdit::Stage { id: feature, stage }),
            Effect::Add { mut feature } => {
                let doc = design.cad.as_ref();
                if doc.is_none_or(|d| d.features.is_empty()) && !matches!(feature.operation, Operation::Band) {
                    let id = if feature.id == 1 { 2 } else { 1 };
                    let component = Component { role: ComponentRole::Shank, ..Component::default() };
                    edits.push(CadEdit::Add { feature: Feature { id, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component }, after: None });
                } else if doc.is_some_and(|d| d.band().is_none()) && is_body(&feature) {
                    feature.component.attach = Attach::Separate;
                }
                added = true;
                edits.push(CadEdit::Add { feature, after: None });
            }
        }
    }
    (edits, added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::touch::funnel::prepare;
    use ringdesign_core::{AlphaLibrary, BuildParams, cad::Stage, mesh, templates};

    fn template(name: &str) -> RingDesign {
        templates::all().iter().find(|t| t.name == name).unwrap().design()
    }
    fn params() -> BuildParams {
        BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..BuildParams::default() }
    }
    /// The design every edit lands on, through the funnel.
    fn landed(d: &RingDesign, edits: &[CadEdit]) -> RingDesign {
        prepare(d, edits, None).unwrap().unwrap().design
    }

    #[test]
    fn a_part_touched_onto_a_plain_band_brings_its_shank_and_joins_it_where_the_finger_was() {
        let d = template("Court band");
        let (edits, id) = part_here(&d, "Cylinder", 90.0, 0.5).unwrap();
        let after = landed(&d, &edits);
        let doc = after.cad.as_ref().unwrap();
        assert_eq!(doc.features.iter().map(|f| (f.id, f.name.as_str())).collect::<Vec<_>>(), [(1, "Procedural shank"), (2, "Cylinder")]);
        assert_eq!(id, 2);
        let post = doc.feature(2).unwrap();
        assert_eq!((post.component.placement.clone(), post.component.attach), (Placement::ring(90.0, 0.5), Attach::Join));
        assert_eq!(doc.feature(1).unwrap().component.role, ComponentRole::Shank);
        // A second part finds the shank already there.
        let (edits, id) = part_here(&after, "Box", 180.0, 0.0).unwrap();
        assert_eq!((edits.len(), id), (1, 3));
        assert!(part_here(&after, "Teapot", 0.0, 0.0).is_err());
    }

    #[test]
    fn a_stone_stands_its_culet_clear_and_a_setting_is_built_round_it_for_the_bench_under_sand() {
        let d = template("Court band");
        let (edits, stone) = stone_here(&d, 90.0, "round-6.5").unwrap();
        let d = landed(&d, &edits);
        let f = d.cad.as_ref().unwrap().feature(stone).unwrap().clone();
        let gem = builders::stone_preset("round-6.5").unwrap().gem();
        let clear = gem.pavilion_mm() + builders::CULET_CLEAR_MM;
        assert_eq!(f.component.placement, Placement::ring(90.0, clear));
        assert!(f.component.reference && f.name == "Round 6.5 mm", "{}", f.name);
        // Claws hold the stone where it stands: nothing moves, a head and a bur are added on it.
        let (edits, head) = setting(&d, None, Some(stone), None, "claw4").unwrap();
        assert_eq!(edits.len(), 2);
        let after = landed(&d, &edits);
        let doc = after.cad.as_ref().unwrap();
        let added: Vec<&Feature> = doc.features.iter().filter(|f| matches!(&f.operation, Operation::Builder { on: Some(s), .. } if *s == stone)).collect();
        assert_eq!(added.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["Four-claw head", "Seat bur"]);
        assert_eq!(head, Some(added[0].id));
        assert!(added.iter().all(|f| f.component.stage == Stage::Bench), "under sand a head is soldered on and its seat drilled after the pour");
        // A bezel sinks the stone to its collet: the stone moves down to what the bezel holds.
        let (edits, _) = setting(&d, None, Some(stone), None, "bezel").unwrap();
        let CadEdit::Placement { placement: Placement::Ring { height_mm, .. }, .. } = &edits[0] else { panic!("{:?}", edits[0]) };
        assert!((height_mm - builders::stand_off_mm("bezel", gem)).abs() < 1e-12 && *height_mm < clear, "{height_mm} against {clear}");
        assert!(setting(&d, None, Some(1), None, "claw4").unwrap_err().contains("not a stone"));
        assert!(setting(&d, None, Some(stone), None, "pave").is_err());
    }

    #[test]
    fn a_height_field_stone_is_set_as_a_part_seated_where_the_built_ring_carries_it() {
        let d = template("Cathedral solitaire stock");
        let (st, frame) = ringdesign_core::stones::stone_frames(&d).into_iter().next().expect("the stock sets its solitaire");
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let (edits, head) = setting(&d, Some(&built.mesh), None, Some(&st.path), "claw6").unwrap();
        let names: Vec<String> = edits.iter().map(|e| match e {
            CadEdit::Add { feature, .. } => feature.name.clone(),
            other => format!("{other:?}"),
        }).collect();
        assert_eq!(names[0], "Procedural shank");
        assert_eq!(names[2..], ["Six-claw head".to_string(), "Seat bur".to_string()]);
        let CadEdit::Add { feature: stone, .. } = &edits[1] else { unreachable!() };
        let Placement::Ring { theta_deg, across_mm, height_mm, .. } = stone.component.placement else { panic!() };
        assert!((theta_deg - st.theta_deg).abs() < 1e-12);
        assert!((across_mm - (frame.girdle[2] - frame.normal[2] * st.stand_off_mm())).abs() < 1e-12);
        // The girdle stands where the height field put it, measured off the built surface: 0.0059 mm at 128 × 64.
        let (hit, n) = ringdesign_core::cad::surface_hit(&built.mesh, theta_deg, across_mm).unwrap();
        let girdle: [f64; 3] = std::array::from_fn(|k| hit[k] + n[k] * height_mm);
        let off = (0..3).map(|k| (girdle[k] - frame.girdle[k]).powi(2)).sum::<f64>().sqrt();
        assert!(off < 0.01, "the girdle lands {off:.4} mm from where the height field set it");
        assert_eq!(head, Some(3));
        assert!(setting(&d, Some(&built.mesh), None, Some(&[99]), "claw6").is_err());
    }

    #[test]
    fn a_mirror_is_refused_on_its_own_plane_and_a_modifier_meets_the_band_as_its_part_does() {
        let d = template("Court band");
        let (edits, id) = part_here(&d, "Cylinder", 60.0, 0.0).unwrap();
        let d = landed(&d, &edits);
        let post = d.cad.as_ref().unwrap().feature(id).unwrap().clone();
        assert!(mirror(&d, &post, Attach::Join, MirrorPlane::Band).unwrap_err().contains("stands on that plane"));
        let CadEdit::Add { feature, .. } = mirror(&d, &post, Attach::Join, MirrorPlane::Section { theta_deg: 90.0 }).unwrap() else { unreachable!() };
        assert_eq!((feature.name.as_str(), feature.component.attach), ("Mirror of Cylinder", Attach::Join));
        let Operation::Pattern { source, kind } = &feature.operation else { panic!() };
        assert_eq!((*source, kind), (id, &PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } }));
        assert!(mirror(&d, &post, Attach::Join, MirrorPlane::Section { theta_deg: 60.0 }).is_err(), "through the head it stands on");
        let CadEdit::Add { feature, .. } = modifier(&d, "Fillet", id, 3).unwrap() else { unreachable!() };
        let Operation::Fillet { source, edges, .. } = &feature.operation else { panic!() };
        assert_eq!((*source, edges.as_slice()), (id, [EdgeRef::bare(3)].as_slice()));
        assert_eq!((feature.component.attach, feature.component.placement.clone()), (Attach::Join, Placement::Free));
        assert!(matches!(modifier(&d, "Chamfer", id, 0).unwrap(), CadEdit::Add { feature: Feature { operation: Operation::Chamfer { .. }, .. }, .. }));
        assert!(modifier(&d, "Box", id, 0).is_err() && modifier(&d, "Fillet", 42, 0).is_err());
    }

    #[test]
    fn a_commands_first_body_brings_its_shank_and_a_ring_of_parts_keeps_it_separate() {
        let plain = template("Court band");
        let body = Feature { id: 1, name: "Box".into(), enabled: true, operation: Operation::Box { size: [1.0; 3] }, component: Component { attach: Attach::Join, ..Component::default() } };
        let (edits, added) = effect_edits(&plain, vec![Effect::Add { feature: body.clone() }]);
        assert!(added);
        let ids: Vec<(Id, bool)> = edits.iter().map(|e| match e {
            CadEdit::Add { feature, .. } => (feature.id, matches!(feature.operation, Operation::Band)),
            other => panic!("{other:?}"),
        }).collect();
        assert_eq!(ids, [(2, true), (1, false)], "the shank takes the id the body did not");
        let parts_only = ringdesign_core::cad::examples::design("twisted-band").unwrap();
        assert!(parts_only.cad.as_ref().unwrap().band().is_none());
        let (edits, _) = effect_edits(&parts_only, vec![Effect::Add { feature: body }, Effect::Stage { feature: 1, stage: Stage::Bench }]);
        let CadEdit::Add { feature, .. } = &edits[0] else { unreachable!() };
        assert_eq!(feature.component.attach, Attach::Separate);
        assert!(matches!(edits[1], CadEdit::Stage { id: 1, stage: Stage::Bench }));
        let (moved, added) = effect_edits(&plain, vec![Effect::Placement { feature: 2, placement: Placement::ring(10.0, 0.0) }]);
        assert!(!added && matches!(&moved[..], [CadEdit::Placement { id: 2, .. }]));
    }

    #[test]
    fn an_array_turns_about_the_stone_its_part_stands_on_or_the_nearest_within_reach() {
        let d = template("Court band");
        let (edits, stone) = stone_here(&d, 90.0, "round-5").unwrap();
        let d = landed(&d, &edits);
        let (edits, head) = setting(&d, None, Some(stone), None, "claw4").unwrap();
        let d = landed(&d, &edits);
        let (edits, far) = part_here(&d, "Sphere", 270.0, 0.0).unwrap();
        let d = landed(&d, &edits);
        let e = ringdesign_core::cad::evaluate(&d, &AlphaLibrary::builtin(), params()).unwrap();
        let of = |id: Id| (d.cad.as_ref().unwrap().feature(id).unwrap().clone(), e.components.iter().find(|c| c.id == id).unwrap().clone());
        let (f, c) = of(head.unwrap());
        assert_eq!(stone_by(&d, &e, &f, &c), Some(stone), "a head stands on its stone");
        let (f, c) = of(far);
        assert_eq!(stone_by(&d, &e, &f, &c), None, "the palm is further than 8 mm from the stone at the top");
    }

    /// The Court band with a 4 × 14 mm plate joined at its top and a 1.5 mm stone on the plate in four claws, the head #4.
    fn plate_with_head() -> RingDesign {
        use ringdesign_core::{
            cad::{FaceSeat, Document},
            gem::{Gem, GemCut},
        };
        let mut d = template("Court band");
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Component::default() } }).unwrap();
        let plate = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.65), ..Component::default() };
        doc.append(Feature { id: 2, name: "Plate".into(), enabled: true, operation: Operation::Box { size: [4.0, 14.0, 1.5] }, component: plate }).unwrap();
        d.cad = Some(doc.clone());
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let host = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 2).unwrap();
        // The plate's top: the planar face turned most nearly along the part's own z.
        let outward = |f: u32| FaceSeat::on(host, f, None, 0.0).ok().and_then(|s| s.face_of(host).ok()).map(|fr| (0..3).map(|k| fr.normal[k] * host.frame.z_axis[k]).sum::<f64>());
        let top = (0..host.body.faces.len() as u32).filter_map(|f| outward(f).map(|w| (f, w))).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
        let gem = Gem::calibrated(GemCut::Round, 1.5);
        let seat = FaceSeat::on(host, top, None, builders::stand_off_mm("claw4", gem)).unwrap();
        doc.append(ringdesign_core::cad::stone_on_face(3, gem, 2, &seat)).unwrap();
        doc.append(builders::feature_on(4, "Four-claw head", builders::CLAW, 3, serde_json::json!({ "prongs": 4 }))).unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn a_face_arrays_ghost_shows_the_copies_it_would_leave_out_refused_and_names_them() {
        let d = plate_with_head();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), params());
        let e = built.parts.evaluated.as_ref().unwrap();
        let head = e.components.iter().find(|c| c.id == 4).expect("the head built");
        let n = head.mesh.faces.len();
        // Four heads 24° apart over 72°: the copies at 48° and 72° stand off the plate's 14 mm.
        let kind = PatternKind::Ring { count: 4, span_deg: 72.0 };
        let c = copies(&d, built.band.as_deref(), e, 4, &head.mesh, &kind).unwrap();
        assert_eq!((c.mesh.faces.len(), c.refused.len(), c.mesh.vertices.len()), (3 * n, 3 * n, 3 * head.mesh.vertices.len()));
        assert!(c.refused[..n].iter().all(|r| !r) && c.refused[n..].iter().all(|r| *r), "the 24° copy stays, the other two are refused");
        assert_eq!(c.note.as_deref(), Some("2 copies stand off the face of #2 Plate, left out: 48°, 72°"));
        // Two over 24° keep both on the plate and say nothing.
        let near = copies(&d, built.band.as_deref(), e, 4, &head.mesh, &PatternKind::Ring { count: 2, span_deg: 24.0 }).unwrap();
        assert!(near.refused.iter().all(|r| !r) && near.note.is_none() && near.mesh.faces.len() == n);
        // Each kept copy is the head carried by the motion the evaluation places it with.
        let motions = pattern::copy_motions(&d, built.band.as_deref(), e, 4, &kind).unwrap();
        let v = head.mesh.vertices[0];
        let at = motions[0].point([f64::from(v.0), f64::from(v.1), f64::from(v.2)]);
        let got = c.mesh.vertices[0];
        assert!((0..3).all(|k| (f64::from([got.0, got.1, got.2][k]) - at[k]).abs() < 1e-4), "{got:?} against {at:?}");
    }
}
