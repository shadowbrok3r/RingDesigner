//! Officina — Keystone, a drafted cartouche: a raised, drafted cartouche on a flat band, pressed
//! taller, its rim rounded by edge signature, with a bright-cut facet in its top. Delft sand.
//! cargo build --release -p ringdesign-core --example officina_keystone
//! target/release/examples/officina_keystone [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{
        self, Attach, Component, ComponentRole, Document, EdgeRef, FaceRef, Feature,
        FeatureStatus, MirrorPlane, Operation, PatternKind, PlaneBase, Profile, Stage, SurfaceKind,
    },
    castability::{self, CastProcess, SandProcess},
    csg, dfm, library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    sketch::{FaceAnchor, Geometry, Id, RegionRef, Sketch},
    stl,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const BORE: f64 = 18.2;
/// Where the cartouche stands round the ring: the top.
const THETA: f64 = 90.0;
/// The work plane sinks under the crest so the boss stays joined where the band curves away.
const PLANE_SINK_MM: f64 = 0.7;
/// The cartouche's plan: round the ring, across the finger, its corner radius, and the straight
/// lines each corner is drawn with.
const CARTOUCHE_LONG_MM: f64 = 7.0;
const CARTOUCHE_WIDE_MM: f64 = 4.8;
const CARTOUCHE_CORNER_MM: f64 = 1.0;
const CORNER_SEGMENTS: usize = 12;
const BOSS_MM: f64 = 1.6;
const BOSS_DRAFT_DEG: f64 = 12.0;
const BOSS_BLEND_MM: f64 = 0.45;
const PULL_MM: f64 = 0.8;
/// The rim radii the native fillet is tried at, largest first.
const RIM_ROUNDS_MM: [f64; 4] = [0.35, 0.3, 0.25, 0.2];
const RIM_CHAMFER_MM: f64 = 0.25;
/// The bright-cut lozenge on the top, round the ring and across. Each half is one sloped plane,
/// level with the top on the long diagonal and `FACET_DEPTH_MM` deep at its apex, so the two
/// halves meet on a ridge and catch the light in two tones.
const FACET_LONG_MM: f64 = 4.6;
const FACET_WIDE_MM: f64 = 2.6;
const FACET_DEPTH_MM: f64 = 0.45;
/// The graver's walls open out above the facet's floor.
const FACET_WALL_DEG: f64 = -20.0;
/// How far the cutter stands above the facet's plane: clear of the top everywhere.
const FACET_CLEAR_MM: f64 = 1.0;
/// The band's outer arrises are rounded this much.
const BAND_EDGE_MM: f64 = 0.35;
const ALLOY: &str = "Gold 14k";

fn draft_params() -> BuildParams {
    BuildParams {
        theta_steps: 768,
        profile_steps: 320,
        ..BuildParams::default()
    }
}
fn export_params() -> BuildParams {
    BuildParams {
        theta_steps: 1536,
        profile_steps: 448,
        ..BuildParams::default()
    }
}

/// Delft clay, gated at the palm, in a 70 mm flask.
fn setup() -> Setup {
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).unwrap().shrink_pct;
    s.recipe.process = CastProcess::SandTwoPart;
    s.recipe.name = format!("Delft clay starting recipe / {ALLOY}");
    s.recipe.calibration_note = "Uncalibrated starting allowance. Confirm with the caster's alloy, pattern material, mould and measured trials.".into();
    s.sample_pitch_mm = 0.10;
    s.flask.width_mm = 70.0;
    s.flask.length_mm = 70.0;
    s.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0.0, -11.0, 0.0],
            end: [0.0, -22.0, 0.0],
            diameter_mm: 3.2,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0.0, -22.0, 0.0],
            end: [0.0, -30.0, 0.0],
            diameter_mm: 5.0,
        },
    ];
    s.bench_notes = "Pour the band and the drafted cartouche as one solid in Delft clay, parted on the band's mid-plane and pulled along the finger. The cartouche's end walls and its flat top stand square to the pull: ram the sand firmly over the top and draw the pattern slowly. Cut the bright-cut facet at the bench after the pour with a graver, then polish the top.".into();
    s
}

/// Flat 6.0 x 2.0 with squared sides, bore 18.2 mm: the plain band the cartouche stands on.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: "Keystone \u{2014} a drafted cartouche".into(),
        ..RingDesign::default()
    };
    d.size = ringdesign_core::resize::size_from_bore(BORE).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = 2.0;
    d.profile.flatten_sides();
    // Square walls, but the outer arrises broken so the band is not cut sheet.
    d.profile.edge_round_mm = BAND_EDGE_MM;
    SandProcess::DelftClay.apply(&mut d.draft);
    CastProcess::SandTwoPart.apply(&mut d.draft);
    d.manufacturing = Some(setup());
    d
}

fn component(role: ComponentRole, attach: Attach, stage: Stage, blend_mm: f64) -> Component {
    Component {
        role,
        material: ALLOY.into(),
        attach,
        stage,
        blend_mm,
        ..Component::default()
    }
}
fn joined() -> Component {
    component(ComponentRole::Head, Attach::Join, Stage::Cast, BOSS_BLEND_MM)
}
fn bare() -> Component {
    component(ComponentRole::Other, Attach::Separate, Stage::Cast, 0.0)
}

fn add(d: &mut RingDesign, name: &str, operation: Operation, component: Component) -> Result<Id> {
    let doc = d.cad.get_or_insert_with(Document::default);
    let id = doc.features.len() as Id + 1;
    doc.append(Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component,
    })?;
    Ok(id)
}

/// A cartouche plan `long` along the sketch's x by `wide` along its y, its corners rounded to
/// radius `r`, each corner arc drawn as `segments` straight lines. Straight sides keep every
/// drafted wall a plane, which the kernel can press-pull and chamfer; a drafted arc is a cone,
/// which it can do neither to.
fn cartouche(name: &str, long: f64, wide: f64, r: f64, segments: usize) -> Sketch {
    let mut s = Sketch {
        name: name.into(),
        ..Sketch::default()
    };
    let (a, b) = (long / 2.0 - r, wide / 2.0 - r);
    let mut corners = Vec::new();
    for (cx, cy, from) in [(a, -b, -90.0f64), (a, b, 0.0), (-a, b, 90.0), (-a, -b, 180.0)] {
        for k in 0..=segments {
            let t = (from + 90.0 * k as f64 / segments as f64).to_radians();
            corners.push([cx + r * t.cos(), cy + r * t.sin()]);
        }
    }
    let ids: Vec<Id> = corners.iter().map(|p| s.point(*p)).collect();
    for k in 0..ids.len() {
        s.entity(Geometry::Line {
            a: ids[k],
            b: ids[(k + 1) % ids.len()],
        });
    }
    s
}

/// A lozenge `long` along x by `wide` along y, split by its long diagonal into two triangles;
/// returns the sketch and an entity on each triangle's rim, upper first.
fn lozenge(name: &str, long: f64, wide: f64) -> (Sketch, Id, Id) {
    let mut s = Sketch {
        name: name.into(),
        ..Sketch::default()
    };
    let w = s.point([-long / 2.0, 0.0]);
    let e = s.point([long / 2.0, 0.0]);
    let n = s.point([0.0, wide / 2.0]);
    let so = s.point([0.0, -wide / 2.0]);
    let upper = s.entity(Geometry::Line { a: e, b: n });
    s.entity(Geometry::Line { a: n, b: w });
    let lower = s.entity(Geometry::Line { a: w, b: so });
    s.entity(Geometry::Line { a: so, b: e });
    s.entity(Geometry::Line { a: w, b: e });
    (s, upper, lower)
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The planar face of part `id` whose normal runs along `normal` and stands furthest out along it.
fn top_face(e: &cad::Evaluated, id: Id, normal: P3) -> Result<(usize, f64)> {
    let c = e
        .components
        .iter()
        .find(|c| c.id == id)
        .with_context(|| format!("feature #{id} built no part"))?;
    let mut best: Option<(usize, f64)> = None;
    for i in 0..c.body.faces.len() {
        let Some(s) = cad::face_signature(&c.body, i, &c.frame) else { continue };
        if s.kind != SurfaceKind::Plane || dot(s.normal, normal) < 0.999 {
            continue;
        }
        let out = dot(s.centre, normal);
        if best.is_none_or(|(_, b)| out > b) {
            best = Some((i, out));
        }
    }
    best.with_context(|| format!("feature #{id} has no planar face along {normal:?}"))
}

/// The edges of part `id` that bound its top face: every edge whose midpoint lies on the top's plane.
fn rim_edges(e: &cad::Evaluated, id: Id, normal: P3, height: f64) -> Result<Vec<EdgeRef>> {
    let c = e.components.iter().find(|c| c.id == id).context("no part")?;
    let edges: Vec<EdgeRef> = (0..c.body.edges.len())
        .filter(|i| {
            cad::edge_signature(&c.body, *i, &c.frame)
                .is_some_and(|s| (dot(s.midpoint, normal) - height).abs() < 1e-4)
        })
        .map(|i| EdgeRef::signed(&c.body, i, &c.frame))
        .collect();
    ensure!(!edges.is_empty(), "the top of #{id} has no rim edges");
    Ok(edges)
}

fn face_ref(e: &cad::Evaluated, id: Id, face: usize) -> FaceRef {
    let c = e.components.iter().find(|c| c.id == id).unwrap();
    FaceRef::signed(&c.body, face, &c.frame)
}

fn evaluated(d: &RingDesign, lib: &AlphaLibrary) -> Result<cad::Evaluated> {
    let e = cad::evaluate(d, lib, draft_params())?;
    if let Some((id, why)) = e.failures().first() {
        bail!("feature #{id} failed: {why}");
    }
    Ok(e)
}

/// What the author found while picking references, for the report.
#[derive(serde::Serialize, Default)]
struct Authored {
    rim_edges: usize,
    rim_finish: String,
    fillet_refusals: Vec<String>,
    top_height_mm: f64,
    proud_of_crest_mm: f64,
}

/// The eight-feature history, each reference picked off an evaluation of the features before it.
fn author(lib: &AlphaLibrary) -> Result<(RingDesign, Authored)> {
    let mut d = band();
    let mut notes = Authored::default();
    let band_id = add(&mut d, "Band", Operation::Band, component(ComponentRole::Shank, Attach::Separate, Stage::Cast, 0.0))?;
    d.cad.as_mut().unwrap().features[0].component.manufacturing = d.manufacturing.clone();
    let plane = add(
        &mut d,
        "Work plane over the top",
        Operation::Plane {
            base: PlaneBase::Tangent { theta_deg: THETA, across_mm: 0.0 },
            offset_mm: -PLANE_SINK_MM,
        },
        bare(),
    )?;
    let mut outline = cartouche("Cartouche", CARTOUCHE_LONG_MM, CARTOUCHE_WIDE_MM, CARTOUCHE_CORNER_MM, CORNER_SEGMENTS);
    outline.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
    let sketch = add(&mut d, "Cartouche sketch", Operation::Sketch { sketch: outline }, bare())?;
    let boss = add(
        &mut d,
        "Drafted boss",
        Operation::Extrude {
            sketch: Profile::Feature { feature: sketch },
            height_mm: BOSS_MM,
            draft_deg: BOSS_DRAFT_DEG,
        },
        joined(),
    )?;
    let e = evaluated(&d, lib)?;
    let normal = e.plane(plane).context("work plane")?.normal;
    let (top, _) = top_face(&e, boss, normal)?;
    let pull = add(
        &mut d,
        "Press-pull the top",
        Operation::PressPull {
            source: boss,
            face: face_ref(&e, boss, top),
            distance_mm: PULL_MM,
        },
        joined(),
    )?;
    let e = evaluated(&d, lib)?;
    let (top, height) = top_face(&e, pull, normal)?;
    let edges = rim_edges(&e, pull, normal, height)?;
    notes.rim_edges = edges.len();
    notes.top_height_mm = height;
    // The native fillet is tried at each radius in turn; where the kernel refuses them all, the
    // section's fallback is a chamfer.
    let rim = add(
        &mut d,
        "Round the rim",
        Operation::Fillet {
            source: pull,
            edges: edges.clone(),
            radius_mm: RIM_ROUNDS_MM[0],
        },
        joined(),
    )?;
    for radius_mm in RIM_ROUNDS_MM {
        d.cad.as_mut().unwrap().features.last_mut().unwrap().operation = Operation::Fillet {
            source: pull,
            edges: edges.clone(),
            radius_mm,
        };
        match evaluated(&d, lib) {
            Ok(_) => {
                notes.rim_finish = format!("Fillet {radius_mm} mm");
                break;
            }
            Err(why) => notes.fillet_refusals.push(format!("{radius_mm} mm: {why:#}")),
        }
    }
    if notes.rim_finish.is_empty() {
        notes.rim_finish = format!("Chamfer {RIM_CHAMFER_MM} mm");
        let f = d.cad.as_mut().unwrap().features.last_mut().unwrap();
        f.name = "Chamfer the rim".into();
        f.operation = Operation::Chamfer {
            source: pull,
            edges,
            base_face: face_ref(&e, pull, top),
            distance_mm: RIM_CHAMFER_MM,
        };
        evaluated(&d, lib)?;
    }
    let e = evaluated(&d, lib)?;
    let (top, height) = top_face(&e, rim, normal)?;
    // The sketch plane leans about the lozenge's long diagonal, falling into the top toward
    // the far apex; the lozenge is drawn stretched across so it lands on the top at its size.
    let lean = (FACET_DEPTH_MM / (FACET_WIDE_MM / 2.0)).atan();
    let (mut facet, upper, _lower) = lozenge("Bright-cut lozenge", FACET_LONG_MM, FACET_WIDE_MM / lean.cos());
    facet.plane.on_face = Some(FaceAnchor { feature: rim, face: face_ref(&e, rim, top) });
    // Turned half a turn on the top, so the first facet cut is the far half, which faces away
    // from the hero camera and reads dark against the table.
    facet.plane.x = [-1.0, 0.0, 0.0];
    facet.plane.y = [0.0, -lean.cos(), -lean.sin()];
    let facet_sketch = add(&mut d, "Leaning facet sketch on the top", Operation::Sketch { sketch: facet }, bare())?;
    let cut = add(
        &mut d,
        "Bright-cut facet",
        Operation::Extrude {
            sketch: Profile::Region {
                feature: facet_sketch,
                region: RegionRef { entity: upper, at: [0.0, FACET_WIDE_MM / 6.0] },
            },
            height_mm: FACET_CLEAR_MM,
            draft_deg: FACET_WALL_DEG,
        },
        component(ComponentRole::Other, Attach::Cut, Stage::Bench, 0.0),
    )?;
    add(
        &mut d,
        "Mirror the facet",
        Operation::Pattern {
            sources: cut.into(),
            kind: PatternKind::Mirror { plane: MirrorPlane::Band },
        },
        component(ComponentRole::Other, Attach::Cut, Stage::Bench, 0.0),
    )?;
    evaluated(&d, lib)?;
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    notes.proud_of_crest_mm = height - crest;
    let _ = band_id;
    Ok((d, notes))
}

#[derive(serde::Serialize)]
struct Release {
    pitch_mm: f64,
    status: String,
    obstructions: usize,
    unresolved_rays: usize,
    fits_flask: bool,
    worst_draft_deg: f64,
    low_draft_area_mm2: f64,
    /// Where each obstruction stands in the world, how deep and over how much area.
    obstructions_at: Vec<([f64; 3], f64, f64)>,
}

#[derive(serde::Serialize)]
struct PatternGate {
    triangles: usize,
    watertight: bool,
    degenerate_faces: usize,
    self_crossings: usize,
    notes: Vec<String>,
}

#[derive(serde::Serialize)]
struct Gates {
    build: [usize; 2],
    triangles: usize,
    within_2m_triangles: bool,
    build_s: f64,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    mesh_self_crossings: usize,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    features: Vec<(Id, String, String)>,
    all_features_ok: bool,
    bore_radius_mm: f64,
    closest_to_axis_mm: f64,
    inside_finger_hole: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    worst_draft_deg: f64,
    undercut_percent: f64,
    /// Share of the surface under the sand's draft or square to the pull: what "with care" means.
    drag_percent: f64,
    marginal_area_mm2: f64,
    vertical_area_mm2: f64,
    total_area_mm2: f64,
    /// Each poured CAD part as judged: name, undercut, marginal, vertical and whole area, worst draft.
    parts_judged: Vec<(String, f64, f64, f64, f64, f64)>,
    thinnest_wall_mm: f64,
    release: Vec<Release>,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    pattern: Option<PatternGate>,
    /// Lost-wax gates, recorded for the lesson only: Keystone pours in sand.
    lost_wax_thickness_below: usize,
    lost_wax_cut_lands: Vec<String>,
    cold_reload_identical: Option<bool>,
    passed: bool,
}

fn crossings_of(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid {
        v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(),
        f: m.faces.clone(),
    })
}

fn made_part_crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let n = match &c.made {
                Some(m) => csg::self_crossings(m.solid()),
                None => csg::self_crossings(&csg::Solid {
                    v: c.trace.positions.clone(),
                    f: c.mesh.faces.clone(),
                }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

fn release_at(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, pitch: f64) -> Result<Release> {
    let mut s = d.manufacturing.clone().unwrap();
    s.sample_pitch_mm = pitch;
    let i = mf::inspect(d, lib, &s, params)?;
    Ok(Release {
        pitch_mm: pitch,
        status: format!("{:?}", i.release.status),
        obstructions: i.release.obstructions.len(),
        unresolved_rays: i.release.unresolved_rays,
        fits_flask: i.release.fits_flask,
        worst_draft_deg: i.release.worst_draft_deg,
        low_draft_area_mm2: i.release.low_draft_area_mm2,
        obstructions_at: i.release.obstructions.iter().map(|o| (o.world, o.depth_mm, o.projected_area_mm2)).collect(),
    })
}

fn gates(
    d: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    built: &mesh::BuildResult,
    build_s: f64,
    cold: Option<bool>,
) -> Result<Gates> {
    let v = &built.report.validation;
    let q = built.report.quality;
    let features: Vec<(Id, String, String)> = built
        .parts
        .evaluated
        .as_ref()
        .map(|e| e.features.iter().map(|f| (f.id, f.name.clone(), format!("{:?}", f.status))).collect())
        .unwrap_or_default();
    let all_ok = built
        .parts
        .evaluated
        .as_ref()
        .is_some_and(|e| e.features.iter().all(|f| f.status == FeatureStatus::Ok));
    let bore = d.inner_radius_mm();
    let closest = built
        .mesh
        .vertices
        .iter()
        .map(|p| (p.0 as f64).hypot(p.1 as f64))
        .fold(f64::MAX, f64::min);
    let inside = built
        .mesh
        .vertices
        .iter()
        .filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01)
        .count();
    let field = castability::judged_field_report(d, lib, &d.draft, 256, 128, Some(built));
    let release = vec![release_at(d, lib, params, 0.100)?, release_at(d, lib, params, 0.075)?];
    let findings = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = ringdesign_core::gems::built_meshes(d, lib, built).len();
    let pattern = {
        let p = mesh::try_build_pattern(d, lib, params)?;
        Some(PatternGate {
            triangles: p.mesh.faces.len(),
            watertight: p.report.validation.watertight,
            degenerate_faces: p.report.quality.degenerate_faces,
            self_crossings: crossings_of(&p.mesh),
            notes: p.parts.notes.clone(),
        })
    };
    let thickness = cad::measure::thickness(&built.mesh, 0.8);
    let lands = dfm::cut_lands(d, built, 0.8);
    let mut g = Gates {
        build: [params.theta_steps, params.profile_steps],
        triangles: built.mesh.faces.len(),
        within_2m_triangles: built.mesh.faces.len() <= 2_000_000,
        build_s,
        watertight: v.watertight,
        boundary_edges: v.boundary_edges,
        non_manifold_edges: v.non_manifold_edges,
        degenerate_faces: q.degenerate_faces,
        mesh_self_crossings: crossings_of(&built.mesh),
        made_parts: made_part_crossings(built),
        solids_notes: built.solids.notes.clone(),
        parts_notes: built.parts.notes.clone(),
        features,
        all_features_ok: all_ok,
        bore_radius_mm: bore,
        closest_to_axis_mm: closest,
        inside_finger_hole: inside,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        worst_draft_deg: field.worst_draft_deg,
        undercut_percent: field.undercut_fraction() * 100.0,
        drag_percent: field.drag_fraction() * 100.0,
        marginal_area_mm2: field.marginal_area_mm2,
        vertical_area_mm2: field.vertical_area_mm2,
        total_area_mm2: field.total_area_mm2,
        parts_judged: field
            .parts
            .iter()
            .map(|p| (p.name.clone(), p.undercut_area_mm2, p.marginal_area_mm2, p.vertical_area_mm2, p.total_area_mm2, p.worst_draft_deg))
            .collect(),
        thinnest_wall_mm: field.thinnest_wall_mm,
        release,
        dfm_findings: findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        stones_reported: stones.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        pattern,
        lost_wax_thickness_below: thickness.below_limit,
        lost_wax_cut_lands: lands.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        cold_reload_identical: cold,
        passed: false,
    };
    // Keystone alone may stand at "castable with care": that verdict is its lesson.
    let verdict_ok = g.field_verdict == castability::Verdict::Castable.label()
        || g.field_verdict == castability::Verdict::Marginal.label();
    g.passed = g.watertight
        && g.degenerate_faces == 0
        && g.mesh_self_crossings == 0
        && g.made_parts.iter().all(|(_, n)| *n == 0)
        && g.solids_notes.is_empty()
        && g.parts_notes.is_empty()
        && g.all_features_ok
        && g.inside_finger_hole == 0
        && verdict_ok
        && g.release.iter().all(|r| r.obstructions == 0 && r.unresolved_rays == 0)
        && g.dfm_findings.is_empty()
        && g.stones_reported as usize == g.stones_previewed
        && g.within_2m_triangles
        && g.pattern.as_ref().is_none_or(|p| p.watertight && p.degenerate_faces == 0 && p.self_crossings == 0)
        && g.cold_reload_identical != Some(false);
    Ok(g)
}

// --- Pictures --------------------------------------------------------------------------------

/// The hero looks down on the cartouche from in front and a little to one side.
const HERO: (f64, f64) = (0.15, 1.15);
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", HERO.0, HERO.1),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

/// The faces of `m` within `radius` of `centre`, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

/// The mesh with a vertex per face corner, each normal averaged over the faces round it that turn
/// less than `crease_deg` from its own: rendering only, so a ridge or a chamfer shades as the
/// crisp edge it is while a fillet stays smooth. Exported positions and topology are untouched.
fn creased(m: &mesh::Mesh, crease_deg: f64) -> mesh::Mesh {
    let limit = crease_deg.to_radians().cos();
    let p = |i: u32| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    };
    let mut normals = Vec::with_capacity(m.faces.len());
    let mut incident: Vec<Vec<(usize, f64)>> = vec![Vec::new(); m.vertices.len()];
    for (k, f) in m.faces.iter().enumerate() {
        let [a, b, c] = f.map(p);
        let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = dot(n, n).sqrt().max(1e-30);
        normals.push(n.map(|v| v / l));
        for (i, &v) in f.iter().enumerate() {
            let (u, w) = (f[(i + 1) % 3], f[(i + 2) % 3]);
            let (pu, pv, pw) = (p(u), p(v), p(w));
            let (du, dw) = ([pu[0] - pv[0], pu[1] - pv[1], pu[2] - pv[2]], [pw[0] - pv[0], pw[1] - pv[1], pw[2] - pv[2]]);
            let angle = (dot(du, dw) / (dot(du, du) * dot(dw, dw)).sqrt().max(1e-30)).clamp(-1.0, 1.0).acos();
            incident[v as usize].push((k, angle));
        }
    }
    let mut out = mesh::Mesh::default();
    out.vertices.reserve(m.faces.len() * 3);
    out.normals.reserve(m.faces.len() * 3);
    for (k, f) in m.faces.iter().enumerate() {
        let base = out.vertices.len() as u32;
        for &v in f {
            let mut sum = [0.0; 3];
            for &(j, w) in &incident[v as usize] {
                if dot(normals[j], normals[k]) >= limit {
                    for a in 0..3 {
                        sum[a] += normals[j][a] * w;
                    }
                }
            }
            let l = dot(sum, sum).sqrt().max(1e-30);
            out.vertices.push(m.vertices[v as usize]);
            out.normals.push(mesh::Vec3((sum[0] / l) as f32, (sum[1] / l) as f32, (sum[2] / l) as f32));
        }
        out.faces.push([base, base + 1, base + 2]);
    }
    split_long(&mut out, RENDER_EDGE_MM);
    out
}

/// Every face of `m` (a vertex per face corner) halved across its longest edge until no edge is
/// longer than `limit`: the renderer shades at the vertices, and a large flat triangle otherwise
/// shows as a wedge of its own tone. Rendering only.
fn split_long(m: &mut mesh::Mesh, limit: f64) {
    let at = |m: &mesh::Mesh, i: u32| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    };
    let mut k = 0;
    while k < m.faces.len() {
        let f = m.faces[k];
        let p = f.map(|i| at(m, i));
        let len = |a: [f64; 3], b: [f64; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        let (e, longest) = (0..3).map(|e| (e, len(p[e], p[(e + 1) % 3]))).fold((0, 0.0), |b, c| if c.1 > b.1 { c } else { b });
        if longest <= limit {
            k += 1;
            continue;
        }
        let (a, b, c) = (f[e], f[(e + 1) % 3], f[(e + 2) % 3]);
        let (va, vb) = (m.vertices[a as usize], m.vertices[b as usize]);
        let (na, nb) = (m.normals[a as usize], m.normals[b as usize]);
        let n = [(na.0 + nb.0) as f64, (na.1 + nb.1) as f64, (na.2 + nb.2) as f64];
        let l = dot(n, n).sqrt().max(1e-30);
        let mid = m.vertices.len() as u32;
        m.vertices.push(mesh::Vec3((va.0 + vb.0) * 0.5, (va.1 + vb.1) * 0.5, (va.2 + vb.2) * 0.5));
        m.normals.push(mesh::Vec3((n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32));
        m.faces[k] = [a, mid, c];
        m.faces.push([mid, b, c]);
    }
}

/// Longest triangle edge the renders shade across.
const RENDER_EDGE_MM: f64 = 0.25;

/// The timeline frames each step on the metal this close to the cartouche.
const TIMELINE_FRAME_MM: f64 = 8.0;
/// Faces sharper than this read as an edge in the renders.
const CREASE_DEG: f64 = 25.0;

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}
impl Canvas {
    fn new(w: usize, h: usize, shade: u8) -> Self {
        Self { w, h, px: vec![shade; w * h * 3] }
    }
    fn blit(&mut self, img: &[u8], iw: usize, ih: usize, x0: usize, y0: usize) {
        for y in 0..ih.min(self.h.saturating_sub(y0)) {
            for x in 0..iw.min(self.w.saturating_sub(x0)) {
                let (s, t) = ((y * iw + x) * 3, ((y0 + y) * self.w + x0 + x) * 3);
                self.px[t..t + 3].copy_from_slice(&img[s..s + 3]);
            }
        }
    }
    /// `text` in the bundled serif, `size` px high, white over whatever is there, from (x0, y0).
    fn text(&mut self, text: &str, size: f32, x0: usize, y0: usize) {
        static FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
        let font = FONT.get_or_init(|| {
            fontdue::Font::from_bytes(
                include_bytes!("../../../assets/fonts/EBGaramond.ttf").as_slice(),
                fontdue::FontSettings::default(),
            )
            .expect("bundled font parses")
        });
        let mut pen = x0 as f32;
        let baseline = y0 as f32 + size;
        for ch in text.chars() {
            let (m, cov) = font.rasterize(ch, size);
            let gx = pen.round() as i64 + m.xmin as i64;
            let gy = (baseline - m.height as f32 - m.ymin as f32).round() as i64;
            for y in 0..m.height {
                for x in 0..m.width {
                    let (px, py) = (gx + x as i64, gy + y as i64);
                    if px < 0 || py < 0 || px as usize >= self.w || py as usize >= self.h {
                        continue;
                    }
                    let a = cov[y * m.width + x] as f32 / 255.0;
                    let t = (py as usize * self.w + px as usize) * 3;
                    for k in 0..3 {
                        self.px[t + k] = (self.px[t + k] as f32 * (1.0 - a) + 245.0 * a) as u8;
                    }
                }
            }
            pen += m.advance_width;
        }
    }
    fn save(&self, path: &Path) -> Result<()> {
        image::save_buffer(path, &self.px, self.w as u32, self.h as u32, image::ColorType::Rgb8)?;
        Ok(())
    }
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let mut sheet = Canvas::new(300 * 3, 300 * 2 + 2 * 28, 24);
    for (k, (name, yaw, pitch)) in VIEWS.iter().enumerate() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3);
        if k < 2 {
            image::save_buffer(out.join(format!("{name}-300.png")), &img, 300, 300, image::ColorType::Rgb8)?;
        }
        let (x, y) = ((k % 3) * 300, (k / 3) * (300 + 28));
        sheet.blit(&img, 300, 300, x, y + 28);
        sheet.text(name, 20.0, x + 8, y + 2);
    }
    sheet.save(&out.join("contact-300.png"))?;
    // The close-up frames on the cartouche and draws the whole ring.
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    let (s, c) = THETA.to_radians().sin_cos();
    let close_mesh = crop(&finished.metal, [crest * c, crest * s, 0.0], 5.5);
    let mut close = vec![render::Part::metal(&close_mesh, render::GOLD)];
    close.extend(finished.parts(render::GOLD));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    // The bare band against the finished ring, at the hero's angle.
    let b = creased(&mesh::try_build(&band(), lib, draft_params())?.mesh, CREASE_DEG);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b, render::GOLD)], HERO.0, HERO.1, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, HERO.0, HERO.1, edge, edge, 3);
    let mut pair = Canvas::new(edge * 2, edge, 0);
    pair.blit(&bare_img, edge, edge, 0, 0);
    pair.blit(&finished_img, edge, edge, edge, 0);
    pair.save(&out.join("bare-vs-finished.png"))?;
    Ok(())
}

/// What a plane or sketch step adds, which builds no metal: a thin blue sheet on the plane, or
/// each of the sketch's regions laid on its plane, standing thick enough to show through whatever
/// the plane lies under; built from a copy of the history through that step.
fn overlay(d: &RingDesign, lib: &AlphaLibrary, through: usize, params: BuildParams) -> Result<Vec<mesh::Mesh>> {
    let doc = d.cad.as_ref().unwrap();
    let f = &doc.features[through];
    let sheets: Vec<(Profile, f64)> = match &f.operation {
        Operation::Plane { .. } => {
            let mut s = Sketch::rectangle(9.0, 7.5);
            s.plane.on_face = Some(FaceAnchor { feature: f.id, face: FaceRef::bare(0) });
            vec![(Profile::Inline(s), 0.06)]
        }
        Operation::Sketch { sketch } => sketch
            .profile_regions()?
            .iter()
            .filter_map(RegionRef::of)
            .map(|region| (Profile::Region { feature: f.id, region }, PLANE_SINK_MM + 0.12))
            .collect(),
        _ => return Ok(Vec::new()),
    };
    let mut copy = d.clone();
    let mut short = Document::default();
    for f in &doc.features[..=through] {
        short.append(f.clone())?;
    }
    let mut ids = Vec::new();
    for (sheet, thick) in sheets {
        let id = short.features.len() as Id + 1;
        short.append(Feature {
            id,
            name: "Overlay".into(),
            enabled: true,
            operation: Operation::Extrude {
                sketch: sheet,
                height_mm: thick,
                draft_deg: 0.0,
            },
            component: bare(),
        })?;
        ids.push(id);
    }
    copy.cad = Some(short);
    let e = cad::evaluate(&copy, lib, params)?;
    ensure!(e.failures().is_empty(), "the overlay for {} failed: {:?}", f.name, e.failures());
    Ok(e.components.iter().filter(|c| ids.contains(&c.id)).map(|c| c.mesh.clone()).collect())
}

/// One contact sheet of the ring after each feature, labelled with the feature's name; a plane or
/// a sketch shows in blue on the ring as it stood.
fn timeline(out: &Path, d: &RingDesign, lib: &AlphaLibrary) -> Result<()> {
    let doc = d.cad.as_ref().unwrap();
    let n = doc.features.len();
    let (cell, label, cols) = (360usize, 34usize, 3usize);
    let rows = n.div_ceil(cols);
    let mut sheet = Canvas::new(cell * cols, (cell + label) * rows, 24);
    let params = BuildParams {
        theta_steps: 512,
        profile_steps: 192,
        ..BuildParams::default()
    };
    for (k, f) in doc.features.iter().enumerate() {
        let mut step = d.clone();
        step.cad.as_mut().unwrap().through = Some(f.id);
        let mut finished = render::finished(&step, lib, params)?;
        finished.metal = creased(&finished.metal, CREASE_DEG);
        let extra = overlay(d, lib, k, params)?;
        // Framed on the top of the ring, where every step happens.
        let crest = d.inner_radius_mm() + d.profile.thickness_mm;
        let (s, c) = THETA.to_radians().sin_cos();
        let frame = crop(&finished.metal, [crest * c, crest * s, 0.0], TIMELINE_FRAME_MM);
        let mut parts = vec![render::Part::metal(&frame, render::GOLD)];
        parts.extend(finished.parts(render::GOLD));
        for m in &extra {
            parts.push(render::Part::tinted_stone(m, [0.25, 0.5, 0.95]));
        }
        let img = render::render_parts_ss(&parts, HERO.0, HERO.1, cell, cell, 2);
        let (x, y) = ((k % cols) * cell, (k / cols) * (cell + label));
        sheet.blit(&img, cell, cell, x, y + label);
        sheet.text(&format!("{}. {}", k + 1, f.name), 22.0, x + 10, y + 4);
    }
    sheet.save(&out.join("timeline.png"))
}

/// The verdict of the bare band, and of the same cartouche on a domed band, beside the ring's own:
/// which surfaces put Keystone at "castable with care".
#[derive(serde::Serialize)]
struct VerdictCause {
    bare_band_verdict: String,
    bare_band_drag_percent: f64,
    ring_verdict: String,
    ring_drag_percent: f64,
    cartouche_vertical_mm2: f64,
    cartouche_area_mm2: f64,
    cartouche_undercut_mm2: f64,
    cartouche_worst_draft_deg: f64,
    domed_bare_verdict: String,
    domed_bare_drag_percent: f64,
    domed_with_cartouche_verdict: String,
    domed_with_cartouche_drag_percent: f64,
    lesson: String,
}

fn verdict_cause(d: &RingDesign, lib: &AlphaLibrary, g: &Gates) -> Result<VerdictCause> {
    let judged = |d: &RingDesign, built: bool| -> Result<castability::FieldReport> {
        let b = if built { Some(mesh::try_build(d, lib, BuildParams { theta_steps: 512, profile_steps: 192, ..BuildParams::default() })?) } else { None };
        Ok(castability::judged_field_report(d, lib, &d.draft, 256, 128, b.as_ref()))
    };
    let mut bare = d.clone();
    bare.cad = None;
    let bare = judged(&bare, false)?;
    // The same history on a LowDome 6.0 x 2.1, Rivet's band: its crest falls away from the pull.
    let mut domed = d.clone();
    domed.profile.apply_style(ProfileStyle::LowDome);
    domed.profile.width_mm = 6.0;
    domed.profile.thickness_mm = 2.1;
    let mut domed_bare = domed.clone();
    domed_bare.cad = None;
    let domed_bare = judged(&domed_bare, false)?;
    let domed = judged(&domed, true)?;
    let part = g.parts_judged.first().cloned().unwrap_or_default();
    let lesson = format!(
        "The verdict is the lesson, and it has two sources. Pulled along the finger in two-part Delft sand, a flat band's broad crest stands square to the pull: the bare Flat 6.0 x 2.0 already reads '{}' at {:.1}% drag, past the {:.0}% the verdict allows. The cartouche adds its own zero-draft faces: its flat top and its two end walls face round the ring, square to the pull like a signet's table, and its {BOSS_DRAFT_DEG} deg draft helps only the two long walls that face across the finger. That is {:.1} of its {:.1} mm2 standing vertical, taking the ring to {:.1}% drag and '{}', with 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm: it releases, but drags and wants a longer rap. Its {:.3} mm2 judged undercut (worst {:.1} deg) is tessellation on the faceted chamfer corners, below the 0.075 mm ray grid. The cartouche alone carries the verdict on a domed band: the same history on Rivet's LowDome 6.0 x 2.1 turns a '{}' band ({:.1}% drag) into '{}' ({:.1}%). No draft angle on the boss can make it Castable; only a draft on the top and the end walls, or another pull, would.",
        bare.verdict.label(),
        bare.drag_fraction() * 100.0,
        castability::DRAG_FRACTION * 100.0,
        part.3,
        part.4,
        g.drag_percent,
        g.field_verdict,
        part.1,
        part.5,
        domed_bare.verdict.label(),
        domed_bare.drag_fraction() * 100.0,
        domed.verdict.label(),
        domed.drag_fraction() * 100.0,
    );
    Ok(VerdictCause {
        bare_band_verdict: bare.verdict.label().into(),
        bare_band_drag_percent: bare.drag_fraction() * 100.0,
        ring_verdict: g.field_verdict.clone(),
        ring_drag_percent: g.drag_percent,
        cartouche_vertical_mm2: part.3,
        cartouche_area_mm2: part.4,
        cartouche_undercut_mm2: part.1,
        cartouche_worst_draft_deg: part.5,
        domed_bare_verdict: domed_bare.verdict.label().into(),
        domed_bare_drag_percent: domed_bare.drag_fraction() * 100.0,
        domed_with_cartouche_verdict: domed.verdict.label().into(),
        domed_with_cartouche_drag_percent: domed.drag_fraction() * 100.0,
        lesson,
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina/keystone"));
    std::fs::create_dir_all(&out)?;
    println!("Keystone");
    let lib = AlphaLibrary::builtin();
    let (mut d, authored) = author(&lib)?;
    let params = if draft { draft_params() } else { export_params() };
    d.build = params;
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    library::save_design(out.join("design.ring.json"), &d)?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let g = gates(&d, &lib, params, &built, build_s, cold)?;
    println!(
        "  {} triangles in {build_s:.1} s; watertight {}; degenerate {}; crossings {}; features ok {}; inside bore {}; verdict {}; release {:?}; dfm {}; gates {}",
        g.triangles,
        g.watertight,
        g.degenerate_faces,
        g.mesh_self_crossings,
        g.all_features_ok,
        g.inside_finger_hole,
        g.field_verdict,
        g.release.iter().map(|r| (r.pitch_mm, r.obstructions, r.unresolved_rays)).collect::<Vec<_>>(),
        g.dfm_findings.len(),
        if g.passed { "passed" } else { "FAILED" }
    );
    for n in g.parts_notes.iter().chain(&g.solids_notes).chain(&g.field_notes).chain(&g.dfm_findings) {
        println!("    note: {n}");
    }
    // The report keeps the other build's block: a draft run refreshes `draft`, an export run the top.
    let path = out.join("report.json");
    let mut report: serde_json::Value = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let block = serde_json::json!({
        "ring": "Keystone",
        "slug": "keystone",
        "process": d.draft.process.label(),
        "sand": "Delft clay",
        "min_draft_deg": d.draft.min_draft_deg,
        "min_section_mm": d.draft.min_section_mm,
        "min_detail_mm": d.draft.min_detail_mm,
        "bore_mm": BORE,
        "size": d.size.display(),
        "authored": authored,
        "verdict_cause": verdict_cause(&d, &lib, &g)?,
        "gates": g,
    });
    if draft {
        report["draft"] = block;
    } else {
        let keep = report.get("draft").cloned();
        report = block;
        if let Some(k) = keep {
            report["draft"] = k;
        }
    }
    std::fs::write(&path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
    }
    let mut finished = render::finished_from(&d, &lib, built);
    finished.metal = creased(&finished.metal, CREASE_DEG);
    renders(&out, &d, &lib, &finished, if draft { 1000 } else { 1600 })?;
    timeline(&out, &d, &lib)?;
    ensure!(g.passed, "Keystone failed its gates; see {}", path.display());
    Ok(())
}
