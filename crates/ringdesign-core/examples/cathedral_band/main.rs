//! Cathedral channel band: a Gothic façade round the ring in five recessed levels between two rails, a different bay every few millimetres. Lost wax.
//! cargo run --release -p ringdesign-core --example cathedral_band -- [OUT_DIR] [--size 9] [--width 8] [--thickness 2.2] [--layout] [--export]
mod facade;
mod field;
mod geom;
mod layout;
mod windows;

use anyhow::{Context, Result, ensure};
use layout::{Layout, LEVELS};
use ringdesign_core::{
    AlphaLibrary, Blend, BuildParams, Layer, LayerEntry, ProfileStyle, RingDesign, RingSize, ShankKind,
    alpha::Procedural,
    castability::{self, CastProcess},
    csg, dfm, library, mesh, render,
    setting::{Stamp, StampTop},
    stl,
    tiling::TilingLayer,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const NAME: &str = "Cathedral façade band";
const DESIGN_FILE: &str = "h-cathedral-facade";

/// Everything the ring is made from, mm unless named.
#[derive(Clone, Debug, serde::Serialize)]
struct Params {
    size: f64,
    width: f64,
    thickness: f64,
    /// Rail across the band from each edge, edge round included.
    rail: f64,
    edge_round: f64,
    comfort: f64,
    /// Depth of each level under the rails' top, the first the channel floor.
    depths: [f64; LEVELS],
    /// Level-1 border along each edge of the channel.
    strip: f64,
    corner_r: f64,
    eps: f64,
    step: f64,
    /// Ballflowers' domed tops over their rims.
    dome: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            size: 9.0,
            width: 8.0,
            thickness: 2.2,
            rail: 1.0,
            edge_round: 0.25,
            comfort: 0.15,
            depths: [0.2, 0.42, 0.64, 0.86, 1.08],
            strip: 0.2,
            corner_r: 0.085,
            eps: 0.02,
            step: 0.005,
            dome: 0.1,
        }
    }
}

fn parse_args() -> Result<(PathBuf, Params, bool, bool)> {
    let mut p = Params::default();
    let mut out = PathBuf::from("target/cathedral-band");
    let (mut layout_only, mut export) = (false, false);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let mut num = |what: &str| -> Result<f64> {
            i += 1;
            args.get(i).with_context(|| format!("{what} needs a value"))?.parse::<f64>().with_context(|| format!("{what} needs a number"))
        };
        match a {
            "--size" => p.size = num("--size")?,
            "--width" => p.width = num("--width")?,
            "--thickness" => p.thickness = num("--thickness")?,
            "--rail" => p.rail = num("--rail")?,
            "--layout" => layout_only = true,
            "--export" => export = true,
            _ if a.starts_with("--") => anyhow::bail!("unknown option {a}"),
            _ => out = PathBuf::from(a),
        }
        i += 1;
    }
    Ok((out, p, layout_only, export))
}

fn band(p: &Params) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = NAME.into();
    d.size = RingSize::new(p.size);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = p.width;
    d.profile.thickness_mm = p.thickness;
    d.profile.crown_mm = 0.0;
    d.profile.edge_round_mm = p.edge_round;
    d.profile.comfort_fit_mm = p.comfort;
    d.profile.side_draft_deg = 0.0;
    d.shank.kind = ShankKind::Uniform;
    CastProcess::LostWax.apply(&mut d.draft);
    // The channel between the rails, its floor level 1.
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(Procedural::ALL[0].label(), &ctx);
    t.repeats_around = 1;
    t.rows = 1;
    t.v_center_mm = 0.5 * ctx.band_v_len_mm;
    t.v_span_mm = p.width - 2.0 * p.rail;
    t.height_mm = p.depths[0];
    t.bias = 1.0;
    t.contrast = 1.0;
    t.invert = false;
    t.feather_mm = 0.0;
    t.edge_mm = 0.0;
    t.gap_mm = 0.0;
    let mut e = LayerEntry::new("Channel", Layer::Tiling(t));
    e.blend = Blend::Subtract;
    d.layers.layers.push(e);
    d
}

/// Plane outlines struck round the ring: walls on the ring's radius, the plane's x laid on the channel floor's arc.
fn stamps(d: &RingDesign, l: &Layout, p: &Params) -> Vec<Stamp> {
    let ctx = d.field_context();
    let crest_r = ctx.crest_radius_mm;
    let floor_r = crest_r - p.depths[0];
    let place = |at: [f64; 2], outline: &[[f64; 2]]| -> (f64, f64, Vec<[f64; 2]>) {
        let theta = (90.0 + (at[0] / crest_r).to_degrees()).rem_euclid(360.0);
        // Mirrored across the band and reversed to stay counter-clockwise.
        let pts = outline.iter().rev().map(|q| [floor_r * ((q[0] - at[0]) / crest_r).sin(), at[1] - q[1]]).collect();
        (theta, 0.5 * ctx.band_v_len_mm - at[1], pts)
    };
    let mut out = Vec::new();
    for o in &l.openings {
        let (theta_deg, v_mm, outline) = place(o.at, &o.outline);
        out.push(Stamp {
            name: o.name.clone(),
            theta_deg,
            v_mm,
            rot_deg: 0.0,
            outline,
            height_mm: 0.3,
            sink_mm: p.depths[o.floor - 1] - p.depths[0],
            draft_deg: 0.0,
            cut: true,
            bench: false,
            along_pull: false,
            tier: 0,
            top: StampTop::Flat,
            fine_cap: true,
            radial: true,
        });
    }
    for r in &l.raised {
        let (theta_deg, v_mm, outline) = place(r.at, &r.outline);
        out.push(Stamp {
            name: r.name.clone(),
            theta_deg,
            v_mm,
            rot_deg: 0.0,
            outline,
            height_mm: p.depths[r.floor - 1] - p.depths[r.top - 1] - p.dome,
            sink_mm: 0.05,
            draft_deg: 0.0,
            cut: false,
            bench: false,
            along_pull: false,
            tier: 1,
            top: StampTop::Dome { crown_mm: p.dome },
            fine_cap: true,
            radial: true,
        });
    }
    out
}

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 1024, profile_steps: 384, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 2048, profile_steps: 640, refine: None, ..BuildParams::default() }
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
}

#[derive(serde::Serialize)]
struct Gates {
    triangles: usize,
    watertight: bool,
    degenerate_faces: usize,
    self_crossings: usize,
    stamps: usize,
    stamped: usize,
    notes: Vec<String>,
    min_radius_mm: f64,
    bore_radius_mm: f64,
    verdict: String,
    thinnest_wall_mm: f64,
    census_floor_mm: f64,
    census_walls: usize,
    census_wall_area_mm2: f64,
    census_edges: usize,
    census_min_mm: Option<f64>,
    /// The thinnest zones: kind, area, thinnest, and where (mm round from the portal, across, radius).
    census_zones: Vec<(String, f64, f64, [f64; 3])>,
    dfm: Vec<String>,
    grams: Vec<(String, f64)>,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, floor: f64) -> Gates {
    let m = &built.mesh;
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let thick = ringdesign_core::cad::measure::thickness(m, floor);
    let min_r = m.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64)).fold(f64::MAX, f64::min);
    let r_out = d.inner_radius_mm() + d.profile.thickness_mm;
    let zone = |z: &ringdesign_core::cad::measure::ThinZone| {
        let [x, y, zz] = z.point;
        let u = ((y.atan2(x) - 0.5 * PI + PI).rem_euclid(2.0 * PI) - PI) * r_out;
        (format!("{:?}", z.kind), z.area_mm2, z.thinnest_mm, [u, -zz, x.hypot(y)])
    };
    let mut walls: Vec<_> = thick.walls.iter().collect();
    walls.sort_by(|a, b| a.thinnest_mm.total_cmp(&b.thinnest_mm));
    let mut edges: Vec<_> = thick.edges.iter().collect();
    edges.sort_by(|a, b| a.thinnest_mm.total_cmp(&b.thinnest_mm));
    Gates {
        triangles: m.faces.len(),
        watertight: m.validate().watertight,
        degenerate_faces: m.quality().degenerate_faces,
        self_crossings: csg::self_crossings(&solid_of(m)),
        stamps: d.stamps.len(),
        stamped: built.solids.stamped,
        notes: built.solids.notes.clone(),
        min_radius_mm: min_r,
        bore_radius_mm: d.inner_radius_mm(),
        verdict: field.verdict.label().to_string(),
        thinnest_wall_mm: field.thinnest_wall_mm,
        census_floor_mm: floor,
        census_walls: thick.below_limit,
        census_wall_area_mm2: thick.wall_area_mm2,
        census_edges: thick.edge_below_limit,
        census_min_mm: thick.sampled_min_mm,
        census_zones: walls.into_iter().take(24).chain(edges.into_iter().take(8)).map(zone).collect(),
        dfm: dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        grams: built.report.metals.iter().map(|w| (w.metal.to_string(), w.grams)).collect(),
    }
}

const VIEWS: [(&str, f64, f64); 5] = [("hero", -0.5, 1.02), ("face", 0.0, PI * 0.5), ("three-quarter", 0.62, 0.78), ("side", 0.0, 0.0), ("palm", PI, 1.0)];

/// Patina by level: the rails polished, each level a step darker down to the oxidised floor.
const GOLD_TONES: [[f32; 3]; 5] = [[0.80, 0.61, 0.29], [0.40, 0.29, 0.13], [0.20, 0.14, 0.065], [0.09, 0.064, 0.032], [0.03, 0.028, 0.026]];
const SILVER: [f32; 3] = [0.95, 0.93, 0.88];
const SILVER_SHADES: [f32; 5] = [0.80, 0.40, 0.19, 0.08, 0.025];
const ROUGHNESS: [f64; 5] = [0.3, 0.55, 0.65, 0.75, 0.85];

/// The faces of `m` picked by `keep`, with their vertex and crease normals.
fn submesh(m: &mesh::Mesh, keep: &[bool]) -> mesh::Mesh {
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    let mut new_of = vec![u32::MAX; m.faces.len()];
    for (k, f) in m.faces.iter().enumerate().filter(|(k, _)| keep[*k]) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        new_of[k] = out.faces.len() as u32;
        out.faces.push(g);
    }
    out.corner_normals = m.corner_normals.iter().filter(|(k, _)| keep[*k as usize]).map(|(k, n)| (new_of[*k as usize], *n)).collect();
    out
}

/// The finished metal split by depth under the rails: polished, then each level; a face takes its deepest corner's level.
fn by_depth(d: &RingDesign, m: &mesh::Mesh, p: &Params) -> Vec<mesh::Mesh> {
    let r_out = d.inner_radius_mm() + d.profile.thickness_mm;
    let half = 0.5 * p.width - p.rail;
    let dp = p.depths;
    let class: Vec<usize> = m
        .faces
        .iter()
        .map(|f| {
            let (mut depth, mut z) = (f64::MIN, 0.0f64);
            for &i in f {
                let v = m.vertices[i as usize];
                depth = depth.max(r_out - (v.0 as f64).hypot(v.1 as f64));
                z = z.max((v.2 as f64).abs());
            }
            if z > half + 0.03 || depth < 0.6 * dp[0] || depth > dp[LEVELS - 1] + 0.3 {
                0
            } else {
                1 + (1..LEVELS).filter(|&k| depth > 0.5 * (dp[k - 1] + dp[k])).count()
            }
        })
        .collect();
    if std::env::var_os("CB_DEBUG").is_some() {
        let counts: Vec<usize> = (0..=LEVELS).map(|k| class.iter().filter(|c| **c == k).count()).collect();
        eprintln!("faces by class: {counts:?}");
    }
    (0..=LEVELS).map(|k| submesh(m, &class.iter().map(|c| *c == k).collect::<Vec<_>>())).collect()
}

fn part(m: &mesh::Mesh, tint: [f32; 3], roughness: f64) -> render::Part<'_> {
    render::Part { mesh: m, tint, gem: false, smooth: true, studio: true, roughness }
}

fn renders(out: &Path, d: &RingDesign, built: &mesh::BuildResult, p: &Params, l: &Layout, edge: usize) -> Result<()> {
    let split = by_depth(d, &built.mesh, p);
    let mut gold = vec![render::Part::metal(&split[0], render::GOLD)];
    let mut silver = vec![part(&split[0], SILVER, 0.14)];
    for k in 0..LEVELS {
        gold.push(part(&split[k + 1], GOLD_TONES[k], ROUGHNESS[k]));
        let s = SILVER_SHADES[k];
        silver.push(part(&split[k + 1], [SILVER[0] * s, SILVER[1] * s, SILVER[2] * s], ROUGHNESS[k]));
    }
    let bright = [render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &gold, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("hero-silver.png"), &silver, VIEWS[0].1, VIEWS[0].2, edge)?;
    render::write_png_parts(out.join("hero-bright.png"), &bright, VIEWS[0].1, VIEWS[0].2, edge)?;
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    for sec in l.facade.sections.iter().filter(|s| !s.name.starts_with("Buttress")) {
        let deg = 90.0 + (0.5 * (sec.x0 + sec.x1) / r).to_degrees();
        let t = deg.to_radians();
        let at = [r * t.cos(), r * t.sin(), 0.0];
        let slug = sec.name.to_lowercase().replace(' ', "-");
        let span = (sec.x1 - sec.x0).max(7.0) + 1.0;
        render::write_png_framed(out.join(format!("close-{slug}.png")), &gold, render::yaw_facing(deg), PI * 0.5, render::Framing::new(at, span), edge)?;
        render::write_png_framed(out.join(format!("close-{slug}-silver.png")), &silver, render::yaw_facing(deg) + 0.3, PI * 0.5 - 0.35, render::Framing::new(at, span), edge)?;
    }
    let top = [0.0, r, 0.0];
    render::write_png_framed(out.join("close-rake.png"), &gold, render::yaw_facing(90.0) - 0.4, PI * 0.5 - 0.5, render::Framing::new(top, 9.0), edge)?;
    render::write_png_framed(out.join("close-bright.png"), &bright, render::yaw_facing(90.0) - 0.4, PI * 0.5 - 0.5, render::Framing::new(top, 9.0), edge)?;
    Ok(())
}

fn main() -> Result<()> {
    let (out, p, layout_only, export) = parse_args()?;
    std::fs::create_dir_all(&out)?;
    let mut d = band(&p);
    let ctx = d.field_context();
    let crest_r = ctx.crest_radius_mm;
    let circumference = 2.0 * PI * crest_r;
    let half = 0.5 * p.width - p.rail;
    let clock = std::time::Instant::now();
    let f = facade::facade(circumference, half - p.strip);
    if let Some(probe) = std::env::var_os("CB_PROBE") {
        let xy: Vec<f64> = probe.to_string_lossy().split(',').filter_map(|v| v.parse().ok()).collect();
        let q = [xy[0], xy[1]];
        for lv in 1..LEVELS {
            let v = f.pieces.iter().filter(|p| p.level == lv).map(|p| p.metal.sd(q)).fold(geom::CAP, f64::min);
            println!("probe ({:.3}, {:.3}) level {lv}: {v:+.4}", q[0], q[1]);
        }
    }
    let settings = layout::Settings { step: p.step, corner_r: p.corner_r, eps: p.eps, margin: 1.5 };
    let l = layout::layout(f, &settings)?;
    println!(
        "{NAME}: size {} ({:.2} mm bore), {} x {} mm, crest r {:.3}, {:.2} mm round, {} pockets and {} ornaments in {:.1} s, most points {}",
        p.size,
        2.0 * d.inner_radius_mm(),
        p.width,
        p.thickness,
        crest_r,
        circumference,
        l.openings.len(),
        l.raised.len(),
        clock.elapsed().as_secs_f64(),
        l.openings.iter().map(|o| o.outline.len()).max().unwrap_or(0)
    );
    for floor in 2..=LEVELS {
        println!("  to level {floor}: {} pockets", l.openings.iter().filter(|o| o.floor == floor).count());
    }
    if let Some(want) = std::env::var_os("CB_LIST") {
        let want = want.to_string_lossy().to_string();
        for o in l.openings.iter().filter(|o| o.name.starts_with(&want)) {
            println!("  {:<34} to {}  at ({:+.3}, {:+.3})  {} pts  area {:.4}", o.name, o.floor, o.at[0], o.at[1], o.outline.len(), field::area(&o.outline));
        }
    }
    layout::write_png(&out.join("layout.png"), &l, half, -0.5 * circumference, 0.5 * circumference, 40.0)?;
    for sec in l.facade.sections.iter().filter(|s| !s.name.starts_with("Buttress")) {
        let slug = sec.name.to_lowercase().replace(' ', "-");
        layout::write_png(&out.join(format!("layout-{slug}.png")), &l, half, sec.x0 - 1.2, sec.x1 + 1.2, 160.0)?;
    }
    if layout_only {
        return Ok(());
    }
    d.stamps = stamps(&d, &l, &p);
    ensure!(d.stamps.iter().all(|s| s.outline.len() <= ringdesign_core::setting::MAX_STAMP_POINTS), "an outline carries too many points");
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    let clock = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, draft_params())?;
    println!("draft build: {} triangles in {:.1} s", built.mesh.faces.len(), clock.elapsed().as_secs_f64());
    let g = gates(&d, &lib, &built, 0.15);
    std::fs::write(out.join("gates-draft.json"), serde_json::to_string_pretty(&g)?)?;
    println!("{}", serde_json::to_string(&g)?);
    renders(&out, &d, &built, &p, &l, 1600)?;
    std::fs::write(out.join("params.json"), serde_json::to_string_pretty(&p)?)?;
    let designs = library::default_design_dir().join("commissions");
    std::fs::create_dir_all(&designs)?;
    library::save_design(designs.join(format!("{DESIGN_FILE}.ring.json")), &d)?;
    library::save_design(out.join("design.ring.json"), &d)?;
    // The saved file reopened cold with a fresh library builds the same metal.
    let reopened = library::load_design(out.join("design.ring.json"))?;
    let mut cold = AlphaLibrary::builtin();
    reopened.bake_all(&mut cold);
    let again = mesh::try_build(&reopened, &cold, draft_params())?;
    let same = again.mesh.vertices == built.mesh.vertices && again.mesh.faces == built.mesh.faces;
    println!("cold reload: {} ({} stamps, {:.3} mm3 against {:.3})", if same { "identical" } else { "DIFFERENT" }, reopened.stamps.len(), again.report.volume_mm3, built.report.volume_mm3);
    ensure!(same, "the saved design does not rebuild the same metal");
    if export {
        let clock = std::time::Instant::now();
        let built = mesh::try_build(&d, &lib, export_params())?;
        println!("export build: {} triangles in {:.1} s", built.mesh.faces.len(), clock.elapsed().as_secs_f64());
        let g = gates(&d, &lib, &built, 0.15);
        std::fs::write(out.join("gates-export.json"), serde_json::to_string_pretty(&g)?)?;
        println!("{}", serde_json::to_string(&g)?);
        let size = format!("{}", p.size).replace('.', "_");
        stl::write_stl(out.join(format!("cathedral-facade-size{size}.stl")), &built.mesh, NAME)?;
    }
    Ok(())
}
