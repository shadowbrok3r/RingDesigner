//! Officina — Rivet, a riveted strap: sixteen large domed rivet heads on the crest line of a low-dome
//! band, a small one between each pair, each seated with a seam bead. Delft sand.
//! cargo build --release -p ringdesign-core --example officina_rivet
//! target/release/examples/officina_rivet [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, bail, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, Stage},
    castability::{self, CastProcess, SandProcess},
    csg, dfm, library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    profile::DropCurve,
    sketch::{Geometry, Id, Sketch, Workplane},
    stl,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const BORE: f64 = 18.2;
/// Where the first rivet stands round the ring: the top.
const THETA: f64 = 90.0;
/// Large rivets round the ring, and a small one between each pair.
const COUNT: u32 = 16;
/// The strap drawn into the band's crown: across the finger and how far it steps proud of the dome.
const STRAP_WIDE_MM: f64 = 3.0;
const STRAP_PROUD_MM: f64 = 0.48;
/// The crown's whole drop, crest to edge, with the strap's step in it.
const CROWN_MM: f64 = 1.2;
/// The drop from the crest (`x` from the crest to the edge, `d` of the crown, both 0..1): a strap
/// top rounded over the crest and roofed at 5 degrees to its edge at x = 0.5, a rounded step, then the dome falling from its foot to the band's edge.
const STRAPPED_CROWN: [[f64; 2]; 16] = [
    [0.0, 0.0],
    [0.05, 0.004],
    [0.1, 0.014],
    [0.2, 0.038],
    [0.32, 0.068],
    [0.42, 0.09],
    [0.465, 0.10],
    [0.485, 0.14],
    [0.5, 0.28],
    [0.515, 0.43],
    [0.535, 0.50],
    [0.6, 0.545],
    [0.7, 0.62],
    [0.8, 0.71],
    [0.9, 0.83],
    [1.0, 1.0],
];
/// The large head: a spherical cap `FOOT` in radius and `RISE` high on a buried shank, its foot
/// plane `SINK` under the strap so the visible dome stands `RISE - SINK` proud.
const LARGE_FOOT_MM: f64 = 1.2;
const LARGE_RISE_MM: f64 = 0.5;
const LARGE_SINK_MM: f64 = 0.12;
const LARGE_BEAD_MM: f64 = 0.10;
/// The small head, half a pitch on.
const SMALL_FOOT_MM: f64 = 0.8;
const SMALL_RISE_MM: f64 = 0.4;
const SMALL_SINK_MM: f64 = 0.10;
const SMALL_BEAD_MM: f64 = 0.08;
/// How far each head's shank runs on under its foot, buried in the strap.
const SHANK_MM: f64 = 0.4;
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
    s.bench_notes = "Pour the band and its 32 rivet heads as one solid in Delft clay, parted on the band's mid-plane and pulled along the finger. Every head is centred on the crest line, so each half of it faces its own mould half. Ram the sand firmly between the heads and draw the pattern slowly; polish the domes and leave the seam beads crisp.".into();
    s
}

/// LowDome 6.0 x 2.1, bore 18.2 mm, its crown redrawn with a 3 mm strap stepped up along the crest.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: "Rivet \u{2014} a riveted strap".into(),
        ..RingDesign::default()
    };
    d.size = ringdesign_core::resize::size_from_bore(BORE).unwrap();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = 2.1;
    d.profile.style = ProfileStyle::Custom;
    d.profile.crown_mm = CROWN_MM;
    d.profile.drop_curve = DropCurve::from_points(&STRAPPED_CROWN);
    SandProcess::DelftClay.apply(&mut d.draft);
    CastProcess::SandTwoPart.apply(&mut d.draft);
    d.manufacturing = Some(setup());
    d
}

fn component(role: ComponentRole, attach: Attach, blend_mm: f64, placement: Placement) -> Component {
    Component {
        role,
        material: ALLOY.into(),
        attach,
        stage: Stage::Cast,
        blend_mm,
        placement,
        ..Component::default()
    }
}

fn head_section(name: &str, foot: f64, rise: f64) -> Sketch {
    let radius = (foot * foot + rise * rise) / (2.0 * rise);
    let mut s = Sketch {
        name: name.into(),
        plane: Workplane::section(),
        ..Sketch::default()
    };
    let bottom = s.point([0.0, -SHANK_MM]);
    let heel = s.point([foot, -SHANK_MM]);
    let rim = s.point([foot, 0.0]);
    // The arc stops a hair short of the axis, where a turned face would close to a point.
    let near = 0.02;
    let crown = s.point([near, rise - radius + (radius * radius - near * near).sqrt()]);
    let apex = s.point([0.0, rise - radius + (radius * radius - near * near).sqrt()]);
    let centre = s.point([0.0, rise - radius]);
    s.entity(Geometry::Line { a: bottom, b: heel });
    s.entity(Geometry::Line { a: heel, b: rim });
    s.entity(Geometry::Arc { center: centre, start: rim, end: crown });
    s.entity(Geometry::Line { a: crown, b: apex });
    s.entity(Geometry::Line { a: apex, b: bottom });
    s
}

/// The head turned whole about its own axis.
fn head(name: &str, foot: f64, rise: f64) -> Operation {
    Operation::Revolve {
        sketch: head_section(name, foot, rise).into(),
        pivot: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        degrees: 360.0,
        in_plane: false,
    }
}

/// The visible footprint of a cap `foot` x `rise` sunk `sink` under the ground: its diameter there.
fn footprint(foot: f64, rise: f64, sink: f64) -> f64 {
    let radius = (foot * foot + rise * rise) / (2.0 * rise);
    2.0 * (radius * radius - (sink - (rise - radius)).powi(2)).sqrt()
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

fn evaluated(d: &RingDesign, lib: &AlphaLibrary) -> Result<cad::Evaluated> {
    let e = cad::evaluate(d, lib, draft_params())?;
    if let Some((id, why)) = e.failures().first() {
        bail!("feature #{id} failed: {why}");
    }
    Ok(e)
}

/// What the author set, for the report.
#[derive(serde::Serialize, Default)]
struct Authored {
    strap_wide_mm: f64,
    strap_proud_mm: f64,
    rivets: u32,
    pitch_deg: f64,
    large_proud_mm: f64,
    large_foot_mm: f64,
    small_proud_mm: f64,
    small_foot_mm: f64,
    gap_mm: f64,
}

/// The five-feature history: the strapped band, one large head, its ring array, one small head, its ring array.
fn author(lib: &AlphaLibrary) -> Result<(RingDesign, Authored)> {
    let mut d = band();
    add(&mut d, "Band", Operation::Band, component(ComponentRole::Shank, Attach::Separate, 0.0, Placement::Free))?;
    d.cad.as_mut().unwrap().features[0].component.manufacturing = d.manufacturing.clone();
    let pitch = 360.0 / f64::from(COUNT);
    let large = add(
        &mut d,
        "Large rivet head",
        head("Large rivet head", LARGE_FOOT_MM, LARGE_RISE_MM),
        component(ComponentRole::Other, Attach::Join, LARGE_BEAD_MM, Placement::ring(THETA, -LARGE_SINK_MM)),
    )?;
    add(
        &mut d,
        "Sixteen round the ring",
        Operation::Pattern {
            sources: large.into(),
            kind: PatternKind::Ring { count: COUNT, span_deg: 360.0 },
        },
        component(ComponentRole::Other, Attach::Join, LARGE_BEAD_MM, Placement::Free),
    )?;
    let small = add(
        &mut d,
        "Small rivet head",
        head("Small rivet head", SMALL_FOOT_MM, SMALL_RISE_MM),
        component(ComponentRole::Other, Attach::Join, SMALL_BEAD_MM, Placement::ring(THETA + pitch / 2.0, -SMALL_SINK_MM)),
    )?;
    add(
        &mut d,
        "Sixteen between them",
        Operation::Pattern {
            sources: small.into(),
            kind: PatternKind::Ring { count: COUNT, span_deg: 360.0 },
        },
        component(ComponentRole::Other, Attach::Join, SMALL_BEAD_MM, Placement::Free),
    )?;
    evaluated(&d, lib)?;
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    let (large_foot, small_foot) = (footprint(LARGE_FOOT_MM, LARGE_RISE_MM, LARGE_SINK_MM), footprint(SMALL_FOOT_MM, SMALL_RISE_MM, SMALL_SINK_MM));
    let notes = Authored {
        strap_wide_mm: STRAP_WIDE_MM,
        strap_proud_mm: STRAP_PROUD_MM,
        rivets: 2 * COUNT,
        pitch_deg: pitch,
        large_proud_mm: LARGE_RISE_MM - LARGE_SINK_MM,
        large_foot_mm: large_foot,
        small_proud_mm: SMALL_RISE_MM - SMALL_SINK_MM,
        small_foot_mm: small_foot,
        gap_mm: (pitch / 2.0).to_radians() * crest - large_foot / 2.0 - small_foot / 2.0,
    };
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
    thinnest_wall_mm: f64,
    release: Vec<Release>,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    pattern: Option<PatternGate>,
    /// Lost-wax gates, recorded for the lesson only: Rivet pours in sand.
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
    })
}

fn gates(
    d: &RingDesign,
    lib: &AlphaLibrary,
    params: BuildParams,
    built: &mesh::BuildResult,
    build_s: f64,
    export: bool,
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
    let pattern = if export {
        let p = mesh::try_build_pattern(d, lib, params)?;
        Some(PatternGate {
            triangles: p.mesh.faces.len(),
            watertight: p.report.validation.watertight,
            degenerate_faces: p.report.quality.degenerate_faces,
            self_crossings: crossings_of(&p.mesh),
            notes: p.parts.notes.clone(),
        })
    } else {
        None
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
    let verdict_ok = g.field_verdict == castability::Verdict::Castable.label();
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

/// The hero looks down on the crest from in front and a little to one side.
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
    // The close-up frames on the top rivets and draws the whole ring.
    let crest = d.inner_radius_mm() + d.profile.thickness_mm;
    let (s, c) = THETA.to_radians().sin_cos();
    let close_mesh = crop(&finished.metal, [crest * c, crest * s, 0.0], 5.0);
    let mut close = vec![render::Part::metal(&close_mesh, render::GOLD)];
    close.extend(finished.parts(render::GOLD));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    // The bare band against the finished ring, at the hero's angle.
    let b = mesh::try_build(&band(), lib, draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], HERO.0, HERO.1, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, HERO.0, HERO.1, edge, edge, 3);
    let mut pair = Canvas::new(edge * 2, edge, 0);
    pair.blit(&bare_img, edge, edge, 0, 0);
    pair.blit(&finished_img, edge, edge, edge, 0);
    pair.save(&out.join("bare-vs-finished.png"))?;
    Ok(())
}

/// One contact sheet of the ring after each feature, labelled with the feature's name.
fn timeline(out: &Path, d: &RingDesign, lib: &AlphaLibrary) -> Result<()> {
    let doc = d.cad.as_ref().unwrap();
    let n = doc.features.len();
    let (cell, label, cols) = (360usize, 34usize, 4usize);
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
        let finished = render::finished(&step, lib, params)?;
        let img = render::render_parts_ss(&finished.parts(render::GOLD), HERO.0, HERO.1, cell, cell, 2);
        let (x, y) = ((k % cols) * cell, (k / cols) * (cell + label));
        sheet.blit(&img, cell, cell, x, y + label);
        sheet.text(&format!("{}. {}", k + 1, f.name), 22.0, x + 10, y + 4);
    }
    sheet.save(&out.join("timeline.png"))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina/rivet"));
    std::fs::create_dir_all(&out)?;
    println!("Rivet");
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
    let g = gates(&d, &lib, params, &built, build_s, !draft, cold)?;
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
        "ring": "Rivet",
        "slug": "rivet",
        "process": d.draft.process.label(),
        "sand": "Delft clay",
        "min_draft_deg": d.draft.min_draft_deg,
        "min_section_mm": d.draft.min_section_mm,
        "min_detail_mm": d.draft.min_detail_mm,
        "bore_mm": BORE,
        "size": d.size.display(),
        "authored": authored,
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
    let finished = render::finished_from(&d, &lib, built);
    renders(&out, &d, &lib, &finished, if draft { 1000 } else { 1600 })?;
    timeline(&out, &d, &lib)?;
    ensure!(g.passed, "Rivet failed its gates; see {}", path.display());
    Ok(())
}
