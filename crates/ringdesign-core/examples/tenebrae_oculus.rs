//! Tenebrae — Oculus, the wheel: seen along the finger the band's side face is a wheel window and the finger its oculus.
//! Twelve pointed lancets and twelve quatrefoils pierce the band from side face to side face inside a sunk tracery
//! field, a blind arcade of pointed arches rides the crown over the lancets, and beads run the crest. Lost wax.
//! cargo build --release -p ringdesign-core --example tenebrae_oculus
//! target/release/examples/tenebrae_oculus [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign, ShankKind,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, PlaneBase, Profile, Stage},
    castability::{self, CastProcess},
    csg, dfm,
    field::MilgrainLayer,
    library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch},
    stl,
};
use serde_json::json;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::path::{Path, PathBuf};

const BORE_MM: f64 = 18.6;
const WIDTH_MM: f64 = 7.0;
const THICKNESS_MM: f64 = 4.6;
const ALLOY: &str = "Silver 925";
/// Lancets round the wheel, and as many quatrefoils between them.
const LIGHTS: usize = 12;
/// Bore-side rail: the lights start this far outside the bore, mm.
const RAIL_MM: f64 = 1.0;
/// Radius the lights start at: the bore plus its rail, mm.
const RAIL_R_MM: f64 = BORE_MM * 0.5 + RAIL_MM;
/// Radius of the lancets' apex, mm.
const APEX_R_MM: f64 = 12.4;
/// Width of a lancet between its jambs, mm.
const LANCET_W_MM: f64 = 1.5;
/// Steps along each half of a lancet's head.
const HEAD_STEPS: usize = 10;
/// Quatrefoil: the lobes' radius, their centres' distance from its middle, and the radius of the cusps' round tips, mm;
/// the radius it is centred on.
const QUAT_LOBE_MM: f64 = 0.45;
const QUAT_CENTRE_MM: f64 = 0.55;
const QUAT_CUSP_MM: f64 = 0.4;
const QUAT_R_MM: f64 = 11.3;
const QUAT_STEPS: usize = 12;
const CUSP_STEPS: usize = 6;
/// Where the first lancet is centred, degrees round the ring; 90 is the crown. The quatrefoils sit halfway between.
const FIRST_LIGHT_DEG: f64 = 90.0;
/// Tracery field sunk into each side face: inner and outer radius and depth, mm.
const FIELD_IN_MM: f64 = 10.2;
const FIELD_OUT_MM: f64 = 12.7;
const FIELD_DEPTH_MM: f64 = 0.6;
const FIELD_DRAFT_DEG: f64 = 20.0;
/// Crown arcade: one pointed arch over each lancet, sunk into the crown. Width round the ring, its foot and apex along the
/// finger from the crest as drawn, and its depth under the crest, mm. It is laid turned half round, so the foot stands at the
/// high side face and the apex points past the crest toward the low one.
const ARCH_W_MM: f64 = 2.8;
const ARCH_FOOT_MM: f64 = -2.55;
const ARCH_APEX_MM: f64 = 1.7;
const ARCH_DEPTH_MM: f64 = 0.7;
const ARCH_STEPS: usize = 10;
/// The crest's milgrain, off: its beads measure under the investment's section.
const WITH_BEADS: bool = false;
const BEAD_MM: f64 = 0.9;
const BEAD_H_MM: f64 = 0.3;
const BEADS: u32 = 72;
/// Crown exponent: 2 is an ellipse over the band's width.
const CROWN_SHAPE: f64 = 1.7;
const HALF_WIDTH_MM: f64 = WIDTH_MM * 0.5;
/// How far past each side face the piercing runs, mm.
const OVERSHOOT_MM: f64 = 0.3;
/// Least metal the investment fills, and the least a tracery bar or rail may be, mm.
const MIN_SECTION_MM: f64 = 0.8;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn setup() -> Setup {
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(castability::SandProcess::DelftClay);
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.name = format!("Oculus / investment / {ALLOY}");
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).unwrap().shrink_pct;
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -13.5, 0.0], end: [0.0, -22.0, 0.0], diameter_mm: 3.5 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -22.0, 0.0], end: [0.0, -32.0, 0.0], diameter_mm: 5.5 },
    ];
    s.bench_notes = "Investment cast in one piece, sprued at the palm. Flush investment from every lancet and quatrefoil with water under pressure. Polish the hub, rim and crown; leave the sunk tracery field and the crown arcade satin, and darken them if the client wants the window read at a distance.".into();
    s
}

/// The flat band: 7.0 wide, 4.6 thick, squared side faces, a shallow elliptic crown, uniform all the way round.
fn band() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Oculus".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = WIDTH_MM;
    d.profile.thickness_mm = THICKNESS_MM;
    d.profile.flatten_sides();
    d.profile.shape_a = CROWN_SHAPE;
    d.profile.edge_round_mm = 0.25;
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Uniform;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.build = draft_params();
    d.manufacturing = Some(setup());
    d
}

fn rot(p: [f64; 2], a: f64) -> [f64; 2] {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}
fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}
/// `steps` points along the circle from `a` to `b`, the way that passes nearest `via`, both ends included.
fn arc_pts(c: [f64; 2], r: f64, a: [f64; 2], b: [f64; 2], via: [f64; 2], steps: usize) -> Vec<[f64; 2]> {
    let ang = |p: [f64; 2]| (p[1] - c[1]).atan2(p[0] - c[0]);
    let (a0, a1, av) = (ang(a), ang(b), ang(via));
    let wrap = |x: f64| x.rem_euclid(TAU);
    let ccw = wrap(a1 - a0);
    let sweep = if wrap(av - a0) <= ccw { ccw } else { ccw - TAU };
    (0..=steps).map(|k| { let t = a0 + sweep * k as f64 / steps as f64; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect()
}

/// A lancet on the +y axis: parallel jambs from a flat sill on the bore rail up to an equilateral pointed head. Counter-clockwise.
fn lancet() -> Vec<[f64; 2]> {
    let h = LANCET_W_MM * 0.5;
    let spring_y = APEX_R_MM - 2.0 * h * 60f64.to_radians().sin();
    // The right head arc is centred on the left springer, radius the lancet's width; the left mirrors it.
    let head = |k: usize| { let a = (60.0 * k as f64 / HEAD_STEPS as f64).to_radians(); [-h + 2.0 * h * a.cos(), spring_y + 2.0 * h * a.sin()] };
    let mut pts = vec![[-h, RAIL_R_MM], [h, RAIL_R_MM]];
    pts.extend((0..HEAD_STEPS).map(head));
    pts.push([0.0, APEX_R_MM]);
    pts.extend((0..HEAD_STEPS).rev().map(|k| { let p = head(k); [-p[0], p[1]] }));
    pts
}

/// A quatrefoil about the origin: four round lobes on the axes and, where they meet, four cusps with round tips as broad as
/// the investment's least section. Counter-clockwise.
fn quatrefoil() -> Vec<[f64; 2]> {
    let (e, r, rc) = (QUAT_CENTRE_MM, QUAT_LOBE_MM, QUAT_CUSP_MM);
    let c = |k: usize| { let a = k as f64 * FRAC_PI_2; [e * a.cos(), e * a.sin()] };
    // Each cusp's tip circle sits on a diagonal, touching the lobes either side of it.
    let s45 = std::f64::consts::FRAC_1_SQRT_2;
    let t = e * s45 + ((r + rc).powi(2) - (e * s45).powi(2)).sqrt();
    let f = |k: usize| { let a = (k as f64 + 0.5) * FRAC_PI_2; [t * a.cos(), t * a.sin()] };
    let touch = |lobe: [f64; 2], tip: [f64; 2]| { let d = sub2(tip, lobe); let l = len2(d); [lobe[0] + r * d[0] / l, lobe[1] + r * d[1] / l] };
    let mut pts = Vec::new();
    for k in 0..4 {
        let (ck, fin, fout, next) = (c(k), f((k + 3) % 4), f(k), c((k + 1) % 4));
        let tip = [ck[0] * 2.0, ck[1] * 2.0];
        pts.extend(arc_pts(ck, r, touch(ck, fin), touch(ck, fout), tip, QUAT_STEPS).into_iter().take(QUAT_STEPS));
        let inner = { let l = len2(fout); [fout[0] * (l - rc) / l, fout[1] * (l - rc) / l] };
        pts.extend(arc_pts(fout, rc, touch(ck, fout), touch(next, fout), inner, CUSP_STEPS).into_iter().take(CUSP_STEPS));
    }
    // Start mid-lobe, a convex corner.
    pts.rotate_left(QUAT_STEPS / 2);
    pts
}

/// A pointed arch for the crown, in a plane square to the crest: `x` round the ring, `y` along the finger. Counter-clockwise.
fn crown_arch() -> Vec<[f64; 2]> {
    let h = ARCH_W_MM * 0.5;
    // An equilateral head, its springing where the apex leaves room for it.
    let spring = ARCH_APEX_MM - 2.0 * h * 60f64.to_radians().sin();
    let mut pts = vec![[-h, ARCH_FOOT_MM], [h, ARCH_FOOT_MM]];
    pts.extend((0..ARCH_STEPS).map(|k| { let a = (60.0 * k as f64 / ARCH_STEPS as f64).to_radians(); [-h + 2.0 * h * a.cos(), spring + 2.0 * h * a.sin()] }));
    pts.push([0.0, ARCH_APEX_MM]);
    pts.extend((1..ARCH_STEPS).rev().map(|k| { let a = (60.0 * k as f64 / ARCH_STEPS as f64).to_radians(); [h - 2.0 * h * a.cos(), spring + 2.0 * h * a.sin()] }));
    pts
}

/// Closed outlines as one sketch on `plane`, each turned onto its angle round the ring and moved out to its radius.
fn outlines_sketch(name: &str, loops: &[(Vec<[f64; 2]>, f64, f64)], plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    for (pts, at_deg, radius) in loops {
        let turn = (at_deg - 90.0).to_radians();
        let ids: Vec<Id> = pts.iter().map(|p| s.point(rot([p[0], p[1] + radius], turn))).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

/// The tracery field: an annulus between the hub and the rim.
fn field_sketch(plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = "Tracery field".into();
    for r in [FIELD_OUT_MM, FIELD_IN_MM] {
        let c = s.point([0.0, 0.0]);
        let rim = s.point([r, 0.0]);
        s.entity(Geometry::Circle { center: c, rim });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

/// Shortest distance between two closed polygons, mm.
fn gap(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    let seg = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (d, e) = (sub2(b, a), sub2(p, a));
        let t = ((e[0] * d[0] + e[1] * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-18)).clamp(0.0, 1.0);
        len2(sub2(p, [a[0] + d[0] * t, a[1] + d[1] * t]))
    };
    let one = |a: &[[f64; 2]], b: &[[f64; 2]]| a.iter().map(|p| (0..b.len()).map(|k| seg(*p, b[k], b[(k + 1) % b.len()])).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
    one(a, b).min(one(b, a))
}
fn placed(pts: &[[f64; 2]], at_deg: f64, radius: f64) -> Vec<[f64; 2]> {
    let turn = (at_deg - 90.0).to_radians();
    pts.iter().map(|p| rot([p[0], p[1] + radius], turn)).collect()
}
fn foil_deg() -> f64 {
    FIRST_LIGHT_DEG + 180.0 / LIGHTS as f64
}

/// The lands the tracery leaves, measured on the drawn outlines; the cuts run straight through, so these hold through the band.
struct Lands {
    lancet_to_quatrefoil_mm: f64,
    bore_rail_mm: f64,
    outer_rail_mm: f64,
    hub_step_to_light_mm: f64,
    rim_step_to_light_mm: f64,
    quatrefoil_cusp_tip_mm: f64,
    arch_to_arch_mm: f64,
    arch_floor_mm: f64,
}
fn lands(d: &RingDesign) -> Lands {
    let bore = d.inner_radius_mm();
    let run_end = bore + d.profile.thickness_mm - d.profile.effective_crown_mm();
    let l = placed(&lancet(), FIRST_LIGHT_DEG, 0.0);
    let f = placed(&quatrefoil(), foil_deg(), QUAT_R_MM);
    let radii = |p: &[[f64; 2]]| p.iter().map(|q| len2(*q)).fold((f64::MAX, f64::MIN), |(lo, hi), r| (lo.min(r), hi.max(r)));
    let ((l_lo, l_hi), (f_lo, f_hi)) = (radii(&l), radii(&f));
    let crest = bore + d.profile.thickness_mm;
    Lands {
        lancet_to_quatrefoil_mm: gap(&l, &f),
        bore_rail_mm: l_lo.min(f_lo) - bore,
        outer_rail_mm: run_end - l_hi.max(f_hi),
        hub_step_to_light_mm: l_lo.min(f_lo) - FIELD_IN_MM,
        rim_step_to_light_mm: FIELD_OUT_MM - l_hi.max(f_hi),
        quatrefoil_cusp_tip_mm: 2.0 * QUAT_CUSP_MM,
        arch_to_arch_mm: TAU * (crest - ARCH_DEPTH_MM) / LIGHTS as f64 - ARCH_W_MM,
        arch_floor_mm: THICKNESS_MM - ARCH_DEPTH_MM,
    }
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}
fn cut() -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, stage: Stage::Cast, ..Default::default() }
}

/// The design: the band; the tracery field sunk into both side faces; the wheel of lancets and quatrefoils pierced
/// through; the crown arcade; the crest beads.
fn author() -> Result<RingDesign> {
    let mut d = band();
    let mut doc = Document::default();
    let plane = |offset_mm: f64| Operation::Plane { base: PlaneBase::Parting, offset_mm };
    let sketch = |sketch: Sketch| Operation::Sketch { sketch };
    let none = Component::default;
    doc.append(feature(1, "Procedural shank", Operation::Band, none()))?;
    doc.append(feature(2, "High side face", plane(HALF_WIDTH_MM), none()))?;
    doc.append(feature(3, "Tracery field between the hub and the rim", sketch(field_sketch(2)), none()))?;
    doc.append(feature(
        4,
        "Sink the tracery field, leaving the hub and the rim standing",
        Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: -FIELD_DEPTH_MM, draft_deg: FIELD_DRAFT_DEG },
        cut(),
    ))?;
    doc.append(feature(
        5,
        "Sink the same field in the low side face",
        Operation::Pattern { sources: cad::pattern::Sources(vec![4]), kind: PatternKind::Mirror { plane: cad::MirrorPlane::Band } },
        cut(),
    ))?;
    doc.append(feature(6, "High side face, lifted clear of the metal", plane(HALF_WIDTH_MM + OVERSHOOT_MM), none()))?;
    let loops = [(lancet(), FIRST_LIGHT_DEG, 0.0), (quatrefoil(), foil_deg(), QUAT_R_MM)];
    doc.append(feature(7, "One bay of the wheel: a pointed lancet and the quatrefoil beside its head", sketch(outlines_sketch("Bay", &loops, 6)), none()))?;
    doc.append(feature(
        8,
        "Pierce the bay through the band from side face to side face",
        Operation::Extrude { sketch: Profile::Feature { feature: 7 }, height_mm: -(WIDTH_MM + 2.0 * OVERSHOOT_MM), draft_deg: 0.0 },
        cut(),
    ))?;
    doc.append(feature(
        9,
        "Wheel the bay round the finger: twelve lancets and twelve quatrefoils",
        Operation::Pattern { sources: cad::pattern::Sources(vec![8]), kind: PatternKind::Ring { count: LIGHTS as u32, span_deg: 360.0 } },
        cut(),
    ))?;
    doc.append(feature(10, "Square to the crest over the first lancet", Operation::Plane { base: PlaneBase::Tangent { theta_deg: FIRST_LIGHT_DEG, across_mm: 0.0 }, offset_mm: 0.0 }, none()))?;
    let mut arch = outlines_sketch("Crown arch", &[(crown_arch(), 270.0, 0.0)], 10);
    arch.name = "Crown arch".into();
    doc.append(feature(11, "A pointed arch on the crown, standing on the high side face's edge", sketch(arch), none()))?;
    doc.append(feature(
        12,
        "Sink the arch into the crown",
        Operation::Extrude { sketch: Profile::Feature { feature: 11 }, height_mm: -ARCH_DEPTH_MM, draft_deg: 10.0 },
        cut(),
    ))?;
    doc.append(feature(
        13,
        "Wheel the arch round the crown: a blind arcade over the lancets",
        Operation::Pattern { sources: cad::pattern::Sources(vec![12]), kind: PatternKind::Ring { count: LIGHTS as u32, span_deg: 360.0 } },
        cut(),
    ))?;
    d.cad = Some(doc);
    if WITH_BEADS {
        let ctx = d.field_context();
        d.layers.layers.push(LayerEntry::new(
            "Crest beads",
            Layer::Milgrain(MilgrainLayer { v_mm: ctx.crest_v_mm, bead_diameter_mm: BEAD_MM, beads_around: BEADS, height_mm: BEAD_H_MM, mirror: false }),
        ));
    }
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
    ("hero", 0.5, 0.45),
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
    // Lost wax: the investment fill replaces the sand pull. Wall thickness is screened on a build light enough for the measure.
    let coarse = BuildParams { theta_steps: 384, profile_steps: 160, ..params };
    let thin_mesh = mesh::try_build(&d, &lib, coarse)?.mesh;
    let thickness = cad::measure::thickness(&thin_mesh, MIN_SECTION_MM);
    println!("  thickness at {} x {} ({} faces): {} rays, min {:?}, {} below {:.1}, {} unresolved at {:?}", coarse.theta_steps, coarse.profile_steps, thin_mesh.faces.len(), thickness.rays, thickness.sampled_min_mm, thickness.below_limit, MIN_SECTION_MM, thickness.unresolved, thickness.point.map(|p| (p[0].hypot(p[1]), p[1].atan2(p[0]).to_degrees(), p[2])));
    let l = lands(&d);
    let grams = built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams);
    let grams_18k = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
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
        "investment_min_section_mm": d.draft.min_section_mm,
        "thickness": {"build": [coarse.theta_steps, coarse.profile_steps], "faces": thin_mesh.faces.len(), "limit_mm": thickness.limit_mm, "rays": thickness.rays, "sampled_min_mm": thickness.sampled_min_mm, "below_limit": thickness.below_limit, "unresolved": thickness.unresolved, "note": thickness.note},
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
        && thickness.rays > 0
        && thickness.below_limit == 0
        && thickness.unresolved == 0
        && findings.is_empty()
        && stone_count == previewed
        && cold != Some(false)
        && built.mesh.faces.len() <= 2_000_000
        && pattern_ok
        && l.lancet_to_quatrefoil_mm >= MIN_SECTION_MM
        && l.bore_rail_mm >= MIN_SECTION_MM
        && l.outer_rail_mm >= MIN_SECTION_MM
        && l.arch_to_arch_mm >= MIN_SECTION_MM
        && l.arch_floor_mm >= MIN_SECTION_MM
        && l.hub_step_to_light_mm >= 0.1
        && l.rim_step_to_light_mm >= 0.1;
    let block = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "gates": gates,
        "lands": {
            "lancet_to_quatrefoil_mm": l.lancet_to_quatrefoil_mm, "bore_rail_mm": l.bore_rail_mm, "outer_rail_mm": l.outer_rail_mm,
            "hub_step_to_light_mm": l.hub_step_to_light_mm, "rim_step_to_light_mm": l.rim_step_to_light_mm,
            "quatrefoil_cusp_tip_width_mm": l.quatrefoil_cusp_tip_mm, "crown_arch_to_arch_mm": l.arch_to_arch_mm, "crown_floor_mm": l.arch_floor_mm,
            "field_floor_web_mm": WIDTH_MM - 2.0 * FIELD_DEPTH_MM,
        },
        "gates_passed": passed,
    });
    let report_path = out.join("report.json");
    let mut report: serde_json::Value = std::fs::read(&report_path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| json!({}));
    report["name"] = json!(d.name);
    report["process"] = json!(d.draft.process.label());
    report["alloy"] = json!(ALLOY);
    report["size"] = json!(d.size.display());
    report["bore_mm"] = json!(built.report.inner_diameter_mm);
    report["band"] = json!({"width_mm": WIDTH_MM, "thickness_mm": THICKNESS_MM, "lights": LIGHTS});
    report["grams_silver_925"] = json!(grams);
    report["grams_gold_18k"] = json!(grams_18k);
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
        "  field {} (lost wax: informational), thinnest wall {:.2}; dfm {}; min r {:.3} vs bore {:.3}; {:.1} g silver, {:.1} g 18k",
        field.verdict.label(),
        field.thinnest_wall_mm,
        findings.len(),
        min_r,
        bore,
        grams,
        grams_18k
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
