//! Cataphracta — Sphenodon, the parietal: a tuatara's serrated sail on the parting line, parting round a flush peridot on the spine, poured in Delft sand.
//! cargo build --release -p ringdesign-core --example cataphracta_sphenodon
//! target/release/examples/cataphracta_sphenodon [OUT_DIR] [--draft] [--verify] [--block-out]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, SandProcess, Verdict},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SeatPadLayer, SeatStyle, SideFacePick, Uv, VGate},
    gem::{Gem, GemCut},
    manufacturing as mf, mesh,
    profile::ShankKey,
    render::{self, Part},
    reptile::svg::{self as rsvg, Params},
    setting::SolidKind,
    skin::{self, Atlas},
    stl,
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade, TilingLayer, WarpField},
};
use serde_json::{Value, json};
use std::{
    f64::consts::PI,
    fmt::Write as _,
    path::{Path, PathBuf},
    time::Instant,
};

const BORE_MM: f64 = 18.6;
const STONE_DEG: f64 = 90.0;
const TEETH: u32 = 30;
const SAIL_MM: f64 = 2.35;
const SAIL_SPAN_MM: f64 = 1.2;
const SAIL_TAPER: f64 = 0.35;
/// The sail's mask: clear of the mound to this far either side of the stone, full height from `SAIL_FULL_DEG` to `SAIL_HOLD_DEG`.
const SAIL_CLEAR_DEG: f64 = 9.0;
const SAIL_FULL_DEG: f64 = 20.0;
const SAIL_HOLD_DEG: f64 = 45.0;
const SAIL_PALM: f64 = 0.3;
/// The share of the sail's height the teeth nearest the mound keep.
const SAIL_SKIRT: f64 = 0.45;
const HERO: (f64, f64) = (0.55, 0.95);
/// Across the sail's top, the flat the gable is opened to, mm.
const SAIL_PLATEAU_MM: f64 = 0.36;
/// The Flat style's crown exponent, opened from its 8 so the crown carries draft off the crest.
const CROWN_EXPONENT: f64 = 3.0;
/// The side faces' tubercles: two staggered rows, this far either side of the face's middle, at this pitch round the ring.
const HIDE_PITCH_MM: f64 = 1.2;
const TUBERCLE_ROW_MM: f64 = 0.5;
const TUBERCLE_D_MM: f64 = 0.74;
const TUBERCLE_MM: f64 = 0.55;
const VENTRAL_HALF_DEG: f64 = 35.0;
const VENTRAL_CELL_MM: f64 = 1.1;
const VENTRAL_MM: f64 = 0.45;
/// The dorsal scales on the crown: rows stepping down from the crest, each row a course of plates round the ring.
const SCALE_ROWS: usize = 4;
const SCALE_TOP_MM: f64 = 0.6;
const SCALE_PITCH_MM: f64 = 0.95;
const SCALE_GROOVE_MM: f64 = 0.36;
/// The crown's reach for the scales, mm of z from the crest: the sail's foot to short of the edge fillet.
const SCALE_FROM_MM: f64 = 0.6;
const SCALE_TO_MM: f64 = 3.45;
const AW: usize = 2048;
const AH: usize = 768;

fn params(draft: bool) -> BuildParams {
    let (t, p) = if draft { (768, 320) } else { (1536, 448) };
    BuildParams { theta_steps: t, profile_steps: p, refine: None, ..BuildParams::default() }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Degrees from `theta` to `centre` the short way round.
fn off(theta: f64, centre: f64) -> f64 {
    (theta - centre + 540.0).rem_euclid(360.0) - 180.0
}

/// The thickness-keyed Flat band in Delft sand: deep under the stone, the palm the reference and the tightest station.
fn band() -> RingDesign {
    let mut d = RingDesign { name: "Sphenodon \u{2014} the parietal".into(), ..RingDesign::default() };
    d.profile.width_mm = 7.5;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 1.2;
    d.profile.shape_a = CROWN_EXPONENT;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).expect("bore");
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale: 1.0, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(0.0, 1.05), key(90.0, 1.18), key(180.0, 1.05), key(270.0, 1.0)];
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    setup.recipe.name = format!("{} / Delft clay", d.name);
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").unwrap().shrink_pct;
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    let palm = d.inner_radius_mm() + d.profile.thickness_mm - 0.2;
    setup.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -palm, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -33.0, 0.0], diameter_mm: 6.0 },
    ];
    setup.bench_notes = "Procedural Flat band, thickness keyed deep under the stone. A tuatara's serrated sail stands on the parting line, \
        its teeth tallest over the shoulders and grading down to a low saw at the palm; it parts round a gypsy mound on the spine that \
        carries a 3.0 mm round peridot flush. Z=0 parting, opposed Z withdrawal. At the bench: drill from the raised dot and cut the flush \
        seat, set the peridot; polish the sail's teeth and the ribbon either side of it; leave the granular side faces satin."
        .into();
    d.draft.process = setup.recipe.process;
    d.draft.sand = setup.recipe.sand;
    d.draft.min_detail_mm = setup.recipe.min_detail_mm;
    d.draft.min_section_mm = setup.recipe.min_section_mm;
    d.draft.min_draft_deg = setup.recipe.min_draft_deg;
    d.manufacturing = Some(setup);
    d
}

fn peridot() -> Gem {
    Gem { preview_tint: Some([0.50, 0.78, 0.12]), ..Gem::calibrated(GemCut::Round, 3.0) }
}

/// A u-only mask: `f(theta)` as black ink over a thin strip, one stop per degree, the same down every row.
fn u_mask(f: impl Fn(f64) -> f64) -> String {
    let mut s = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="360" height="8" viewBox="0 0 360 8"><defs><linearGradient id="m" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="360" y2="0">"##);
    for k in 0..=360 {
        let t = k as f64;
        let _ = write!(s, r##"<stop offset="{:.5}" stop-color="#000" stop-opacity="{:.4}"/>"##, t / 360.0, f(t).clamp(0.0, 1.0));
    }
    s.push_str(r##"</linearGradient></defs><rect width="360" height="8" fill="url(#m)"/></svg>"##);
    s
}

/// The sail's height round the ring: low teeth at the mound's skirt, full over both shoulders, a low saw at the palm.
fn sail_height(theta: f64) -> f64 {
    let a = off(theta, STONE_DEG).abs();
    if a < SAIL_CLEAR_DEG {
        return 0.0;
    }
    let rise = (SAIL_SKIRT + (1.0 - SAIL_SKIRT) * smoothstep(SAIL_CLEAR_DEG, SAIL_FULL_DEG, a)) * smoothstep(SAIL_CLEAR_DEG, SAIL_CLEAR_DEG + 1.0, a);
    let fall = 0.5 + 0.5 * (PI * ((a - SAIL_HOLD_DEG).max(0.0) / (180.0 - SAIL_HOLD_DEG))).cos();
    rise * (SAIL_PALM + (1.0 - SAIL_PALM) * fall)
}

/// The crown's dorsal scales, mm over the crown at `along` mm round the ring and `z` mm across: courses of plates stepping down from the crest, the plates of each course staggered a half pitch from the last and parted by grooves no deeper than the step, so nothing rises walking away from the parting line.
fn dorsal_scales(along: f64, z: f64) -> f64 {
    let a = z.abs();
    if a >= SCALE_TO_MM {
        return 0.0;
    }
    let width = (SCALE_TO_MM - SCALE_FROM_MM) / SCALE_ROWS as f64;
    let k = if a < SCALE_FROM_MM { 0 } else { (((a - SCALE_FROM_MM) / width) as usize).min(SCALE_ROWS - 1) };
    let step = SCALE_TOP_MM / SCALE_ROWS as f64;
    let top = SCALE_TOP_MM - step * k as f64;
    // Round the ring: a plate with rounded shoulders, then a groove as deep as one step.
    let shift = if k % 2 == 1 { 0.5 * SCALE_PITCH_MM } else { 0.0 };
    let x = (along - shift).rem_euclid(SCALE_PITCH_MM);
    let from_joint = x.min(SCALE_PITCH_MM - x);
    let half = 0.5 * (SCALE_PITCH_MM - SCALE_GROOVE_MM);
    let plate = (1.0 - ((0.5 * SCALE_PITCH_MM - from_joint) / (half + 0.5 * SCALE_GROOVE_MM)).powi(2)).max(0.0).sqrt() * smoothstep(0.0, 0.5 * SCALE_GROOVE_MM + 0.05, from_joint);
    // Across, each plate eases down a little toward its outer edge, never below the next course.
    let row_from = if k == 0 { 0.0 } else { SCALE_FROM_MM + width * k as f64 };
    let row_to = SCALE_FROM_MM + width * (k + 1) as f64;
    let ease = 0.45 * step * smoothstep(row_from, row_to, a);
    top - step + (step - ease) * plate
}

struct Layers {
    names: Vec<String>,
    /// What the draft rule took from each painted layer as it was painted.
    bites: Vec<(String, skin::ClampReport)>,
}

/// The sail's tooth: the C-R7 generator round the ring, its gable opened to a plateau across the parting line so a mesh row either side of it cannot tip the apex off the plane.
fn sail_svg(pitch: f64) -> String {
    let svg = rsvg::sail(&Params::new(pitch, SAIL_SPAN_MM, 0.4, 1.0));
    let half = 0.5 * SAIL_PLATEAU_MM / SAIL_SPAN_MM;
    let peak = r##"<stop offset="0.5000" stop-color="#000" stop-opacity="1.0000"/>"##;
    assert!(svg.contains(peak), "the sail generator's gable changed");
    svg.replacen(peak, &format!(r##"<stop offset="{:.4}" stop-color="#000" stop-opacity="1.0000"/><stop offset="{:.4}" stop-color="#000" stop-opacity="1.0000"/>"##, 0.5 - half, 0.5 + half), 1)
}

/// The side faces' tile: two staggered rows of enlarged tubercles, `cols` to a row, spread across the face's height, each a dome.
fn hide_tile(w: f64, h: f64, cols: usize) -> String {
    let pitch = w / cols as f64;
    let mut body = String::new();
    let mut defs = String::from(r##"<radialGradient id="g" cx="0.5" cy="0.5" r="0.5">"##);
    for k in 0..=10 {
        let t = k as f64 / 10.0;
        let _ = write!(defs, r##"<stop offset="{:.3}" stop-color="#000" stop-opacity="{:.4}"/>"##, 0.35 + 0.65 * t, (1.0 - t * t).sqrt());
    }
    defs.push_str("</radialGradient>");
    // The 0.5 iso-line sits at 0.35 + 0.65 * sqrt(0.75) of the radius.
    let r = 0.5 * TUBERCLE_D_MM / (0.35 + 0.65 * 0.75f64.sqrt());
    for (j, y) in [0.5 * h - TUBERCLE_ROW_MM, 0.5 * h + TUBERCLE_ROW_MM].into_iter().enumerate() {
        for i in 0..cols {
            let x = (i as f64 + 0.25 + 0.5 * j as f64) * pitch;
            for dx in [-w, 0.0, w] {
                let cx = x + dx;
                if cx + r > 0.0 && cx - r < w {
                    let _ = write!(body, r##"<circle cx="{cx:.4}" cy="{y:.4}" r="{r:.4}" fill="url(#g)"/>"##);
                }
            }
        }
    }
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs>{defs}</defs>{body}</svg>"##)
}

/// The ventral field round the palm: full over 270 +- 35 deg, fading over 12 deg either side.
fn ventral(theta: f64) -> f64 {
    1.0 - smoothstep(VENTRAL_HALF_DEG, VENTRAL_HALF_DEG + 12.0, off(theta, 270.0).abs())
}

/// A tiling fitted to both side faces, graded about the stone.
fn side_tiling(alpha: &str, ctx: &ringdesign_core::FieldContext, height: f64, repeats: u32, taper: f64) -> Result<TilingLayer> {
    let mut t = TilingLayer::default_for(alpha, ctx);
    ensure!(t.fit_to_side_faces(ctx, SIDE_FACE_MIN_DRAFT_DEG), "the band has no side faces");
    t.height_mm = height;
    t.repeats_around = repeats;
    if taper > 0.0 {
        t.grade = Some(TileGrade { taper, theta_deg: STONE_DEG, law: GradeLaw::Cosine, isotropic: false });
    }
    Ok(t)
}

/// The design at `stage`: the block-out carries the band, the sail, the hide's rows and the stone.
fn design(block_out: bool) -> Result<(RingDesign, AlphaLibrary, Layers)> {
    let mut d = band();
    let ctx = d.field_context();
    let pitch = ctx.circumference_mm / TEETH as f64;
    d.svgs.push(SvgAlpha { name: "Sail".into(), svg: sail_svg(pitch), invert: false });
    d.svgs.push(SvgAlpha { name: "Sail height".into(), svg: u_mask(sail_height), invert: false });
    let mut t = TilingLayer::default_for("Sail", &ctx);
    t.repeats_around = TEETH;
    t.rows = 1;
    t.v_center_mm = ctx.crest_v_mm;
    t.v_span_mm = SAIL_SPAN_MM;
    t.feather_mm = 0.0;
    t.height_mm = SAIL_MM;
    t.offset_u = 0.5;
    t.grade = Some(TileGrade { taper: SAIL_TAPER, theta_deg: STONE_DEG, law: GradeLaw::Cosine, isotropic: false });
    let mut sail = LayerEntry::new("The sail", Layer::Tiling(t));
    sail.blend = Blend::Max;
    sail.mask = Some("Sail height".into());
    d.layers.layers.push(sail);

    // The crown's dorsal scales, painted on the bare band's atlas and held to the sand's draft rule.
    let a = Atlas::of(&d, AW, AH)?;
    let circ = ctx.circumference_mm;
    let mut alpha = a.paint("Dorsal scales", |s| dorsal_scales(s.theta / 360.0 * circ, s.p[2]) / SCALE_TOP_MM);
    let bite = skin::draft_clamp(&a, &mut alpha, SCALE_TOP_MM)?;
    println!("  Dorsal scales: the draft rule cut {} texels, at most {:.3} mm", bite.texels_cut, bite.worst_mm);
    let mut bites = vec![("Dorsal scales".to_string(), bite)];
    let scales = ringdesign_core::Alpha::from_png16("Dorsal scales", &alpha.to_png16()?)?;
    let mut e = skin::hide_layer(&d, "Dorsal scales", SCALE_TOP_MM, ringdesign_core::field::Window::default());
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    let mut painted = vec![scales];

    if block_out {
        return finish(d, painted, bites);
    }
    // The side faces: granular skin with a row of enlarged tubercles, wandering along the ring.
    let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).and_then(|f| f.low).ok_or_else(|| anyhow::anyhow!("no low side face"))?;
    let face_h = faces.1 - faces.0;
    let cols = 2usize;
    let repeats = (ctx.circumference_mm / (cols as f64 * HIDE_PITCH_MM)).round() as u32;
    println!("  side face {:.2} mm tall, {repeats} tubercle tiles", face_h);
    d.svgs.push(SvgAlpha { name: "Tubercle rows".into(), svg: hide_tile(ctx.circumference_mm / repeats as f64, face_h, cols), invert: false });
    d.svgs.push(SvgAlpha { name: "Dorsal field".into(), svg: u_mask(|t| 1.0 - ventral(t)), invert: false });
    let mut t = side_tiling("Tubercle rows", &ctx, TUBERCLE_MM, repeats, 0.0)?;
    let mid = 0.5 * (faces.0 + faces.1);
    t.warp = Some(WarpField { points: (0..8).map(|k| [k as f64 / 8.0, mid + if k % 2 == 0 { 0.25 } else { -0.25 }]).collect(), strength: 0.8, falloff_mm: 2.5 });
    let mut e = LayerEntry::new("Tubercle rows", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.2;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.mask = Some("Dorsal field".into());
    d.layers.layers.push(e);

    // The belly round the palm: squarish ventral scales.
    let rows = (face_h / VENTRAL_CELL_MM).round().max(1.0);
    let cell_v = face_h / rows;
    let repeats = (ctx.circumference_mm / VENTRAL_CELL_MM).round() as u32;
    d.svgs.push(SvgAlpha { name: "Ventral squares".into(), svg: rsvg::paver(&Params::new(ctx.circumference_mm / repeats as f64, cell_v, 0.42, 0.45)), invert: false });
    d.svgs.push(SvgAlpha { name: "Ventral field".into(), svg: u_mask(ventral), invert: false });
    let mut t = side_tiling("Ventral squares", &ctx, VENTRAL_MM, repeats, 0.0)?;
    t.rows = rows as u32;
    let mut e = LayerEntry::new("Ventral squares", Layer::Tiling(t));
    e.blend = Blend::SmoothMax;
    e.soft_mm = 0.2;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.mask = Some("Ventral field".into());
    d.layers.layers.push(e);

    let _ = (&mut painted, &mut bites);
    finish(d, painted, bites)
}

/// The parietal seat on the spine, then the bake.
fn finish(mut d: RingDesign, painted: Vec<ringdesign_core::Alpha>, bites: Vec<(String, skin::ClampReport)>) -> Result<(RingDesign, AlphaLibrary, Layers)> {
    let ctx = d.field_context();
    let mut seat = SeatPadLayer { theta_deg: STONE_DEG, v_mm: ctx.crest_v_mm, style: SeatStyle::GypsyMound, crown: 1.0, blend_mm: 0.45, solid: SolidKind::Flush, through: true, ..SeatPadLayer::default() };
    seat.fit_stone(peridot());
    seat.height_mm = 1.05;
    seat.mark_mm = 0.6;
    let mut e = LayerEntry::new("Parietal peridot", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    if std::env::var_os("SPHENODON_BARE").is_some() {
        d.layers.layers.clear();
    }
    if let Ok(mute) = std::env::var("SPHENODON_MUTE") {
        d.layers.layers.retain(|e| !mute.split(',').any(|m| m == e.name));
    }
    let mut lib = AlphaLibrary::default();
    for a in painted {
        lib.insert(a);
    }
    d.bake_all(&mut lib);
    let names = d.layers.layers.iter().map(|e| e.name.clone()).collect();
    Ok((d, lib, Layers { names, bites }))
}

/// The draft rule held over the whole relief: what `skin::draft_clamp` would take from the composite, painted on the bare atlas.
fn clamp_audit(d: &RingDesign, lib: &AlphaLibrary) -> Result<Vec<(String, skin::ClampReport)>> {
    let mut bare = d.clone();
    bare.layers.layers.clear();
    let a = Atlas::of(&bare, 2048, 256)?;
    let ctx = d.field_context();
    let mut out = Vec::new();
    for e in &d.layers.layers {
        let mut solo = d.clone();
        solo.layers.layers = vec![e.clone()];
        let top = 2.0;
        let mut alpha = a.paint(e.name.clone(), |s| {
            let uv = Uv { u: ctx.u_of_theta(s.theta), v: s.v };
            solo.layers.height(uv, &ctx, lib) / top
        });
        let r = skin::draft_clamp(&a, &mut alpha, top)?;
        out.push((e.name.clone(), r));
    }
    let mut alpha = a.paint("composite", |s| d.layers.height(Uv { u: ctx.u_of_theta(s.theta), v: s.v }, &ctx, lib) / 2.0);
    out.push(("composite".into(), skin::draft_clamp(&a, &mut alpha, 2.0)?));
    Ok(out)
}

fn release_json(r: &mf::release::ReleaseReport) -> Value {
    json!({
        "status": format!("{:?}", r.status),
        "obstructions": r.obstructions.len(),
        "unresolved_rays": r.unresolved_rays,
        "at": r.obstructions.iter().map(|o| json!({ "depth_mm": o.depth_mm, "area_mm2": o.projected_area_mm2, "theta_deg": o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0), "z_mm": o.world[2] })).collect::<Vec<_>>(),
    })
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Every gate at one build size, as JSON, with whether they all passed.
struct Gated {
    json: Value,
    passed: bool,
    built: mesh::BuildResult,
    pattern: mesh::BuildResult,
    inspection: mf::Inspection,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str, painted: &[(String, skin::ClampReport)]) -> Result<Gated> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = ms(t);
    let v = &built.report.validation;
    let degenerate = built.report.quality.degenerate_faces;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    let nearest = built.mesh.vertices.iter().map(|q| (q.0 as f64).hypot(q.1 as f64) - bore).fold(f64::MAX, f64::min);
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let drag = 100.0 * (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9);
    let shares = castability::attribute_drag(d, lib, &field);
    let dfm = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report(d, 0.0).map_or(0, |r| r.stone_count as usize);
    let gems = ringdesign_core::gems::built_meshes(d, lib, &built);
    let preview = gems.iter().map(|(m, _)| m.faces.len()).sum::<usize>();
    let preview_stones = ringdesign_core::stones::stone_frames(d).len();
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(d, lib, &setup, p)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let r = &inspection.release;
    let bites = clamp_audit(d, lib)?;
    let worst_bite = bites.iter().chain(painted).map(|b| b.1.worst_mm).fold(0.0, f64::max);
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let triangles = built.mesh.faces.len();
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && degenerate == 0),
        ("0 self-crossings on the ring", crossings == 0),
        ("solids notes empty, every stamp resolved", built.solids.notes.is_empty() && built.solids.stamped == d.stamps.len()),
        ("nothing inside the finger hole", inside == 0),
        ("field Castable under SandTwoPart", field.process == CastProcess::SandTwoPart && field.verdict == Verdict::Castable),
        ("ray release clean at 0.100 mm", r.obstructions.is_empty() && r.unresolved_rays == 0),
        ("ray release clean at 0.075 mm", release_fine.obstructions.is_empty() && release_fine.unresolved_rays == 0),
        ("draft-clamp bite at most 0.05 mm", worst_bite <= 0.05),
        ("0 DFM findings", dfm.is_empty()),
        ("stones report equals the preview", stones == preview_stones && (stones == 0) == (preview == 0)),
        ("within 2 million triangles", triangles <= 2_000_000),
        ("casting pattern closed, 0 degenerate, 0 crossings", pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0),
    ];
    let passed = list.iter().all(|g| g.1);
    println!("[{label}] {} triangles in {build_ms:.0} ms", triangles);
    for (g, ok) in &list {
        println!("  {} {g}", if *ok { "pass" } else { "FAIL" });
    }
    println!("  field {} (worst draft {:.2} deg, undercut {:.4} mm2, drag {drag:.1}%) {:?}", field.verdict.label(), field.worst_draft_deg, field.undercut_area_mm2, field.notes);
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    for o in r.obstructions.iter().chain(&release_fine.obstructions) {
        println!("  obstruction {:.3} mm deep at theta {:.1}, z {:.2}", o.depth_mm, o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0), o.world[2]);
    }
    let json = json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": triangles, "ms": build_ms },
        "process": format!("{:?}", d.draft.process),
        "sand": format!("{:?}", d.draft.sand),
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3 },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "resolved": built.solids.resolved, "notes": built.solids.notes },
        "finger_hole": { "bore_radius_mm": bore, "vertices_inside": inside, "nearest_margin_mm": nearest },
        "field": { "verdict": field.verdict.label(), "process": format!("{:?}", field.process), "worst_draft_deg": field.worst_draft_deg, "undercut_area_mm2": field.undercut_area_mm2, "marginal_area_mm2": field.marginal_area_mm2, "vertical_area_mm2": field.vertical_area_mm2, "total_area_mm2": field.total_area_mm2, "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes },
        "drag_pct": drag,
        "drag_by_layer": shares.iter().map(|s| json!({ "layer": s.layer, "marginal_mm2": s.marginal_mm2, "vertical_mm2": s.vertical_mm2 })).collect::<Vec<_>>(),
        "release_0100": release_json(r),
        "release_0075": release_json(&release_fine),
        "clamp_audit": { "note": "Painted layers are clamped as they are painted (painted_bites); the draft rule is then run again over every layer and the composite on the bare atlas to show what it would still take.", "worst_mm": worst_bite, "painted_bites": painted.iter().map(|(n, c)| json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>(), "layers": bites.iter().map(|(n, c)| json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>() },
        "dfm_findings": dfm.iter().map(|f| json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "report_count": stones, "preview_count": preview_stones, "preview_faces": preview },
        "casting_pattern": { "watertight": pv.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings, "triangles": pattern.mesh.faces.len() },
        "gates": list.iter().map(|(g, ok)| json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "gates_passed": passed,
    });
    Ok(Gated { json, passed, built, pattern, inspection })
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

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", HERO.0, HERO.1),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", 1.25, 1.05),
    ("reverse", PI, 0.8),
];

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, p: BuildParams, draft: bool) -> Result<()> {
    let finished = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(d, lib, built) };
    let parts = finished.parts(render::GOLD);
    let edge = if draft { 1100 } else { 1600 };
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("hero-300.png"), &parts, HERO.0, HERO.1, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    // A contact sheet of four views at 300 px each.
    let tiles: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3], VIEWS[2]].iter().map(|(_, y, pt)| render::render_parts_ss(&parts, *y, *pt, 300, 300, 3)).collect();
    let mut sheet = vec![0u8; 600 * 600 * 3];
    for (k, t) in tiles.iter().enumerate() {
        let (ox, oy) = ((k % 2) * 300, (k / 2) * 300);
        for y in 0..300 {
            sheet[((oy + y) * 600 + ox) * 3..((oy + y) * 600 + ox + 300) * 3].copy_from_slice(&t[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 600, 600, image::ColorType::Rgb8)?;
    // The stone close-up: from above the mound, tilted toward the sail.
    render::write_png_parts(out.join("stones.png"), &parts, 0.25, 1.25, edge)?;
    let bare = band();
    let b = mesh::try_build(&bare, lib, p)?;
    let e = if draft { 700 } else { 1000 };
    let left = render::render_parts_ss(&[Part::metal(&b.mesh, render::GOLD)], HERO.0, HERO.1, e, e, 3);
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, e, e, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &left, &right, e)?;
    Ok(())
}

fn write(out: &Path, draft: bool, verify: bool, block_out: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let (mut d, lib, layers) = design(block_out)?;
    let p = params(draft);
    d.build = p;
    let mut blocks = serde_json::Map::new();
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" }, &layers.bites)?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320", &layers.bites)?;
        passed &= dr.passed;
        blocks.insert("draft".into(), dr.json);
        blocks.insert("export".into(), main.json.clone());
    } else {
        blocks.insert("draft".into(), main.json.clone());
    }
    ringdesign_core::library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let mut reload = Value::Null;
    if verify {
        let t = Instant::now();
        let saved = ringdesign_core::library::load_design(out.join("design.ring.json"))?;
        let cold = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold, p)?;
        let same = rebuilt.mesh.vertices == main.built.mesh.vertices && rebuilt.mesh.faces == main.built.mesh.faces && rebuilt.mesh.normals == main.built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical vertices, faces and normals" } else { "CHANGED" });
        reload = json!({ "identical": same, "vertices": rebuilt.mesh.vertices.len(), "faces": rebuilt.mesh.faces.len(), "ms": ms(t) });
        passed &= same;
    }
    let setup = d.manufacturing.clone().unwrap();
    let stone = d.layers.layers.iter().find_map(|e| match &e.layer {
        Layer::SeatPad(s) => Some(json!({ "theta_deg": s.theta_deg, "v_mm": s.v_mm, "diameter_mm": s.diameter_mm, "height_mm": s.height_mm, "blend_mm": s.blend_mm, "style": format!("{:?}", s.style) })),
        _ => None,
    });
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": "sphenodon",
        "stage": if block_out { "block-out" } else { "full" },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "layers": layers.names,
        "seat": stone,
        "sail": { "teeth": TEETH, "height_mm": SAIL_MM, "span_mm": SAIL_SPAN_MM, "plateau_mm": SAIL_PLATEAU_MM, "grade": "Cosine", "taper": SAIL_TAPER, "clear_deg": SAIL_CLEAR_DEG, "full_deg": SAIL_FULL_DEG, "hold_deg": SAIL_HOLD_DEG, "skirt_share": SAIL_SKIRT, "palm_share": SAIL_PALM },
        "dorsal_scales": { "rows": SCALE_ROWS, "top_mm": SCALE_TOP_MM, "pitch_mm": SCALE_PITCH_MM, "groove_mm": SCALE_GROOVE_MM, "from_mm": SCALE_FROM_MM, "to_mm": SCALE_TO_MM, "painted_bites": layers.bites.iter().map(|(n, c)| json!({ "layer": n, "texels_cut": c.texels_cut, "worst_mm": c.worst_mm })).collect::<Vec<_>>() },
        "design_bytes": bytes,
        "draft": blocks.get("draft"),
        "export": blocks.get("export"),
        "cold_reload": reload,
        "gates_passed": passed,
        "manufacturing": mf::package::report(&d, &setup, &main.inspection, false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    stl::write_stl(out.join("finished-metal.stl"), &main.built.mesh, &d.name)?;
    stl::write_stl(out.join("casting-pattern.stl"), &main.pattern.mesh, &format!("{} / casting pattern", d.name))?;
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &main.built);
    let mut stones_json = Vec::new();
    for (k, (m, _)) in gems.iter().enumerate() {
        let name = if k == 0 { "reference-peridot.stl".to_string() } else { format!("reference-peridot-{k}.stl") };
        stl::write_stl(out.join(&name), m, "peridot")?;
        stones_json.push(json!({ "file": name, "gem": "Peridot 3.0 round", "tint": [0.50, 0.78, 0.12] }));
    }
    std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": stones_json, "report": ringdesign_core::stones::report(&d, 0.0).map(|r| json!({ "stone_count": r.stone_count, "total_carats": r.total_carats, "tight_pairs": r.tight_pairs, "crowding": r.crowding.len(), "fill_floor_mm": r.fill_floor_mm, "warnings": r.any_warnings() })) }))?)?;
    renders(out, &d, &lib, &main.built, p, draft)?;
    let art = out.join("artwork");
    std::fs::create_dir_all(&art)?;
    for s in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", s.name.replace(' ', "-"))), &s.svg)?;
    }
    println!("gates {}", if passed { "all pass" } else { "FAILED" });
    ensure!(passed, "gates failed");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("showcase/cataphracta/sphenodon"));
    write(&out, flag("--draft"), flag("--verify"), flag("--block-out"))
}
