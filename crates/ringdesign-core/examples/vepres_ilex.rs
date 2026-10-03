//! Vepres — Ilex, the Holly King's standard: a holly leaf struck along the parting line of factory 006 between two
//! garnet berries, poured in Delft sand through the sand master.
//! cargo build --release -p ringdesign-core --example vepres_ilex
//! target/release/examples/vepres_ilex [OUT_DIR] [--draft] [--verify] [--blockout] [--resize-check]
#![recursion_limit = "256"]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    alpha::{ProcRecipe, Procedural},
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle, VGate, Window},
    tiling::TilingLayer,
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    library, manufacturing as mf, mesh,
    render,
    setting::{SolidKind, Stamp, StampRow, StampTop, RowPath, stamp_row},
    skin::{Atlas, Hide},
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const AW: usize = 2048;
const AH: usize = 768;

/// The face, length round the ring by width across it: the plan's 16 x 17, lengthened so a 7.5 mm leaf fits each
/// side of the berries; and the stock's native face as the fallback.
const FACE: (f64, f64) = (19.2, 17.0);
const NATIVE: (f64, f64) = (16.0, 21.0);
const BORE_MM: f64 = 18.6;
/// The lost-wax floor Logan set for Vepres: thinnest section, mm.
const MIN_SECTION_MM: f64 = 0.8;
/// The census's edge reach, mm: two floors, so a spine opening at 28 deg or more reads as an edge (named in the report).
const EDGE_REACH_MM: f64 = 1.6;
/// Height-field relief read through a one-cell tent (`RingDesign::crisp_relief`).
const CRISP: bool = true;
/// The native stock for lost wax; the sand master only for the sand measure the report keeps as a bonus.
const SAND: bool = false;

/// Holly's red, a little brighter than a garnet's own black-red so the berry reads at arm's length.
/// How a spine's flanks fall from its point: 1 is straight, so the section widens at least as fast as it leaves the
/// point and the census reads it as an edge, not a web (1.6 made needles under the 0.8 mm floor).
const SPINE_POWER: f64 = 1.0;
/// Margin half-width where a leaf's base and tip are rounded off, mm.
const END_ROUND_MM: f64 = 0.2;

const GARNET_TINT: [f32; 3] = [0.52, 0.03, 0.06];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The ring's own Delft pour: z = 0 parting, opposed z withdrawal, 18k yellow gold.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.name = "Ilex / investment / Gold 18k".into();
    // Lost wax (Logan, 2026-10-03): the sand rules were what held the sprig back, so the ring is judged as an
    // investment casting with a 0.8 mm minimum section and no pull rule.
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Factory 006, native stock, lost wax: sprue from the palm. After the cast: cut the vein strokes, drill \
        each garnet seat on its raised mark, cut it to the measured stone and burnish flush. Polish the leaves, the halos \
        round them and the palm; leave the table's stipple and the flanks' grain as cast."
        .into();
    s
}

/// Factory 006 Square at `face` (length, width), on its own Flat chart: the native stock for lost wax, or the sand
/// master, as rounds 1 to 3 had it.
fn stock(face: (f64, f64), sand: bool) -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "006").context("no stock 006")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, if sand { sand_master(preset.load()?)? } else { preset.load()? })?;
    // The sand envelope only for the sand master: lost wax needs no fill along the pull, and on the native stock the
    // envelope would fill each seat's shadow along z into a groove.
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    d.name = "Ilex \u{2014} the Holly King's standard".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = face.1;
    d.shank.head.length_mm = face.0;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    // The chart comes from THIS stock before anything is drawn on it.
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = export_params();
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.manufacturing = Some(s);
    // Steep height-field relief (the stipple, the stem, the seat skirts) read through a one-cell tent, so a wall
    // across the build grid lies straight instead of stepping a row at a time.
    d.crisp_relief = CRISP;
    Ok(d)
}

/// The worst sand the envelope must add anywhere round the ring, mm, and where (`stock_spike`'s measure).
fn fill_mm(d: &RingDesign) -> (f64, f64) {
    let mut worst = (0.0, 0.0);
    for k in 0..360 {
        let theta = k as f64 + 0.5;
        let l = d.section_at(theta, 512, None, None);
        let n = l.pts.len();
        let mut reach = [vec![f64::MIN; 400], vec![f64::MIN; 400]];
        for i in 0..n {
            let (p, q) = (&l.pts[i], &l.pts[(i + 1) % n]);
            let steps = ((q.z - p.z).abs() / 0.01).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (r, z) = (p.r + (q.r - p.r) * t, p.z + (q.z - p.z) * t);
                let bin = (z.abs() / 0.05) as usize;
                if bin < 400 {
                    let side = &mut reach[usize::from(z < 0.0)];
                    side[bin] = side[bin].max(r);
                }
            }
        }
        for side in &reach {
            let mut beyond = f64::MIN;
            for r in side.iter().rev().filter(|r| **r > f64::MIN) {
                beyond = beyond.max(*r);
                if beyond - r > worst.0 {
                    worst = (beyond - r, theta);
                }
            }
        }
    }
    worst
}

/// Step 1: can 006 go to 16 x 17 directly? Bare build, envelope fill, mesh health and the bare pull.
fn resize_check(face: (f64, f64), sand: bool) -> Result<Value> {
    let d = stock(face, sand)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let lib = AlphaLibrary::builtin();
    let built = mesh::try_build(&bare, &lib, draft_params());
    let (fill, fill_at) = fill_mm(&d);
    let mut out = json!({ "face_mm": [face.0, face.1], "sand_master": sand, "envelope_fill_mm": fill, "envelope_fill_theta_deg": fill_at });
    match built {
        Ok(b) => {
            let q = b.mesh.quality();
            let top = b.mesh.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
            let (x, z) = b.mesh.vertices.iter().filter(|v| v.1 as f64 > top - 0.15).fold((0.0f64, 0.0f64), |(x, z), v| (x.max(v.0.abs() as f64), z.max(v.2.abs() as f64)));
            let (inspection, fine) = pull(&d, &lib, coarse_params())?;
            out["builds"] = json!(true);
            let census = wall_census(&b.mesh);
            out["wall_census"] = json!({ "clean": census.clean(), "wall_samples": census.below_limit, "unresolved": census.unresolved, "walls": census.walls.iter().take(6).map(|w| json!([w.area_mm2, w.thinnest_mm, w.point])).collect::<Vec<_>>() });
            out["watertight"] = json!(b.report.validation.watertight);
            out["degenerate_faces"] = json!(q.degenerate_faces);
            out["table_mm"] = json!([2.0 * x, 2.0 * z]);
            out["bore_mm"] = json!(b.report.inner_diameter_mm);
            out["pull_384"] = json!(release_line(&inspection.release));
            out["pull_0075"] = json!(release_line(&fine));
        }
        Err(e) => {
            out["builds"] = json!(false);
            out["refused"] = json!(format!("{e:#}"));
        }
    }
    Ok(out)
}

/// The lost-wax wall census at the 0.8 mm floor, with the edge reach widened to `EDGE_REACH_MM`: a holly spine is a
/// point that opens at about 40 deg, narrower than the default reach (one floor) reads as an edge.
fn wall_census(m: &mesh::Mesh) -> ringdesign_core::cad::measure::Thickness {
    let mut o = ringdesign_core::cad::measure::CensusOptions::floor(MIN_SECTION_MM);
    o.edge_reach_mm = Some(EDGE_REACH_MM);
    ringdesign_core::cad::measure::census(m, &o)
}

fn release_line(r: &mf::release::ReleaseReport) -> String {
    let depth = r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max);
    format!("{:?}, {} obstructions (deepest {depth:.3} mm), {} unresolved", r.status, r.obstructions.len(), r.unresolved_rays)
}

/// The ring's own pull at `params`: the inspection at 0.100 mm and the rays again at 0.075 mm.
fn pull(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(mf::Inspection, mf::release::ReleaseReport)> {
    let s = d.manufacturing.clone().unwrap_or_else(setup);
    let inspection = mf::inspect(d, lib, &s, params)?;
    let mut fine = s.clone();
    fine.sample_pitch_mm = 0.075;
    let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    Ok((inspection, release))
}

// --- Holly -------------------------------------------------------------------------------------------------------

fn garnet(d_mm: f64) -> Gem {
    let mut g = Gem::cabochon(GemCut::Round, d_mm);
    g.preview_tint = Some(GARNET_TINT);
    g
}

/// A holly leaf's outline, stalk at -x and tip at +x: a pointed blade whose margin dips into `bay` mm bays between
/// `spines` sharp spines a side, each standing proud of the blade and leaning tipward, then the spined tip. Each margin
/// is a function of x, so every line along the finger crosses the leaf once: the monotone rule by construction.
fn holly(len: f64, wid: f64, spines: usize, bay: f64, with_stalk: bool) -> Vec<[f64; 2]> {
    // A short stalk at the base, an eighth of the whole and at least 0.35 mm.
    let stalk = if with_stalk { (0.12 * len).max(0.35) } else { 0.0 };
    let stalk_half = (0.035 * wid).max(0.15);
    let l = len - stalk;
    let x0 = -0.5 * len + stalk;
    let proud = 0.085 * wid;
    let w = 0.5 * wid - proud;
    // The blade: widest a little behind the middle, a short rounded base and a long point.
    let env = |x: f64| {
        let s = ((x - x0) / l).clamp(0.0, 1.0);
        let k = s.powf(0.85);
        w * (4.0 * k * (1.0 - k)).max(0.0).powf(0.7)
    };
    // Spine stations from a fifth of the way along; the tip closes the last bay.
    let stations: Vec<f64> = (0..spines).map(|k| x0 + l * (0.22 + 0.62 * k as f64 / (spines.max(2) - 1) as f64)).collect();
    let spike_w = 0.45 * l / (spines as f64 + 1.0);
    let margin = |x: f64| {
        // The bay between neighbouring stations, deepest at its middle and falling steeply from each spine.
        let mut knots = vec![x0 + 0.06 * l];
        knots.extend(stations.iter().copied());
        knots.push(0.5 * l);
        let i = knots.iter().rposition(|k| *k <= x).unwrap_or(0).min(knots.len() - 2);
        let (a, b) = (knots[i], knots[i + 1]);
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        let depth = if i == 0 { 0.25 * bay } else if i + 1 == knots.len() - 1 { 0.7 * bay } else { bay };
        let dip = if x < a { 0.0 } else { depth * (PI * t).sin().max(0.0).powf(0.6) };
        // Each spine a sharp point leaning tipward: steeper on its tip side.
        let spike: f64 = stations
            .iter()
            .map(|c| {
                let d = if x <= *c { (c - x) / spike_w } else { (x - c) / (0.55 * spike_w) };
                proud * (1.0 - d).max(0.0).powf(SPINE_POWER)
            })
            .sum();
        let blade = (env(x) - dip + spike).max(0.0);
        // The stalk runs into the blade's base.
        let into = 1.0 - ((x - x0) / (0.06 * l)).clamp(0.0, 1.0);
        // Its end rounded off, so no edge of the outline runs along the pull.
        let end = ((x + 0.5 * len) / (2.0 * stalk_half)).clamp(0.0, 1.0);
        if with_stalk { blade.max(stalk_half * into * (end * (2.0 - end)).sqrt()) } else { blade }
    };
    let step = 0.03;
    let count = (len / step).round() as usize;
    let upper: Vec<[f64; 2]> = (1..count).map(|i| -0.5 * len + len * i as f64 / count as f64).map(|x| [x, margin(x)]).filter(|p| p[1] > 1e-3).collect();
    // Both ends rounded off where the margin is `END_ROUND_MM`: a needle point is a web under the 0.8 mm floor.
    let first = upper.iter().position(|p| p[1] >= END_ROUND_MM).unwrap_or(0);
    let last = upper.iter().rposition(|p| p[1] >= END_ROUND_MM).unwrap_or(upper.len() - 1);
    let upper = &upper[first..=last];
    let cap = |c: [f64; 2], from: f64, to: f64| -> Vec<[f64; 2]> {
        // A half-ellipse from angle `from` to `to` about the end point, as wide as the margin there.
        (1..12).map(|i| from + (to - from) * i as f64 / 12.0).map(|t| [c[0] + END_ROUND_MM * t.cos(), c[1] * t.sin()]).collect()
    };
    let (a, b) = (upper[0], upper[upper.len() - 1]);
    let mut out: Vec<[f64; 2]> = cap([a[0], a[1]], 0.5 * PI, 1.5 * PI);
    out.extend(upper.iter().map(|p| [p[0], -p[1]]));
    out.extend(cap([b[0], b[1]], -0.5 * PI, 0.5 * PI));
    out.extend(upper.iter().rev().copied());
    if std::env::var("ILEX_DEBUG").is_ok() {
        eprintln!("holly {len}x{wid}: {} pts, check {:?}, crossing {:?}", out.len(), ringdesign_core::outline::check(&out), ringdesign_core::outline::self_crossing(&out));
        eprintln!("{:?}", out.iter().map(|p| [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0]).collect::<Vec<_>>());
    }
    out
}

/// A leaf outline grown by `r` all round, from its upper margin wherever `x >= x_from`: the disc dilation of a
/// region bounded by `y = ±m(x)`, so spines stay spines and bays stay bays, and the end at `x_from` is rounded.
fn grown(outline: &[[f64; 2]], r: f64, x_from: f64) -> Vec<[f64; 2]> {
    let upper: Vec<[f64; 2]> = outline.iter().copied().filter(|p| p[1] >= 0.0 && p[0] >= x_from).collect();
    let (lo, hi) = upper.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[0]), hi.max(p[0])));
    let margin = |x: f64| upper.iter().filter(|p| (p[0] - x).abs() <= r).map(|p| p[1] + (r * r - (p[0] - x).powi(2)).max(0.0).sqrt()).fold(0.0, f64::max);
    let (x0, x1) = (lo - r, hi + r);
    let count = ((x1 - x0) / 0.03).round() as usize;
    let top: Vec<[f64; 2]> = (1..count).map(|i| x0 + (x1 - x0) * i as f64 / count as f64).map(|x| [x, margin(x)]).filter(|p| p[1] > 1e-3).collect();
    let mut out = vec![[x0, 0.0]];
    out.extend(top.iter().map(|p| [p[0], -p[1]]));
    out.push([x1, 0.0]);
    out.extend(top.iter().rev().copied());
    out
}

/// A struck holly leaf: stalk at -x, tip at +x, midrib on y = 0, a wall `height` mm at the margin and a cushion
/// rising `crown` mm over it to the midrib, creaseless over the spines.
fn holly_stamp(name: &str, at: (f64, f64), len: f64, wid: f64, spines: usize, spine_depth: f64, height: f64, crown: f64) -> Stamp {
    Stamp {
        name: name.into(),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg: 0.0,
        // No stalk: a stalk narrow enough for the kernel to join (0.3 mm) stands as a web under the 0.8 mm floor, and
        // a wider one will not join. The blade's base runs to the berries instead.
        outline: holly(len, wid, spines, spine_depth, false),
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        // The cap gridded at half the pitch: at the coarse pitch a spined margin stair-steps.
        fine_cap: true,
        tier: 0,
        top: StampTop::Pillow { crown_mm: crown },
    }
}

/// A tapered stroke from `p0` to `p1`, `w` wide at its widest, as a closed counter-clockwise outline.
fn stroke(p0: [f64; 2], p1: [f64; 2], w: f64, swell: f64) -> Vec<[f64; 2]> {
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let l = dx.hypot(dy).max(1e-9);
    let n = [-dy / l, dx / l];
    let steps = ((l / 0.04).ceil() as usize).max(8);
    // Widest a `swell` share of the way along, running to a point at each end.
    let half = |t: f64| {
        let u = if t < swell { t / swell } else { (1.0 - t) / (1.0 - swell) };
        0.5 * w * (u.clamp(0.0, 1.0) * (2.0 - u.clamp(0.0, 1.0))).sqrt()
    };
    let at = |t: f64, side: f64| [p0[0] + dx * t + side * n[0] * half(t), p0[1] + dy * t + side * n[1] * half(t)];
    let mut out: Vec<[f64; 2]> = (0..=steps).map(|i| at(i as f64 / steps as f64, -1.0)).collect();
    out.extend((1..steps).rev().map(|i| at(i as f64 / steps as f64, 1.0)));
    out
}

/// A leaf's veins cut at the bench as separate strokes: a midrib and tapering laterals leaning to the tip, each floor
/// following the cushion at its depth.
fn leaf_veins(leaf: &Stamp, len: f64, pairs: usize) -> Vec<Stamp> {
    if std::env::var("ILEX_NO_VEINS").is_ok() {
        return Vec::new();
    }
    let stalk = (0.12 * len).max(0.35);
    let (x0, x1) = (-0.5 * len + stalk * 0.6, 0.5 * len - 0.45);
    let cut = |name: String, outline: Vec<[f64; 2]>, sink: f64| Stamp {
        name,
        theta_deg: leaf.theta_deg,
        v_mm: leaf.v_mm,
        rot_deg: leaf.rot_deg,
        outline,
        height_mm: 0.1,
        sink_mm: sink,
        draft_deg: 0.0,
        cut: true,
        bench: true,
        along_pull: leaf.along_pull,
        fine_cap: true,
        tier: 1,
        top: StampTop::Flat,
    };
    let mut out = vec![cut(format!("{} midrib", leaf.name), stroke([x0, 0.0], [x1, 0.0], 0.24, 0.25), 0.22)];
    let lean = 42f64.to_radians();
    for i in 0..pairs {
        let t = 0.16 + 0.62 * i as f64 / (pairs.max(2) - 1) as f64;
        let x = x0 + (x1 - x0) * t;
        // Laterals reach most of the way to the margin, shortening toward the tip.
        let reach = 0.21 * len * (1.0 - 0.55 * t);
        for side in [1.0f64, -1.0] {
            let tip = [x + reach * lean.cos(), side * reach * lean.sin()];
            out.push(cut(format!("{} vein {}{}", leaf.name, i + 1, if side > 0.0 { "a" } else { "b" }), if side > 0.0 { stroke([x, 0.0], tip, 0.13, 0.3) } else { stroke(tip, [x, 0.0], 0.13, 0.7) }, 0.12));
        }
    }
    out
}

/// A garnet berry flush in the metal (a gypsy setting), cast with a raised drill mark.
fn berry(d: &mut RingDesign, name: &str, at: (f64, f64), gem: Gem) {
    let mut seat = SeatPadLayer {
        theta_deg: at.0,
        v_mm: at.1,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        mark_mm: 0.6,
        ..Default::default()
    };
    seat.fit_stone(gem);
    seat.height_mm = 0.0;
    let mut e = LayerEntry::new(name, Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
}

/// What was struck and where, for the report.
#[derive(Default, serde::Serialize)]
struct Placed {
    face_mm: [f64; 2],
    leaf_at: Vec<[f64; 2]>,
    face_leaf_mm: Vec<f64>,
    berries_at: Vec<[f64; 2]>,
    folds_along_mm: Vec<f64>,
    reach_mm: f64,
    garland: Vec<String>,
    cheek: Vec<String>,
    crest: [f64; 2],
    table_half_mm: f64,
    nudged: Vec<String>,
}

/// The nearest table sample to a point of it, `x` round the ring and `z` across, as the chart's `(theta, v)`.
fn on_face(a: &Atlas, x: f64, z: f64) -> (f64, f64) {
    let s = a
        .samples
        .iter()
        .filter(|s| s.p[1] > a.top - 1.5)
        .min_by(|p, q| ((p.p[0] - x).powi(2) + (p.p[2] - z).powi(2)).total_cmp(&((q.p[0] - x).powi(2) + (q.p[2] - z).powi(2))))
        .expect("a table");
    (s.theta, s.v)
}

/// The nearest sample squarely on a head's wall to `(x, y)` seen along the finger, on the `side` of the parting line.
fn on_cheek(a: &Atlas, x: f64, y: f64, side: f64) -> Option<(f64, f64)> {
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && a.cheek(s) > 0.9)
        .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
        .filter(|(d, _)| *d < 0.15)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, s)| (s.theta, s.v))
}

/// Face berries, x round the ring and z across it from the table's centre: one at the sprig's heart and the bunch's
/// other two just below it.
const FACE_BERRIES: [(f64, f64); 3] = [(0.0, 0.0), (-1.25, 2.3), (1.25, 2.3)];
/// The face leaves: the stalk's distance from the head's centre and the longest leaf's length and width, mm.
const FACE_LEAF: (f64, f64, f64) = (1.25, 7.5, 5.5);
/// Each face leaf's wall at the margin, and the cushion rising over it to the midrib, mm: 0.8 mm at the crown.
const FACE_LEAF_EAVES: f64 = 0.7;
const FACE_LEAF_CROWN: f64 = 0.3;
/// The leaf tip stays this far inside the table's end, mm.
const TIP_CLEAR_MM: f64 = 0.45;
/// Arc from the garland's first leaf to its last, mm.
const GARLAND_SPAN_MM: f64 = 19.0;
const GARLAND_COUNT: u32 = 7;
/// Each garland leaf turns off the stem by this much, alternately, about its stalk.
const GARLAND_SPLAY_DEG: f64 = 25.0;
/// Garland leaves: wall at the margin and the cushion over it, mm.
const GARLAND_EAVES: f64 = 0.45;
const GARLAND_CROWN: f64 = 0.3;
/// The polished halo kept round each garland leaf and either side of the stem, mm.
const GARLAND_HALO_MM: f64 = 0.3;
/// The garland's stem wire, width and height, mm.
const STEM_MM: (f64, f64) = (0.85, 0.32);
/// The stem's capsules: their spacing along the line and how far each overlaps the next, mm; their wall draft.
const STEM_PITCH_MM: f64 = 1.8;
const STEM_OVERLAP_MM: f64 = 1.2;
const STEM_DRAFT_DEG: f64 = 25.0;
/// How much narrower the stem ends than it starts: none, since the stem's width is the 0.8 mm floor.
const STEM_TAPER: f64 = 0.0;
/// The table's stipple: depth, and its inset from the table's edge, mm.
const MATTE_DEPTH_MM: f64 = 0.035;
const MATTE_INSET_MM: f64 = 0.8;
const STIPPLE_CELL_MM: f64 = 1.2;
/// The shoulders' stipple: the band of chart v it covers (centre, span), how far past the table's end it starts,
/// and how far round from the head it stops, leaving the palm bare.
const SHANK_V: (f64, f64) = (9.45, 8.6);
const SHOULDER_GAP_DEG: f64 = 7.0;
/// Rows across the shank's stipple band: about 4.3 mm of metal, so two cells of about 2.2 mm.
const SHANK_ROWS: u32 = 2;
/// Chart v per millimetre across the shank's outer face (10.3 v over 5 mm, measured on the atlas).
const SHANK_V_PER_MM: f64 = 2.07;
const PALM_BARE_FROM_DEG: f64 = 160.0;
/// The polished halo left round every leaf and berry on the table, mm.
const HALO_MM: f64 = 0.4;

/// A cheek spray, seen along the finger with x round the ring and y up from the axis: two leaves (centre, length,
/// width, turn of the tip from +x in degrees) either side of the berries. The native wall is too short below the bunch
/// for a third leaf, and a small one in each corner read as a cross (tried in round 5). The native wall is a crescent over the bore.
const CHEEK_LEAVES: [([f64; 2], f64, f64, f64); 2] = [([-4.65, 11.4], 5.2, 2.7, 172.0), ([4.65, 11.4], 5.2, 2.7, 8.0)];
/// The cheek's bunch: a touching triangle, one above and two below (two above read as eyes). The native wall is only 2.15 mm tall (2.6 mm
/// to where its flat ends under the table's round) over the bore's crown, so the berries are 1.0 mm, set flush, and
/// spread so the metal between neighbouring seats stays near the 0.8 mm floor.
const CHEEK_BERRIES: [[f64; 2]; 3] = [[0.0, 12.8], [-1.05, 11.3], [1.05, 11.3]];
const CHEEK_BERRY_MM: f64 = 1.0;
/// The cheek leaves' wall at the margin and cushion over it, mm. Taller walls than 0.3 mm are refused by the kernel on
/// this wall ("two cuts cross inside a face"), so the height is in the cushion.
const CHEEK_LEAF_EAVES: f64 = 0.3;
const CHEEK_LEAF_CROWN: f64 = 0.35;
const CHEEK_SIDES: [f64; 2] = [1.0, -1.0];

/// The flat of the table in chart terms: its theta range at the centre line and its `v` range at the head's centre,
/// from the atlas samples that face up within 0.1 mm of the top.
fn table_extent(a: &Atlas) -> ((f64, f64), (f64, f64)) {
    let flat: Vec<&ringdesign_core::skin::Sample> = a.samples.iter().filter(|s| s.p[1] > a.top - 0.1 && s.n[1] > 0.98).collect();
    let thetas = flat.iter().filter(|s| s.p[2].abs() < 0.3).map(|s| s.theta);
    let vs = flat.iter().filter(|s| s.p[0].abs() < 0.3).map(|s| s.v);
    let range = |it: &mut dyn Iterator<Item = f64>| it.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    (range(&mut thetas.into_iter()), range(&mut vs.into_iter()))
}

fn author(face: (f64, f64), blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock(face, SAND)?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let mut placed = Placed { face_mm: [face.0, face.1], reach_mm: hide.reach(), folds_along_mm: hide.folds(&a, 12.0), ..Default::default() };
    let crest = hide.crest_at(&a, 0.0);
    placed.crest = [crest.0, crest.1];
    // The table's half-length along the centre line: where the line turns down over the head's end walls (a crease
    // in the native mesh well inside the table is not one).
    let head = d.shank.head.length_mm;
    let table_half = placed.folds_along_mm.iter().map(|f| f.abs()).filter(|f| *f > 0.4 * head).fold(0.5 * head, f64::min);
    placed.table_half_mm = table_half;
    // The face: a sprig of two leaves end to end along the table's centre line, stalks meeting at the berries.
    let (stalk, longest, wid) = FACE_LEAF;
    let mut halos: Vec<Vec<[f64; 3]>> = Vec::new();
    let len = (longest.min(table_half - TIP_CLEAR_MM - stalk) * 10.0).floor() / 10.0;
    ensure!(len > 4.4, "no room for a face leaf: table half-length {table_half:.2} mm");
    for (k, sign) in [-1.0f64, 1.0].into_iter().enumerate() {
        let at = hide.crest_at(&a, sign * (stalk + 0.5 * len));
        let mut leaf = holly_stamp(&format!("Face leaf, {}", k + 1), at, len, wid, 3, 0.6, FACE_LEAF_EAVES, FACE_LEAF_CROWN);
        // The tip leads away from the heart; theta grows toward -x.
        leaf.rot_deg = if sign < 0.0 { 180.0 } else { 0.0 };
        placed.leaf_at.push([leaf.theta_deg, leaf.v_mm]);
        placed.face_leaf_mm.push(len);
        // The halo: the leaf's margin grown by `HALO_MM`, masked out of the stipple.
        halos.push(world_outline(&d, &leaf, &grown(&leaf.outline, HALO_MM, -0.5 * len)));
        d.stamps.push(leaf.clone());
        if !blockout {
            d.stamps.extend(leaf_veins(&leaf, len, 4));
        }
    }
    let berries: Vec<(f64, f64)> = FACE_BERRIES.into_iter().map(|(x, z)| if z == 0.0 { hide.crest_at(&a, x) } else { on_face(&a, x, z) }).collect();
    for at in &berries {
        // A disc `HALO_MM` beyond the stone's girdle.
        let at = Stamp { theta_deg: at.0, v_mm: at.1, rot_deg: 0.0, ..d.stamps[0].clone() };
        halos.push(world_outline(&d, &at, &ringdesign_core::outline::circle(2.0 + 2.0 * HALO_MM)));
    }
    for (k, at) in berries.into_iter().enumerate() {
        placed.berries_at.push([at.0, at.1]);
        berry(&mut d, &format!("Face berry, {}", k + 1), at, garnet(2.0));
    }
    // The cheeks: a spray each side.
    for (side_k, side) in CHEEK_SIDES.into_iter().enumerate().filter(|_| std::env::var("ILEX_NO_CHEEK").is_err()) {
        for (k, (c, len, wid, turn)) in CHEEK_LEAVES.into_iter().enumerate() {
            let at = on_cheek(&a, c[0], c[1], side).with_context(|| format!("no cheek at {c:?}"))?;
            let mut leaf = holly_stamp(&format!("Cheek leaf {}, {}", side_k + 1, k + 1), at, len, wid, 3, 0.4, CHEEK_LEAF_EAVES, CHEEK_LEAF_CROWN);
            // Theta runs one way round on the cheek facing +z and the other on its mirror.
            leaf.rot_deg = if side > 0.0 { turn } else { 180.0 - turn };
            placed.cheek.push(format!("{} at {:.2} deg, v {:.3}", leaf.name, at.0, at.1));
            d.stamps.push(leaf.clone());
            if !blockout {
                d.stamps.extend(leaf_veins(&leaf, len, if len > 4.0 { 3 } else { 2 }));
            }
        }
        for (k, c) in CHEEK_BERRIES.into_iter().enumerate() {
            let at = on_cheek(&a, c[0], c[1], side).with_context(|| format!("no cheek at {c:?}"))?;
            placed.cheek.push(format!("Cheek berry {}, {} at {:.2} deg, v {:.3}", side_k + 1, k + 1, at.0, at.1));
            berry(&mut d, &format!("Cheek berry {}, {}", side_k + 1, k + 1), at, garnet(CHEEK_BERRY_MM));
        }
    }
    let (stems, garland_halos) = if !blockout && std::env::var("ILEX_NO_GARLAND").is_err() { garland(&mut d, &a, &hide, &mut placed)? } else { (Vec::new(), Vec::new()) };
    // The stipple last of the relief, after the seats and the stem, whose Max would flatten it.
    if !blockout && std::env::var("ILEX_NO_MATTE").is_err() {
        table_matte(&mut d, &a, &halos);
        shoulder_stipple(&mut d, &a, &stems, &garland_halos);
    }
    // Outlines to a tenth of a micron: far below the build grid, and it keeps the template inside its budget.
    for st in &mut d.stamps {
        for p in &mut st.outline {
            *p = p.map(|c| (c * 1e4).round() / 1e4);
        }
    }
    // The textures travel in the design as recipes, baked here as a cold reload bakes them.
    let lib = mf::source_library(&d, &lib).into_owned();
    Ok((d, lib, placed))
}

/// An outline in a stamp's own plane carried onto the ring, mm.
fn world_outline(d: &RingDesign, at: &Stamp, outline: &[[f64; 2]]) -> Vec<[f64; 3]> {
    let frame = at.frame(d, &d.field_context());
    outline.iter().map(|p| frame.point([p[0], p[1], 0.0])).collect()
}

/// The table's chart `v` as a line in world `z` (the table is flat), fitted to the atlas: `(v at z = 0, v per mm)`.
fn table_v_of_z(a: &Atlas) -> (f64, f64) {
    let pts: Vec<(f64, f64)> = a.samples.iter().filter(|s| s.p[1] > a.top - 0.1 && s.n[1] > 0.98 && s.p[0].abs() < 0.3).map(|s| (s.p[2], s.v)).collect();
    let n = pts.len() as f64;
    let (mz, mv) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let k = pts.iter().map(|p| (p.0 - mz) * (p.1 - mv)).sum::<f64>() / pts.iter().map(|p| (p.0 - mz).powi(2)).sum::<f64>();
    (mv - k * mz, k)
}

/// The table's ground: one fine hammered stipple over the whole flat of the table, inset `MATTE_INSET_MM` from its
/// edge, worked at the bench with a matting punch. Its mask, carried in the design as SVG, is the inset table with a
/// polished halo cut round every leaf and berry, so nothing stands on the stipple.
fn table_matte(d: &mut RingDesign, a: &Atlas, halos: &[Vec<[f64; 3]>]) {
    let ctx = d.field_context();
    d.recipes.push(ProcRecipe { name: "Ilex matte".into(), kind: Procedural::Hammered, repeats: 1, quarter_turns: 0, gamma: 1.0, invert: false });
    let mut t = TilingLayer::default_for("Ilex matte", &ctx);
    let ((t0, t1), (v0, v1)) = table_extent(a);
    let (v_at, v_per_mm) = table_v_of_z(a);
    // The chart, as the mask samples it: x the arc round the ring, y the chart v.
    let (circ, band) = (ctx.circumference_mm, ctx.band_v_len_mm);
    let to_svg = |p: [f64; 3]| [p[1].atan2(p[0]).to_degrees().rem_euclid(360.0) / 360.0 * circ, v_at + v_per_mm * p[2]];
    let deg_per_mm = (1.0 / a.top).to_degrees();
    let inset_deg = MATTE_INSET_MM * deg_per_mm;
    let (x0, x1) = ((t0 + inset_deg) / 360.0 * circ, (t1 - inset_deg) / 360.0 * circ);
    let (y0, y1) = (v0 + MATTE_INSET_MM * v_per_mm.abs(), v1 - MATTE_INSET_MM * v_per_mm.abs());
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{circ:.4}\" height=\"{band:.4}\" viewBox=\"0 0 {circ:.4} {band:.4}\">");
    // A soft edge, about 0.12 mm, so neither the inset nor a halo steps with the build grid.
    svg.push_str(BLUR_OPEN);
    svg.push_str(&format!("<rect x=\"{x0:.4}\" y=\"{y0:.4}\" width=\"{:.4}\" height=\"{:.4}\" rx=\"0.6\" fill=\"#000\"/>", x1 - x0, y1 - y0));
    for h in halos {
        let pts: Vec<[f64; 2]> = h.iter().map(|p| to_svg(*p)).collect();
        svg.push_str(&format!("<path d=\"{}\" fill=\"#fff\"/>", svg_path(&pts, true)));
    }
    svg.push_str("</g></svg>");
    d.svgs.push(ringdesign_core::svg::SvgAlpha { name: "Ilex table mask".into(), svg, invert: false });
    t.height_mm = MATTE_DEPTH_MM;
    t.v_center_mm = 0.5 * (v0 + v1);
    t.v_span_mm = v1 - v0;
    // Square cells about STIPPLE_CELL_MM across in real millimetres both ways (chart v is not millimetres here), so
    // the finest pit holds the 0.15 mm detail and no pit is drawn out into a dash.
    t.repeats_around = (ctx.circumference_mm / STIPPLE_CELL_MM).round() as u32;
    t.rows = ((v1 - v0) / v_per_mm.abs() / STIPPLE_CELL_MM).round().max(1.0) as u32;
    t.feather_mm = 0.0;
    t.continuous = true;
    let mut e = LayerEntry::new("Table stipple", Layer::Tiling(t));
    e.blend = Blend::Subtract;
    e.window = Window { enabled: true, theta_deg: 0.5 * (t0 + t1), span_deg: t1 - t0, fade_deg: 0.0, invert: false, v_gate: VGate::Off };
    e.mask = Some("Ilex table mask".into());
    // Worked at the bench with a matting punch.
    e.bench_only = true;
    d.layers.layers.push(e);
}

/// Every atlas sample hashed by position, so a point on the ring finds its chart `(theta, v)` fast.
struct ChartGrid<'a> {
    a: &'a Atlas,
    cells: std::collections::HashMap<[i32; 3], Vec<usize>>,
}

impl<'a> ChartGrid<'a> {
    const CELL: f64 = 0.3;
    fn new(a: &'a Atlas) -> Self {
        let mut cells: std::collections::HashMap<[i32; 3], Vec<usize>> = std::collections::HashMap::new();
        for (i, s) in a.samples.iter().enumerate() {
            cells.entry(s.p.map(|c| (c / Self::CELL).floor() as i32)).or_default().push(i);
        }
        Self { a, cells }
    }
    /// The nearest sample's `(theta, v)`.
    fn chart(&self, p: [f64; 3]) -> (f64, f64) {
        let k = p.map(|c| (c / Self::CELL).floor() as i32);
        let mut best = (f64::MAX, (0.0, 0.0));
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &i in self.cells.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                        let s = &self.a.samples[i];
                        let d = (s.p[0] - p[0]).powi(2) + (s.p[1] - p[1]).powi(2) + (s.p[2] - p[2]).powi(2);
                        if d < best.0 {
                            best = (d, (s.theta, s.v));
                        }
                    }
                }
            }
        }
        best.1
    }
}

/// Opens a group blurred by about 0.12 mm, so a mask's edges fall off softly.
const BLUR_OPEN: &str = "<defs><filter id=\"soft\" x=\"-0.01\" y=\"-0.05\" width=\"1.02\" height=\"1.1\"><feGaussianBlur stdDeviation=\"0.12\"/></filter></defs><g filter=\"url(#soft)\">";

/// An SVG path through chart points `[x, y]`, closed or open.
fn svg_path(pts: &[[f64; 2]], closed: bool) -> String {
    let mut d = format!("M{:.3} {:.3}", pts[0][0], pts[0][1]);
    // Every third point: the outlines are sampled at 0.03 mm, finer than the mask's raster.
    for p in pts[1..].iter().step_by(3).chain(pts.last()) {
        d.push_str(&format!(" L{:.3} {:.3}", p[0], p[1]));
    }
    if closed { d + " Z" } else { d }
}

/// The same stipple down both shoulders, from just past the head's end walls to where the shank turns under toward
/// the palm, which stays bare. The table's polished frame and the head's end walls part the two. Its SVG mask keeps
/// a polished halo round each garland leaf and along the stem, so the stem and leaves stand on polish.
fn shoulder_stipple(d: &mut RingDesign, a: &Atlas, stems: &[Vec<[f64; 2]>], halos: &[Vec<[f64; 3]>]) {
    let ctx = d.field_context();
    let ((t0, t1), _) = table_extent(a);
    let (circ, band) = (ctx.circumference_mm, ctx.band_v_len_mm);
    let x_of = |theta_deg: f64| theta_deg / 360.0 * circ;
    let grid = ChartGrid::new(a);
    let spans = [(t0 - SHOULDER_GAP_DEG, 90.0 - PALM_BARE_FROM_DEG), (t1 + SHOULDER_GAP_DEG, 90.0 + PALM_BARE_FROM_DEG)];
    let (v0, v1) = (SHANK_V.0 - 0.5 * SHANK_V.1, SHANK_V.0 + 0.5 * SHANK_V.1);
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{circ:.4}\" height=\"{band:.4}\" viewBox=\"0 0 {circ:.4} {band:.4}\">");
    svg.push_str(BLUR_OPEN);
    // Each shape drawn three times, a turn apart, so whatever crosses theta 0 lands whole.
    let copies = [-circ, 0.0, circ];
    for (from, to) in spans {
        let (lo, hi) = (x_of(from.min(to)), x_of(from.max(to)));
        for c in copies.into_iter().filter(|c| hi + c > 0.0 && lo + c < circ) {
            svg.push_str(&format!("<rect x=\"{:.4}\" y=\"{v0:.4}\" width=\"{:.4}\" height=\"{:.4}\" fill=\"#000\"/>", lo + c, hi - lo, v1 - v0));
        }
    }
    // The stem's halo: a stroke as wide as the stem and its halo, in chart v (about 2.07 v per mm on the shank).
    let width_v = (STEM_MM.0 + 2.0 * GARLAND_HALO_MM) * SHANK_V_PER_MM;
    // The copies a turn either way, only for a shape that crosses theta 0.
    let needed = |pts: &[[f64; 2]]| -> Vec<f64> { copies.iter().copied().filter(|c| pts.iter().any(|p| (0.0..=circ).contains(&(p[0] + c)))).collect() };
    for stem in stems {
        let pts: Vec<[f64; 2]> = stem.iter().map(|p| [x_of(p[0]), p[1]]).collect();
        for c in needed(&pts) {
            let moved: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] + c, p[1]]).collect();
            svg.push_str(&format!("<path d=\"{}\" fill=\"none\" stroke=\"#fff\" stroke-width=\"{width_v:.4}\" stroke-linecap=\"round\"/>", svg_path(&moved, false)));
        }
    }
    for h in halos {
        let mut last: Option<f64> = None;
        let pts: Vec<[f64; 2]> = h
            .iter()
            .map(|p| {
                let (theta, v) = grid.chart(*p);
                let theta = last.map_or(theta, |prev| prev + crate_wrap(theta - prev));
                last = Some(theta);
                [x_of(theta), v]
            })
            .collect();
        for c in needed(&pts) {
            let moved: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] + c, p[1]]).collect();
            svg.push_str(&format!("<path d=\"{}\" fill=\"#fff\"/>", svg_path(&moved, true)));
        }
    }
    svg.push_str("</g></svg>");
    d.svgs.push(ringdesign_core::svg::SvgAlpha { name: "Ilex shoulder mask".into(), svg, invert: false });
    let mut t = TilingLayer::default_for("Ilex matte", &ctx);
    t.height_mm = MATTE_DEPTH_MM;
    t.v_center_mm = SHANK_V.0;
    t.v_span_mm = SHANK_V.1;
    t.repeats_around = (ctx.circumference_mm / STIPPLE_CELL_MM).round() as u32;
    t.rows = SHANK_ROWS;
    t.feather_mm = 0.0;
    t.continuous = true;
    for (name, (from, to)) in ["Shoulder stipple, left", "Shoulder stipple, right"].into_iter().zip(spans) {
        let mut e = LayerEntry::new(name, Layer::Tiling(t.clone()));
        e.blend = Blend::Subtract;
        e.bench_only = true;
        e.mask = Some("Ilex shoulder mask".into());
        e.window = Window { enabled: true, theta_deg: 0.5 * (from + to), span_deg: (to - from).abs() + 2.0, fade_deg: 1.0, invert: false, v_gate: VGate::Off };
        d.layers.layers.push(e);
    }
}

/// The garland's stem down each shoulder: short overlapping capsules struck along the band's centre line, each a
/// flat-topped drafted stamp, so its edges are true geometry. Returns each side's path in chart degrees and `v`, for the
/// stipple's mask.
fn garland_stem(d: &mut RingDesign, a: &Atlas, hide: &Hide, from_along: f64, to_along: f64) -> Vec<Vec<[f64; 2]>> {
    let mut paths = Vec::new();
    let count = ((to_along - from_along) / STEM_PITCH_MM).ceil() as usize;
    let pitch = (to_along - from_along) / count as f64;
    // A stadium a pitch and an overlap long, `w` wide: straight sides, round ends.
    let capsule = |w: f64| -> Vec<[f64; 2]> {
        let len = pitch + STEM_OVERLAP_MM;
        (0..48)
            .map(|i| {
                let t = std::f64::consts::TAU * i as f64 / 48.0;
                let (c, sn) = (t.cos(), t.sin());
                let x = if c.abs() > 1e-9 { 0.5 * (len - w) * c.signum() } else { 0.0 } + 0.5 * w * c;
                [x, 0.5 * w * sn]
            })
            .collect()
    };
    for (side, sign) in [("left", -1.0f64), ("right", 1.0)] {
        let mut pts = Vec::new();
        for k in 0..count {
            let along = sign * (from_along + pitch * (k as f64 + 0.5));
            let at = hide.crest_at(a, along);
            d.stamps.push(Stamp {
                name: format!("Garland stem, {side} {}", k + 1),
                theta_deg: at.0,
                v_mm: at.1,
                rot_deg: 0.0,
                // Tapering from the head toward the palm.
                outline: capsule(STEM_MM.0 * (1.0 - STEM_TAPER * k as f64 / (count - 1).max(1) as f64)),
                height_mm: STEM_MM.1,
                sink_mm: 0.3,
                // Steep drafted walls under a flat top: every capsule the same height, so the overlaps leave no joint.
                draft_deg: STEM_DRAFT_DEG,
                cut: false,
                bench: false,
                along_pull: false,
                fine_cap: true,
                tier: 0,
                top: StampTop::Flat,
            });
        }
        let mut last: Option<f64> = None;
        for k in 0..=count {
            let (theta, v) = hide.crest_at(a, sign * (from_along + pitch * k as f64));
            let theta = last.map_or(theta, |prev| prev + crate_wrap(theta - prev));
            last = Some(theta);
            pts.push([theta, v]);
        }
        paths.push(pts);
    }
    paths
}

fn crate_wrap(d: f64) -> f64 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

/// Seven graded leaves a side down both shoulders on the band's centre line, each turned off the stem by turns and
/// pivoted on its stalk so the stalk stays on the stem while the blade leans off it.
fn garland(d: &mut RingDesign, a: &Atlas, hide: &Hide, placed: &mut Placed) -> Result<(Vec<Vec<[f64; 2]>>, Vec<Vec<[f64; 3]>>)> {
    let past_folds = placed.folds_along_mm.iter().map(|f| f.abs()).fold(0.0, f64::max) + 1.0;
    let first = past_folds + 2.8;
    // The stem starts a millimetre past the fold, where the line has turned down the head's end wall.
    // It ends under the last leaf, which covers its tip.
    let stems = garland_stem(d, a, hide, past_folds, first + GARLAND_SPAN_MM + 0.8);
    let mut halos = Vec::new();
    let from = hide.crest_at(a, -first);
    // The line runs on through theta 0: unwrap the far end below the near one.
    let to = from.0 - (from.0 - hide.crest_at(a, -(first + GARLAND_SPAN_MM)).0).rem_euclid(360.0);
    let mut leaf = holly_stamp("Garland leaf", (0.0, 0.0), 4.2, 2.4, 3, 0.35, GARLAND_EAVES, GARLAND_CROWN);
    // Stations run down the shoulder toward theta 0: the tip leads away from the head.
    leaf.rot_deg = 180.0;
    let row = StampRow { stamp: leaf, path: RowPath::PartingLine, from_deg: from.0, to_deg: to, count: GARLAND_COUNT, taper: 0.24, fold_clear_mm: 1.0, mirror_shoulders: true };
    let struck = stamp_row(d, &row);
    let ctx = d.field_context();
    for s in struck {
        let k: usize = s.name.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(1);
        let splay = if k % 2 == 0 { GARLAND_SPLAY_DEG } else { -GARLAND_SPLAY_DEG };
        let len = s.outline.iter().map(|p| p[0]).fold(f64::MIN, f64::max) - s.outline.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        // Turn the leaf, then slide it across the line by the half-leaf's swing, whichever way keeps its stalk on
        // the stem (the band's mid-plane, z = 0).
        let shift = 0.5 * len * splay.to_radians().sin().abs();
        let m = [1.0, -1.0]
            .into_iter()
            .map(|sign| {
                let mut m = s.clone();
                m.rot_deg += splay;
                m.v_mm += sign * shift;
                let stalk = m.frame(d, &ctx).point([-0.5 * len, 0.0, 0.0]);
                (stalk[2].abs(), m)
            })
            .min_by(|p, q| p.0.total_cmp(&q.0))
            .map(|(_, m)| m)
            .expect("two candidates");
        placed.garland.push(format!("{} at {:.2} deg, v {:.3}, turned {:.2} deg", m.name, m.theta_deg, m.v_mm, m.rot_deg));
        let lo = m.outline.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        halos.push(world_outline(d, &m, &grown(&m.outline, GARLAND_HALO_MM, lo)));
        d.stamps.push(m);
    }
    Ok((stems, halos))
}

/// Where a mesh's degenerate triangles lie (zero area or zero length), as their centroids.
fn degenerate_at(m: &mesh::Mesh) -> Vec<[f64; 3]> {
    m.faces
        .iter()
        .filter_map(|f| {
            let p = f.map(|i| m.vertices[i as usize]).map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]);
            let (u, w) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
            let c = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
            let area = 0.5 * dot(c, c).sqrt();
            (area < 1e-10).then(|| [0, 1, 2].map(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.0))
        })
        .collect()
}

/// The kernel refuses a few otherwise sound placements ("two cuts cross inside a face"), or joins one with a sliver
/// of zero-area triangles, and which depends on the build size. Nudge each refused stamp, or the cast stamp nearest a
/// degenerate triangle of the finished ring or the casting pattern, with its veins, by a hair (a third of a degree, a
/// hundredth of a millimetre) until every build size joins them all cleanly.
fn settle(d: &mut RingDesign, lib: &AlphaLibrary, sizes: &[BuildParams], log: &mut Vec<String>) -> Result<()> {
    for attempt in 1..=12 {
        let mut refused = std::collections::BTreeSet::new();
        let ctx = d.field_context();
        let origins: Vec<(String, [f64; 3])> = d.stamps.iter().filter(|s| !s.bench).map(|s| (s.name.clone(), s.frame(d, &ctx).origin)).collect();
        let nearest = |p: [f64; 3]| origins.iter().min_by(|a, b| {
            let da = (a.1[0] - p[0]).powi(2) + (a.1[1] - p[1]).powi(2) + (a.1[2] - p[2]).powi(2);
            let db = (b.1[0] - p[0]).powi(2) + (b.1[1] - p[1]).powi(2) + (b.1[2] - p[2]).powi(2);
            da.total_cmp(&db)
        }).map(|o| o.0.clone());
        for p in sizes {
            let b = mesh::try_build(d, lib, p.clone())?;
            refused.extend(b.solids.notes.iter().filter_map(|n| n.split_once(": could not be").map(|(name, _)| name.to_string())));
            if std::env::var("ILEX_DEBUG").is_ok() {
                eprintln!("ring {}x{} notes {:?}", p.theta_steps, p.profile_steps, b.solids.notes);
                eprintln!("ring {}x{} degenerate at {:?}", p.theta_steps, p.profile_steps, degenerate_at(&b.mesh));
            }
            refused.extend(degenerate_at(&b.mesh).into_iter().filter_map(&nearest));
            if refused.is_empty() {
                let pattern = mesh::try_build_pattern(d, lib, p.clone())?;
                if std::env::var("ILEX_DEBUG").is_ok() {
                    eprintln!("pattern {}x{} degenerate at {:?}", p.theta_steps, p.profile_steps, degenerate_at(&pattern.mesh));
                }
                refused.extend(degenerate_at(&pattern.mesh).into_iter().filter_map(&nearest));
            }
            if !refused.is_empty() {
                break;
            }
        }
        if refused.is_empty() {
            return Ok(());
        }
        let sign = if attempt % 2 == 0 { -1.0 } else { 1.0 };
        for s in d.stamps.iter_mut().filter(|s| refused.iter().any(|r| s.name == *r || s.name.starts_with(&format!("{r} ")))) {
            s.rot_deg += sign * 0.35 * attempt as f64;
            s.v_mm += sign * 0.011 * attempt as f64;
            if refused.contains(&s.name) {
                log.push(format!("{}: nudged to rot {:.2} deg, v {:.3} (attempt {attempt})", s.name, s.rot_deg, s.v_mm));
            }
        }
    }
    anyhow::bail!("stamps still refused after 12 nudges: {log:?}")
}

// --- Gates -------------------------------------------------------------------------------------------------------

fn self_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() })
}

/// The closest any vertex comes to the finger axis, less the bore radius, mm.
fn bore_margin(d: &RingDesign, m: &mesh::Mesh) -> f64 {
    let r = d.inner_radius_mm();
    m.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64) - r).fold(f64::MAX, f64::min)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Metal vertices standing inside each cabochon, 0.03 mm in from its surface.
fn metal_in_stones(d: &RingDesign, m: &mesh::Mesh) -> Vec<(String, usize)> {
    ringdesign_core::stones::stone_frames(d)
        .into_iter()
        .map(|(st, f)| {
            let (ra, rb, h) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03, st.gem.depth_mm() - 0.03);
            let n = m
                .vertices
                .iter()
                .filter(|p| {
                    let q = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
                    let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                    z > 0.03 && z < h && (x / ra).powi(2) + (y / rb).powi(2) < 1.0 - (z / h).powi(2)
                })
                .count();
            (st.label, n)
        })
        .collect()
}

/// Stones the gem preview draws: its triangles welded into connected pieces.
fn preview_count(d: &RingDesign, lib: &AlphaLibrary) -> usize {
    let v = ringdesign_core::gems::preview_vertices(d, lib);
    let mut index = std::collections::HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let n = p[j];
            p[j] = r;
            j = n;
        }
        r
    }
    for tri in v.chunks_exact(36) {
        let ids: Vec<usize> = (0..3)
            .map(|k| {
                let key = [tri[k * 12], tri[k * 12 + 1], tri[k * 12 + 2]].map(|c| (c * 1e3).round() as i64);
                *index.entry(key).or_insert_with(|| {
                    parent.push(parent.len());
                    parent.len() - 1
                })
            })
            .collect();
        for k in 1..3 {
            let (a, b) = (find(&mut parent, ids[0]), find(&mut parent, ids[k]));
            parent[a] = b;
        }
    }
    let n = parent.len();
    (0..n).filter(|&i| find(&mut parent, i) == i).count()
}

/// Every gate at one build size, and whether all are green.
fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool, mesh::BuildResult)> {
    let started = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = self_crossings(&built.mesh);
    let margin = bore_margin(d, &built.mesh);
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    let (inspection, fine) = pull(d, lib, params)?;
    let coarse_release = &inspection.release;
    let monotone: Vec<(String, bool)> = d
        .stamps
        .iter()
        .filter(|s| !s.bench && !s.along_pull)
        .map(|s| {
            let r = s.parting_monotone(d);
            if let (Err(bad), true) = (&r, std::env::var("ILEX_DEBUG").is_ok()) {
                eprintln!("{} not monotone at {:?}", s.name, bad.iter().map(|i| s.outline[*i]).map(|p| [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0]).collect::<Vec<_>>());
            }
            (s.name.clone(), r.is_ok())
        })
        .collect();
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = preview_count(d, lib);
    let inside = metal_in_stones(d, &built.mesh);
    let closest_gap = stones.as_ref().and_then(|s| s.closest.as_ref()).map_or(f64::MAX, |p| p.gap_mm.min(p.gap_deep_mm));
    let closest = stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm));
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let castable = field.verdict == castability::Verdict::Castable;
    let lost_wax = d.draft.process == CastProcess::LostWax;
    // Lost wax: the wall census on the finished ring's own mesh (every face by area). Its gate is `clean()`:
    // assessed, nothing unresolved and no wall sample under the floor; the edges (tips, lips) are recorded as read.
    let census = wall_census(&built.mesh);
    let walls_ok = census.clean() && field.thinnest_wall_mm >= MIN_SECTION_MM - 1e-9;
    let sand_ok = coarse_release.obstructions.is_empty()
        && coarse_release.unresolved_rays == 0
        && fine.obstructions.is_empty()
        && fine.unresolved_rays == 0
        && monotone.iter().all(|(_, ok)| *ok);
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && margin >= -0.01
        && castable
        && (if lost_wax { walls_ok } else { sand_ok })
        && findings.is_empty()
        && reported == previewed
        && closest_gap >= 0.1
        && inside.iter().all(|(_, n)| *n == 0)
        && built.mesh.faces.len() <= 2_000_000;
    let g = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "min_angle_deg": q.min_angle_deg,
        "self_crossings": crossings,
        "made_parts": [],
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "stamps_struck": built.solids.stamped,
        "seats_resolved": built.solids.resolved,
        "bore_margin_mm": margin,
        "process": d.draft.process.label(),
        "min_section_mm": d.draft.min_section_mm,
        "thinnest_wall_mm_floor": MIN_SECTION_MM,
        "thinnest_wall_pass": walls_ok,
        "wall_census": json!({ "clean": census.clean(), "assessed": census.assessed, "rays": census.rays, "unresolved": census.unresolved, "wall_samples": census.below_limit, "wall_area_mm2": census.wall_area_mm2, "walls": census.walls, "edge_samples": census.edge_below_limit, "edge_area_mm2": census.edge_area_mm2, "edges": census.edges, "edge_reach_mm": census.edge_reach_mm, "pitch_mm": census.pitch_mm, "sampled_min_mm": census.sampled_min_mm, "at": census.point, "note": census.note }),
        "sand_bonus": json!({ "pulls_from_sand_as_is": sand_ok, "why": "Lost wax ring: the pull, the ray release and the parting-line rule are measured and reported here, never gated." }),
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0100": release_line(coarse_release),
        "release_0100_obstructions": coarse_release.obstructions.len(),
        "release_0100_low_draft_area_mm2": coarse_release.low_draft_area_mm2,
        "release_0100_sand_findings": coarse_release.sand_findings.iter().map(|f| format!("{} at [{:.2}, {:.2}, {:.2}]", f.message, f.point[0], f.point[1], f.point[2])).collect::<Vec<_>>(),
        "release_0100_notes": coarse_release.notes,
        "release_0100_unresolved": coarse_release.unresolved_rays,
        "release_0075": release_line(&fine),
        "release_0075_obstructions": fine.obstructions.len(),
        "release_0075_obstruction_at": fine.obstructions.iter().map(|o| format!("[{:.2}, {:.2}, {:.2}] {:.3} mm deep", o.world[0], o.world[1], o.world[2], o.depth_mm)).collect::<Vec<_>>(),
        "release_0075_unresolved": fine.unresolved_rays,
        "clamp_bites_mm": [],
        "clamp_note": "No painted relief: nothing passes through skin::draft_clamp (and lost wax has no clamp gate).",
        "parting_monotone": monotone,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "metal_inside_stones": inside,
        "stone_carats": stones.as_ref().map_or(0.0, |s| s.total_carats),
        "stone_warnings": warnings,
        "closest_stones": closest,
        "grams_18k": built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams),
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders -----------------------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn save_rgb(path: &Path, rgb: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The 300 px read and a contact sheet of every view at that size.
    let small: Vec<Vec<u8>> = VIEWS.iter().map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3)).collect();
    save_rgb(&out.join("hero-300.png"), &small[0], 300, 300)?;
    save_rgb(&out.join("face-300.png"), &small[1], 300, 300)?;
    let (cols, rows) = (3, 2);
    let mut sheet = vec![0u8; cols * 300 * rows * 300 * 3];
    for (i, img) in small.iter().enumerate() {
        let (cx, cy) = (i % cols, i / cols);
        for y in 0..300 {
            let dst = ((cy * 300 + y) * cols * 300 + cx * 300) * 3;
            sheet[dst..dst + 900].copy_from_slice(&img[y * 900..(y + 1) * 900]);
        }
    }
    save_rgb(&out.join("contact-300.png"), &sheet, cols * 300, rows * 300)?;
    // The stones close-up frames the face.
    let frames = ringdesign_core::stones::stone_frames(d);
    let top = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    let centre = if frames.is_empty() { [0.0, top, 0.0] } else { [0.0, top - 1.0, 0.0] };
    render::write_png_framed(out.join("stones.png"), &parts, 0.3, 1.15, render::Framing::new(centre, 11.0), edge)?;
    // Close-ups, framed on the whole ring, never a cropped mesh: one face leaf from above, a cheek spray square on,
    // and the garland down a shoulder.
    let leaf_centre = [-4.7, top, 0.0];
    render::write_png_framed(out.join("leaf.png"), &parts, render::yaw_facing(90.0), 1.35, render::Framing::new(leaf_centre, 5.5), edge)?;
    let cheek_y = top - 4.0;
    render::write_png_framed(out.join("cheek.png"), &parts, 0.0, 0.1, render::Framing::new([0.0, cheek_y, 8.5], 7.0), edge)?;
    // The garland seen square on, down the right shoulder: the camera faces ring angle 150 deg.
    let (sin, cos) = 150f64.to_radians().sin_cos();
    let r = 11.6;
    render::write_png_framed(out.join("garland.png"), &parts, render::yaw_facing(150.0), 1.3, render::Framing::new([r * cos, r * sin, 0.0], 7.0), edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.stamps.clear();
    bare.layers.layers.clear();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    Ok(())
}

// --- Main --------------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/ilex"));
    std::fs::create_dir_all(&out)?;
    println!("Ilex");
    let step1 = vec![resize_check(FACE, SAND)?, resize_check(NATIVE, SAND)?];
    for s in &step1 {
        println!("  resize {s}");
    }
    if args.iter().any(|a| a == "--resize-check") {
        std::fs::write(out.join("resize-check.json"), serde_json::to_vec_pretty(&step1)?)?;
        return Ok(());
    }
    // Step 1 settled: the 16 x 17 face where it builds clean with at most 0.3 mm of envelope, else the native face.
    let resized = &step1[0];
    let face = if resized["builds"] == json!(true) && resized["envelope_fill_mm"].as_f64().unwrap_or(9.0) <= 0.3 && resized["watertight"] == json!(true) {
        FACE
    } else {
        NATIVE
    };
    println!("  face {} x {} mm", face.0, face.1);
    let (mut d, lib, mut placed) = author(face, blockout)?;
    let sizes = if draft { vec![coarse_params(), draft_params()] } else { vec![coarse_params(), draft_params(), export_params()] };
    settle(&mut d, &lib, &sizes, &mut placed.nudged)?;
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: pass {draft_pass}");
    if std::env::var("ILEX_QUICK").is_ok() {
        std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&json!({ "placed": placed, "draft": draft_gates }))?)?;
        let b = if draft { draft_built } else { mesh::try_build(&d, &lib, export_params())? };
        return renders(&out, &d, &lib, b, if draft { 800 } else { 1600 });
    }
    let (coarse_gates, coarse_pass, _) = gates(&d, &lib, coarse_params())?;
    println!("  384 x 192: pass {coarse_pass}");
    let (export_gates, export_pass, built) = if draft { (Value::Null, true, draft_built) } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: pass {p}");
        (g, p, b)
    };
    library::save_design(out.join("design.ring.json"), &d)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
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
    let mut pattern_gates = Value::Null;
    let mut pattern_pass = true;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.mesh.quality();
        let pc = self_crossings(&pattern.mesh);
        let pv = pattern.mesh.validate();
        pattern_pass = pv.watertight && pq.degenerate_faces == 0 && pc == 0;
        pattern_gates = json!({ "triangles": pattern.mesh.faces.len(), "watertight": pv.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "pass": pattern_pass });
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let fin = render::finished_from(&d, &lib, mesh::try_build(&d, &lib, params)?);
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-garnet.stl".to_string() } else { format!("reference-garnet-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Ilex reference stone")?;
            materials.push(json!({ "mesh": file, "name": "Garnet", "tint": tint, "ior": 1.79, "dispersion": 0.024, "roughness": 0.065, "transmission": 0.55 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    let all = draft_pass && coarse_pass && export_pass && pattern_pass && cold != Some(false) && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": "ilex",
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "process_decision": "Lost wax (Logan, 2026-10-03): the sand gates were what held the sprig back. 0.8 mm minimum section, no pull rule; the sand numbers are kept in each gate block as a bonus.",
        "crisp_relief": d.crisp_relief,
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "resize_check": step1,
        "face_mm": [face.0, face.1],
        "placed": placed,
        "stamps": d.stamps.iter().map(|s| json!({ "name": s.name, "theta_deg": s.theta_deg, "v_mm": s.v_mm, "bench": s.bench, "cut": s.cut, "tier": s.tier, "along_pull": s.along_pull, "height_mm": s.height_mm })).collect::<Vec<_>>(),
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "design_bytes": text.len(),
        "design_format": serde_json::from_str::<Value>(&text)?.get("format_version").cloned(),
        "draft": draft_gates,
        "coarse_384x192": coarse_gates,
        "export": export_gates,
        "casting_pattern": pattern_gates,
        "cold_reload_identical": cold,
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if all { "passed" } else { "FAILED" });
    ensure!(all || draft, "Ilex failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
