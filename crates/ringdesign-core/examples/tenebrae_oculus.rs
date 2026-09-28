//! Tenebrae — Oculus, the wheel: twenty-four lancet lights pierce a flat band from side face to side face, so the side face is a wheel window and the finger its oculus. Delft two-part sand.
//! cargo build --release -p ringdesign-core --example tenebrae_oculus
//! target/release/examples/tenebrae_oculus [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus,
        Operation, PatternKind, PlaneBase, Profile, Stage,
    },
    castability::{self, CastProcess, SandProcess},
    csg, dfm, library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch},
    stl,
};
use serde_json::json;
use std::path::{Path, PathBuf};

const BORE_MM: f64 = 18.6;
const WIDTH_MM: f64 = 7.0;
const THICKNESS_MM: f64 = 4.6;
const LIGHTS: usize = 24;
/// Mullion bar between two lights, mm.
const BAR_MM: f64 = 0.9;
/// Bore-side rail: the lights start this far outside the bore, mm.
const RAIL_MM: f64 = 1.0;
/// Radius of the lights' pointed heads' apex, mm.
const APEX_R_MM: f64 = 12.4;
/// Radius of the lights' spring line, mm.
const SPRING_R_MM: f64 = 10.8;
/// Draft on each half of a light, degrees from the pull.
const LIGHT_DRAFT_DEG: f64 = 3.5;
/// Crown exponent: 2 is an ellipse over the band's width.
const CROWN_SHAPE: f64 = 1.7;
const HALF_WIDTH_MM: f64 = WIDTH_MM * 0.5;
/// How far past each side face the pierce runs, mm.
const OVERSHOOT_MM: f64 = 0.3;
/// Where the first light is centred, degrees round the ring; 90 is the crown.
const FIRST_LIGHT_DEG: f64 = 90.0;
/// Radius the lights start at: the bore plus its rail, mm.
const RAIL_R_MM: f64 = BORE_MM * 0.5 + RAIL_MM;
/// How much fuller the drag's lights are drawn than the cope's, per side, mm.


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

fn setup() -> Setup {
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.name = "Oculus / Delft clay / Silver 925".into();
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").unwrap().shrink_pct;
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, sand and measured trials.".into();
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel {
            kind: mf::ChannelKind::Gate,
            start: [0.0, -13.5, 0.0],
            end: [0.0, -22.0, 0.0],
            diameter_mm: 4.0,
        },
        mf::Channel {
            kind: mf::ChannelKind::Sprue,
            start: [0.0, -22.0, 0.0],
            end: [0.0, -32.0, 0.0],
            diameter_mm: 6.0,
        },
    ];
    s.bench_notes = "Two-part Delft clay, parting on the band's mid-plane, pull along the finger. Each half of every light is a drafted sand pin hanging from its own mould half; ram the pins firmly and trial one flask before a run. Dress the parting seam through the lights with a needle file; polish the side faces and crown, leave the lights satin.".into();
    s
}

/// The flat band: 7.0 wide, 4.6 thick, squared side faces, uniform all the way round.
fn band() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Oculus".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = WIDTH_MM;
    d.profile.thickness_mm = THICKNESS_MM;
    d.profile.flatten_sides();
    // An elliptic crown, not the flat one: the crown's walls then carry draft to the parting plane.
    d.profile.shape_a = CROWN_SHAPE;
    d.profile.edge_round_mm = 0.25;
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Uniform;
    let s = setup();
    d.draft.process = CastProcess::SandTwoPart;
    SandProcess::DelftClay.apply(&mut d.draft);
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.build = draft_params();
    d.manufacturing = Some(s);
    d
}

fn rot(p: [f64; 2], a: f64) -> [f64; 2] {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

/// A lancet light's outline on the +y axis, `inset` inside its nominal: the inner arc on the bore rail, two jambs parallel to
/// their mullions, and a pointed head of two arcs springing tangent from the jambs and meeting on the axis at the apex.
struct Lancet {
    alpha: f64,
    /// Jamb: x = y tan(alpha) - off, nominal.
    off: f64,
    inner_r: f64,
    spring: [f64; 2],
    /// Centre and radius of the right head arc, nominal; the left is its mirror.
    centre: [f64; 2],
    radius: f64,
}
impl Lancet {
    fn new(lights: usize, bar: f64, inner_r: f64, spring_r: f64, apex_r: f64) -> Self {
        let alpha = std::f64::consts::PI / lights as f64;
        let off = bar * 0.5 / alpha.cos();
        let spring = [spring_r * alpha.tan() - off, spring_r];
        let n = [-alpha.cos(), alpha.sin()];
        let apex_of = |rho: f64| {
            let c = [spring[0] + rho * n[0], spring[1] + rho * n[1]];
            c[1] + (rho * rho - c[0] * c[0]).max(0.0).sqrt()
        };
        // The head's radius that puts the apex at `apex_r`, by bisection between a round head and a very sharp one.
        let (mut lo, mut hi) = (spring[0] / alpha.cos(), 40.0);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if apex_of(mid) < apex_r { lo = mid } else { hi = mid }
        }
        let radius = 0.5 * (lo + hi);
        Self { alpha, off, inner_r, spring, centre: [spring[0] + radius * n[0], spring[1] + radius * n[1]], radius }
    }
    /// The closed outline `inset` inside the nominal, right jamb up, head, left jamb down, inner arc back.
    fn outline(&self, inset: f64, head_steps: usize, inner_steps: usize) -> Vec<[f64; 2]> {
        let (t, ca) = (self.alpha.tan(), self.alpha.cos());
        let off = self.off + inset / ca;
        let r_in = self.inner_r + inset;
        // Inner corner: the jamb x = y t - off meets the circle of radius r_in.
        let y_in = {
            let (a, b, c) = (1.0 + t * t, -2.0 * t * off, off * off - r_in * r_in);
            (-b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a)
        };
        let n = [-ca, self.alpha.sin()];
        let spring = [self.spring[0] + inset * n[0], self.spring[1] + inset * n[1]];
        let rho = self.radius - inset;
        let c = self.centre;
        let a0 = (spring[1] - c[1]).atan2(spring[0] - c[0]);
        let apex_y = c[1] + (rho * rho - c[0] * c[0]).sqrt();
        let a1 = (apex_y - c[1]).atan2(-c[0]);
        let mut pts = vec![[y_in * t - off, y_in]];
        for k in 0..=head_steps {
            let a = a0 + (a1 - a0) * k as f64 / head_steps as f64;
            pts.push([c[0] + rho * a.cos(), c[1] + rho * a.sin()]);
        }
        let right: Vec<[f64; 2]> = pts[1..pts.len() - 1].to_vec();
        pts.extend(right.iter().rev().map(|p| [-p[0], p[1]]));
        pts.push([-(y_in * t - off), y_in]);
        let b0 = y_in.atan2(-(y_in * t - off));
        let b1 = y_in.atan2(y_in * t - off);
        for k in 1..inner_steps {
            let b = b0 + (b1 - b0) * k as f64 / inner_steps as f64;
            pts.push([r_in * b.cos(), r_in * b.sin()]);
        }
        pts
    }
}

/// One light drawn `inset_mm` inside its nominal outline, turned onto `at_deg`: the section a drafted wall passes through at some height.
fn light(name: &str, inset_mm: f64, at_deg: f64, plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    let turn = (at_deg - 90.0).to_radians();
    let points: Vec<Id> = Lancet::new(LIGHTS, BAR_MM, RAIL_R_MM, SPRING_R_MM, APEX_R_MM)
        .outline(inset_mm, 16, 8)
        .into_iter()
        .map(|p| s.point(rot(p, turn)))
        .collect();
    // Straight edges, one per facet, so the loft rules each wall between matching corners.
    for k in 0..points.len() {
        s.entity(Geometry::Line { a: points[k], b: points[(k + 1) % points.len()] });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

/// The lands the wheel leaves, measured from the sketch numbers: mullion, bore rail and outer rail.
struct Lands {
    mullion_mm: f64,
    bore_rail_mm: f64,
    outer_rail_mm: f64,
    light_width_inner_mm: f64,
    light_width_spring_mm: f64,
    light_height_mm: f64,
}
fn lands(d: &RingDesign) -> Lands {
    let alpha = std::f64::consts::PI / LIGHTS as f64;
    let off = BAR_MM * 0.5 / alpha.cos();
    let bore = d.inner_radius_mm();
    let crown = d.profile.effective_crown_mm();
    let run_end = bore + d.profile.thickness_mm - crown;
    let jamb = |y: f64| y * alpha.tan() - off;
    Lands {
        mullion_mm: BAR_MM,
        bore_rail_mm: bore + RAIL_MM - bore,
        outer_rail_mm: run_end - APEX_R_MM,
        light_width_inner_mm: 2.0 * jamb(bore + RAIL_MM),
        light_width_spring_mm: 2.0 * jamb(SPRING_R_MM),
        light_height_mm: APEX_R_MM - (bore + RAIL_MM),
    }
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}
fn cut() -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, stage: Stage::Cast, ..Default::default() }
}

/// The design: the band, the high side-face plane, the wheel sketch, its drafted cut to the parting plane and the mirrored drag half.
fn author() -> Result<RingDesign> {
    let mut d = band();
    let mut doc = Document::default();
    let t = LIGHT_DRAFT_DEG.to_radians().tan();
    let face = HALF_WIDTH_MM + OVERSHOOT_MM;
    let plane = |offset_mm: f64| Operation::Plane { base: PlaneBase::Parting, offset_mm };
    let sketch = |sketch: Sketch| Operation::Sketch { sketch };
    let none = Component::default;
    doc.append(feature(1, "Procedural shank", Operation::Band, none()))?;
    doc.append(feature(2, "High side face, lifted clear of the metal", plane(face), none()))?;
    doc.append(feature(3, "Parting plane", plane(0.0), none()))?;
    doc.append(feature(4, "Low side face, lifted clear of the metal", plane(-face), none()))?;
    // Each section is the light the drafted wall passes through at its plane: nominal at the side face, narrowest at the parting plane.
    doc.append(feature(5, "One lancet light, as it opens on the high side face", sketch(light("Light, high face", -OVERSHOOT_MM * t, FIRST_LIGHT_DEG, 2)), none()))?;
    doc.append(feature(6, "Its waist on the parting plane", sketch(light("Light, waist", HALF_WIDTH_MM * t, FIRST_LIGHT_DEG, 3)), none()))?;
    doc.append(feature(7, "The same light on the low side face", sketch(light("Light, low face", -OVERSHOOT_MM * t, FIRST_LIGHT_DEG, 4)), none()))?;
    doc.append(feature(
        8,
        "Pierce one light through the band, drafted from both side faces to its waist",
        Operation::Loft { sections: vec![Profile::Feature { feature: 7 }, Profile::Feature { feature: 6 }, Profile::Feature { feature: 5 }] },
        cut(),
    ))?;
    doc.append(feature(
        9,
        "Wheel the light round the finger: twenty-four lights",
        Operation::Pattern { sources: cad::pattern::Sources(vec![8]), kind: PatternKind::Ring { count: LIGHTS as u32, span_deg: 360.0 } },
        cut(),
    ))?;
    d.cad = Some(doc);
    Ok(d)
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(),
        f: m.faces.clone(),
    }
}
fn mesh_of(s: &csg::Solid) -> mesh::Mesh {
    let mut m = mesh::Mesh::default();
    m.vertices = s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect();
    m.faces = s.f.clone();
    m.normals = vec![mesh::Vec3(0.0, 0.0, 1.0); m.vertices.len()];
    m
}

fn cuboid(lo: [f64; 3], hi: [f64; 3]) -> csg::Solid {
    let v: Vec<[f64; 3]> = (0..8).map(|i: usize| std::array::from_fn(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] })).collect();
    let quads: [[u32; 4]; 6] = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
    let f = quads.iter().flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect();
    csg::Solid { v, f }
}

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Views: yaw about the finger axis, pitch from looking along the finger (0) to down onto the crown (pi/2).
const VIEWS: &[(&str, f64, f64)] = &[
    ("hero", 0.42, 0.40),
    ("face", 0.0, 0.0),
    ("palm", std::f64::consts::PI, 1.05),
    ("side", 0.0, std::f64::consts::FRAC_PI_2),
    ("crown", 0.0, 1.25),
    ("shoulder", -0.9, 0.62),
    ("reverse", 2.6, 0.5),
];

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, *yaw, *pitch, edge)?;
    }
    for name in ["hero", "face"] {
        let (_, yaw, pitch) = VIEWS.iter().find(|v| v.0 == name).unwrap();
        render::write_png_parts(out.join(format!("{name}-300.png")), &parts, *yaw, *pitch, 300)?;
    }
    // Stones: none; the close-up is the wheel seen square along the finger.
    render::write_png_parts(out.join("stones.png"), &parts, 0.0, 0.0, edge)?;
    // Section: the half ring behind the plane through the axis at theta 0, seen square to the cut.
    let solid = solid_of(&built.mesh);
    let keep = cuboid([-30.0, -30.0, -10.0], [0.0, 30.0, 10.0]);
    if let Ok(half) = csg::combine(&solid, &keep, csg::Op::Intersect) {
        let m = mesh_of(&half);
        let mut p = render::Part::metal(&m, render::GOLD);
        p.smooth = false;
        render::write_png_parts(out.join("section.png"), &[p], -std::f64::consts::FRAC_PI_2, 1.2, edge)?;
    }
    let bare = band();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let (_, yaw, pitch) = VIEWS[0];
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    // Contact sheet at 300 px: hero, face, palm, side.
    let tiles: Vec<Vec<u8>> = ["hero", "face", "side", "palm"]
        .iter()
        .map(|n| {
            let (_, yaw, pitch) = VIEWS.iter().find(|v| v.0 == *n).unwrap();
            render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3)
        })
        .collect();
    let mut sheet = vec![0u8; 1200 * 300 * 3];
    for (k, t) in tiles.iter().enumerate() {
        for y in 0..300 {
            sheet[y * 3600 + k * 900..y * 3600 + (k + 1) * 900].copy_from_slice(&t[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 1200, 300, image::ColorType::Rgb8)?;
    let _ = d;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/oculus"));
    std::fs::create_dir_all(&out)?;
    println!("Oculus");
    let mut d = author()?;
    let lib = AlphaLibrary::builtin();
    let params = if draft { draft_params() } else { export_params() };
    d.build = params;
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    println!(
        "  {} triangles in {build_s:.1} s; watertight {}; degenerate {}",
        built.mesh.faces.len(),
        v.watertight,
        q.degenerate_faces
    );
    let features: Vec<(Id, String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| {
            let s = match &r.status {
                FeatureStatus::Ok => "Ok".to_string(),
                FeatureStatus::Suppressed => "Suppressed".into(),
                FeatureStatus::Failed(m) => format!("Failed: {m}"),
                FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
            };
            (r.id, r.name.clone(), s)
        })
        .collect();
    for (id, name, s) in &features {
        println!("    #{id} {name}: {s}");
    }
    let made: Vec<(String, usize)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| (c.name.clone(), csg::self_crossings(&solid_of(&c.mesh))))
        .collect();
    if std::env::var("OCULUS_DEBUG").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            let b = c.mesh.bounds();
            println!("    part {} {}: {} faces, bounds {:?}", c.id, c.name, c.mesh.faces.len(), b);
            render::write_png_parts(out.join(format!("debug-part-{}.png", c.id)), &[render::Part::metal(&c.mesh, render::GOLD)], 0.0, 0.0, 800)?;
        }
    }
    let ring_crossings = csg::self_crossings(&solid_of(&built.mesh));
    let bore = d.inner_radius_mm();
    let inside = built
        .mesh
        .vertices
        .iter()
        .filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01)
        .count();
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let l = lands(&d);
    let grams = built.report.metals.iter().find(|m| m.metal == "Silver 925").map_or(0.0, |m| m.grams);
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
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
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let pattern_ok = pattern.report.validation.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0;
    let stone_count = stones_report.as_ref().map_or(0, |s| s.stone_count) as usize;
    let gates = json!({
        "watertight": v.watertight,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "made_part_self_crossings": made,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "features": features.iter().map(|(id, n, s)| json!({"id": id, "name": n, "status": s})).collect::<Vec<_>>(),
        "stamps_unresolved": built.solids.notes.len(),
        "bore_radius_mm": bore,
        "min_vertex_radius_mm": min_r,
        "vertices_inside_bore": inside,
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0_100": {"status": format!("{:?}", inspection.release.status), "obstructions": inspection.release.obstructions.len(), "unresolved": inspection.release.unresolved_rays},
        "release_0_075": {"status": format!("{:?}", release_fine.status), "obstructions": release_fine.obstructions.len(), "unresolved": release_fine.unresolved_rays},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones_reported": stone_count,
        "stones_previewed": previewed,
        "cold_reload_identical": cold,
        "triangles": built.mesh.faces.len(),
        "pattern": {"watertight": pattern.report.validation.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings},
    });
    let passed = v.watertight
        && q.degenerate_faces == 0
        && ring_crossings == 0
        && made.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && features.iter().all(|(_, _, s)| s == "Ok")
        && inside == 0
        && field.verdict == castability::Verdict::Castable
        && inspection.release.obstructions.is_empty()
        && inspection.release.unresolved_rays == 0
        && release_fine.obstructions.is_empty()
        && release_fine.unresolved_rays == 0
        && findings.is_empty()
        && stone_count == previewed
        && cold != Some(false)
        && built.mesh.faces.len() <= 2_000_000
        && pattern_ok
        && l.mullion_mm >= 0.8
        && l.bore_rail_mm >= 0.8
        && l.outer_rail_mm >= 0.8;
    let block = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "gates": gates,
        "lands": {
            "mullion_mm": l.mullion_mm, "bore_rail_mm": l.bore_rail_mm, "outer_rail_mm": l.outer_rail_mm,
            "light_width_inner_mm": l.light_width_inner_mm, "light_width_spring_mm": l.light_width_spring_mm, "light_height_mm": l.light_height_mm,
            "axial_web_mm": WIDTH_MM,
        },
        "gates_passed": passed,
    });
    let report_path = out.join("report.json");
    let mut report: serde_json::Value = std::fs::read(&report_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));
    report["name"] = json!(d.name);
    report["process"] = json!(format!("{} / {}", d.draft.process.label(), "Delft clay"));
    report["alloy"] = json!("Silver 925");
    report["size"] = json!(d.size.display());
    report["bore_mm"] = json!(built.report.inner_diameter_mm);
    report["band"] = json!({"width_mm": WIDTH_MM, "thickness_mm": THICKNESS_MM, "lights": LIGHTS});
    report["grams_silver_925"] = json!(grams);
    report["design_bytes"] = json!(design_bytes);
    report["cad_features"] = json!(d.cad.as_ref().map_or(0, |c| c.features.len()));
    report["layers"] = json!(d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>());
    report["draft_settings"] = json!(d.draft);
    report[if draft { "draft" } else { "export" }] = block;
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
    }
    renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.4}% undercut, worst draft {:.2}), thinnest wall {:.2}; release 0.100: {} obstructions {} unresolved; 0.075: {} / {}; dfm {}; min r {:.3} vs bore {:.3}; {:.1} g silver",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays,
        release_fine.obstructions.len(),
        release_fine.unresolved_rays,
        findings.len(),
        min_r,
        bore,
        grams
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in built.parts.notes.iter().chain(&built.solids.notes).chain(&field.notes) {
        println!("    note: {n}");
    }
    println!("  gates {}", if passed { "passed" } else { "FAILED" });
    ensure!(passed || std::env::var("OCULUS_ALLOW_FAIL").is_ok(), "Oculus failed its gates; see {}", report_path.display());
    Ok(())
}
