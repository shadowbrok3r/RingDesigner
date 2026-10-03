//! Officina — Sigil, a quartered seal on the round: factory stock 013 poured bare in Delft, the seal cut at the bench.
//! cargo build --release -p ringdesign-core --example officina_sigil
//! target/release/examples/officina_sigil [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign,
    cad::{Attach, Boolean, Component, ComponentRole, Document, FaceRef, Feature, Operation, PlaneBase, Profile, Stage},
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    field::BorderLayer,
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, RegionRef, Sketch, Workplane},
    stl,
};
use serde::Serialize;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const BORE_MM: f64 = 18.2;
const TABLE_PLANE: u64 = 2;
const SEAL_SKETCH: u64 = 3;
const SEAL_PRISM: u64 = 4;
const CROWN_SKETCH: u64 = 5;
const CROWN_SLAB: u64 = 6;
const SEAL_SUNK: u64 = 7;
/// The heater shield on the table, in the sketch's millimetres (x round the ring, y along the finger):
/// half its width, its chief line, where its straight flanks turn into the two arcs that meet at the
/// point (each struck from the opposite flank, radius the shield's width), and the outline groove's width.
const SHIELD_HALF_MM: f64 = 2.7;
const SHIELD_CHIEF_MM: f64 = 2.75;
const SHIELD_FLANK_MM: f64 = 0.9;
const OUTLINE_MM: f64 = 0.35;
/// Half the raised cross that quarters the shield, and the height of its arm across.
const CROSS_HALF_MM: f64 = 0.15;
const FESS_MM: f64 = -0.3;
const BORDURE_IN_MM: f64 = 4.15;
const BORDURE_OUT_MM: f64 = 4.65;
/// How deep the seal is sunk, measured square into the table's crown wherever it falls.
const SEAL_DEPTH_MM: f64 = 0.42;
/// The cutter's walls: their draft (square, as a graver cuts them; a drafted prism of these regions does
/// not tessellate closed), and how far below the table plane its prism reaches.
const WALL_DRAFT_DEG: f64 = 0.0;
const PRISM_MM: f64 = 2.0;
/// The crown is drawn this far to one side of the table's middle, and the slab runs twice it round the ring.
const SLAB_HALF_MM: f64 = 5.5;
/// The sand master's own mirror seam on the crest line (about 1 micron deep between 0 and 55 degrees)
/// reads as undercut; a bead this size fills it, and nothing a render can see.
const SEAM_WIDTH_MM: f64 = 0.5;
const SEAM_HEIGHT_MM: f64 = 0.004;
/// No bead on the seal's edges: an intaglio strikes its impression from crisp walls, and the shield's
/// 58-degree feet, where the cross meets the base arcs, are too sharp for a rolling ball (it folds there).
const EDGE_BREAK_MM: f64 = 0.0;
/// The satin the cuts are left in, against the polished table.
const SATIN: f64 = 0.66;
/// The sunk seal is left oxidised as well as satin, as Logan's own signets keep their recesses: yellow
/// gold's reflectance darkened, so the struck quarters read as a tincture against the bright metal.
const OXIDISED: [f32; 3] = [0.40, 0.29, 0.12];
/// The stock's source mesh is refined to 0.55 mm facets, which a polished render shows as stepped
/// highlights though they stand within microns of the true surface; its shading normals are diffused
/// over this reach, never across an edge sharper than 20 degrees.
const DIFFUSE_MM: f64 = 0.6;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// Factory stock 013 at its native size as the Delft sand master with its envelope, at an 18.2 mm bore.
fn stock() -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "013").context("stock 013")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, sand_master(preset.load()?)?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = true;
    let (length, width) = (d.shank.head.length_mm, d.profile.width_mm);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.shank.head.length_mm = length;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM)?;
    let chart = SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() };
    d.imported_base.as_mut().unwrap().chart = Some(chart);
    SandProcess::DelftClay.apply(&mut d.draft);
    d.name = "Sigil \u{2014} a quartered seal on the round".into();
    Ok(d)
}

/// The stock with the parting seam dressed: what the pattern maker's file does to the line before ramming.
fn dressed() -> Result<RingDesign> {
    let mut d = stock()?;
    let ctx = d.field_context();
    let seam = BorderLayer { v_mm: ctx.crest_v_mm, width_mm: SEAM_WIDTH_MM, height_mm: SEAM_HEIGHT_MM, mirror: false, ..Default::default() };
    d.layers.layers.push(LayerEntry::new("Parting seam dressed", Layer::Border(seam)));
    d.manufacturing = Some(setup(&d));
    Ok(d)
}

/// Officina's Delft recipe, flask and channels (`workshop_collection.rs`'s `setup()`), parting at z = 0.
fn setup(d: &RingDesign) -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").unwrap().shrink_pct;
    s.recipe.process = CastProcess::SandTwoPart;
    s.recipe.name = "Delft clay starting recipe / Gold 18k".into();
    s.recipe.calibration_note = "Uncalibrated starting allowance. Confirm with the caster's alloy, pattern material, mold, and measured trials.".into();
    s.sample_pitch_mm = 0.10;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 70.;
    s.flask.length_mm = 70.;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0., -11., 0.], end: [0., -22., 0.], diameter_mm: 3.2 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0., -22., 0.], end: [0., -30., 0.], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Pour factory stock 013 bare from the drafted sand master, parting at z = 0. At the bench, cut the seal into the table: the two sunk quarters (first and fourth) and the sunk bordure, each to its flat floor; break the cut edges and polish the raised quarters and the rim.".into();
    let _ = d;
    s
}

fn feature(id: u64, name: &str, operation: Operation, attach: Attach, stage: Stage) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component: Component { role: ComponentRole::Other, attach, stage, ..Default::default() } }
}

fn on_plane(name: &str, plane: u64) -> Sketch {
    let mut s = Sketch { name: name.into(), ..Sketch::default() };
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
    s
}

/// A sketch whose points are shared wherever two curves meet, so every junction is an endpoint.
struct Drawn {
    s: Sketch,
    at: Vec<([f64; 2], u64)>,
    /// Drawn turned half round, so the shield's chief stands toward the top of the face view and its first
    /// quarter to the viewer's left (the sketch's x runs round the ring toward world -x, its y along the finger).
    flip: bool,
}

impl Drawn {
    fn new(s: Sketch) -> Self {
        Self { s, at: Vec::new(), flip: false }
    }
    fn f(&self, xy: [f64; 2]) -> [f64; 2] {
        if self.flip { [-xy[0], -xy[1]] } else { xy }
    }
    fn p(&mut self, xy: [f64; 2]) -> u64 {
        let xy = self.f(xy);
        if let Some((_, id)) = self.at.iter().find(|(q, _)| (q[0] - xy[0]).hypot(q[1] - xy[1]) < 1e-9) {
            return *id;
        }
        let id = self.s.point(xy);
        self.at.push((xy, id));
        id
    }
    fn line(&mut self, a: [f64; 2], b: [f64; 2]) -> u64 {
        let (a, b) = (self.p(a), self.p(b));
        self.s.entity(Geometry::Line { a, b })
    }
    /// Counterclockwise about `c` from `a` to `b`.
    fn arc(&mut self, c: [f64; 2], a: [f64; 2], b: [f64; 2]) -> u64 {
        let center = self.s.point(self.f(c));
        let (start, end) = (self.p(a), self.p(b));
        self.s.entity(Geometry::Arc { center, start, end })
    }
    fn circle(&mut self, r: f64) -> u64 {
        let center = self.s.point([0.0, 0.0]);
        let rim = self.s.point([r, 0.0]);
        self.s.entity(Geometry::Circle { center, rim })
    }
}

/// The seal: a round bordure (two circles, one region with a hole), and inside it a heater shield. The
/// shield's outline is one loop; inside it a second loop runs round the raised cross and the two bright
/// quarters (second and third) it holds, the outline groove's width in from the edge. A loop inside a loop is
/// a hole, so the sunk field (the groove, and the first and fourth quarters it opens into) is one region with
/// the cross as its hole. The two regions the bench sinks are named.
struct Seal {
    sketch: Sketch,
    sunk: Vec<RegionRef>,
}

fn seal() -> Seal {
    let mut d = Drawn::new(on_plane("Seal", TABLE_PLANE));
    d.flip = true;
    let (a, ch, y1, w, yh) = (SHIELD_HALF_MM, SHIELD_CHIEF_MM, SHIELD_FLANK_MM, CROSS_HALF_MM, FESS_MM);
    let (up, down) = (yh + w, yh - w);
    let (cr, cl) = ([-a, y1], [a, y1]);
    let (r, ri) = (2.0 * a, 2.0 * a - OUTLINE_MM);
    let (ai, chi) = (a - OUTLINE_MM, ch - OUTLINE_MM);
    // Each flank arc is struck from the opposite flank: x on the right arc, and on the left, at height y.
    let on_right = |r: f64, y: f64| -a + (r * r - (y - y1).powi(2)).sqrt();
    let on_left = |r: f64, y: f64| a - (r * r - (y - y1).powi(2)).sqrt();
    let foot = y1 - (ri * ri - (a + w).powi(2)).sqrt();
    let point = |r: f64| y1 - (r * r - a * a).sqrt();
    // The shield's edge: chief, flanks, and the two arcs to the point.
    let chief = d.line([-a, ch], [a, ch]);
    d.line([a, ch], [a, y1]);
    d.arc(cr, [0.0, point(r)], [a, y1]);
    d.arc(cl, [-a, y1], [0.0, point(r)]);
    d.line([-a, y1], [-a, ch]);
    // The raised cross and its two bright quarters, one loop: from the top of the cross along the second
    // quarter's chief and flank, down the right arc to the cross's arm, in to the cross, down its foot to the
    // base, round the point and up the third quarter's flank to the arm, and back up the cross.
    d.line([-w, chi], [ai, chi]);
    d.line([ai, chi], [ai, y1]);
    d.arc(cr, [on_right(ri, down), down], [ai, y1]);
    d.line([on_right(ri, down), down], [w, down]);
    d.line([w, down], [w, foot]);
    d.arc(cr, [0.0, point(ri)], [w, foot]);
    d.arc(cl, [on_left(ri, up), up], [0.0, point(ri)]);
    d.line([on_left(ri, up), up], [-w, up]);
    d.line([-w, up], [-w, chi]);
    let outer = d.circle(BORDURE_OUT_MM);
    d.circle(BORDURE_IN_MM);
    let mid = 0.5 * (BORDURE_IN_MM + BORDURE_OUT_MM);
    let sunk = [(outer, [0.0, mid]), (chief, [-0.5 * a, 0.5 * (ch + up)])]
        .map(|(entity, at)| RegionRef { entity, at: d.f(at) })
        .to_vec();
    Seal { sketch: d.s, sunk }
}

/// The table's crown across the finger, read off the bare stock: its height over each station along
/// the finger, averaged round the ring across the seal's own width there.
fn crown(bare: &mesh::Mesh) -> Vec<[f64; 2]> {
    let reach = BORDURE_OUT_MM + 0.2;
    (0..=48)
        .map(|k| {
            let z = -reach + 2.0 * reach * k as f64 / 48.0;
            let half = (BORDURE_OUT_MM * BORDURE_OUT_MM - z * z).max(0.0).sqrt();
            let tops: Vec<f64> = (-4..=4).filter_map(|i| top_at(bare, half * i as f64 / 4.0, z)).collect();
            let y = tops.iter().sum::<f64>() / tops.len().max(1) as f64;
            [(y * 1e4).round() / 1e4, (z * 1e4).round() / 1e4]
        })
        .collect()
}

/// The slab the seal is sunk to: on the crown plane (x out from the finger's axis, y along the finger),
/// everything above the crown let down by the seal's depth.
fn crown_sketch(crown: &[[f64; 2]]) -> Sketch {
    // Square to the ring at the slab's near end: x out from the finger's axis, y along the finger.
    let mut s = Sketch { name: "Crown".into(), ..Sketch::default() };
    s.plane = Workplane { origin: [-SLAB_HALF_MM, 0.0, 0.0], x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
    let roof = crown.iter().map(|p| p[0]).fold(f64::MIN, f64::max) + 1.5;
    let mut ids: Vec<u64> = crown.iter().map(|&[y, z]| s.point([y - SEAL_DEPTH_MM, z])).collect();
    let (first, last) = (crown[0][1], crown[crown.len() - 1][1]);
    ids.push(s.point([roof, last]));
    ids.push(s.point([roof, first]));
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

fn document(crown: &[[f64; 2]]) -> Result<Document> {
    let mut doc = Document::default();
    doc.append(feature(1, "Stock 013", Operation::Band, Attach::Separate, Stage::Cast))?;
    doc.append(feature(TABLE_PLANE, "Table plane", Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: 0.0 }, Attach::Separate, Stage::Cast))?;
    let seal = seal();
    doc.append(feature(SEAL_SKETCH, "Seal", Operation::Sketch { sketch: seal.sketch }, Attach::Separate, Stage::Cast))?;
    let regions = seal.sunk;
    doc.append(feature(
        SEAL_PRISM,
        "Seal cutter",
        Operation::Extrude { sketch: Profile::Regions { feature: SEAL_SKETCH, regions }, height_mm: -PRISM_MM, draft_deg: WALL_DRAFT_DEG },
        Attach::Separate,
        Stage::Bench,
    ))?;
    doc.append(feature(CROWN_SKETCH, "Crown", Operation::Sketch { sketch: crown_sketch(crown) }, Attach::Separate, Stage::Cast))?;
    // The slab is our own sweep, a mesh, so the intersection runs through csg: the crown carried round
    // the ring along a straight path from the crown's own plane, square to it, the slab's whole width.
    let mut path = Sketch { name: "Round the ring".into(), ..Sketch::default() };
    path.plane = Workplane { origin: [0.0, 12.0, 0.0], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
    let (a, b) = (path.point([-SLAB_HALF_MM, 0.0]), path.point([SLAB_HALF_MM, 0.0]));
    path.entity(Geometry::Line { a, b });
    doc.append(feature(
        CROWN_SLAB,
        "Crown let down",
        Operation::Twist { sketch: Profile::Feature { feature: CROWN_SKETCH }, path, degrees: 0.0, end_scale: 1.0 },
        Attach::Separate,
        Stage::Bench,
    ))?;
    let mut sunk = feature(SEAL_SUNK, "Seal sunk", Operation::Boolean { a: SEAL_PRISM, b: CROWN_SLAB, kind: Boolean::Intersect }, Attach::Cut, Stage::Bench);
    sunk.component.blend_mm = EDGE_BREAK_MM;
    sunk.component.bench_notes = "Cut at the bench after the pour, square into the table's crown, its walls crisp for a clean impression and its floor and walls left satin; the pattern is the bare stock.".into();
    doc.append(sunk)?;
    Ok(doc)
}

fn design() -> Result<RingDesign> {
    let mut d = dressed()?;
    let bare = mesh::try_build(&d, &AlphaLibrary::default(), draft_params())?;
    d.cad = Some(document(&crown(&bare.mesh))?);
    Ok(d)
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

#[derive(Serialize, Default)]
struct Gates {
    watertight: bool,
    degenerate_faces: usize,
    ring_self_crossings: usize,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    features: Vec<(u64, String, String)>,
    features_ok: bool,
    bore_radius_mm: f64,
    min_vertex_radius_mm: f64,
    bore_clear: bool,
    field_verdict_192x128: String,
    field_verdict_256x128: String,
    field_undercut_percent: f64,
    field_worst_draft_deg: f64,
    field_notes: Vec<String>,
    release_0100: (usize, usize, String),
    release_0075: (usize, usize, String),
    thinnest_wall_mm: f64,
    dfm_findings: Vec<String>,
    cut_lands_08: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    triangles: usize,
    within_two_million: bool,
    cold_reload_identical: Option<bool>,
    pattern_watertight: Option<bool>,
    pattern_degenerate_faces: Option<usize>,
    pattern_self_crossings: Option<usize>,
    pattern_triangles: Option<usize>,
    passed: bool,
}

#[derive(Serialize)]
struct Report {
    name: String,
    process: String,
    size: String,
    bore_mm: f64,
    build: [usize; 2],
    build_s: f64,
    seal: serde_json::Value,
    gates: Gates,
    draft: Option<serde_json::Value>,
    template_gate: Option<serde_json::Value>,
    design_bytes: u64,
    grams_18k: f64,
    cad_features: Vec<String>,
    notes: Vec<String>,
}

/// What the report's reader should know that the numbers do not say.
fn notes() -> Vec<String> {
    vec![
        format!("Process: Delft sand on the drafted sand master of stock 013 at its native face, bore {BORE_MM} mm. Bare, that master fields Castable with care (0.07% at -0.8 deg at 192 x 128): a mirror seam about 1 micron deep on the crest line between 0 and 55 deg. A {SEAM_WIDTH_MM} x {SEAM_HEIGHT_MM} mm round bead on the crest line ('Parting seam dressed') fills it; it is invisible in every render and the field reads Castable."),
        "Every cut is Stage::Bench, so the casting pattern is the bare stock; the seal is in the finished ring and the renders, not in the pour.".into(),
        format!("The table is crowned across the finger (0.29 mm down at 2 mm, 0.68 mm at 4 mm) and flat round the ring, so the seal is sunk square into the crown: its cutter is the seal's regions extruded ({SEAL_PRISM}) and intersected with a slab whose underside is the crown let down {SEAL_DEPTH_MM} mm ({CROWN_SLAB}, a sweep of the crown sketch round the ring). Depths are in `seal`."),
        "Finish: the sunk seal (floors and walls) is left oxidised satin, as Logan's own signets keep their recesses; the table, cross and raised quarters are polished. The renders split the faces by geometry: on the let-down floor, or a wall standing on it inside the bordure.".into(),
        format!("Render shading only: the stock's source mesh (and so finished-metal.stl) carries 0.55 mm facets, which stand within microns of the true surface but read as stepped highlights on a polished shank; on the shank the polished metal's shading normals are diffused over {DIFFUSE_MM} mm, never across an edge sharper than 20 deg, blending to the build's own normals on the table, and the table round the seal shades from its own faces so every cut edge is one line. Geometry, STL and gates are untouched; mesh.json's minimum angle is the stock mesh's and the csg seams' slivers, which the gates allow (watertight, 0 degenerate faces). The README says so."),
        "Edges: the seal's walls are square and crisp, as a graver leaves an intaglio for a clean impression; a rolling-ball bead folds at the shield's acute corners (its point, and the cross's feet on the base arcs), and the kernel cannot tessellate a drafted prism of these regions closed.".into(),
        "Lesson: seven features, as Logan decided. The crown is a sketch on a work plane of its own (no Plane feature), square to the ring at the slab's near end.".into(),
    ]
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, params: BuildParams, verify: Option<&Path>, pattern: bool) -> Result<Gates> {
    let mut g = Gates::default();
    let v = &built.report.validation;
    g.watertight = v.watertight;
    g.degenerate_faces = built.report.quality.degenerate_faces;
    g.ring_self_crossings = csg::self_crossings(&solid_of(&built.mesh));
    g.made_parts = built
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
        .collect();
    g.solids_notes = built.solids.notes.clone();
    g.parts_notes = built.parts.notes.clone();
    if let Some(e) = &built.parts.evaluated {
        g.features = e.features.iter().map(|r| (r.id, r.name.clone(), format!("{:?}", r.status))).collect();
        g.features_ok = e.features.iter().all(|r| r.status.is_ok());
    }
    g.bore_radius_mm = d.inner_radius_mm();
    g.min_vertex_radius_mm = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    g.bore_clear = g.min_vertex_radius_mm >= g.bore_radius_mm - 0.01;
    let field = castability::judged_field_report(d, lib, &d.draft, 192, 128, Some(built));
    let field256 = castability::judged_field_report(d, lib, &d.draft, 256, 128, Some(built));
    g.field_verdict_192x128 = field.verdict.label().into();
    g.field_verdict_256x128 = field256.verdict.label().into();
    g.field_undercut_percent = field.undercut_fraction() * 100.0;
    g.field_worst_draft_deg = field.worst_draft_deg;
    g.field_notes = field.notes.clone();
    g.thinnest_wall_mm = field.thinnest_wall_mm;
    let setup = d.manufacturing.clone().context("setup")?;
    let inspection = mf::inspect(d, lib, &setup, params)?;
    g.release_0100 = (inspection.release.obstructions.len(), inspection.release.unresolved_rays, format!("{:?}", inspection.release.status));
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    g.release_0075 = (release.obstructions.len(), release.unresolved_rays, format!("{:?}", release.status));
    g.dfm_findings = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    g.cut_lands_08 = dfm::cut_lands(d, built, 0.8).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    g.stones_reported = stones.as_ref().map_or(0, |s| s.stone_count);
    g.stones_previewed = ringdesign_core::gems::built_meshes(d, lib, built).len();
    g.triangles = built.mesh.faces.len();
    g.within_two_million = g.triangles <= 2_000_000;
    if let Some(path) = verify {
        let saved = library::load_design(path)?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        g.cold_reload_identical = Some(rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals);
    }
    if pattern {
        let p = mesh::try_build_pattern(d, lib, params)?;
        g.pattern_watertight = Some(p.mesh.validate().watertight);
        g.pattern_degenerate_faces = Some(p.mesh.quality().degenerate_faces);
        g.pattern_self_crossings = Some(csg::self_crossings(&solid_of(&p.mesh)));
        g.pattern_triangles = Some(p.mesh.faces.len());
    }
    g.passed = g.watertight
        && g.degenerate_faces == 0
        && g.ring_self_crossings == 0
        && g.made_parts.iter().all(|(_, n)| *n == 0)
        && g.solids_notes.is_empty()
        && g.parts_notes.is_empty()
        && g.features_ok
        && g.bore_clear
        && g.field_verdict_192x128 == castability::Verdict::Castable.label()
        && g.field_verdict_256x128 == castability::Verdict::Castable.label()
        && g.release_0100.0 == 0
        && g.release_0100.1 == 0
        && g.release_0075.0 == 0
        && g.release_0075.1 == 0
        && g.dfm_findings.is_empty()
        && g.stones_reported as usize == g.stones_previewed
        && g.within_two_million
        && g.cold_reload_identical != Some(false)
        && g.pattern_watertight != Some(false)
        && g.pattern_degenerate_faces.is_none_or(|n| n == 0)
        && g.pattern_self_crossings.is_none_or(|n| n == 0);
    Ok(g)
}

// --- Pictures ------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.55, 0.95),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: usize, h: usize, rgb: [u8; 3]) -> Self {
        Self { w, h, px: rgb.repeat(w * h) }
    }
    fn blit(&mut self, img: &[u8], iw: usize, ih: usize, x0: usize, y0: usize) {
        for y in 0..ih.min(self.h.saturating_sub(y0)) {
            let n = iw.min(self.w.saturating_sub(x0)) * 3;
            let at = ((y0 + y) * self.w + x0) * 3;
            self.px[at..at + n].copy_from_slice(&img[y * iw * 3..y * iw * 3 + n]);
        }
    }
    fn text(&mut self, s: &str, x0: usize, baseline: usize, size: f32, rgb: [u8; 3]) {
        static FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
        let font = FONT.get_or_init(|| fontdue::Font::from_bytes(&include_bytes!("../../../assets/fonts/EBGaramond.ttf")[..], fontdue::FontSettings::default()).expect("font"));
        let mut pen = x0 as f32;
        for ch in s.chars() {
            let (m, bitmap) = font.rasterize(ch, size);
            let top = baseline as i64 - m.height as i64 - m.ymin as i64;
            for y in 0..m.height {
                for x in 0..m.width {
                    let a = bitmap[y * m.width + x] as f32 / 255.0;
                    let (px, py) = (pen as i64 + m.xmin as i64 + x as i64, top + y as i64);
                    if a > 0.0 && px >= 0 && py >= 0 && (px as usize) < self.w && (py as usize) < self.h {
                        let at = (py as usize * self.w + px as usize) * 3;
                        for k in 0..3 {
                            self.px[at + k] = (self.px[at + k] as f32 * (1.0 - a) + rgb[k] as f32 * a) as u8;
                        }
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

fn frame(parts: &[render::Part], yaw: f64, pitch: f64, edge: usize) -> Vec<u8> {
    render::render_parts_ss(parts, yaw, pitch, edge, edge, 3)
}

/// The crown polyline the seal's slab was drawn from, as (height, along the finger) pairs.
fn crown_of(d: &RingDesign) -> Vec<[f64; 2]> {
    let doc = d.cad.as_ref().unwrap();
    let Some(Operation::Sketch { sketch }) = doc.features.iter().find(|f| f.id == CROWN_SKETCH).map(|f| &f.operation) else { return Vec::new() };
    let mut pts: Vec<[f64; 2]> = sketch.points.iter().map(|p| [p.xy[0] + SEAL_DEPTH_MM, p.xy[1]]).collect();
    pts.truncate(pts.len().saturating_sub(2));
    pts
}

/// The table's height at `z` along the finger, read off the crown polyline.
fn crown_at(crown: &[[f64; 2]], z: f64) -> f64 {
    for w in crown.windows(2) {
        if (w[0][1]..=w[1][1]).contains(&z) {
            let t = (z - w[0][1]) / (w[1][1] - w[0][1]).max(1e-12);
            return w[0][0] + (w[1][0] - w[0][0]) * t;
        }
    }
    f64::NAN
}

/// The built ring split by finish: the polished metal, what the bench cut (left satin), and the tools a
/// rolled-back step still shows. A face is the cut's when it lies on the floor the crown let down, or is a
/// wall standing on it inside the bordure; the edge-break bead above each wall stays polished, so the finish
/// changes on the geometry's own line.
fn finishes(d: &RingDesign, built: &mesh::BuildResult) -> (mesh::Mesh, mesh::Mesh, mesh::Mesh) {
    let m = &built.mesh;
    let crown = crown_of(d);
    let tool = |f: &[u32; 3]| f.iter().all(|&v| m.origin.get(v as usize).and_then(|o| built.parts.feature_of(*o)).is_some_and(|id| id == SEAL_PRISM || id == CROWN_SLAB));
    let cut = |f: &[u32; 3]| {
        let [a, b, c] = f.map(|i| m.vertices[i as usize]);
        let centre = [(a.0 + b.0 + c.0) as f64 / 3.0, (a.1 + b.1 + c.1) as f64 / 3.0, (a.2 + b.2 + c.2) as f64 / 3.0];
        if centre[0].hypot(centre[2]) > BORDURE_OUT_MM + 0.05 || centre[1] < 5.0 {
            return false;
        }
        let floor = crown_at(&crown, centre[2]) - SEAL_DEPTH_MM;
        if !floor.is_finite() || centre[1] < floor - 0.02 {
            return false;
        }
        let (u, v) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) as f64).sqrt().max(1e-30);
        let ny = n[1] as f64 / l;
        let on_floor = ny > 0.95 && (centre[1] - floor).abs() < 0.008;
        let wall = ny.abs() < 0.5 && centre[1] < floor + SEAL_DEPTH_MM + 0.3;
        on_floor || wall
    };
    let split = |which: u8| {
        let mut out = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..Default::default() };
        let mut cursor = 0usize;
        for (i, f) in m.faces.iter().enumerate() {
            let class = if tool(f) { 2 } else if cut(f) { 1 } else { 0 };
            if class == which {
                while m.corner_normals.get(cursor).is_some_and(|c| (c.0 as usize) < i) {
                    cursor += 1;
                }
                if let Some((fi, n)) = m.corner_normals.get(cursor)
                    && *fi as usize == i
                {
                    out.corner_normals.push((out.faces.len() as u32, *n));
                }
                out.faces.push(*f);
            }
        }
        out
    };
    let (mut polished, mut satin, tools) = (split(0), split(1), split(2));
    creased(&mut satin, 30.0);
    // The build's vertex normals on a cut's top edge lean halfway into the wall, so inside the bordure's
    // outer rim the polished metal reads its own, from its own faces; beyond it, where the stock's table
    // edge zigzags across the build grid, the build's smooth normals stay. Then both are diffused.
    let own = own_normals(&polished);
    for (i, n) in polished.normals.iter_mut().enumerate() {
        let p = polished.vertices[i];
        if p.1 > 11.0 && (p.0 as f64).hypot(p.2 as f64) < BORDURE_OUT_MM + 0.1 {
            *n = own[i];
        }
    }
    // Diffused on the shank only: on the table and its edge the build's normals already shade clean, and
    // diffusing across the edge's zigzag would serrate it. The two meet in a blend 0.4 mm wide just past the table.
    let soft = diffused(&polished, 20.0, DIFFUSE_MM, (2.0 * PI * BORE_MM / 2.0) / 1536.0);
    for (i, n) in polished.normals.iter_mut().enumerate() {
        let p = polished.vertices[i];
        let r = (p.0 as f64).hypot(p.2 as f64);
        let t = if p.1 < 9.5 { 1.0 } else { ((r - 5.05) / 0.4).clamp(0.0, 1.0) };
        let t = (t * t * (3.0 - 2.0 * t)) as f32;
        let m = [n.0 + (soft[i].0 - n.0) * t, n.1 + (soft[i].1 - n.1) * t, n.2 + (soft[i].2 - n.2) * t];
        let l = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt().max(1e-20);
        *n = mesh::Vec3(m[0] / l, m[1] / l, m[2] / l);
    }
    (polished, satin, tools)
}

/// Corner normals for every face of `m` from its own geometry: each corner averages the faces round
/// its vertex that turn less than `deg` from this one, so planar floors shade flat and walls keep their edge.
fn creased(m: &mut mesh::Mesh, deg: f64) {
    creased_where(m, deg, |_| true);
}

/// [`creased`] for the faces `keep` picks, every other face keeping the corner normals it had.
fn creased_where(m: &mut mesh::Mesh, deg: f64, keep: impl Fn(&[mesh::Vec3; 3]) -> bool) {
    let p = |i: u32| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    };
    let normals: Vec<[f64; 3]> = m
        .faces
        .iter()
        .map(|f| {
            let (a, b, c) = (p(f[0]), p(f[1]), p(f[2]));
            let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
        })
        .collect();
    let unit = |n: [f64; 3]| {
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
        n.map(|x| x / l)
    };
    let picked: Vec<bool> = m.faces.iter().map(|f| keep(&f.map(|i| m.vertices[i as usize]))).collect();
    let mut around: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
    for (i, f) in m.faces.iter().enumerate().filter(|(i, _)| picked[*i]) {
        for &v in f {
            around.entry(v).or_default().push(i);
        }
    }
    // A picked face's corner looks only at picked faces: pick a region a little wider than the creases it is for.
    let cos = deg.to_radians().cos();
    let old: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    m.corner_normals = m
        .faces
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            if !picked[i] {
                return old.get(&(i as u32)).map(|c| (i as u32, *c));
            }
            let own = unit(normals[i]);
            let corner = |v: u32| {
                let mut sum = [0.0; 3];
                for &j in &around[&v] {
                    let n = normals[j];
                    let u = unit(n);
                    if u[0] * own[0] + u[1] * own[1] + u[2] * own[2] >= cos {
                        for k in 0..3 {
                            sum[k] += n[k];
                        }
                    }
                }
                let n = unit(sum);
                mesh::Vec3(n[0] as f32, n[1] as f32, n[2] as f32)
            };
            Some((i as u32, f.map(corner)))
        })
        .collect();
}

/// Area-weighted vertex normals of `m` from its own faces.
fn own_normals(m: &mesh::Mesh) -> Vec<mesh::Vec3> {
    let mut sum = vec![[0.0f64; 3]; m.vertices.len()];
    for f in &m.faces {
        let [a, b, c] = f.map(|i| m.vertices[i as usize]);
        let (u, v) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        for &i in f {
            for k in 0..3 {
                sum[i as usize][k] += n[k] as f64;
            }
        }
    }
    sum.iter()
        .zip(&m.normals)
        .map(|(n, old)| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if l > 1e-30 { mesh::Vec3((n[0] / l) as f32, (n[1] / l) as f32, (n[2] / l) as f32) } else { *old }
        })
        .collect()
}

/// Vertex normals of `m` diffused over about `reach_mm` (the mesh's vertices stand about `step_mm` apart):
/// each pass averages a vertex's normal with its neighbours', never across an edge turning more than `deg`.
fn diffused(m: &mesh::Mesh, deg: f64, reach_mm: f64, step_mm: f64) -> Vec<mesh::Vec3> {
    let n = m.vertices.len();
    let mut normals: Vec<[f32; 3]> = m.normals.iter().map(|v| [v.0, v.1, v.2]).collect();
    if normals.len() != n {
        return m.normals.clone();
    }
    let cos = deg.to_radians().cos() as f32;
    let mut edges: Vec<(u32, u32)> = Vec::with_capacity(m.faces.len() * 3);
    for f in &m.faces {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            let (a, b) = (a.min(b), a.max(b));
            let (p, q) = (normals[a as usize], normals[b as usize]);
            if p[0] * q[0] + p[1] * q[1] + p[2] * q[2] >= cos {
                edges.push((a, b));
            }
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let passes = ((reach_mm / step_mm).powi(2)).ceil().clamp(1.0, 400.0) as usize;
    let mut next = normals.clone();
    for _ in 0..passes {
        next.copy_from_slice(&normals);
        for &(a, b) in &edges {
            let (p, q) = (normals[a as usize], normals[b as usize]);
            for k in 0..3 {
                next[a as usize][k] += q[k];
                next[b as usize][k] += p[k];
            }
        }
        for v in &mut next {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-20);
            *v = v.map(|x| x / l);
        }
        std::mem::swap(&mut normals, &mut next);
    }
    normals.into_iter().map(|v| mesh::Vec3(v[0], v[1], v[2])).collect()
}

/// The ring's parts as the bench finishes them: polished metal, satin cuts, and any stones.
fn dressed_parts<'a>(polished: &'a mesh::Mesh, satin: &'a mesh::Mesh, stones: &'a [(mesh::Mesh, [f32; 3])]) -> Vec<render::Part<'a>> {
    let mut parts = vec![render::Part::metal(polished, render::GOLD)];

    if !satin.faces.is_empty() {
        let mut s = render::Part::metal(satin, OXIDISED);
        s.roughness = SATIN;
        parts.push(s);
    }
    parts.extend(stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    parts
}

/// A thin square tube along `pts`, for drawing a sketch or a plane's edge into a timeline cell.
fn tube(pts: &[[f64; 3]], normal: [f64; 3], half: f64, out: &mut mesh::Mesh) {
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let t = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        if l < 1e-9 {
            continue;
        }
        let side = cross(t, normal).map(|v| v / l);
        let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        let base = out.vertices.len() as u32;
        for p in [a, b] {
            for (u, v) in corners {
                let q: [f64; 3] = std::array::from_fn(|k| p[k] + side[k] * half * u + normal[k] * half * v);
                out.vertices.push(mesh::Vec3(q[0] as f32, q[1] as f32, q[2] as f32));
                let n: [f64; 3] = std::array::from_fn(|k| side[k] * u + normal[k] * v);
                out.normals.push(mesh::Vec3(n[0] as f32, n[1] as f32, n[2] as f32));
            }
        }
        for k in 0..4u32 {
            let (i, j) = (k, (k + 1) % 4);
            out.faces.push([base + i, base + j, base + 4 + j]);
            out.faces.push([base + i, base + 4 + j, base + 4 + i]);
        }
    }
}

/// A sketch's curves laid on the work plane, as polylines in the world.
fn sketch_lines(s: &Sketch, plane: &ringdesign_core::cad::WorkPlane) -> Vec<Vec<[f64; 3]>> {
    let at = |xy: [f64; 2]| -> [f64; 3] { std::array::from_fn(|k| plane.origin[k] + plane.x[k] * xy[0] + plane.y[k] * xy[1] + plane.normal[k] * 0.03) };
    let p = |id| s.at(id).unwrap_or([0.0; 2]);
    let arc = |c: [f64; 2], a0: f64, a1: f64, r: f64| (0..=48).map(|i| a0 + (a1 - a0) * i as f64 / 48.0).map(|a| at([c[0] + r * a.cos(), c[1] + r * a.sin()])).collect::<Vec<_>>();
    s.entities
        .iter()
        .map(|e| match &e.geometry {
            Geometry::Line { a, b } => vec![at(p(*a)), at(p(*b))],
            Geometry::Polyline { points, closed } => points.iter().chain(points.first().filter(|_| *closed)).map(|id| at(p(*id))).collect(),
            Geometry::Circle { center, rim } => {
                let (c, r) = (p(*center), p(*rim));
                arc(c, 0.0, 2.0 * PI, (r[0] - c[0]).hypot(r[1] - c[1]))
            }
            Geometry::Arc { center, start, end } => {
                let (c, a, b) = (p(*center), p(*start), p(*end));
                let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
                let mut a1 = (b[1] - c[1]).atan2(b[0] - c[0]);
                if a1 <= a0 {
                    a1 += 2.0 * PI;
                }
                arc(c, a0, a1, (a[0] - c[0]).hypot(a[1] - c[1]))
            }
            _ => vec![],
        })
        .collect()
}

/// The highest point of `m` straight above (x, z), cast down the y axis.
fn top_at(m: &mesh::Mesh, x: f64, z: f64) -> Option<f64> {
    let mut best: Option<f64> = None;
    for f in &m.faces {
        let [a, b, c] = f.map(|i| m.vertices[i as usize]);
        let (ax, az, bx, bz, cx, cz) = (a.0 as f64, a.2 as f64, b.0 as f64, b.2 as f64, c.0 as f64, c.2 as f64);
        if x < ax.min(bx).min(cx) || x > ax.max(bx).max(cx) || z < az.min(bz).min(cz) || z > az.max(bz).max(cz) {
            continue;
        }
        let det = (bz - cz) * (ax - cx) + (cx - bx) * (az - cz);
        if det.abs() < 1e-14 {
            continue;
        }
        let l1 = ((bz - cz) * (x - cx) + (cx - bx) * (z - cz)) / det;
        let l2 = ((cz - az) * (x - cx) + (ax - cx) * (z - cz)) / det;
        let l3 = 1.0 - l1 - l2;
        if l1 < -1e-9 || l2 < -1e-9 || l3 < -1e-9 {
            continue;
        }
        let y = l1 * a.1 as f64 + l2 * b.1 as f64 + l3 * c.1 as f64;
        if y > 0.0 {
            best = Some(best.map_or(y, |v: f64| v.max(y)));
        }
    }
    best
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize, draft: bool) -> Result<()> {
    let stones = ringdesign_core::gems::built_meshes(d, lib, built);
    let (polished, satin, _) = finishes(d, built);
    let parts = dressed_parts(&polished, &satin, &stones);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // No stones: the close-up is the seal itself, straight on and raking.
    // Framed on a box round the table, drawing the whole ring, so no cropped edge shows.
    let mut framing = mesh::Mesh::default();
    for k in 0..8 {
        let (x, y, z) = (if k & 1 == 0 { -6.5 } else { 6.5 }, if k & 2 == 0 { 9.0 } else { 13.0 }, if k & 4 == 0 { -6.5 } else { 6.5 });
        framing.vertices.push(mesh::Vec3(x, y, z));
        framing.normals.push(mesh::Vec3(0.0, 1.0, 0.0));
    }
    let mut close = vec![render::Part::metal(&framing, render::GOLD)];
    close.extend(dressed_parts(&polished, &satin, &[]));
    if std::env::var_os("SIGIL_DEBUG").is_some() {
        let plain = [render::Part::metal(&framing, render::GOLD), render::Part::metal(&built.mesh, render::GOLD)];
        render::write_png_parts("/tmp/claude-0/dbg-plain.png", &plain, 0.25, 1.25, edge)?;
        let only = [render::Part::metal(&framing, render::GOLD), render::Part::metal(&satin, OXIDISED)];
        render::write_png_parts("/tmp/claude-0/dbg-satin.png", &only, 0.25, 1.25, edge)?;
    }
    render::write_png_parts(out.join("stones.png"), &close, 0.25, 1.25, edge)?;
    // The 300 px read and its contact sheet.
    let mut sheet = Canvas::new(300 * 3, 300 * 2 + 0, [255, 255, 255]);
    for (k, (name, yaw, pitch)) in VIEWS.iter().enumerate() {
        let img = frame(&parts, *yaw, *pitch, 300);
        if matches!(*name, "hero" | "face") {
            image::save_buffer(out.join(format!("{name}-300.png")), &img, 300, 300, image::ColorType::Rgb8)?;
        }
        sheet.blit(&img, 300, 300, (k % 3) * 300, (k / 3) * 300);
    }
    sheet.save(&out.join("contact-300.png"))?;
    // Bare stock beside the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.cad = None;
    let b = mesh::try_build(&bare, lib, if draft { draft_params() } else { BuildParams { theta_steps: 1024, profile_steps: 384, ..Default::default() } })?;
    let mut bare_mesh = b.mesh;
    bare_mesh.normals = diffused(&bare_mesh, 20.0, DIFFUSE_MM, (2.0 * PI * BORE_MM / 2.0) / 1536.0);
    let left = frame(&[render::Part::metal(&bare_mesh, render::GOLD)], 0.55, 0.95, edge);
    let right = frame(&parts, 0.55, 0.95, edge);
    let mut pair = Canvas::new(edge * 2, edge, [255, 255, 255]);
    pair.blit(&left, edge, edge, 0, 0);
    pair.blit(&right, edge, edge, edge, 0);
    pair.save(&out.join("bare-vs-finished.png"))?;
    Ok(())
}

/// The ring after each feature (`Document::through`), one labelled cell per step, at the hero's angle.
/// The camera every timeline frame shares: from the side the crown is drawn on, the table turned toward it.
const TIMELINE_VIEW: (f64, f64) = (-0.7, 1.15);
/// How much of a tool body a timeline frame shows over the ring it will cut.
const TOOL_OPACITY: f64 = 0.35;

/// The ring after each feature (`Document::through`), one labelled cell per step, all from one camera: the
/// plane or sketch a step lays down drawn over the ring, and any tool body standing at a third of its tone.
fn timeline(out: &Path, d: &RingDesign, lib: &AlphaLibrary) -> Result<()> {
    let doc = d.cad.as_ref().unwrap();
    let ids: Vec<(u64, String)> = doc.features.iter().map(|f| (f.id, f.name.clone())).collect();
    let sketch_of = |id: u64| doc.features.iter().find(|f| f.id == id).and_then(|f| match &f.operation {
        Operation::Sketch { sketch } => Some(sketch.clone()),
        _ => None,
    });
    let cell = 400;
    let label = 54;
    let cols = 4;
    let rows = ids.len().div_ceil(cols);
    let mut sheet = Canvas::new(cols * cell, rows * (cell + label), [255, 255, 255]);
    for (k, (id, name)) in ids.iter().enumerate() {
        let mut step = d.clone();
        step.cad.as_mut().unwrap().through = Some(*id);
        let built = mesh::try_build(&step, lib, BuildParams { theta_steps: 512, profile_steps: 256, ..Default::default() })?;
        let (polished, satin, tools) = finishes(d, &built);
        let planes = built.parts.evaluated.as_ref().map(|e| e.planes.clone()).unwrap_or_default();
        let table = planes.iter().find(|p| p.id == TABLE_PLANE).copied();
        // The frame each step draws: the plane it lays down or draws on (origin, x, y, normal, extent) and the sketch on it.
        type Frame = ([f64; 3], [f64; 3], [f64; 3], [f64; 3], [f64; 4]);
        let on_table = table.map(|p| (p.origin, p.x, p.y, p.normal, [-5.4, 5.4, -5.4, 5.4]));
        let crown = sketch_of(CROWN_SKETCH).map(|s| {
            let (x, y) = (s.plane.x, s.plane.y);
            let n = [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
            (s.plane.origin, x, y, n, [10.8, 14.4, -5.2, 5.2])
        });
        let (plane, sketch): (Option<Frame>, Option<u64>) = match *id {
            TABLE_PLANE => (on_table, None),
            SEAL_SKETCH | SEAL_PRISM => (on_table, Some(SEAL_SKETCH)),
            CROWN_SKETCH | CROWN_SLAB => (crown, Some(CROWN_SKETCH)),
            _ => (None, None),
        };
        let mut guide = mesh::Mesh::default();
        if let Some((o, x, y, n, [u0, u1, v0, v1])) = plane {
            let at = |u: f64, v: f64| -> [f64; 3] { std::array::from_fn(|k| o[k] + x[k] * u + y[k] * v + n[k] * 0.03) };
            tube(&[at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1), at(u0, v0)], n, 0.05, &mut guide);
            if let Some(sketch) = sketch.and_then(sketch_of) {
                let wp = ringdesign_core::cad::WorkPlane { id: 0, origin: o, x, y, normal: n };
                for line in sketch_lines(&sketch, &wp) {
                    tube(&line, n, 0.045, &mut guide);
                }
            }
        }
        let mut parts = dressed_parts(&polished, &satin, &[]);
        let mut g = render::Part::metal(&guide, [0.30, 0.72, 1.0]);
        g.studio = false;
        if !guide.faces.is_empty() {
            parts.push(g);
        }
        let (yaw, pitch) = TIMELINE_VIEW;
        let mut img = frame(&parts, yaw, pitch, cell);
        if !tools.faces.is_empty() {
            let mut t = render::Part::metal(&tools, [0.45, 0.62, 0.95]);
            t.studio = false;
            parts.push(t);
            let with = frame(&parts, yaw, pitch, cell);
            for (a, b) in img.iter_mut().zip(&with) {
                *a = (*a as f64 * (1.0 - TOOL_OPACITY) + *b as f64 * TOOL_OPACITY).round() as u8;
            }
        }
        let (x, y) = ((k % cols) * cell, (k / cols) * (cell + label));
        sheet.blit(&img, cell, cell, x, y + label);
        sheet.text(&format!("{} \u{00b7} {name}", k + 1), x + 14, y + 38, 30.0, [40, 34, 26]);
    }
    sheet.save(&out.join("timeline.png"))?;
    Ok(())
}

/// Depth of the seal at named points, read off the built mesh straight down from the bare stock. Points
/// are named as the shield is drawn (x to the viewer's right, y to its chief); the sketch is drawn turned
/// half round, and its x runs round the ring toward world -x at the table.
fn seal_depths(bare: &mesh::Mesh, built: &mesh::Mesh) -> serde_json::Value {
    let depth = |dx: f64, dy: f64| match (top_at(bare, dx, -dy), top_at(built, dx, -dy)) {
        (Some(a), Some(b)) => ((a - b) * 1e3).round() / 1e3,
        _ => f64::NAN,
    };
    let mid = 0.5 * (BORDURE_IN_MM + BORDURE_OUT_MM);
    let groove = SHIELD_CHIEF_MM - 0.5 * OUTLINE_MM;
    let flank = SHIELD_HALF_MM - 0.5 * OUTLINE_MM;
    serde_json::json!({
        "first_quarter_mm": [depth(-1.2, 1.2), depth(-0.8, 2.4), depth(-2.3, 0.4), depth(-0.5, 0.3)],
        "fourth_quarter_mm": [depth(1.2, -1.2), depth(0.4, -3.0), depth(1.9, -0.8), depth(0.4, -0.7)],
        "raised_quarters_mm": [depth(1.2, 1.2), depth(-1.2, -1.2)],
        "grooves_mm": { "second_chief": depth(1.5, groove), "second_flank": depth(flank, 1.6), "third_flank": depth(-2.0, -1.5) },
        "bordure_mm": { "along_finger": [depth(0.0, mid), depth(0.0, -mid)], "round_ring": [depth(mid, 0.0), depth(-mid, 0.0)], "diagonal": depth(mid * 0.7071, mid * 0.7071) },
        "background_mm": depth(0.0, 3.55),
        "rim_mm": depth(BORDURE_OUT_MM + 0.15, 0.0),
    })
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina/sigil"));
    std::fs::create_dir_all(&out)?;
    let lib = AlphaLibrary::default();
    let d = design()?;
    let params = if draft { draft_params() } else { export_params() };
    println!("Sigil at {} x {}", params.theta_steps, params.profile_steps);
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
    let g = gates(&d, &lib, &built, params, verify.then(|| out.join("design.ring.json")).as_deref(), !draft)?;
    let mut bare = d.clone();
    bare.cad = None;
    let bare_built = mesh::try_build(&bare, &lib, params)?;
    let seal = seal_depths(&bare_built.mesh, &built.mesh);
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let previous: Option<serde_json::Value> = std::fs::read(out.join("report.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let carry = |key: &str| previous.as_ref().and_then(|p| p.get(key)).cloned().filter(|v| !v.is_null());
    let mut report = Report {
        name: d.name.clone(),
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: built.report.inner_diameter_mm,
        build: [params.theta_steps, params.profile_steps],
        build_s,
        seal,
        gates: g,
        draft: None,
        template_gate: carry("template_gate"),
        design_bytes,
        grams_18k: grams,
        cad_features: d.cad.as_ref().unwrap().features.iter().map(|f| format!("#{} {} ({})", f.id, f.name, f.operation.label())).collect(),
        notes: notes(),
    };
    // The export run keeps the draft run's gates in its `draft` block.
    if draft {
        report.draft = None;
    } else {
        report.draft = carry("draft").or_else(|| previous.as_ref().filter(|p| p.get("build") == Some(&serde_json::json!([768, 320]))).and_then(|p| p.get("gates").cloned()));
    }
    let json = serde_json::to_value(&report)?;
    let json = if draft {
        // A draft run writes its gates where the export run will find them.
        let mut j = previous.unwrap_or(serde_json::json!({}));
        if j.get("build") == Some(&serde_json::json!([1536, 448])) {
            j["draft"] = json["gates"].clone();
            j
        } else {
            let mut j = json.clone();
            j["draft"] = json["gates"].clone();
            j
        }
    } else {
        json
    };
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&json)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&serde_json::json!({ "stones": [] }))?)?;
    }
    renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 }, draft)?;
    timeline(&out, &d, &lib)?;
    let g = &report.gates;
    println!(
        "  {} triangles in {build_s:.1} s; watertight {}; degenerate {}; crossings {}; features ok {}; bore clear {} ({:.3} vs {:.3})",
        g.triangles, g.watertight, g.degenerate_faces, g.ring_self_crossings, g.features_ok, g.bore_clear, g.min_vertex_radius_mm, g.bore_radius_mm
    );
    println!(
        "  field {} / {} ({:.4}% at {:.2} deg); release 0.100 {:?}, 0.075 {:?}; dfm {}; stones {}/{}; cold {:?}; pattern {:?} {:?} {:?}",
        g.field_verdict_192x128, g.field_verdict_256x128, g.field_undercut_percent, g.field_worst_draft_deg, g.release_0100, g.release_0075, g.dfm_findings.len(), g.stones_reported, g.stones_previewed, g.cold_reload_identical, g.pattern_watertight, g.pattern_degenerate_faces, g.pattern_self_crossings
    );
    for n in g.solids_notes.iter().chain(&g.parts_notes).chain(&g.dfm_findings) {
        println!("    note: {n}");
    }
    println!("  seal {}", report.seal);
    println!("  gates {}", if g.passed { "passed" } else { "FAILED" });
    ensure!(g.passed, "Sigil failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
