//! Officina — Torsade, a rope-edged collet: two twisted ropes along the band's arrises, butting into a cabochon's collet, cast in lost wax.
//! cargo build --release -p ringdesign-core --example officina_torsade
//! target/release/examples/officina_torsade [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{
        self, Attach, Component, ComponentRole, Document, FaceRef, Feature, FeatureStatus, MirrorPlane, Operation,
        PatternKind, PlaneBase, Placement, Profile, builders,
    },
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, Sketch},
    stl,
};
use serde::Serialize;
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};
use std::time::Instant;

type P3 = [f64; 3];

const BORE_MM: f64 = 18.2;
const WIDTH_MM: f64 = 4.4;
const THICKNESS_MM: f64 = 1.8;
/// The rope's axis: its radius round the finger and its height off the parting plane.
const PATH_R_MM: f64 = 10.68;
const PATH_Z_MM: f64 = 2.1;
/// Where the rope leaves the collet and where it comes back to it, the long way round the palm.
fn env_f(k: &str, d: f64) -> f64 {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}
fn start_deg() -> f64 {
    env_f("START", 100.0)
}
fn end_deg() -> f64 {
    env_f("END", 80.0)
}
/// Eight full turns: a slow lay that reads as rope, under the ten a twisted sweep takes.
const TWIST_DEG: f64 = 2880.0;
/// Three round strands laid about the rope's axis: each strand's radius and its centre's offset.
const STRANDS: usize = 3;
const STRAND_R_MM: f64 = 0.5;
const STRAND_OFFSET_MM: f64 = 0.3;
/// The fillet rounding each groove between two strands.
const GROOVE_R_MM: f64 = 0.10;
fn rope_blend_mm() -> f64 {
    std::env::var("ROPE_BLEND").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0)
}
const MIN_SECTION_MM: f64 = 0.8;
/// The collet, turned in one piece: its base on the band, the girdle's height, the lip's height over it, the wall's radii.
const COLLET_BASE_MM: f64 = 9.95;
const GIRDLE_MM: f64 = 11.55;
const LIP_RISE_MM: f64 = 0.4;
/// The wall's outer radius where it meets the band, and where it stands upright under the lip.
const COLLET_FOOT_R_MM: f64 = 3.6;
fn collet_r_mm() -> f64 {
    env_f("COLLET_R", 4.4)
}
/// The wall's bore, a hair over the stone's girdle.
const COLLET_BORE_R_MM: f64 = 3.55;
/// Where the flared lower wall turns upright, below the girdle.
const COLLET_KNEE_MM: f64 = 11.2;
const COLLET_RIM_MM: f64 = 0.2;
/// The lip's two rounds, together nearly a half-round across the wall.
const LIP_ROUND_MM: f64 = 0.4;
/// The seat under the stone stands this far below its flat back.
const SEAT_GAP_MM: f64 = 0.02;
const CREST_MM: f64 = BORE_MM / 2.0 + THICKNESS_MM;
/// The crest's barrel crown: the drop from the middle of the crest to each arris.
const CROWN_MM: f64 = 0.2;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn screen_params() -> BuildParams {
    BuildParams { theta_steps: 128, profile_steps: 48, ..BuildParams::default() }
}

fn gem() -> Gem {
    Gem { preview_tint: Some([0.42, 0.03, 0.05]), ..Gem::cabochon(GemCut::Round, 7.0) }
}

/// The workshop's investment recipe, flask and channels, in 18k gold.
fn setup() -> mf::Setup {
    let alloy = "Gold 18k";
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = alloy.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(alloy).unwrap().shrink_pct;
    s.recipe.process = CastProcess::LostWax;
    s.recipe.name = format!("Investment casting starting recipe / {alloy}");
    s.recipe.sand = None;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.min_detail_mm = 0.2;
    s.recipe.min_draft_deg = 0.0;
    s.sample_pitch_mm = 0.10;
    s.recipe.calibration_note = "Uncalibrated starting allowance. Confirm with the caster's alloy, pattern material, mold, and measured trials.".into();
    s.flask.width_mm = 70.;
    s.flask.length_mm = 70.;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0., -11., 0.], end: [0., -22., 0.], diameter_mm: 3.2 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0., -22., 0.], end: [0., -30., 0.], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Investment cast in one piece with the collet. Clean investment from the rope's strand grooves with a soft brush. Polish the side faces and the bore, keep the ropes bright, and burnish the collet's lip over the cabochon.".into();
    s
}

/// The bare stock: Flat 4.4 x 1.8 with flattened sides, bore 18.2, lost wax.
fn band() -> RingDesign {
    let mut d = RingDesign { name: "Torsade \u{2014} a rope-edged collet".into(), ..RingDesign::default() };
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = WIDTH_MM;
    d.profile.thickness_mm = THICKNESS_MM;
    d.profile.crown_mm = CROWN_MM;
    d.profile.shape_a = 2.0;
    d.profile.comfort_fit_mm = 0.3;
    d.profile.flatten_sides();
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.manufacturing = Some(setup());
    d
}

fn feature(id: u64, name: &str, operation: Operation, role: ComponentRole) -> Feature {
    Feature {
        id,
        name: name.into(),
        enabled: true,
        operation,
        component: Component { role, material: "Gold 18k".into(), ..Component::default() },
    }
}

fn on_plane(s: &mut Sketch, plane: u64) {
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
}

/// The rope's path in the plane over the arris: one arc about the finger, counter-clockwise from the start the long way round.
fn path_sketch() -> Sketch {
    let mut s = Sketch::default();
    s.name = "Path".into();
    let at = |deg: f64| {
        let (sn, c) = deg.to_radians().sin_cos();
        [PATH_R_MM * c, PATH_R_MM * sn]
    };
    let center = s.point([0.0, 0.0]);
    let start = s.point(at(start_deg()));
    let end = s.point(at(end_deg()));
    s.entity(Geometry::Arc { center, start, end });
    on_plane(&mut s, 2);
    s
}

/// The rope's section in the plane through the finger's axis at the path's start: three round strands about the path, each groove between two rounded by a small fillet.
fn section_sketch() -> Sketch {
    let mut s = Sketch::default();
    s.name = "Section".into();
    let o = [PATH_R_MM, PATH_Z_MM];
    let step = 2.0 * PI / STRANDS as f64;
    let half = step / 2.0;
    let (c, r, f) = (STRAND_OFFSET_MM, STRAND_R_MM, GROOVE_R_MM);
    let lobe = |k: usize| -> [f64; 2] {
        let a = PI / 2.0 + step * k as f64;
        [o[0] + c * a.cos(), o[1] + c * a.sin()]
    };
    // Each groove's fillet circle touches both strands: its centre on the bisector, r + f from each strand's centre.
    let reach = c * half.cos() + ((r + f).powi(2) - (c * half.sin()).powi(2)).sqrt();
    let groove = |k: usize| -> [f64; 2] {
        let a = PI / 2.0 + step * k as f64 + half;
        [o[0] + reach * a.cos(), o[1] + reach * a.sin()]
    };
    let touch = |l: [f64; 2], g: [f64; 2]| -> [f64; 2] {
        let t = r / (r + f);
        [l[0] + (g[0] - l[0]) * t, l[1] + (g[1] - l[1]) * t]
    };
    let n = STRANDS;
    // Groove k lies between strand k and strand k + 1: where it leaves strand k and where it meets strand k + 1.
    let leave: Vec<_> = (0..n).map(|k| s.point(touch(lobe(k), groove(k)))).collect();
    let meet: Vec<_> = (0..n).map(|k| s.point(touch(lobe((k + 1) % n), groove(k)))).collect();
    for k in 0..n {
        let center = s.point(lobe(k));
        s.entity(Geometry::Arc { center, start: meet[(k + n - 1) % n], end: leave[k] });
        let center = s.point(groove(k));
        s.entity(Geometry::Arc { center, start: meet[k], end: leave[k] });
    }
    on_plane(&mut s, 4);
    s
}

/// A closed polyline with its corners rounded: each corner `(point, radius)`, a radius of 0 kept sharp; the rounds as arcs, the runs between them as lines.
fn rounded(s: &mut Sketch, corners: &[([f64; 2], f64)]) {
    let n = corners.len();
    let unit2 = |a: [f64; 2]| {
        let l = a[0].hypot(a[1]);
        [a[0] / l, a[1] / l]
    };
    // Where each corner's round leaves the incoming run and joins the outgoing one, as sketch points.
    let mut ends = Vec::with_capacity(n);
    for k in 0..n {
        let (p, r) = corners[k];
        let (a, b) = (corners[(k + n - 1) % n].0, corners[(k + 1) % n].0);
        if r <= 0.0 {
            let id = s.point(p);
            ends.push((id, id));
            continue;
        }
        let u = unit2([a[0] - p[0], a[1] - p[1]]);
        let v = unit2([b[0] - p[0], b[1] - p[1]]);
        let half = (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0).acos() / 2.0;
        let t = r / half.tan();
        let bis = unit2([u[0] + v[0], u[1] + v[1]]);
        let c = [p[0] + bis[0] * r / half.sin(), p[1] + bis[1] * r / half.sin()];
        let t1 = [p[0] + u[0] * t, p[1] + u[1] * t];
        let t2 = [p[0] + v[0] * t, p[1] + v[1] * t];
        let (i1, i2, ic) = (s.point(t1), s.point(t2), s.point(c));
        let turn = (t1[0] - c[0]) * (t2[1] - c[1]) - (t1[1] - c[1]) * (t2[0] - c[0]);
        let (start, end) = if turn > 0.0 { (i1, i2) } else { (i2, i1) };
        s.entity(Geometry::Arc { center: ic, start, end });
        ends.push((i1, i2));
    }
    for k in 0..n {
        let (a, b) = (ends[k].1, ends[(k + 1) % n].0);
        s.entity(Geometry::Line { a, b });
    }
}

/// The collet's half-section in the plane through the finger's axis and the stone's: radius out from the finger along x, distance from the stone's axis along y. A wall flaring up from the band, standing upright under a rounded lip, round a solid seat under the stone.
fn collet_sketch() -> Sketch {
    let mut s = Sketch::default();
    s.name = "Collet section".into();
    s.plane.x = [0.0, 1.0, 0.0];
    s.plane.y = [0.0, 0.0, 1.0];
    let (lo, top, seat) = (COLLET_BASE_MM, GIRDLE_MM + LIP_RISE_MM, GIRDLE_MM - SEAT_GAP_MM);
    let (ro, ri) = (collet_r_mm(), COLLET_BORE_R_MM);
    rounded(
        &mut s,
        &[
            ([lo, 0.0], 0.0),
            ([lo, COLLET_FOOT_R_MM], COLLET_RIM_MM),
            ([COLLET_KNEE_MM, ro], 0.0),
            ([top, ro], LIP_ROUND_MM),
            ([top, ri], LIP_ROUND_MM),
            ([seat, ri], 0.0),
            ([seat, 0.0], 0.0),
        ],
    );
    s
}

fn author() -> Result<RingDesign> {
    let mut d = band();
    let g = gem();
    let mut doc = Document::default();
    doc.append(feature(1, "Band", Operation::Band, ComponentRole::Shank))?;
    doc.append(feature(2, "Plane over the arris", Operation::Plane { base: PlaneBase::Parting, offset_mm: PATH_Z_MM }, ComponentRole::Other))?;
    doc.append(feature(3, "Path", Operation::Sketch { sketch: path_sketch() }, ComponentRole::Other))?;
    doc.append(feature(4, "Plane across the path", Operation::Plane { base: PlaneBase::Section { theta_deg: start_deg() }, offset_mm: 0.0 }, ComponentRole::Other))?;
    doc.append(feature(5, "Section", Operation::Sketch { sketch: section_sketch() }, ComponentRole::Other))?;
    let mut rope = feature(
        6,
        "Rope",
        Operation::Twist { sketch: Profile::Feature { feature: 5 }, path: path_sketch(), degrees: TWIST_DEG, end_scale: 1.0 },
        ComponentRole::Shank,
    );
    rope.component.attach = Attach::Join;
    rope.component.blend_mm = rope_blend_mm();
    doc.append(rope)?;
    let mut mirror = feature(7, "Mirror across the band", Operation::Pattern { sources: 6.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, ComponentRole::Shank);
    mirror.component.attach = Attach::Join;
    mirror.component.blend_mm = rope_blend_mm();
    doc.append(mirror)?;
    let mut collet = feature(
        8,
        "Collet",
        Operation::Revolve { sketch: collet_sketch().into(), pivot: [0.0; 3], axis: [0.0, 1.0, 0.0], degrees: 360.0, in_plane: false },
        ComponentRole::Setting,
    );
    collet.component.attach = Attach::Join;
    doc.append(collet)?;
    let mut stone = builders::stone_feature(9, g, Placement::ring(90.0, GIRDLE_MM - CREST_MM));
    stone.name = "Garnet cabochon 7.0".into();
    doc.append(stone)?;
    d.cad = Some(doc);
    Ok(d)
}

/// The design rolled back to feature `id`, as the timeline's marker builds it.
fn through(d: &RingDesign, id: u64) -> RingDesign {
    let mut t = d.clone();
    if let Some(doc) = t.cad.as_mut() {
        doc.through = (id < doc.features.last().map_or(0, |f| f.id)).then_some(id);
    }
    t
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
}

/// Every made part's self-crossings, as placed.
fn crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let n = match &c.made {
                Some(m) => csg::self_crossings(m.solid()),
                None => csg::self_crossings(&csg::Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

#[derive(Serialize, Default, Clone)]
struct MeshGate {
    triangles: usize,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    self_crossings: usize,
}
fn mesh_gate(m: &mesh::Mesh) -> MeshGate {
    let v = m.validate();
    MeshGate {
        triangles: m.faces.len(),
        watertight: v.watertight,
        boundary_edges: v.boundary_edges,
        non_manifold_edges: v.non_manifold_edges,
        degenerate_faces: m.quality().degenerate_faces,
        self_crossings: csg::self_crossings(&solid_of(m)),
    }
}

#[derive(Serialize, Default, Clone)]
struct Wall {
    piece: String,
    triangles: usize,
    rays: usize,
    sampled_min_mm: Option<f64>,
    at: Option<[f64; 3]>,
    below_limit: usize,
    unresolved: usize,
    note: String,
}

#[derive(Serialize, Default, Clone)]
struct Gates {
    build: [usize; 2],
    build_s: f64,
    metal: MeshGate,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    features: Vec<(u64, String, String)>,
    features_ok: bool,
    bore_radius_mm: f64,
    nearest_to_axis_mm: f64,
    inside_finger_hole: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    thickness_build: [usize; 2],
    thickness_sampled_min_mm: Option<f64>,
    thickness_clean: bool,
    thickness: Vec<Wall>,
    cut_lands: Vec<String>,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    stone_warnings: Vec<String>,
    crowding_tight_pairs: usize,
    /// The stone's lowest point over the collet's seat, mm: the stone sits clear of the metal under it.
    stone_over_seat_mm: f64,
    pattern: Option<MeshGate>,
    cold_reload_identical: Option<bool>,
    within_2m_triangles: bool,
    passed: bool,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, verify: Option<&Path>) -> Result<(Gates, mesh::BuildResult)> {
    let started = Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let features: Vec<(u64, String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|f| {
            let s = match &f.status {
                FeatureStatus::Ok => "Ok".to_string(),
                other => format!("{other:?}"),
            };
            (f.id, f.name.clone(), s)
        })
        .collect();
    let features_ok = features.len() == d.cad.as_ref().map_or(0, |c| c.features.len()) && features.iter().all(|f| f.2 == "Ok");
    let bore = d.inner_radius_mm();
    let nearest = built.mesh.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(f64::MAX, f64::min);
    let inside = built.mesh.vertices.iter().filter(|v| (v.0 as f64).hypot(v.1 as f64) < bore - 0.01).count();
    let field = castability::judged_field_report(d, lib, &d.draft, 256, 128, Some(&built));
    // The screen samples meshes up to 250k triangles: the whole ring when it fits, else the bare band and each made part alone, at the preview chord.
    let screen = mesh::try_build(d, lib, screen_params())?;
    let mut pieces: Vec<(String, &mesh::Mesh)> = vec![("Whole ring".into(), &screen.mesh)];
    let bare = mesh::try_build(&through(d, 1), lib, screen_params())?;
    if screen.mesh.faces.len() > 250_000 {
        pieces = vec![("Band".into(), &bare.mesh)];
        pieces.extend(
            screen.parts.evaluated.iter().flat_map(|e| e.components.iter()).filter(|c| !c.settings.reference).map(|c| (c.name.clone(), &c.mesh)),
        );
    }
    let thickness: Vec<Wall> = pieces
        .iter()
        .map(|(name, m)| {
            let t = cad::measure::thickness(m, MIN_SECTION_MM);
            Wall { piece: name.clone(), triangles: m.faces.len(), rays: t.rays, sampled_min_mm: t.sampled_min_mm, at: t.point, below_limit: t.below_limit, unresolved: t.unresolved, note: t.note.into() }
        })
        .collect();
    let lands: Vec<String> = dfm::cut_lands(d, &built, MIN_SECTION_MM).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built);
    let previewed = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| c.settings.reference && c.settings.stone_id.is_some())
        .count();
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let pattern = Some(mesh_gate(&mesh::try_build_pattern(d, lib, params)?.mesh));
    let cold = match verify {
        Some(path) => {
            let saved = library::load_design(path)?;
            let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
            let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
            Some(rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals)
        }
        None => None,
    };
    let metal = mesh_gate(&built.mesh);
    let mut g = Gates {
        build: [params.theta_steps, params.profile_steps],
        build_s,
        within_2m_triangles: metal.triangles <= 2_000_000,
        metal,
        made_parts: crossings(&built),
        solids_notes: built.solids.notes.clone(),
        parts_notes: built.parts.notes.clone(),
        features,
        features_ok,
        bore_radius_mm: bore,
        nearest_to_axis_mm: nearest,
        inside_finger_hole: inside,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        thickness_build: [screen_params().theta_steps, screen_params().profile_steps],
        thickness_sampled_min_mm: thickness.iter().filter_map(|w| w.sampled_min_mm).reduce(f64::min),
        thickness_clean: thickness.iter().all(|w| w.rays > 0 && w.below_limit == 0 && w.unresolved == 0),
        thickness,
        cut_lands: lands,
        dfm_findings: findings,
        stones_reported: stones.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        stone_warnings: warnings,
        crowding_tight_pairs: stones.as_ref().map_or(0, |s| s.tight_pairs),
        stone_over_seat_mm: ringdesign_core::gems::built_meshes(d, lib, &built)
            .iter()
            .flat_map(|(m, _)| m.vertices.iter().map(|v| v.1 as f64))
            .fold(f64::MAX, f64::min)
            - (GIRDLE_MM - SEAT_GAP_MM),
        pattern,
        cold_reload_identical: cold,
        passed: false,
    };
    let p = g.pattern.clone().unwrap_or_default();
    g.passed = g.metal.watertight
        && g.metal.degenerate_faces == 0
        && g.metal.self_crossings == 0
        && g.made_parts.iter().all(|(_, n)| *n == 0)
        && g.solids_notes.is_empty()
        && g.parts_notes.is_empty()
        && g.features_ok
        && g.inside_finger_hole == 0
        && g.thickness_clean
        && g.cut_lands.is_empty()
        && g.dfm_findings.is_empty()
        && g.stones_reported as usize == g.stones_previewed
        && g.crowding_tight_pairs == 0
        && g.stone_over_seat_mm >= 0.0
        && p.watertight
        && p.degenerate_faces == 0
        && p.self_crossings == 0
        && g.within_2m_triangles
        && g.cold_reload_identical != Some(false);
    Ok((g, built))
}

// --- Pictures -----------------------------------------------------------------------------------

/// A thin tube along `pts` for the timeline's construction guides; drawn, never built.
fn tube(pts: &[P3], r: f64, closed: bool) -> mesh::Mesh {
    let mut m = mesh::Mesh::default();
    let n = pts.len();
    let around = 10;
    for i in 0..n {
        let prev = pts[if i == 0 { if closed { n - 1 } else { 0 } } else { i - 1 }];
        let next = pts[if i + 1 == n { if closed { 0 } else { n - 1 } } else { i + 1 }];
        let t = unit([next[0] - prev[0], next[1] - prev[1], next[2] - prev[2]]);
        let up = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
        let a = unit(cross(t, up));
        let b = cross(t, a);
        for k in 0..around {
            let phi = 2.0 * PI * k as f64 / around as f64;
            let (s, c) = phi.sin_cos();
            let nrm = [a[0] * c + b[0] * s, a[1] * c + b[1] * s, a[2] * c + b[2] * s];
            let p = pts[i];
            m.vertices.push(mesh::Vec3((p[0] + nrm[0] * r) as f32, (p[1] + nrm[1] * r) as f32, (p[2] + nrm[2] * r) as f32));
            m.normals.push(mesh::Vec3(nrm[0] as f32, nrm[1] as f32, nrm[2] as f32));
        }
    }
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let j = (i + 1) % n;
        for k in 0..around {
            let k2 = (k + 1) % around;
            let (a, b, c, e) = ((i * around + k) as u32, (i * around + k2) as u32, (j * around + k) as u32, (j * around + k2) as u32);
            m.faces.push([a, c, b]);
            m.faces.push([b, c, e]);
        }
    }
    m
}
fn unit(a: P3) -> P3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The construction a feature adds, drawn as thin blue wire: the planes' outlines, the path and the section.
fn guides(id: u64) -> Vec<mesh::Mesh> {
    let mut out = Vec::new();
    let ring_pt = |deg: f64, r: f64, z: f64| {
        let (s, c) = deg.to_radians().sin_cos();
        [r * c, r * s, z]
    };
    let path = || {
        let n = 240;
        (0..=n).map(|k| ring_pt(start_deg() + (end_deg() + 360.0 - start_deg()) * k as f64 / n as f64, PATH_R_MM, PATH_Z_MM)).collect::<Vec<_>>()
    };
    let plane2 = || {
        let h = 13.5;
        vec![[-h, -h, PATH_Z_MM], [h, -h, PATH_Z_MM], [h, h, PATH_Z_MM], [-h, h, PATH_Z_MM]]
    };
    let plane4 = || vec![ring_pt(start_deg(), 8.2, -3.0), ring_pt(start_deg(), 13.4, -3.0), ring_pt(start_deg(), 13.4, 3.6), ring_pt(start_deg(), 8.2, 3.6)];
    let section = || {
        let step = 2.0 * PI / STRANDS as f64;
        (0..96)
            .map(|k| {
                let a = 2.0 * PI * k as f64 / 96.0;
                let rr = (0..STRANDS)
                    .map(|j| {
                        let l = PI / 2.0 + step * j as f64;
                        let d = a - l;
                        let cc = STRAND_OFFSET_MM * d.cos();
                        cc + (STRAND_R_MM.powi(2) - (STRAND_OFFSET_MM * d.sin()).powi(2)).sqrt()
                    })
                    .fold(0.0, f64::max);
                ring_pt(start_deg(), PATH_R_MM + rr * a.cos(), PATH_Z_MM + rr * a.sin())
            })
            .collect::<Vec<_>>()
    };
    match id {
        2 => out.push(tube(&plane2(), 0.09, true)),
        3 => {
            out.push(tube(&plane2(), 0.06, true));
            out.push(tube(&path(), 0.12, false));
        }
        4 => {
            out.push(tube(&path(), 0.08, false));
            out.push(tube(&plane4(), 0.09, true));
        }
        5 => {
            out.push(tube(&path(), 0.08, false));
            out.push(tube(&plane4(), 0.06, true));
            out.push(tube(&section(), 0.06, true));
        }
        _ => {}
    }
    out
}

const GUIDE_TINT: [f32; 3] = [0.25, 0.48, 0.95];

fn save_rgb(path: &Path, img: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, img, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// A sheet of equal tiles, `cols` wide, each with its caption under it.
fn sheet(tiles: &[(Vec<u8>, String)], edge: usize, cols: usize, path: &Path) -> Result<()> {
    let font = fontdue::Font::from_bytes(include_bytes!("../../../assets/fonts/EBGaramond.ttf").as_slice(), fontdue::FontSettings::default())
        .map_err(|e| anyhow::anyhow!(e))?;
    let caption = 30;
    let rows = tiles.len().div_ceil(cols);
    let (w, h) = (cols * edge, rows * (edge + caption));
    let mut out = vec![245u8; w * h * 3];
    for (i, (img, label)) in tiles.iter().enumerate() {
        let (x0, y0) = ((i % cols) * edge, (i / cols) * (edge + caption));
        for y in 0..edge {
            let row = &img[y * edge * 3..(y + 1) * edge * 3];
            out[((y0 + y) * w + x0) * 3..((y0 + y) * w + x0 + edge) * 3].copy_from_slice(row);
        }
        let size = 19.0;
        let glyphs: Vec<_> = label.chars().map(|c| font.rasterize(c, size)).collect();
        let width: i32 = glyphs.iter().map(|(m, _)| m.advance_width.round() as i32).sum();
        let mut pen = x0 as i32 + (edge as i32 - width).max(8) / 2;
        let base = (y0 + edge) as i32 + 22;
        for (m, bitmap) in &glyphs {
            for gy in 0..m.height {
                for gx in 0..m.width {
                    let a = bitmap[gy * m.width + gx] as f32 / 255.0;
                    let (px, py) = (pen + m.xmin + gx as i32, base - m.height as i32 - m.ymin + gy as i32);
                    if px < x0 as i32 || px >= (x0 + edge) as i32 || py < 0 || py >= h as i32 {
                        continue;
                    }
                    let o = (py as usize * w + px as usize) * 3;
                    for c in 0..3 {
                        out[o + c] = (out[o + c] as f32 * (1.0 - a) + 30.0 * a) as u8;
                    }
                }
            }
            pen += m.advance_width.round() as i32;
        }
    }
    save_rgb(path, &out, w, h)
}

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.55, 0.95),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, params: BuildParams, edge: usize) -> Result<Vec<(String, f64)>> {
    let fin = dressed(d, lib, built);
    let parts = parts_of(&fin);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let mut small = Vec::new();
    for (name, yaw, pitch) in VIEWS.iter().take(4) {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 4);
        if *name == "hero" || *name == "face" {
            save_rgb(&out.join(format!("{name}-300.png")), &img, 300, 300)?;
        }
        small.push((img, name.to_string()));
    }
    sheet(&small, 300, 4, &out.join("contact-300.png"))?;
    // The collet and the rope ends close up, framed on the metal within 7 mm of the stone.
    let frames = ringdesign_core::stones::stone_frames(d);
    let c = frames.first().map(|(_, f)| f.girdle).unwrap_or([0.0, 10.9, 0.0]);
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0) + 0.45, 1.0, render::Framing::new([c[0], c[1] - 0.8, c[2]], 7.0), edge)?;
    let b = mesh::try_build(&band(), lib, params)?;
    let bare = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let finished = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&bare[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&finished[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    // The timeline: the ring after each feature, the construction it adds drawn in blue.
    let mut tiles = Vec::new();
    let mut timing = Vec::new();
    let doc = d.cad.as_ref().context("no CAD document")?;
    for f in &doc.features {
        let t = through(d, f.id);
        let started = Instant::now();
        let tb = mesh::try_build(&t, lib, draft_params())?;
        timing.push((f.name.clone(), started.elapsed().as_secs_f64()));
        let tf = dressed(&t, lib, &tb);
        let g = guides(f.id);
        let mut ps = parts_of(&tf);
        ps.extend(g.iter().map(|m| render::Part::metal(m, GUIDE_TINT)));
        let img = render::render_parts_ss(&ps, 0.55, 0.95, 300, 300, 3);
        tiles.push((img, format!("{}. {}", f.id, f.name)));
    }
    sheet(&tiles, 300, 3, &out.join("timeline.png"))?;
    Ok(timing)
}

/// The finished ring with each stone welded and its normals averaged, so a cabochon's dome shades smooth.
fn dressed(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> render::Finished {
    let stones = ringdesign_core::gems::built_meshes(d, lib, built).into_iter().map(|(m, t)| (weld(&m), t)).collect();
    render::Finished { metal: built.mesh.clone(), stones }
}
fn parts_of(fin: &render::Finished) -> Vec<render::Part<'_>> {
    let mut parts = fin.parts(render::GOLD);
    for p in parts.iter_mut().skip(1) {
        p.smooth = true;
    }
    parts
}
fn weld(m: &mesh::Mesh) -> mesh::Mesh {
    let mut out = mesh::Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in &m.faces {
        let g = f.map(|i| {
            let v = m.vertices[i as usize];
            let key = [v.0, v.1, v.2].map(|c| (c * 1e4).round() as i64);
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(v);
                (out.vertices.len() - 1) as u32
            })
        });
        if g[0] != g[1] && g[1] != g[2] && g[0] != g[2] {
            out.faces.push(g);
        }
    }
    let mut n = vec![[0.0f32; 3]; out.vertices.len()];
    for f in &out.faces {
        let [a, b, c] = f.map(|i| out.vertices[i as usize]);
        let (e1, e2) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
        let x = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        for &i in f {
            for k in 0..3 {
                n[i as usize][k] += x[k];
            }
        }
    }
    out.normals = n
        .iter()
        .map(|v| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            mesh::Vec3(v[0] / l, v[1] / l, v[2] / l)
        })
        .collect();
    out
}

/// The rope's own cost: the twisted sweep alone, then each csg join, timed at `params`.
#[derive(Serialize)]
struct RopeTiming {
    build: [usize; 2],
    rope_length_mm: f64,
    twist_turns: f64,
    lay_pitch_mm: f64,
    strand_pitch_mm: f64,
    lay_angle_deg: f64,
    rope_triangles: usize,
    band_only_s: f64,
    one_rope_joined_s: f64,
    both_ropes_joined_s: f64,
    with_collet_s: f64,
    first_join_s: f64,
    mirror_join_s: f64,
}
fn rope_timing(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<RopeTiming> {
    let time = |id: u64| -> Result<(f64, mesh::BuildResult)> {
        let t = through(d, id);
        let s = Instant::now();
        let b = mesh::try_build(&t, lib, params)?;
        Ok((s.elapsed().as_secs_f64(), b))
    };
    let (band_s, _) = time(5)?;
    let (one_s, one) = time(6)?;
    let (two_s, _) = time(7)?;
    let (all_s, _) = time(9)?;
    let rope_triangles = one
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| c.name == "Rope")
        .map(|c| c.mesh.faces.len())
        .sum();
    let length = PATH_R_MM * (end_deg() + 360.0 - start_deg()).to_radians();
    let turns = TWIST_DEG / 360.0;
    let pitch = length / turns;
    let outer = STRAND_OFFSET_MM + STRAND_R_MM;
    Ok(RopeTiming {
        build: [params.theta_steps, params.profile_steps],
        rope_length_mm: length,
        twist_turns: turns,
        lay_pitch_mm: pitch,
        strand_pitch_mm: pitch / STRANDS as f64,
        lay_angle_deg: (2.0 * PI * outer / pitch).atan().to_degrees(),
        rope_triangles,
        band_only_s: band_s,
        one_rope_joined_s: one_s,
        both_ropes_joined_s: two_s,
        with_collet_s: all_s,
        first_join_s: one_s - band_s,
        mirror_join_s: two_s - one_s,
    })
}

#[derive(Serialize)]
struct Report {
    name: String,
    slug: &'static str,
    process: String,
    size: String,
    bore_mm: f64,
    stone: String,
    cad_features: Vec<String>,
    design_bytes: u64,
    design_format: u64,
    rope: Option<RopeTiming>,
    feature_build_s: Vec<(String, f64)>,
    draft: Gates,
    export: Option<Gates>,
    gates_passed: bool,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina/torsade"));
    std::fs::create_dir_all(&out)?;
    println!("Torsade");
    let lib = AlphaLibrary::builtin();
    let d = author()?;
    let design_path = out.join("design.ring.json");
    library::save_design(&design_path, &d)?;
    let text = std::fs::read_to_string(&design_path)?;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let (draft_gates, draft_built) = gates(&d, &lib, draft_params(), draft.then_some(design_path.as_path()).filter(|_| verify))?;
    print_gates("draft", &draft_gates);
    let (export_gates, built, params) = if draft {
        (None, draft_built, draft_params())
    } else {
        let (g, b) = gates(&d, &lib, export_params(), verify.then_some(design_path.as_path()))?;
        print_gates("export", &g);
        (Some(g), b, export_params())
    };
    let rope = rope_timing(&d, &lib, params).ok();
    if let Some(r) = &rope {
        println!(
            "  rope {:.1} mm, {} turns, lay pitch {:.2} mm, strand pitch {:.2} mm, lay {:.0} deg, {} triangles; band {:.2} s, + rope {:.2} s, + mirror {:.2} s, + collet {:.2} s",
            r.rope_length_mm, r.twist_turns, r.lay_pitch_mm, r.strand_pitch_mm, r.lay_angle_deg, r.rope_triangles, r.band_only_s, r.first_join_s, r.mirror_join_s, r.with_collet_s - r.both_ropes_joined_s
        );
    }
    let timing = renders(&out, &d, &lib, &built, params, if draft { 1000 } else { 1600 })?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let fin = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(&d, &lib, &built) };
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-cabochon.stl".to_string() } else { format!("reference-cabochon-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Torsade reference stone")?;
            materials.push(json!({ "mesh": file, "name": "Garnet cabochon 7.0", "tint": tint, "ior": 1.54, "dispersion": 0.013, "roughness": 0.065, "transmission": 0.35 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    let gates_passed = draft_gates.passed && export_gates.as_ref().is_none_or(|g| g.passed);
    let report = Report {
        name: d.name.clone(),
        slug: "torsade",
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: built.report.inner_diameter_mm,
        stone: "Round cabochon 7.0 mm in a cast collet".into(),
        cad_features: d.cad.as_ref().unwrap().features.iter().map(|f| format!("{}. {} ({})", f.id, f.name, f.operation.label())).collect(),
        design_bytes: text.len() as u64,
        design_format,
        rope,
        feature_build_s: timing,
        draft: draft_gates,
        export: export_gates,
        gates_passed,
    };
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("  gates {}", if gates_passed { "passed" } else { "FAILED" });
    Ok(())
}

fn print_gates(label: &str, g: &Gates) {
    println!(
        "  {label} {}x{}: {} tris in {:.1} s; watertight {}; degenerate {}; crossings {}; parts with crossings {}; features ok {}; nearest axis {:.3} (bore {:.3}); field {}; thickness min {:?} clean {}; lands {}; dfm {}; stones {}/{} ({:.2} over seat); pattern {:?}; cold {:?}; passed {}",
        g.build[0],
        g.build[1],
        g.metal.triangles,
        g.build_s,
        g.metal.watertight,
        g.metal.degenerate_faces,
        g.metal.self_crossings,
        g.made_parts.iter().filter(|(_, n)| *n > 0).count(),
        g.features_ok,
        g.nearest_to_axis_mm,
        g.bore_radius_mm,
        g.field_verdict,
        g.thickness_sampled_min_mm,
        g.thickness_clean,
        g.cut_lands.len(),
        g.dfm_findings.len(),
        g.stones_reported,
        g.stones_previewed,
        g.stone_over_seat_mm,
        g.pattern.as_ref().map(|p| (p.watertight, p.degenerate_faces, p.self_crossings)),
        g.cold_reload_identical,
        g.passed
    );
    for (id, name, s) in g.features.iter().filter(|f| f.2 != "Ok") {
        println!("    feature {id} {name}: {s}");
    }
    for n in g.solids_notes.iter().chain(&g.parts_notes).chain(&g.dfm_findings).chain(&g.cut_lands) {
        println!("    note: {n}");
    }
}
