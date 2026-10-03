//! Officina — Fenestra, windows along the pull: a basket solitaire, an oval 7 × 5 laid along the ring, whose swelling
//! shoulders are pierced right through their side faces by drop windows cut along the finger, three down each shoulder and
//! mirrored to the other. Lost wax.
//! cargo build --release -p ringdesign-core --example officina_fenestra
//! target/release/examples/officina_fenestra [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    profile::ShankKey,
    cad::{
        self, Document, Feature, FeatureStatus, Operation, PatternKind, Placement,
        builders::{self, cutters},
    },
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    sketch::Id,
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const BORE_MM: f64 = 18.2;
const WIDTH_MM: f64 = 3.2;
const THICKNESS_MM: f64 = 2.4;
/// The shoulders' swell, as stations round the ring (theta, width scale, thickness scale), mirrored about the head: one
/// continuous taper from the head, where the band carries the basket, down to the band's own at the sides, the side faces
/// leaning by a near-steady angle down the shoulders where the windows go.
const SWELL: &[(f64, f64, f64)] = &[(90.0, 1.88, 1.72), (80.0, 1.68, 1.66), (68.0, 1.53, 1.58), (55.0, 1.49, 1.51), (40.0, 1.443, 1.44), (26.0, 1.40, 1.33), (10.0, 1.15, 1.14), (-30.0, 1.0, 1.0)];
const ALLOY: &str = "Gold 18k";
/// The stone: an oval 7 × 5 laid along the ring.
const STONE_L_MM: f64 = 7.0;
const STONE_W_MM: f64 = 5.0;
/// The oval's preview colour: a cornflower sapphire.
const STONE_TINT: [f32; 3] = [0.16, 0.3, 0.78];
/// The first window's centre, degrees round the ring (90 is the head); the array steps down the shoulder from it.
const FIRST_WINDOW_DEG: f64 = 63.0;
/// Windows down each shoulder and the angle they step by.
const WINDOWS: u32 = 3;
const WINDOW_STEP_DEG: f64 = 15.0;
/// A window's length round the ring and its width across the wall, and the bright cut round its mouth, mm.
const WINDOW_L_MM: f64 = 2.0;
const WINDOW_W_MM: f64 = 1.4;
const WINDOW_CHAMFER_MM: f64 = 0.15;
/// Which way the drop's point turns in the window's plane, degrees: 180 points it up the shoulder at the head.
const WINDOW_TURN_DEG: f64 = 180.0;
/// How far the stone sits below the basket's own stand-off, so the base rail is sunk into the collar and the claws rise out of
/// metal, mm.
const STONE_SINK_MM: f64 = 0.9;
/// The band's edge round, which rolls the light off every arris, mm.
const EDGE_ROUND_MM: f64 = 0.45;
/// The comfort roll on the bore's edges, which rounds the inner arrises as the edge round does the outer, mm.
const COMFORT_MM: f64 = 0.15;
/// The basket's claw and rail wire, mm.
const WIRE_MM: f64 = 1.4;
/// The windows' centre radius, mid-wall down the shoulders, mm.
const WINDOW_R_MM: f64 = 10.8;
/// The windows' lean along the ring, matching the side faces' gentle taper down the shoulders so the far mouth meets its face
/// square; the near mouth carries the bright cut, degrees.
const WINDOW_LEAN_DEG: f64 = -1.5;
/// Least metal the investment fills, mm.
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
    s.recipe.name = format!("Fenestra / investment / {ALLOY}");
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).unwrap().shrink_pct;
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.5, 0.0], end: [0.0, -19.0, 0.0], diameter_mm: 3.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -19.0, 0.0], end: [0.0, -29.0, 0.0], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Investment cast in one piece with the basket, sprued at the palm. Flush investment from the six windows with water under pressure. Polish the band, the basket and the windows' bright cuts; set the oval in the four claws.".into();
    s
}

/// The band: Flat 3.2 × 2.4 with squared side faces, swelling on the shoulders toward the head.
fn band() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Fenestra".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = WIDTH_MM;
    d.profile.thickness_mm = THICKNESS_MM;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = EDGE_ROUND_MM;
    d.profile.comfort_fit_mm = COMFORT_MM;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = SWELL
        .iter()
        .flat_map(|&(theta, w, t)| {
            let key = |theta_deg: f64| ShankKey { theta_deg: theta_deg.rem_euclid(360.0), width_scale: w, thickness_scale: t, ..Default::default() };
            let mirror = 180.0 - theta;
            if (mirror - theta).abs() < 1e-9 || (mirror - theta).rem_euclid(360.0) < 1e-9 { vec![key(theta)] } else { vec![key(theta), key(mirror)] }
        })
        .collect();
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.build = draft_params();
    d.manufacturing = Some(setup());
    d
}

fn gem() -> Gem {
    Gem { l_mm: STONE_L_MM, preview_tint: Some(STONE_TINT), ..Gem::calibrated(GemCut::Oval, STONE_W_MM) }
}

fn named(mut f: Feature, name: &str) -> Feature {
    f.name = name.into();
    f
}

/// The feature tree, in the order the timeline teaches it.
fn author() -> Result<RingDesign> {
    let mut d = band();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Band: Flat 3.2 × 2.4, its shoulders swelling to the head".into(), enabled: true, operation: Operation::Band, component: Default::default() })?;
    let g = gem();
    let at = Placement::ring(90.0, builders::stand_off_mm("basket", g) - STONE_SINK_MM);
    doc.append(named(builders::stone_feature(2, g, at), "Oval 7 × 5, laid along the ring"))?;
    let mut basket = builders::feature_on(3, "Basket head: paired claws, two rails", builders::BASKET, 2, json!({"prongs": 4, "rails": 2, "wire_mm": WIRE_MM, "grouping": "Jaws"}));
    // A made basket head, soldered to the cast shank at the bench and set there: its claws are notched for the girdle, finer
    // than the investment's section, so it is not poured with the shank.
    basket.component.stage = cad::Stage::Bench;
    doc.append(basket)?;
    // The seat is cut with a bur at the bench once the ring is cast, as it is when the stone is set: it notches the claws and
    // opens the pilot to the finger, so the pour fills the claws whole.
    let mut bur = builders::feature_on(4, "Seat bur, through to the finger, cut at the bench", builders::BUR, 2, json!({"through": true}));
    bur.component.stage = cad::Stage::Bench;
    doc.append(bur)?;
    d.cad = Some(doc);
    // The first window, sized from the side face where it sits, then set to the lesson's drop.
    let (p, n) = side_face_point(&d, FIRST_WINDOW_DEG);
    let at = cutters::pierce_at(&d, FIRST_WINDOW_DEG, 0.0, Some((p, n)), cutters::Shape::Drop)?;
    let mut w = cutters::pierce_feature(5, cutters::Shape::Drop, &at);
    // Stood on the high side face itself, so the ring array reseats each copy on the face at its own angle and every window
    // keeps its bright cut as the band tapers; leaned to the taper, so the far mouth meets its face square.
    w.component.placement = Placement::Side { theta_deg: FIRST_WINDOW_DEG, radius_mm: WINDOW_R_MM, face: ringdesign_core::field::SideFacePick::High, height_mm: 0.0, spin_deg: 0.0, tilt_deg: WINDOW_LEAN_DEG };
    w.name = "Drop window, through both side faces along the finger".into();
    if let Operation::Builder { params, .. } = &mut w.operation {
        params["length_mm"] = json!(WINDOW_L_MM);
        params["width_mm"] = json!(WINDOW_W_MM);
        params["chamfer_mm"] = json!(WINDOW_CHAMFER_MM);
        params["turn_deg"] = json!(WINDOW_TURN_DEG);
    }
    let doc = d.cad.as_mut().unwrap();
    doc.append(w)?;
    let span = -WINDOW_STEP_DEG * (WINDOWS - 1) as f64;
    doc.append(pattern(6, "Array the window down the shoulder", 5, PatternKind::Ring { count: WINDOWS, span_deg: span }))?;
    let mirror = || PatternKind::Mirror { plane: cad::MirrorPlane::Section { theta_deg: 90.0 } };
    doc.append(pattern(7, "Mirror the first window to the other shoulder", 5, mirror()))?;
    doc.append(pattern(8, "Mirror the array to the other shoulder", 6, mirror()))?;
    Ok(d)
}

fn pattern(id: Id, name: &str, source: Id, kind: PatternKind) -> Feature {
    let mut f = Feature { id, name: name.into(), enabled: true, operation: Operation::Pattern { sources: cad::pattern::Sources(vec![source]), kind }, component: Default::default() };
    f.component.attach = cad::Attach::Cut;
    f
}

/// A point mid-wall on the +Z side face at `theta_deg`, and its normal.
fn side_face_point(d: &RingDesign, theta_deg: f64) -> ([f64; 3], [f64; 3]) {
    let r_in = d.inner_radius_mm();
    let m = mesh::try_build(&band(), &AlphaLibrary::builtin(), draft_params()).expect("bare band");
    // The bare band's outer radius at theta, read from the mesh near the mid-plane.
    let t = theta_deg.to_radians();
    let r_out = m
        .mesh
        .vertices
        .iter()
        .filter(|v| (v.1 as f64).atan2(v.0 as f64).to_degrees().sub_abs(theta_deg) < 0.5)
        .map(|v| (v.0 as f64).hypot(v.1 as f64))
        .fold(r_in, f64::max);
    let r = 0.5 * (r_in + r_out);
    let z = m.mesh.vertices.iter().map(|v| v.2 as f64).fold(0.0, f64::max);
    let _ = d;
    ([r * t.cos(), r * t.sin(), z], [0.0, 0.0, 1.0])
}

trait SubAbs {
    fn sub_abs(self, o: f64) -> f64;
}
impl SubAbs for f64 {
    fn sub_abs(self, o: f64) -> f64 {
        (self - o).abs()
    }
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
}

/// Views: yaw about the finger axis, pitch from looking along the finger (0) to down onto the head (pi/2).
const VIEWS: &[(&str, f64, f64)] = &[
    ("hero", -0.7, 0.5),
    ("face", 0.0, FACE_PITCH),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.0, 0.35),
    ("reverse", PI - 0.5, 0.35),
];

/// The face view, tipped a little off square so the near side face's windows show beside the head.
const FACE_PITCH: f64 = 1.25;
/// The timeline's camera: three-quarters from a little under the band, so it sees the seat bur's pilot in the bore as well as
/// the windows in the side face.
const TIMELINE_VIEW: (f64, f64) = (-0.75, -0.32);

/// The stones close-up: the head and the top windows from a three-quarter view.
const STONES_VIEW: (f64, f64) = (-0.45, 0.7);

fn view(name: &str) -> (f64, f64) {
    let v = VIEWS.iter().find(|v| v.0 == name).unwrap();
    (v.1, v.2)
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

/// The renderer's empty background, which [`shot`] replaces with the studio's daylight backdrop.
const VOID: u8 = 18;

/// A supersampled frame on a light backdrop, so the windows show daylight through them: the parts drawn at `ss` times the
/// size, every untouched pixel given the backdrop's soft top-to-bottom fall, then box-filtered down.
fn shot(parts: &[render::Part], yaw: f64, pitch: f64, w: usize, h: usize, ss: usize) -> Vec<u8> {
    shot_framed(parts, yaw, pitch, None, w, h, ss)
}

/// [`shot`] framed on a point, the whole ring drawn.
fn shot_framed(parts: &[render::Part], yaw: f64, pitch: f64, framing: Option<render::Framing>, w: usize, h: usize, ss: usize) -> Vec<u8> {
    let (bw, bh) = (w * ss, h * ss);
    let mut big = match framing {
        Some(f) => render::render_parts_framed(parts, yaw, pitch, f, bw, bh, 1),
        None => render::render_parts_ss(parts, yaw, pitch, bw, bh, 1),
    };
    for y in 0..bh {
        let t = y as f64 / (bh - 1).max(1) as f64;
        let grey = [206.0 - 30.0 * t, 204.0 - 30.0 * t, 200.0 - 30.0 * t];
        for x in 0..bw {
            let k = (y * bw + x) * 3;
            if big[k] == VOID && big[k + 1] == VOID && big[k + 2] == VOID {
                for c in 0..3 {
                    big[k + c] = grey[c] as u8;
                }
            }
        }
    }
    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            for c in 0..3 {
                let mut sum = 0usize;
                for dy in 0..ss {
                    for dx in 0..ss {
                        sum += big[((y * ss + dy) * bw + x * ss + dx) * 3 + c] as usize;
                    }
                }
                out[(y * w + x) * 3 + c] = (sum / (ss * ss)) as u8;
            }
        }
    }
    out
}

fn save(path: &Path, parts: &[render::Part], yaw: f64, pitch: f64, edge: usize) -> Result<()> {
    image::save_buffer(path, &shot(parts, yaw, pitch, edge, edge, 3), edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Text drawn into an RGB image, dark on the light backdrop.
fn label(img: &mut [u8], w: usize, h: usize, x0: usize, y0: usize, text: &str, px: f32) {
    static FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
    let font = FONT.get_or_init(|| {
        fontdue::Font::from_bytes(&include_bytes!("../../../assets/fonts/EBGaramond.ttf")[..], fontdue::FontSettings::default()).unwrap()
    });
    let mut x = x0 as f32;
    for ch in text.chars() {
        let (m, bm) = font.rasterize(ch, px);
        let top = y0 as i64 + px as i64 - m.height as i64 - m.ymin as i64;
        for j in 0..m.height {
            for i in 0..m.width {
                let (px_x, px_y) = (x as i64 + m.xmin as i64 + i as i64, top + j as i64);
                if px_x < 0 || px_y < 0 || px_x as usize >= w || px_y as usize >= h {
                    continue;
                }
                let a = bm[j * m.width + i] as f32 / 255.0;
                let k = (px_y as usize * w + px_x as usize) * 3;
                for c in 0..3 {
                    img[k + c] = (img[k + c] as f32 * (1.0 - a) + 40.0 * a) as u8;
                }
            }
        }
        x += m.advance_width;
    }
}

/// One tile per feature: the ring rolled back to it, with its stones, and the feature's number and name.
fn timeline(out: &Path, d: &RingDesign, lib: &AlphaLibrary) -> Result<()> {
    let doc = d.cad.as_ref().unwrap();
    let (tile, cols) = (420usize, 4usize);
    let rows = doc.features.len().div_ceil(cols);
    let (w, h) = (tile * cols, tile * rows);
    let mut sheet = vec![200u8; w * h * 3];
    let (yaw, pitch) = TIMELINE_VIEW;
    for (k, f) in doc.features.iter().enumerate() {
        let mut step = d.clone();
        step.cad.as_mut().unwrap().through = Some(f.id);
        let fin = render::finished(&step, lib, draft_params())?;
        let (bright, dark) = window_walls(&fin.metal);
        let img = shot(&dressed(&bright, &dark, &fin), yaw, pitch, tile, tile, 2);
        let (cx, cy) = (k % cols * tile, k / cols * tile);
        for y in 0..tile {
            sheet[((cy + y) * w + cx) * 3..((cy + y) * w + cx + tile) * 3].copy_from_slice(&img[y * tile * 3..(y + 1) * tile * 3]);
        }
        label(&mut sheet, w, h, cx + 10, cy + 8, &format!("{}  {}", f.id, f.name), 17.0);
    }
    image::save_buffer(out.join("timeline.png"), &sheet, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The metal split for the back light: the windows' walls apart from the polished rest.
fn window_walls(m: &mesh::Mesh) -> (mesh::Mesh, mesh::Mesh) {
    let wall = |f: &[u32; 3]| {
        let from_part = f.iter().all(|&i| m.origin.get(i as usize).is_some_and(|&o| o >= mesh::SOLID_VERTEX));
        let Some((a, b, c)) = m.triangle(f) else { return false };
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let theta = ((a[1] + b[1] + c[1]) / 3.0).atan2((a[0] + b[0] + c[0]) / 3.0).to_degrees();
        let r = ((a[0] + b[0] + c[0]) / 3.0).hypot((a[1] + b[1] + c[1]) / 3.0);
        from_part && l > 1e-12 && (n[2] / l).abs() < 0.5 && (theta - 90.0).abs() > 14.0 && r < 11.8
    };
    let mut bright = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..mesh::Mesh::default() };
    let mut dark = bright.clone();
    let corners: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    for (i, f) in m.faces.iter().enumerate() {
        let target = if wall(f) { &mut dark } else { &mut bright };
        if let Some(c) = corners.get(&(i as u32)) {
            target.corner_normals.push((target.faces.len() as u32, *c));
        }
        target.faces.push(*f);
    }
    (bright, dark)
}
/// The windows' walls in the back light: a pale warm daylight, bright enough that even a wall turned from the key reads lit.
const BACKLIT: [f32; 3] = [3.6, 3.5, 3.3];

/// The windows' walls under the studio's back light, which shows through the shoulders: bright and soft.
fn backlit(m: &mesh::Mesh) -> render::Part<'_> {
    let mut p = render::Part::metal(m, BACKLIT);
    p.roughness = render::POLISHES[2].1 as f64;
    p.studio = false;
    p
}

fn dressed<'a>(bright: &'a mesh::Mesh, dark: &'a mesh::Mesh, fin: &'a render::Finished) -> Vec<render::Part<'a>> {
    let mut parts = vec![render::Part::metal(bright, render::GOLD), backlit(dark)];
    parts.extend(fin.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    parts
}

fn renders(out: &Path, lib: &AlphaLibrary, fin: &render::Finished, edge: usize) -> Result<()> {
    let (bright, dark) = window_walls(&fin.metal);
    if std::env::var("FENESTRA_DEBUG").is_ok() {
        let parted = fin.metal.origin.iter().filter(|&&o| o >= mesh::SOLID_VERTEX).count();
        println!("    window walls: {} faces of {}; {} of {} vertices from parts", dark.faces.len(), fin.metal.faces.len(), parted, fin.metal.origin.len());
    }
    let parts = dressed(&bright, &dark, fin);
    if let Ok(list) = std::env::var("FENESTRA_TRY") {
        for (k, yp) in list.split(';').enumerate() {
            let v: Vec<f64> = yp.split(',').filter_map(|x| x.parse().ok()).collect();
            save(&out.join(format!("try-{k}.png")), &parts, v[0], v[1], 300)?;
        }
    }
    for (name, yaw, pitch) in VIEWS {
        save(&out.join(format!("{name}.png")), &parts, *yaw, *pitch, edge)?;
    }
    for name in ["hero", "face"] {
        let (yaw, pitch) = view(name);
        save(&out.join(format!("{name}-300.png")), &parts, yaw, pitch, 300)?;
    }
    // Stones: the head and the first windows, framed on the metal within 35° of the top.
    // Stones: the whole ring framed on the stone, so the head and the top windows fill the picture.
    let (mut c, mut n) = ([0.0f64; 3], 0usize);
    for (m, _) in &fin.stones {
        for v in &m.vertices {
            c = [c[0] + v.0 as f64, c[1] + v.1 as f64, c[2] + v.2 as f64];
            n += 1;
        }
    }
    let centre = if n > 0 { c.map(|x| x / n as f64) } else { [0.0, 13.0, 0.0] };
    let close = shot_framed(&parts, STONES_VIEW.0, STONES_VIEW.1, Some(render::Framing::new([centre[0], centre[1] - 1.5, centre[2]], 6.5)), edge, edge, 3);
    image::save_buffer(out.join("stones.png"), &close, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (yaw, pitch) = view("hero");
    let bare_img = shot(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = shot(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    let tiles: Vec<Vec<u8>> = ["hero", "face", "side", "palm"]
        .iter()
        .map(|n| {
            let (yaw, pitch) = view(n);
            shot(&parts, yaw, pitch, 300, 300, 3)
        })
        .collect();
    let mut sheet = vec![0u8; 1200 * 300 * 3];
    for (k, t) in tiles.iter().enumerate() {
        for y in 0..300 {
            sheet[y * 3600 + k * 900..y * 3600 + (k + 1) * 900].copy_from_slice(&t[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 1200, 300, image::ColorType::Rgb8)?;
    Ok(())
}

/// The faces of `m` whose every corner `keep` keeps, as a mesh of their own.
fn crop(m: &mesh::Mesh, keep: impl Fn([f64; 3]) -> bool) -> mesh::Mesh {
    let at = |i: u32| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    };
    let mut c = mesh::Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| keep(at(i)))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                c.vertices.push(m.vertices[i as usize]);
                c.normals.push(m.normals[i as usize]);
                (c.vertices.len() - 1) as u32
            })
        });
        c.faces.push(g);
    }
    c
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina/fenestra"));
    std::fs::create_dir_all(&out)?;
    println!("Fenestra");
    let mut d = author()?;
    if let Ok(off) = std::env::var("FENESTRA_OFF") {
        for id in off.split(',').filter_map(|v| v.parse::<Id>().ok()) {
            if let Some(f) = d.cad.as_mut().unwrap().features.iter_mut().find(|f| f.id == id) {
                f.enabled = false;
            }
        }
    }
    let lib = AlphaLibrary::builtin();
    let params = if draft { draft_params() } else { export_params() };
    d.build = params;
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    println!("  {} triangles in {build_s:.1} s; watertight {}; degenerate {}", built.mesh.faces.len(), v.watertight, q.degenerate_faces);
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
    let made: Vec<(String, usize)> =
        built.parts.evaluated.iter().flat_map(|e| e.components.iter()).map(|c| (c.name.clone(), csg::self_crossings(&solid_of(&c.mesh)))).collect();
    if std::env::var("FENESTRA_DEBUG").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            if c.id == 3 {
                let all = thin_rays_every(&c.mesh, MIN_SECTION_MM, 1);
                let worst = all.iter().map(|x| x.0).fold(f64::MAX, f64::min);
                println!("    basket every face: {} of {} below, worst {worst:.3}", all.len(), c.mesh.faces.len());
                for (v, c, _) in all.iter().filter(|x| x.0 < 0.7).take(12) {
                    println!("      {v:.3} at r {:.2} θ {:.1} z {:.2}", c[0].hypot(c[1]), c[1].atan2(c[0]).to_degrees(), c[2]);
                }
            }
            let at = t.point.map(|p| (p[0].hypot(p[1]), p[1].atan2(p[0]).to_degrees(), p[2]));
            println!("    part {} {}: {} faces, bounds {:?}; thickness min {:?} at {at:?}, {} below", c.id, c.name, c.mesh.faces.len(), c.mesh.bounds(), t.sampled_min_mm, t.below_limit);
        }
    }
    let ring_crossings = csg::self_crossings(&solid_of(&built.mesh));
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01).count();
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let field = castability::judged_field_report(&d, &lib, &d.draft, 256, 128, Some(&built));
    let findings = dfm::findings_in(&d, &lib);
    let lands = dfm::cut_lands(&d, &built, MIN_SECTION_MM);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    // Wall thickness is screened on a build light enough for the measure.
    let coarse = BuildParams { theta_steps: 384, profile_steps: 160, ..params };
    // The section floor is the investment's: it is measured on what is poured, the shank with its windows. The basket head and
    // the seat are the bench's, soldered and cut after the pour, so they are suppressed here and their own sections recorded.
    let mut poured = d.clone();
    for f in poured.cad.as_mut().unwrap().features.iter_mut() {
        if f.component.stage == cad::Stage::Bench {
            f.enabled = false;
        }
    }
    let thin_mesh = mesh::try_build_pattern(&poured, &lib, coarse)?.mesh;
    let head_thin = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .find(|c| c.id == 3)
        .map(|c| cad::measure::thickness(&c.mesh, MIN_SECTION_MM));
    let finished_thin = cad::measure::thickness(&mesh::try_build(&d, &lib, coarse)?.mesh, MIN_SECTION_MM);
    let thickness = cad::measure::thickness(&thin_mesh, MIN_SECTION_MM);
    println!(
        "  thickness at {} x {} ({} faces): {} rays, min {:?}, {} below {:.1}, {} unresolved at {:?}",
        coarse.theta_steps,
        coarse.profile_steps,
        thin_mesh.faces.len(),
        thickness.rays,
        thickness.sampled_min_mm,
        thickness.below_limit,
        MIN_SECTION_MM,
        thickness.unresolved,
        thickness.point.map(|p| (p[0].hypot(p[1]), p[1].atan2(p[0]).to_degrees(), p[2]))
    );
    if let Ok(t) = std::env::var("FENESTRA_FACE") {
        let t: f64 = t.parse()?;
        let b = mesh::try_build(&band(), &lib, draft_params())?.mesh;
        let mut bins = std::collections::BTreeMap::new();
        for v in &b.vertices {
            let th = (v.1 as f64).atan2(v.0 as f64).to_degrees();
            if (th - t).abs() < 0.3 && v.2 > 0.0 {
                let r = (v.0 as f64).hypot(v.1 as f64);
                let e = bins.entry((r * 5.0) as i64).or_insert(f64::MIN);
                *e = f64::max(*e, v.2 as f64);
            }
        }
        for (r, z) in bins {
            println!("    face r {:.1}: z {z:.3}", r as f64 / 5.0);
        }
    }
    if std::env::var("FENESTRA_HEAD").is_ok() {
        let head = crop(&thin_mesh, |p| p[1] > 11.0 && (p[1].atan2(p[0]).to_degrees() - 90.0).abs() < 25.0);
        let all = thin_rays_every(&head, MIN_SECTION_MM, 1);
        println!("    head every face: {} of {} below", all.len(), head.faces.len());
        let shoulders = crop(&thin_mesh, |p| { let t = (p[1].atan2(p[0]).to_degrees() - 90.0).abs(); (12.0..70.0).contains(&t) && p[1] > 0.0 });
        let mut sh: Vec<_> = thin_rays_every(&shoulders, MIN_SECTION_MM, 1).into_iter().filter(|x| x.0 > 0.05).collect();
        sh.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!("    shoulders every face: {} of {} below past edge slivers", sh.len(), shoulders.faces.len());
        for (v, c, n) in sh.iter().step_by((sh.len() / 14).max(1)) {
            println!("      {v:.3} at r {:.2} θ {:.1} z {:.2} in {:?}", c[0].hypot(c[1]), c[1].atan2(c[0]).to_degrees(), c[2], n.map(|x| (x * 100.0).round() / 100.0));
        }
        let claws: Vec<_> = all.iter().filter(|x| x.1[0].hypot(x.1[1]) > 13.8 && x.0 > 0.05).collect();
        let worst = claws.iter().map(|x| x.0).fold(f64::MAX, f64::min);
        println!("    claws (r > 13.8, past edge slivers): {} below, worst {worst:.3}", claws.len());
        let mut hist = [0usize; 8];
        for c in &claws {
            hist[((c.1[0].hypot(c.1[1]) - 13.8) / 0.25).floor().clamp(0.0, 7.0) as usize] += 1;
        }
        println!("    by radius from 13.8 in 0.25 steps: {hist:?}");
        for c in claws.iter().step_by(37) {
            println!("      {:.3} at r {:.2} θ {:.1} z {:.2} in {:?}", c.0, c.1[0].hypot(c.1[1]), c.1[1].atan2(c.1[0]).to_degrees(), c.1[2], c.2.map(|x| (x * 100.0).round() / 100.0));
        }
        let mut sorted = all.clone();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (v, c, n) in sorted.iter().take(14) {
            println!("      {v:.3} at r {:.2} θ {:.1} z {:.2} in {:?}", c[0].hypot(c[1]), c[1].atan2(c[0]).to_degrees(), c[2], n.map(|x| (x * 100.0).round() / 100.0));
        }
    }
    if std::env::var("FENESTRA_DEBUG").is_ok() {
        for (v, c, n) in thin_rays(&thin_mesh, MIN_SECTION_MM) {
            println!("    thin {v:.3} mm at r {:.2} θ {:.1} z {:.2}, inward {:?}", c[0].hypot(c[1]), c[1].atan2(c[0]).to_degrees(), c[2], n.map(|x| (x * 100.0).round() / 100.0));
        }
    }
    let grams = built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams);
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let pattern_ok = pattern.report.validation.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0;
    let stone_count = stones_report.as_ref().map_or(0, |s| s.stone_count) as usize;
    let crowding = stones_report.as_ref().and_then(|s| s.crowding_note());
    let gates = json!({
        "watertight": v.watertight,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "made_part_self_crossings": made,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "features": features.iter().map(|(id, n, s)| json!({"id": id, "name": n, "status": s})).collect::<Vec<_>>(),
        "bore_radius_mm": bore,
        "min_vertex_radius_mm": min_r,
        "vertices_inside_bore": inside,
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "investment_min_section_mm": d.draft.min_section_mm,
        "thickness_finished_informational": {"rays": finished_thin.rays, "sampled_min_mm": finished_thin.sampled_min_mm, "below_limit": finished_thin.below_limit, "note": "The finished ring after the bench's seat bur, which notches the claws for the girdle and opens the pilot; not the pour."},
        "head_thickness_informational": head_thin.as_ref().map(|t| json!({"rays": t.rays, "sampled_min_mm": t.sampled_min_mm, "below_limit": t.below_limit, "note": "The made basket head alone: each claw notched by the stone's envelope for the girdle, set at the bench."})),
        "thickness": {"mesh": "the poured shank: the casting pattern with the bench's basket head and seat bur suppressed", "build": [coarse.theta_steps, coarse.profile_steps], "faces": thin_mesh.faces.len(), "limit_mm": thickness.limit_mm, "rays": thickness.rays, "sampled_min_mm": thickness.sampled_min_mm, "below_limit": thickness.below_limit, "unresolved": thickness.unresolved, "note": thickness.note},
        "cut_lands": lands.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones_reported": stone_count,
        "stones_previewed": previewed,
        "crowding": crowding,
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
        && thickness.rays > 0
        && thickness.below_limit == 0
        && thickness.unresolved == 0
        && lands.is_empty()
        && findings.is_empty()
        && stone_count == previewed
        && cold != Some(false)
        && built.mesh.faces.len() <= 2_000_000
        && pattern_ok;
    let block = json!({"build": [params.theta_steps, params.profile_steps], "build_s": build_s, "gates": gates, "gates_passed": passed});
    let report_path = out.join("report.json");
    let mut report: serde_json::Value = std::fs::read(&report_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    report["name"] = json!(d.name);
    report["process"] = json!(d.draft.process.label());
    report["alloy"] = json!(ALLOY);
    report["size"] = json!(d.size.display());
    report["bore_mm"] = json!(built.report.inner_diameter_mm);
    report["band"] = json!({"style": "Flat", "width_mm": WIDTH_MM, "thickness_mm": THICKNESS_MM, "shank": "Keyframes", "swell_keys": SWELL});
    report["stone"] = json!({"cut": "Oval", "l_mm": STONE_L_MM, "w_mm": STONE_W_MM, "spin_deg": 0.0, "spin_note": "spin 0 lays a stone's length round the ring (cad.rs stone_frame turns the seat frame a quarter); the section's spin 90 would lay the oval across the finger."});
    report["windows"] = json!({"per_shoulder": WINDOWS, "first_deg": FIRST_WINDOW_DEG, "step_deg": WINDOW_STEP_DEG, "length_mm": WINDOW_L_MM, "width_mm": WINDOW_W_MM, "chamfer_mm": WINDOW_CHAMFER_MM});
    report["grams_gold_18k"] = json!(grams);
    report["design_bytes"] = json!(design_bytes);
    report["cad_features"] = json!(d.cad.as_ref().map_or(0, |c| c.features.len()));
    report["draft_settings"] = json!(d.draft);
    report[if draft { "draft" } else { "export" }] = block;
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    let fin = render::finished_from(&d, &lib, built);
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &fin.metal, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        for (k, (m, _)) in fin.stones.iter().enumerate() {
            stl::write_stl(out.join(format!("reference-oval-{}.stl", k + 1)), m, "Oval 7 x 5")?;
        }
    }
    if let Some(s) = &stones_report {
        let g = gem();
        let stones = json!({
            "stone_count": s.stone_count,
            "total_carats": s.total_carats,
            "stones": [{"name": builders::gem_label(g), "cut": "Oval", "l_mm": g.l_mm, "w_mm": g.w_mm, "depth_mm": g.depth_mm(), "theta_deg": 90.0, "spin_deg": 0.0, "setting": "Basket: four claws, two rails", "seat": "bur, through"}],
            "previewed": fin.stones.len(),
            "crowding": s.crowding_note(),
        });
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&stones)?)?;
    }
    renders(&out, &lib, &fin, if draft { 1000 } else { 1600 })?;
    if !args.iter().any(|a| a == "--no-timeline") {
        timeline(&out, &d, &lib)?;
    }
    println!("  field {} (lost wax: informational); dfm {}; lands {}; min r {:.3} vs bore {:.3}; {:.1} g 18k", field.verdict.label(), findings.len(), lands.len(), min_r, bore, grams);
    for f in findings.iter().chain(&lands) {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in built_notes(&report) {
        println!("    note: {n}");
    }
    println!("  gates {}", if passed { "passed" } else { "FAILED" });
    ensure!(passed || std::env::var("FENESTRA_ALLOW_FAIL").is_ok(), "Fenestra failed its gates; see {}", report_path.display());
    Ok(())
}

/// Every ray `cad::measure::thickness` samples that falls under `limit`: its length, start and inward direction.
fn thin_rays(m: &mesh::Mesh, limit: f64) -> Vec<(f64, [f64; 3], [f64; 3])> {
    thin_rays_every(m, limit, m.faces.len().div_ceil(384).max(1))
}

/// [`thin_rays`] from every `stride`-th face.
fn thin_rays_every(m: &mesh::Mesh, limit: f64, stride: usize) -> Vec<(f64, [f64; 3], [f64; 3])> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let tris: Vec<_> = m.faces.iter().filter_map(|f| m.triangle(f)).collect();
    let mut out = Vec::new();
    for (i, (a, b, c)) in tris.iter().enumerate().step_by(stride) {
        let n = cross(sub(*b, *a), sub(*c, *a));
        let l = dot(n, n).sqrt();
        if l < 1e-12 {
            continue;
        }
        let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let d = n.map(|v| -v / l);
        let mut best = f64::MAX;
        for (j, (p, q, r)) in tris.iter().enumerate() {
            if j == i {
                continue;
            }
            let (e1, e2) = (sub(*q, *p), sub(*r, *p));
            let h = cross(d, e2);
            let det = dot(e1, h);
            if det.abs() < 1e-12 {
                continue;
            }
            let s = sub(o, *p);
            let u = dot(s, h) / det;
            if !(-1e-8..=1.0 + 1e-8).contains(&u) {
                continue;
            }
            let qq = cross(s, e1);
            let v = dot(d, qq) / det;
            if v < -1e-8 || u + v > 1.0 + 1e-8 {
                continue;
            }
            let t = dot(e2, qq) / det;
            if t > 1e-5 {
                best = best.min(t);
            }
        }
        if best < limit {
            out.push((best, o, d));
        }
    }
    out
}

fn built_notes(report: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for b in ["draft", "export"] {
        for k in ["solids_notes", "parts_notes"] {
            if let Some(a) = report[b]["gates"][k].as_array() {
                out.extend(a.iter().filter_map(|s| s.as_str().map(String::from)));
            }
        }
    }
    out
}
