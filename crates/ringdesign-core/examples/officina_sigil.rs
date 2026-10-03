//! Officina — Sigil, a quartered seal on the round: factory stock 013 poured bare in Delft, the seal cut at the bench.
//! cargo build --release -p ringdesign-core --example officina_sigil
//! target/release/examples/officina_sigil [OUT_DIR] [--draft] [--verify] [--spike]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, FaceRef, Feature, Operation, PlaneBase, Profile, Stage},
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    field::BorderLayer,
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, RegionRef, Sketch},
    stl,
};
use serde::Serialize;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const BORE_MM: f64 = 18.2;
const PLANE: u64 = 2;
const FIELD_SKETCH: u64 = 3;
const BORDURE_SKETCH: u64 = 6;
/// The field's radius on the table, and the bordure's inner and outer rims.
const FIELD_R_MM: f64 = 3.9;
/// Half the raised cross that parts the quarters.
const CROSS_HALF_MM: f64 = 0.15;
const BORDURE_IN_MM: f64 = 4.15;
const BORDURE_OUT_MM: f64 = 4.65;
/// How far below the table plane each cut's flat floor stands. The table is flat round the ring and
/// crowned across the finger (0.68 mm down at 4 mm), so a floor this deep still sinks the quarters'
/// tips 0.27 mm and the bordure's ends 0.2 mm.
const QUARTER_FLOOR_MM: f64 = 0.95;
const BORDURE_FLOOR_MM: f64 = 1.0;
/// The sand master's own mirror seam on the crest line (about 1 micron deep between 0 and 55 degrees)
/// reads as undercut; a bead this size fills it, and nothing a render can see.
const SEAM_WIDTH_MM: f64 = 0.5;
const SEAM_HEIGHT_MM: f64 = 0.004;
/// Wall draft on every bench cut, so the walls catch the light, and the bead that breaks each cut's top edge.
const WALL_DRAFT_DEG: f64 = 8.0;
const EDGE_BREAK_MM: f64 = 0.08;
/// The satin the cuts are left in, against the polished table.
const SATIN: f64 = 0.66;
/// The matting punch on the sunk quarters' floors: Petra Sancta's dots for Or. Pitch, punch radius,
/// how far the punch bites below the floor, and how far the dots keep from the walls.
const MAT_PITCH_MM: f64 = 0.38;
const MAT_R_MM: f64 = 0.11;
const MAT_BITE_MM: f64 = 0.07;
const MAT_INSET_MM: f64 = 0.3;
const MATTING_SKETCH: u64 = 8;

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

fn on_plane(name: &str) -> Sketch {
    let mut s = Sketch { name: name.into(), ..Sketch::default() };
    s.plane.on_face = Some(FaceAnchor { feature: PLANE, face: FaceRef::bare(0) });
    s
}

/// The field: four quarter arcs (NE, NW, SW, SE) of radius `r`, parted by a cross `2 * half` wide whose
/// arms run out to the rim; each quarter closes on its arc and two lines meeting at its inner corner.
fn field(r: f64, half: f64) -> (Sketch, [u64; 4]) {
    let mut s = on_plane("Field");
    let c = s.point([0.0, 0.0]);
    let reach = (r * r - half * half).sqrt();
    let arcs = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].map(|(sx, sy): (f64, f64)| {
        let corner = s.point([sx * half, sy * half]);
        // Counterclockwise from the arm on x to the arm on y in the first quarter, and on round.
        let (from, to) = if sx * sy > 0.0 { ([sx * reach, sy * half], [sx * half, sy * reach]) } else { ([sx * half, sy * reach], [sx * reach, sy * half]) };
        let (a, b) = (s.point(from), s.point(to));
        let arc = s.entity(Geometry::Arc { center: c, start: a, end: b });
        s.entity(Geometry::Line { a: b, b: corner });
        s.entity(Geometry::Line { a: corner, b: a });
        arc
    });
    (s, arcs)
}

/// The matting: punch dots on a hex grid filling the two sunk quarters (first and fourth), each its own region.
fn matting() -> Sketch {
    let mut s = on_plane("Matting");
    let row = MAT_PITCH_MM * 3f64.sqrt() / 2.0;
    let lo = CROSS_HALF_MM + MAT_INSET_MM;
    let reach = FIELD_R_MM - MAT_INSET_MM;
    for (sx, sy) in [(1.0, 1.0), (-1.0, -1.0)] {
        let mut j = 0;
        loop {
            let y = lo + j as f64 * row;
            if y > reach {
                break;
            }
            let mut x = lo + if j % 2 == 1 { MAT_PITCH_MM / 2.0 } else { 0.0 };
            while x.hypot(y) <= reach {
                let c = s.point([sx * x, sy * y]);
                let rim = s.point([sx * x + MAT_R_MM, sy * y]);
                s.entity(Geometry::Circle { center: c, rim });
                x += MAT_PITCH_MM;
            }
            j += 1;
        }
    }
    s
}

/// Two circles about the centre: one region with a hole.
fn bordure(inner: f64, outer: f64) -> Sketch {
    let mut s = on_plane("Bordure");
    for r in [inner, outer] {
        let c = s.point([0.0, 0.0]);
        let rim = s.point([r, 0.0]);
        s.entity(Geometry::Circle { center: c, rim });
    }
    s
}

fn document() -> Result<Document> {
    let mut doc = Document::default();
    doc.append(feature(1, "Stock 013", Operation::Band, Attach::Separate, Stage::Cast))?;
    doc.append(feature(PLANE, "Table plane", Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: 0.0 }, Attach::Separate, Stage::Cast))?;
    let (sketch, [ne, _, sw, _]) = field(FIELD_R_MM, CROSS_HALF_MM);
    doc.append(feature(FIELD_SKETCH, "Field quartered", Operation::Sketch { sketch }, Attach::Separate, Stage::Cast))?;
    let cut = |id, name: &str, sketch: Profile, depth: f64| {
        let mut f = feature(id, name, Operation::Extrude { sketch, height_mm: -depth, draft_deg: WALL_DRAFT_DEG }, Attach::Cut, Stage::Bench);
        f.component.blend_mm = EDGE_BREAK_MM;
        f.component.bench_notes = "Cut at the bench after the pour, its top edge broken and its floor and walls left satin; the pattern is the bare stock.".into();
        f
    };
    let q = 0.45 * FIELD_R_MM;
    doc.append(cut(4, "First quarter sunk", Profile::Region { feature: FIELD_SKETCH, region: RegionRef { entity: ne, at: [q, q] } }, QUARTER_FLOOR_MM))?;
    doc.append(cut(5, "Fourth quarter sunk", Profile::Region { feature: FIELD_SKETCH, region: RegionRef { entity: sw, at: [-q, -q] } }, QUARTER_FLOOR_MM))?;
    doc.append(feature(BORDURE_SKETCH, "Bordure", Operation::Sketch { sketch: bordure(BORDURE_IN_MM, BORDURE_OUT_MM) }, Attach::Separate, Stage::Cast))?;
    doc.append(cut(7, "Bordure sunk", Profile::Feature { feature: BORDURE_SKETCH }, BORDURE_FLOOR_MM))?;
    doc.append(feature(MATTING_SKETCH, "Matting", Operation::Sketch { sketch: matting() }, Attach::Separate, Stage::Cast))?;
    let mut punch = cut(9, "Quarters matted", Profile::Feature { feature: MATTING_SKETCH }, QUARTER_FLOOR_MM + MAT_BITE_MM);
    if let Operation::Extrude { draft_deg, .. } = &mut punch.operation {
        *draft_deg = 0.0;
    }
    punch.component.blend_mm = 0.0;
    punch.component.bench_notes = "Matted at the bench with a round punch after the quarters are sunk: Or, in Petra Sancta's dots.".into();
    doc.append(punch)?;
    Ok(doc)
}

fn design() -> Result<RingDesign> {
    let mut d = dressed()?;
    d.cad = Some(document()?);
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

/// The built ring split by finish: the polished metal, and what the bench cuts made, left satin. A face
/// is a cut's when it lies on a cut's flat floor, or is a drafted wall standing on one inside the bordure;
/// the edge-break bead above each wall stays polished, so the finish changes on the geometry's own line.
fn finishes(built: &mesh::BuildResult) -> (mesh::Mesh, mesh::Mesh) {
    let m = &built.mesh;
    let top = built.parts.evaluated.as_ref().and_then(|e| e.plane(PLANE)).map_or(0.0, |p| p.origin[1]);
    let floors = [top - QUARTER_FLOOR_MM, top - BORDURE_FLOOR_MM, top - QUARTER_FLOOR_MM - MAT_BITE_MM];
    let cut = |f: &[u32; 3]| {
        let [a, b, c] = f.map(|i| m.vertices[i as usize]);
        let centre = [(a.0 + b.0 + c.0) as f64 / 3.0, (a.1 + b.1 + c.1) as f64 / 3.0, (a.2 + b.2 + c.2) as f64 / 3.0];
        if centre[0].hypot(centre[2]) > BORDURE_OUT_MM + 0.05 || centre[1] < floors[2] - 0.02 {
            return false;
        }
        let (u, v) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]) as f64).sqrt().max(1e-30);
        let ny = n[1] as f64 / l;
        let on_floor = ny > 0.9995 && floors.iter().any(|y| (centre[1] - y).abs() < 0.005);
        let wall = ny.abs() < 0.5;
        on_floor || wall
    };
    let split = |keep: bool| {
        let mut out = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..Default::default() };
        let mut cursor = 0usize;
        for (i, f) in m.faces.iter().enumerate() {
            if cut(f) == keep {
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
    let (a, mut b) = (split(false), split(true));
    creased(&mut b, 30.0);
    (a, b)
}

/// Corner normals for every face of `m` from its own geometry: each corner averages the faces round
/// its vertex that turn less than `deg` from this one, so planar floors shade flat and walls keep their edge.
fn creased(m: &mut mesh::Mesh, deg: f64) {
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
    let mut around: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
    for (i, f) in m.faces.iter().enumerate() {
        for &v in f {
            around.entry(v).or_default().push(i);
        }
    }
    let cos = deg.to_radians().cos();
    m.corner_normals = m
        .faces
        .iter()
        .enumerate()
        .map(|(i, f)| {
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
            (i as u32, f.map(corner))
        })
        .collect();
}

/// The ring's parts as the bench finishes them: polished metal, satin cuts, and any stones.
fn dressed_parts<'a>(polished: &'a mesh::Mesh, satin: &'a mesh::Mesh, stones: &'a [(mesh::Mesh, [f32; 3])]) -> Vec<render::Part<'a>> {
    let mut parts = vec![render::Part::metal(polished, render::GOLD)];
    if !satin.faces.is_empty() {
        let mut s = render::Part::metal(satin, render::GOLD);
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
    let (polished, satin) = finishes(built);
    let parts = dressed_parts(&polished, &satin, &stones);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // No stones: the close-up is the seal itself, straight on and raking.
    let (seal_p, seal_s) = (crop(&polished, [0.0, 12.0, 0.0], 7.5), crop(&satin, [0.0, 12.0, 0.0], 7.5));
    let close = dressed_parts(&seal_p, &seal_s, &[]);
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
    let left = frame(&[render::Part::metal(&b.mesh, render::GOLD)], 0.55, 0.95, edge);
    let right = frame(&parts, 0.55, 0.95, edge);
    let mut pair = Canvas::new(edge * 2, edge, [255, 255, 255]);
    pair.blit(&left, edge, edge, 0, 0);
    pair.blit(&right, edge, edge, edge, 0);
    pair.save(&out.join("bare-vs-finished.png"))?;
    Ok(())
}

/// The ring after each feature (`Document::through`), one labelled cell per step, at the hero's angle.
fn timeline(out: &Path, d: &RingDesign, lib: &AlphaLibrary) -> Result<()> {
    let doc = d.cad.as_ref().unwrap();
    let ids: Vec<(u64, String)> = doc.features.iter().map(|f| (f.id, f.name.clone())).collect();
    let cell = 360;
    let label = 54;
    let cols = 3;
    let rows = ids.len().div_ceil(cols);
    let mut sheet = Canvas::new(cols * cell, rows * (cell + label), [255, 255, 255]);
    for (k, (id, name)) in ids.iter().enumerate() {
        let mut step = d.clone();
        step.cad.as_mut().unwrap().through = Some(*id);
        let built = mesh::try_build(&step, lib, BuildParams { theta_steps: 512, profile_steps: 256, ..Default::default() })?;
        let (polished, satin) = finishes(&built);
        let mut guide = mesh::Mesh::default();
        if let Some(plane) = built.parts.evaluated.as_ref().and_then(|e| e.plane(PLANE)) {
            let n = plane.normal;
            if matches!(*id, 2 | 3 | 6 | 8) {
                let h = 6.2;
                let at = |u: f64, v: f64| -> [f64; 3] { std::array::from_fn(|k| plane.origin[k] + plane.x[k] * u + plane.y[k] * v + n[k] * 0.03) };
                tube(&[at(-h, -h), at(h, -h), at(h, h), at(-h, h), at(-h, -h)], n, 0.05, &mut guide);
            }
            for f in doc.features.iter().filter(|f| f.id <= *id) {
                let shown = match f.id {
                    FIELD_SKETCH => matches!(*id, 3..=5),
                    BORDURE_SKETCH => *id == 6,
                    MATTING_SKETCH => *id == 8,
                    _ => false,
                };
                if let (true, Operation::Sketch { sketch }) = (shown, &f.operation) {
                    for line in sketch_lines(sketch, plane) {
                        tube(&line, n, 0.045, &mut guide);
                    }
                }
            }
        }
        let mut parts = dressed_parts(&polished, &satin, &[]);
        let mut g = render::Part::metal(&guide, [0.30, 0.72, 1.0]);
        g.studio = false;
        if !guide.faces.is_empty() {
            parts.push(g);
        }
        let img = frame(&parts, 0.3, 1.2, cell);
        let (x, y) = ((k % cols) * cell, (k / cols) * (cell + label));
        sheet.blit(&img, cell, cell, x, y + label);
        sheet.text(&format!("{} \u{00b7} {name}", k + 1), x + 14, y + 38, 30.0, [40, 34, 26]);
    }
    sheet.save(&out.join("timeline.png"))?;
    Ok(())
}

fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    let corners: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    for (fi, f) in m.faces.iter().enumerate().filter(|(_, f)| f.iter().all(|&i| near(i))) {
        if let Some(c) = corners.get(&(fi as u32)) {
            out.corner_normals.push((out.faces.len() as u32, *c));
        }
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

/// Depth of each cut where it is shallowest and deepest, read off the built mesh along the seal's two axes.
fn seal_depths(bare: &mesh::Mesh, built: &mesh::Mesh) -> serde_json::Value {
    let depth = |x: f64, z: f64| match (top_at(bare, x, z), top_at(built, x, z)) {
        (Some(a), Some(b)) => a - b,
        _ => f64::NAN,
    };
    // The sketch's x runs round the ring toward world -x at the table, its y along the finger.
    let q = 0.45 * FIELD_R_MM;
    let b = 0.5 * (BORDURE_IN_MM + BORDURE_OUT_MM);
    serde_json::json!({
        "first_quarter_mm": { "centre": depth(-0.3, 0.3), "mid": depth(-q, q), "tip_along_finger": depth(-0.35, FIELD_R_MM - 0.35), "tip_round_ring": depth(-(FIELD_R_MM - 0.35), 0.35) },
        "second_quarter_raised_mm": depth(q, q),
        "bordure_mm": { "round_ring": depth(b, 0.0), "along_finger": depth(0.0, b) },
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
