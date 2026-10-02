//! Vepres — Ilex, the Holly King's standard: a holly leaf struck along the parting line of factory 006 between two
//! garnet berries, poured in Delft sand through the sand master.
//! cargo build --release -p ringdesign-core --example vepres_ilex
//! target/release/examples/vepres_ilex [OUT_DIR] [--draft] [--verify] [--blockout] [--resize-check]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    castability::{self, SandProcess},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle},
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

/// The face asked for, length round the ring by width across it, and the stock's native fallback.
const FACE: (f64, f64) = (16.0, 17.0);
const NATIVE: (f64, f64) = (16.0, 21.0);
const BORE_MM: f64 = 18.6;

/// Holly's red, a little brighter than a garnet's own black-red so the berry reads at arm's length.
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
    s.recipe.name = "Ilex / Delft clay / Gold 18k".into();
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Factory 006 through the sand master, z = 0 parting, opposed z withdrawal. Every leaf on the face and \
        shoulders lies on the parting line with its midrib on it; the cheek leaves stand along the pull. After the pour: cut \
        the vein combs, drill each garnet seat on its raised mark, cut it to the measured stone and burnish flush. Polish \
        the field and the palm; leave the veins satin."
        .into();
    s
}

/// Factory 006 Square through the sand master at `face` (length, width), on its own Flat chart.
fn stock(face: (f64, f64)) -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "006").context("no stock 006")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, sand_master(preset.load()?)?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = true;
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
fn resize_check(face: (f64, f64)) -> Result<Value> {
    let d = stock(face)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let lib = AlphaLibrary::builtin();
    let built = mesh::try_build(&bare, &lib, draft_params());
    let (fill, fill_at) = fill_mm(&d);
    let mut out = json!({ "face_mm": [face.0, face.1], "envelope_fill_mm": fill, "envelope_fill_theta_deg": fill_at });
    match built {
        Ok(b) => {
            let q = b.mesh.quality();
            let top = b.mesh.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
            let (x, z) = b.mesh.vertices.iter().filter(|v| v.1 as f64 > top - 0.15).fold((0.0f64, 0.0f64), |(x, z), v| (x.max(v.0.abs() as f64), z.max(v.2.abs() as f64)));
            let (inspection, fine) = pull(&d, &lib, coarse_params())?;
            out["builds"] = json!(true);
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
fn holly(len: f64, wid: f64, spines: usize, bay: f64) -> Vec<[f64; 2]> {
    let l = len;
    let x0 = -0.5 * l;
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
                proud * (1.0 - d).max(0.0).powf(1.6)
            })
            .sum();
        (env(x) - dip + spike).max(0.0)
    };
    let step = 0.03;
    let count = (l / step).round() as usize;
    let upper: Vec<[f64; 2]> = (1..count).map(|i| x0 + l * i as f64 / count as f64).map(|x| [x, margin(x)]).filter(|p| p[1] > 1e-3).collect();
    let mut out: Vec<[f64; 2]> = vec![[x0, 0.0]];
    out.extend(upper.iter().map(|p| [p[0], -p[1]]));
    out.push([0.5 * l, 0.0]);
    out.extend(upper.iter().rev().copied());
    if std::env::var("ILEX_DEBUG").is_ok() {
        eprintln!("holly {len}x{wid}: {} pts, check {:?}, crossing {:?}", out.len(), ringdesign_core::outline::check(&out), ringdesign_core::outline::self_crossing(&out));
        eprintln!("{:?}", out.iter().map(|p| [(p[0] * 100.0).round() / 100.0, (p[1] * 100.0).round() / 100.0]).collect::<Vec<_>>());
    }
    out
}

/// A struck holly leaf: stalk at -x, tip at +x, midrib on y = 0, its gable ridge the midrib.
fn holly_stamp(name: &str, at: (f64, f64), len: f64, wid: f64, spines: usize, spine_depth: f64, height: f64, rise: f64) -> Stamp {
    Stamp {
        name: name.into(),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg: 0.0,
        outline: holly(len, wid, spines, spine_depth),
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        fine_cap: false,
        tier: 0,
        top: StampTop::Gable { rise_mm: rise, axis_deg: 0.0 },
    }
}

/// A leaf's veins as one bench comb: a midrib `len` long and `pairs` laterals a side leaning to the tip, `w` wide.
fn vein_comb(len: f64, pairs: usize, w: f64) -> Vec<[f64; 2]> {
    let h = 0.5 * w;
    let (x0, x1) = (-0.5 * len, 0.5 * len);
    let lean = 50f64.to_radians();
    let roots: Vec<f64> = (0..pairs).map(|i| x0 + len * (0.18 + 0.62 * i as f64 / (pairs.max(2) - 1) as f64)).collect();
    // Laterals shorten toward the tip with the blade.
    let reach = |x: f64| 0.30 * len * (1.0 - ((x - x0) / len - 0.35).abs() * 1.1).max(0.35);
    let (c, s) = (lean.cos(), lean.sin());
    let mut lower = vec![[x0, -h * 0.6]];
    for &x in &roots {
        let r = reach(x);
        lower.push([x - h / s, -h]);
        lower.push([x - h / s + r * c, -h - r * s]);
        lower.push([x + h / s + r * c, -h - r * s]);
        lower.push([x + h / s, -h]);
    }
    lower.push([x1, -0.02]);
    let mut out = lower.clone();
    out.extend(lower.iter().rev().skip(1).take(lower.len() - 2).map(|p| [p[0], -p[1]]));
    out.push([x0, h * 0.6]);
    out
}

fn bench_veins(name: &str, leaf: &Stamp, len: f64, pairs: usize) -> Stamp {
    Stamp {
        name: name.into(),
        theta_deg: leaf.theta_deg,
        v_mm: leaf.v_mm,
        rot_deg: leaf.rot_deg,
        outline: vein_comb(len, pairs, 0.16),
        height_mm: 0.1,
        sink_mm: 0.15,
        draft_deg: 0.0,
        cut: true,
        bench: true,
        along_pull: leaf.along_pull,
        fine_cap: false,
        tier: 1,
        top: StampTop::Flat,
    }
}

/// How a berry is set: in a cast gypsy mound `mound` mm across, or flush in the surface with no mound.
#[derive(Clone, Copy)]
enum Set {
    Mound(f64),
    /// Flush on the parting line, where a raised drill mark pulls.
    FlushMarked,
    /// Flush off the line, where any mark or mound faces its own mould half: wholly bench work.
    FlushBench,
}

/// A garnet berry in a flush (gypsy) setting.
fn berry(d: &mut RingDesign, name: &str, at: (f64, f64), gem: Gem, set: Set) {
    let mut seat = SeatPadLayer {
        theta_deg: at.0,
        v_mm: at.1,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        mark_mm: if matches!(set, Set::FlushBench) { 0.0 } else { 0.6 },
        ..Default::default()
    };
    seat.fit_stone(gem);
    match set {
        Set::Mound(m) => seat.diameter_mm = m,
        _ => seat.height_mm = 0.0,
    }
    let mut e = LayerEntry::new(name, Layer::SeatPad(seat));
    e.blend = Blend::Max;
    e.bench_only = matches!(set, Set::FlushBench);
    d.layers.layers.push(e);
}

/// Away from the head's centre the parting line runs a hair off the ring's tangent, so a long leaf struck square
/// to the tangent lifts its tip off the line. Turn it, by the least that works, until it lies along the line.
fn level_on_line(d: &RingDesign, s: &mut Stamp) {
    let base = s.rot_deg;
    for k in 0..=60 {
        for sign in [1.0, -1.0] {
            s.rot_deg = base + sign * 0.05 * k as f64;
            if s.parting_monotone(d).is_ok() {
                return;
            }
        }
    }
    s.rot_deg = base;
}

/// What was struck and where, for the report.
#[derive(Default, serde::Serialize)]
struct Placed {
    face_mm: [f64; 2],
    leaf_at: Vec<[f64; 2]>,
    berries_at: Vec<[f64; 2]>,
    folds_along_mm: Vec<f64>,
    reach_mm: f64,
    garland: Vec<String>,
    cheek: Vec<String>,
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

/// Face berries: one on the parting line at the sprig's heart, and the cluster's other two just off it.
const FACE_BERRIES: [(f64, f64, bool); 3] = [(0.0, 0.0, true), (-1.15, 1.95, false), (1.15, 1.95, false)];
/// The face leaves: the stalk's distance from the head's centre and the leaf's length and width, mm.
const FACE_LEAF: (f64, f64, f64) = (1.7, 5.3, 4.4);

/// A cheek spray, seen along the finger with x round the ring and y up from the axis: leaves (centre, length,
/// width, turn of the tip from +x in degrees) round a cluster of three berries.
const CHEEK_LEAVES: [([f64; 2], f64, f64, f64); 3] = [([-4.3, 11.6], 5.2, 3.0, 185.0), ([4.4, 12.0], 4.8, 2.7, -3.0), ([5.9, 9.2], 3.2, 2.0, -35.0)];
const CHEEK_BERRIES: [[f64; 2]; 3] = [[-0.95, 12.6], [0.95, 12.6], [0.0, 11.5]];
const CHEEK_SIDES: [f64; 2] = [1.0, -1.0];

fn author(face: (f64, f64), blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock(face)?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let mut placed = Placed { face_mm: [face.0, face.1], reach_mm: hide.reach(), folds_along_mm: hide.folds(&a, 12.0), ..Default::default() };
    // The face: a sprig of two leaves end to end on the parting line, midribs on it, stalks meeting at the berries.
    let (stalk, len, wid) = FACE_LEAF;
    let long = if face.1 > 19.0 { 0.5 } else { 0.0 };
    for (k, sign) in [-1.0f64, 1.0].into_iter().enumerate() {
        let at = hide.crest_at(&a, sign * (stalk + 0.5 * (len + long)));
        placed.leaf_at.push([at.0, at.1]);
        if std::env::var("ILEX_DEBUG").is_ok() {
            let p = a.point(at.0, at.1);
            let q = a.point(at.0 + 0.5, at.1);
            eprintln!("face leaf {k} at {at:?}: point {p:?}, 0.5 deg on {q:?}");
        }
        let mut leaf = holly_stamp(&format!("Face leaf, {}", k + 1), at, len + long, wid, 3, 0.55, 0.6, 0.6);
        // The tip leads away from the heart; theta grows toward +x.
        leaf.rot_deg = if sign < 0.0 { 180.0 } else { 0.0 };
        level_on_line(&d, &mut leaf);
        d.stamps.push(leaf.clone());
        if !blockout {
            d.stamps.push(bench_veins(&format!("Face leaf veins, {}", k + 1), &leaf, (len + long) * 0.86, 4));
        }
    }
    for (k, (x, z, line)) in FACE_BERRIES.into_iter().enumerate().filter(|(_, b)| b.2 || std::env::var("ILEX_NO_OFFLINE").is_err()) {
        let at = if line { hide.crest_at(&a, x) } else { on_face(&a, x, z) };
        placed.berries_at.push([at.0, at.1]);
        // On the line a gypsy mound casts and pulls; off it any mound's inner flank faces its own mould half, so
        // those two are drilled flush into the table at the bench.
        berry(&mut d, &format!("Face berry, {}", k + 1), at, garnet(2.0), if line { Set::FlushMarked } else { Set::FlushBench });
    }
    // The cheeks face the pull: a spray each side, leaves struck along it.
    for (side_k, side) in CHEEK_SIDES.into_iter().enumerate().filter(|_| std::env::var("ILEX_NO_CHEEK").is_err()) {
        for (k, (c, len, wid, turn)) in CHEEK_LEAVES.into_iter().enumerate() {
            let at = on_cheek(&a, c[0], c[1], side).with_context(|| format!("no cheek at {c:?}"))?;
            let mut leaf = holly_stamp(&format!("Cheek leaf {}, {}", side_k + 1, k + 1), at, len, wid, 3, 0.4, 0.6, 0.25);
            leaf.along_pull = true;
            // Theta runs one way round on the cheek facing +z and the other on its mirror.
            leaf.rot_deg = if side > 0.0 { turn } else { 180.0 - turn };
            placed.cheek.push(format!("{} at {:.2} deg, v {:.3}", leaf.name, at.0, at.1));
            d.stamps.push(leaf.clone());
            if !blockout {
                d.stamps.push(bench_veins(&format!("Cheek leaf veins {}, {}", side_k + 1, k + 1), &leaf, len * 0.86, 3));
            }
        }
        for (k, c) in CHEEK_BERRIES.into_iter().enumerate() {
            let at = on_cheek(&a, c[0], c[1], side).with_context(|| format!("no cheek at {c:?}"))?;
            placed.cheek.push(format!("Cheek berry {}, {} at {:.2} deg, v {:.3}", side_k + 1, k + 1, at.0, at.1));
            berry(&mut d, &format!("Cheek berry {}, {}", side_k + 1, k + 1), at, garnet(1.8), Set::Mound(2.3));
        }
    }
    if !blockout {
        garland(&mut d, &a, &hide, &mut placed)?;
    }
    Ok((d, lib, placed))
}

/// Four graded leaves a side on the parting line down both shoulders.
fn garland(d: &mut RingDesign, a: &Atlas, hide: &Hide, placed: &mut Placed) -> Result<()> {
    let start = hide.crest_at(a, -(0.5 * d.shank.head.length_mm + 1.0 + 2.1));
    let leaf = holly_stamp("Garland leaf", (0.0, 0.0), 4.2, 2.4, 3, 0.35, 0.45, 0.18);
    let row = StampRow { stamp: leaf, path: RowPath::PartingLine, from_deg: start.0, to_deg: start.0 - 32.0, count: 4, taper: 0.24, fold_clear_mm: 1.0, mirror_shoulders: true };
    let struck = stamp_row(d, &row);
    placed.garland = struck.iter().map(|s| format!("{} at {:.2} deg, v {:.3}", s.name, s.theta_deg, s.v_mm)).collect();
    d.stamps.extend(struck);
    Ok(())
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
    let closest = stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm));
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let castable = field.verdict == castability::Verdict::Castable;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && margin >= -0.01
        && castable
        && coarse_release.obstructions.is_empty()
        && coarse_release.unresolved_rays == 0
        && fine.obstructions.is_empty()
        && fine.unresolved_rays == 0
        && monotone.iter().all(|(_, ok)| *ok)
        && findings.is_empty()
        && reported == previewed
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
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0100": release_line(coarse_release),
        "release_0100_obstructions": coarse_release.obstructions.len(),
        "release_0100_unresolved": coarse_release.unresolved_rays,
        "release_0075": release_line(&fine),
        "release_0075_obstructions": fine.obstructions.len(),
        "release_0075_unresolved": fine.unresolved_rays,
        "clamp_bites_mm": [],
        "clamp_note": "No painted relief: nothing passes through skin::draft_clamp.",
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

fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
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
    let close_metal = crop(&fin.metal, centre, 11.0);
    let mut close = vec![render::Part::metal(&close_metal, render::GOLD)];
    close.extend(fin.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    render::write_png_parts(out.join("stones.png"), &close, 0.3, 1.15, edge)?;
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
    let step1 = vec![resize_check(FACE)?, resize_check(NATIVE)?];
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
    let (d, lib, placed) = author(face, blockout)?;
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: {draft_gates}");
    let (coarse_gates, coarse_pass, _) = gates(&d, &lib, coarse_params())?;
    println!("  384 x 192: pass {coarse_pass}");
    let (export_gates, export_pass, built) = if draft { (Value::Null, true, draft_built) } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: {g}");
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
        "sand": "Delft clay, 3.0 deg draft, 0.8 mm section, 0.30 mm detail",
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
