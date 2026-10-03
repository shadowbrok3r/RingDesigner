//! Cataphracta — Chamaeleo, the casque: a chameleon's head on factory 001, its helmet rising along the parting line
//! to the occiput behind an alexandrite, its tail curled on each cheek, its hide granular; Delft sand.
//! cargo build --release -p ringdesign-core --example cataphracta_chamaeleo
//! target/release/examples/cataphracta_chamaeleo [OUT_DIR] [--draft] [--verify] [--blockout] [--probe]
#![recursion_limit = "256"]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary,
    curve::{CurveLayer, WireProfile},
    svg::SvgAlpha,
    tiling::{ChartSpace, TilingLayer}, BuildParams, ProfileStyle, RingDesign,
    castability::{self, SandProcess},
    csg, dfm,
    field::{Blend, Window, GroupLayer, Layer, LayerEntry, LayerStack, SandClamp, SeatPadLayer, SeatStyle},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    library, manufacturing as mf, mesh, outline, render,
    setting::{RowPath, SolidKind, Stamp, StampRow, StampTop, stamp_row},
    skin::{Atlas, Hide},
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const AW: usize = 2048;
const AH: usize = 768;

/// The face, length round the ring by width across it (the stock guard refuses the brief's 13).
const FACE: (f64, f64) = (17.0, 14.5);
const BORE_MM: f64 = 18.6;

/// Alexandrite in daylight: the green it shows before the candle turns it red.
const ALEXANDRITE_TINT: [f32; 3] = [0.18, 0.50, 0.38];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The ring's own Delft pour: z = 0 parting, opposed z withdrawal, sterling.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.name = "Chamaeleo / Delft clay / Silver 925".into();
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Factory 001 through the sand master, z = 0 parting, opposed z withdrawal. The head is struck in \
        profile on the parting line: every plate and tier crosses the line once and steps down away from it; the tail \
        coils stand along the pull on the cheeks. After the pour: drill the eye on its raised mark, cut the seat to the \
        measured alexandrite and burnish it flush. Polish the casque and the eye; leave the ground satin."
        .into();
    s
}

/// Factory 001 Cushion through the sand master at `face` (length, width), on its own Flat chart.
fn stock(face: (f64, f64)) -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "001").context("no stock 001")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, sand_master(preset.load()?)?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = true;
    d.name = "Chamaeleo \u{2014} the casque".into();
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

/// Step 0: the bare 001 at 17 x 14.5, before any relief: its field verdict, its pull and its mesh.
fn bare_study(face: (f64, f64)) -> Result<Value> {
    let d = stock(face)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    let lib = AlphaLibrary::builtin();
    let b = mesh::try_build(&bare, &lib, draft_params())?;
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let (inspection, fine) = pull(&d, &lib, coarse_params())?;
    let top = b.mesh.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    let (x, z) = b.mesh.vertices.iter().filter(|v| v.1 as f64 > top - 0.15).fold((0.0f64, 0.0f64), |(x, z), v| (x.max(v.0.abs() as f64), z.max(v.2.abs() as f64)));
    Ok(json!({
        "face_mm": [face.0, face.1],
        "watertight": b.report.validation.watertight,
        "degenerate_faces": b.mesh.quality().degenerate_faces,
        "table_mm": [2.0 * x, 2.0 * z],
        "bore_mm": b.report.inner_diameter_mm,
        "field_verdict": field.verdict.label(),
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "pull_384": release_line(&inspection.release),
        "pull_0075": release_line(&fine),
    }))
}

// --- The chameleon ------------------------------------------------------------------------------------------------
//
// The table is the head seen from above, snout at -x (falling theta), occiput at +x. The alexandrite sits on its
// boss at the snout's base; behind it the casque rises along the parting line to the occiput and falls at the
// table's end, flanked by two temporal crests stepping down away from it. Every plate is drawn as
// `-w(x) <= y <= w(x)`, so each line along the pull crosses it once, across the line: the sand's monotone rule by
// construction. The table's flanks are granulated at the bench; the cheeks and walls carry cast granules and the
// lateral stripe; each cheek carries the curled tail.

/// The alexandrite's station on the parting line, mm along, and the stone: a 5 x 4 cushion.
const STONE_ALONG: f64 = -4.0;
const STONE_MM: (f64, f64) = (5.0, 4.0);
/// The boss the stone is cut into: its height over the table (the casque's base), crown and skirt.
const BOSS_MM: f64 = 0.8;
/// The crests: where the outer one starts behind the boss and ends before the table's end, mm along.
const CREST_FROM: f64 = -0.6;
const CREST_TO: f64 = 7.6;
/// Half widths across the line: the outer temporal crest, the inner one, the casque.
const HALF_W: (f64, f64, f64) = (4.6, 3.45, 2.0);
/// Heights: each temporal crest step, the casque's eaves, its rise at the front and at the occiput.
const STEP_MM: f64 = 0.4;
const CASQUE_MM: f64 = 0.25;
const CASQUE_RISE: (f64, f64) = (0.2, 1.55);
/// Where the casque peaks, mm along.
const OCCIPUT: f64 = 5.0;
/// Granules: tile, land, height; the bench cuts the table's lands this deep.
const GRANULE_TILE: f64 = 5.0;
const GRANULE_LAND: f64 = 0.25;
const GRANULE_R: [f64; 3] = [0.7, 0.5, 0.33];
/// The table's bench grain: radii and land.
const GRAIN_R: [f64; 3] = [0.6, 0.42, 0.3];
const GRAIN_LAND: f64 = 0.2;
/// The walls' granules run this far either side of the head's centre, degrees: the shoulders, not the palm.
const WALL_SPAN_DEG: f64 = 70.0;
/// How squarely a wall must face the pull to take cast granules: |n_z| over this.
const SIDE_NZ: f64 = 0.97;
const GRANULE_MM: f64 = 0.35;
const TABLE_GRAIN_MM: f64 = 0.3;
/// The polished halo the granules leave round every struck form, mm.
const HALO_MM: f64 = 0.35;

/// A plate's outline from `x0` to `x1` between `lo` and `hi`, rounded into each end over `round0` and `round1` mm:
/// counter-clockwise, a point at most every 0.09 mm.
fn plate(x0: f64, x1: f64, round0: f64, round1: f64, lo: &dyn Fn(f64) -> f64, hi: &dyn Fn(f64) -> f64) -> Vec<[f64; 2]> {
    let shape = |x: f64| {
        let a = ((x - x0) / round0).clamp(0.0, 1.0);
        let b = ((x1 - x) / round1).clamp(0.0, 1.0);
        // A quarter ellipse into each end, so the end is round and no edge runs along the pull.
        (a * (2.0 - a)).sqrt() * (b * (2.0 - b)).sqrt()
    };
    let n = (((x1 - x0) / 0.002).ceil() as usize).max(8);
    let xs: Vec<f64> = (1..n).map(|i| x0 + (x1 - x0) * i as f64 / n as f64).collect();
    let lower: Vec<[f64; 2]> = xs.iter().map(|&x| [x, lo(x) * shape(x)]).collect();
    let upper: Vec<[f64; 2]> = xs.iter().rev().map(|&x| [x, hi(x) * shape(x)]).collect();
    let mut dense = vec![[x0, 0.0]];
    dense.extend(lower);
    dense.push([x1, 0.0]);
    dense.extend(upper);
    thin(&dense, 0.07)
}

/// A plate symmetric about the line, `half(x)` either side.
fn band_plate(x0: f64, x1: f64, round0: f64, round1: f64, half: &dyn Fn(f64) -> f64) -> Vec<[f64; 2]> {
    plate(x0, x1, round0, round1, &|x| -half(x), half)
}

/// Keep a point once it stands `step` from the last kept one, then split any edge over 0.09 mm.
fn thin(dense: &[[f64; 2]], step: f64) -> Vec<[f64; 2]> {
    let mut out = vec![dense[0]];
    for p in &dense[1..] {
        let q = out.last().unwrap();
        if (p[0] - q[0]).hypot(p[1] - q[1]) >= step {
            out.push(*p);
        }
    }
    // The closing edge must not be a sliver.
    while out.len() > 3 {
        let (a, b) = (out[out.len() - 1], out[0]);
        if (a[0] - b[0]).hypot(a[1] - b[1]) < 0.5 * step {
            out.pop();
        } else {
            break;
        }
    }
    let mut fine = Vec::with_capacity(out.len() * 2);
    for i in 0..out.len() {
        let (a, b) = (out[i], out[(i + 1) % out.len()]);
        let k = ((a[0] - b[0]).hypot(a[1] - b[1]) / 0.09).ceil().max(1.0) as usize;
        for j in 0..k {
            let t = j as f64 / k as f64;
            fine.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    fine
}

/// A plate struck at the head's own frame: its outline drawn in head millimetres, placed at the head's centre.
fn struck(name: &str, frame: (f64, f64, f64), outline: Vec<[f64; 2]>, height: f64, tier: u8, top: StampTop) -> Stamp {
    Stamp {
        name: name.into(),
        theta_deg: frame.0,
        v_mm: frame.1,
        rot_deg: frame.2,
        outline,
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        fine_cap: true,
        tier,
        top,
    }
}

/// Away from the head's centre the parting line runs a hair off the ring's tangent: turn the frame by the least that
/// lays every given stamp on the line.
fn level_on_line(d: &RingDesign, stamps: &mut [Stamp]) -> bool {
    let base: Vec<f64> = stamps.iter().map(|s| s.rot_deg).collect();
    // In hundredths of a degree: off a symmetric station the line's own tangent tips a few thousandths out of
    // the table's plane, and a ridge on it must lie on it to the micron.
    for k in 0..=300 {
        for sign in [1.0, -1.0] {
            for (s, b) in stamps.iter_mut().zip(&base) {
                s.rot_deg = b + sign * 0.01 * k as f64;
            }
            if stamps.iter().all(|s| s.parting_monotone(d).is_ok()) {
                return true;
            }
        }
    }
    for (s, b) in stamps.iter_mut().zip(&base) {
        s.rot_deg = *b;
    }
    false
}

fn alexandrite() -> Gem {
    Gem { l_mm: STONE_MM.0, preview_tint: Some(ALEXANDRITE_TINT), ..Gem::calibrated(GemCut::Cushion, STONE_MM.1) }
}

/// The nearest sample squarely on a head's wall to `(x, y)` seen along the finger, on the `side` of the parting line.
fn on_cheek(a: &Atlas, x: f64, y: f64, side: f64) -> Option<(f64, f64)> {
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && a.cheek(s) > 0.9)
        .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
        .filter(|(d, _)| *d < 0.2)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, s)| (s.theta, s.v))
}

/// What was struck and where, for the report.
#[derive(Default, serde::Serialize)]
struct Placed {
    face_mm: [f64; 2],
    head_frame: [f64; 3],
    stone_at: [f64; 2],
    boss: Value,
    folds_along_mm: Vec<f64>,
    reach_mm: f64,
    cheek: Vec<String>,
    levelled: bool,
    keel: Vec<String>,
    crest: Vec<String>,
}

/// The tail on the cheek, seen along the finger (x round the ring, y up from the axis): it runs along the crescent
/// over the bore and curls where the cheek is tallest, behind the occiput. Knots of its run, then the coil's centre,
/// outer and inner radius and turns, then its width at the root and at the tip, mm. The cheek holds no circle wider
/// than 1.8 mm in radius (a crescent 1.9 mm tall at its middle over the bore), so the coil fills that circle.
const TAIL_RUN: [[f64; 2]; 6] = [[-6.6, 10.25], [-5.0, 11.0], [-2.0, 11.72], [0.0, 11.9], [2.0, 11.9], [3.5, 12.3]];
const COIL_C: [f64; 2] = [5.62, 11.05];
const COIL_R: (f64, f64, f64) = (1.5, 0.32, 1.45);
const TAIL_W: (f64, f64) = (0.9, 0.4);
const TAIL_MM: f64 = 0.8;

/// The tail's centreline on the cheek, a point every 0.02 mm or so: the run, then the coil turning clockwise inward.
fn tail_line() -> Vec<[f64; 2]> {
    let (r_out, r_in, turns) = COIL_R;
    // The coil enters at its top, heading +x.
    let entry = [COIL_C[0], COIL_C[1] + r_out];
    let mut knots: Vec<[f64; 2]> = TAIL_RUN.to_vec();
    knots.push(entry);
    let mut pts = Vec::new();
    for i in 0..knots.len() - 1 {
        let p0 = knots[i.saturating_sub(1)];
        let (p1, p2) = (knots[i], knots[i + 1]);
        // The coil's own tangent at its entry, so the run flows into it.
        let p3 = if i + 2 < knots.len() { knots[i + 2] } else { [entry[0] + 1.2, entry[1]] };
        let seg = (p2[0] - p1[0]).hypot(p2[1] - p1[1]);
        let n = ((seg / 0.02).ceil() as usize).max(4);
        for k in 0..n {
            let t = k as f64 / n as f64;
            let (t2, t3) = (t * t, t * t * t);
            let c = |a: f64, b: f64, c: f64, d: f64| 0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            pts.push([c(p0[0], p1[0], p2[0], p3[0]), c(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    let span = turns * 2.0 * PI;
    let n = (span * r_out / 0.02).ceil() as usize;
    for k in 0..=n {
        let a = span * k as f64 / n as f64;
        // Tighter toward the tip, as a chameleon's tail coils.
        let r = r_out + (r_in - r_out) * a / span;
        let phi = 0.5 * PI - a;
        pts.push([COIL_C[0] + r * phi.cos(), COIL_C[1] + r * phi.sin()]);
    }
    pts
}

/// The tail as a closed outline on the cheek: the centreline offset by its tapering half width, ends rounded.
fn tail_outline() -> Vec<[f64; 2]> {
    let c = tail_line();
    let n = c.len();
    let mut len = vec![0.0; n];
    for i in 1..n {
        len[i] = len[i - 1] + (c[i][0] - c[i - 1][0]).hypot(c[i][1] - c[i - 1][1]);
    }
    let total = len[n - 1];
    let half = |i: usize| 0.5 * (TAIL_W.0 + (TAIL_W.1 - TAIL_W.0) * (len[i] / total).powf(0.7));
    let normal = |i: usize| {
        let (a, b) = (c[i.saturating_sub(1)], c[(i + 1).min(n - 1)]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy).max(1e-12);
        [-dy / l, dx / l]
    };
    let side = |i: usize, s: f64| {
        let m = normal(i);
        [c[i][0] + s * half(i) * m[0], c[i][1] + s * half(i) * m[1]]
    };
    let mut dense: Vec<[f64; 2]> = (0..n).map(|i| side(i, -1.0)).collect();
    // The tip's round cap.
    let (t, m) = (c[n - 1], normal(n - 1));
    let h = half(n - 1);
    for k in 1..24 {
        let ang = (-m[1]).atan2(-m[0]) + PI * k as f64 / 24.0;
        dense.push([t[0] + h * ang.cos(), t[1] + h * ang.sin()]);
    }
    dense.extend((0..n).rev().map(|i| side(i, 1.0)));
    // The root's round cap.
    let (r0, m0) = (c[0], normal(0));
    let h0 = half(0);
    for k in 1..24 {
        let ang = m0[1].atan2(m0[0]) + PI * k as f64 / 24.0;
        dense.push([r0[0] + h0 * ang.cos(), r0[1] + h0 * ang.sin()]);
    }
    let mut out = thin(&dense, 0.06);
    if outline::area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// A cheek-drawn outline (x round the ring, y up from the axis) as a stamp standing along the pull at the cheek
/// point nearest `centre` on the `side` of the parting line, its outline turned into the stamp's own frame.
fn cheek_stamp(a: &Atlas, name: &str, centre: [f64; 2], side: f64, drawn: &[[f64; 2]], height: f64) -> Result<Stamp> {
    let at = on_cheek(a, centre[0], centre[1], side).with_context(|| format!("no cheek at {centre:?}"))?;
    let p = a.point(at.0, at.1);
    let (q, r) = (a.point(at.0 - 0.05, at.1), a.point(at.0 + 0.05, at.1));
    // The frame `Stamp::frame` stands along the pull: x the ring's way toward rising theta, flattened; y = n x x.
    let (dx, dy) = (r[0] - q[0], r[1] - q[1]);
    let l = dx.hypot(dy);
    let x = [dx / l, dy / l];
    let y = [-side * x[1], side * x[0]];
    let mut out: Vec<[f64; 2]> = drawn
        .iter()
        .map(|w| {
            let (ox, oy) = (w[0] - p[0], w[1] - p[1]);
            [ox * x[0] + oy * x[1], ox * y[0] + oy * y[1]]
        })
        .collect();
    if outline::area(&out) < 0.0 {
        out.reverse();
    }
    Ok(Stamp {
        name: name.into(),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg: 0.0,
        outline: out,
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: true,
        fine_cap: true,
        tier: 0,
        top: StampTop::Dome { crown_mm: 0.25 },
    })
}

/// The keel's run down each shoulder, mm along the parting line from the head's centre, its width and height.
const KEEL: (f64, f64, f64, f64) = (13.2, 36.0, 2.6, 0.55);
/// The dorsal crest on the occiput's shoulder: first and last cone, mm along, count, and the first cone's diameter.
const CREST: (f64, f64, u32, f64) = (17.0, 34.5, 8, 1.5);

/// The spine: a knife keel on the parting line down both shoulders (it gives the shank's crest the draft the bare
/// stock lacks there), and on the occiput's side the dorsal crest, a graded row of cones standing on it.
fn dorsal(d: &mut RingDesign, a: &Atlas, hide: &Hide, along_per_x: f64, placed: &mut Placed) -> Result<()> {
    let (from, to, width, height) = KEEL;
    for (name, sign) in [("Keel, snout side", -1.0f64), ("Keel, occiput side", 1.0)] {
        let mut pts: Vec<[f64; 2]> = Vec::new();
        let mut last: Option<f64> = None;
        let steps = 48;
        for k in 0..=steps {
            let along = along_per_x * sign * (from + (to - from) * k as f64 / steps as f64);
            let (theta, v) = hide.crest_at(a, along);
            let theta = match last {
                None => theta,
                Some(prev) => prev + ((theta - prev + 180.0).rem_euclid(360.0) - 180.0),
            };
            last = Some(theta);
            pts.push([theta / 360.0, v]);
        }
        if pts[0][0] > pts[pts.len() - 1][0] {
            pts.reverse();
        }
        placed.keel.push(format!("{name}: {:.1} to {:.1} deg", 360.0 * pts[0][0], 360.0 * pts[pts.len() - 1][0]));
        let c = CurveLayer { points: pts, repeats_around: 1, closed: false, width_mm: width, height_mm: height, profile: WireProfile::Knife, taper: 0.3, mirror_v: false };
        let mut e = LayerEntry::new(name, Layer::Curve(c));
        e.blend = Blend::Max;
        d.layers.layers.push(e);
    }
    let (c0, c1, count, dia) = CREST;
    let (t0, t1) = (hide.crest_at(a, along_per_x * c0).0, hide.crest_at(a, along_per_x * c1).0);
    let cone = Stamp {
        name: "Dorsal cone".into(),
        theta_deg: 0.0,
        v_mm: 0.0,
        rot_deg: 0.0,
        outline: outline::circle(dia),
        height_mm: 0.15,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        fine_cap: true,
        tier: 0,
        top: StampTop::Cone { apex_mm: 0.85, at: [0.0, 0.0], tip_mm: 0.25 },
    };
    let row = StampRow { stamp: cone, path: RowPath::PartingLine, from_deg: t0, to_deg: t1, count, taper: 0.5, fold_clear_mm: 1.0, mirror_shoulders: false };
    let struck = stamp_row(d, &row);
    placed.crest = struck.iter().map(|s| format!("{} at {:.2} deg, v {:.3}", s.name, s.theta_deg, s.v_mm)).collect();
    d.stamps.extend(struck);
    Ok(())
}

/// A stamp's outline in the chart, as SVG page coordinates over the unrolled band (x = theta's share of the
/// table's circumference, y = v): each outline point carried to the world through the stamp's frame and back to the
/// chart by the nearest atlas sample.
fn chart_outline(d: &RingDesign, a: &Atlas, s: &Stamp, circ: f64) -> Vec<[f64; 2]> {
    let f = s.frame(d, &d.field_context());
    let near: Vec<&ringdesign_core::skin::Sample> = a
        .samples
        .iter()
        .filter(|q| {
            let o = [q.p[0] - f.origin[0], q.p[1] - f.origin[1], q.p[2] - f.origin[2]];
            o[0] * o[0] + o[1] * o[1] + o[2] * o[2] < 144.0
        })
        .collect();
    s.outline
        .iter()
        .step_by(2)
        .map(|p| {
            let w: [f64; 3] = std::array::from_fn(|k| f.origin[k] + p[0] * f.x[k] + p[1] * f.y[k]);
            let q = near
                .iter()
                .min_by(|m, n| {
                    let dm = (m.p[0] - w[0]).powi(2) + (m.p[1] - w[1]).powi(2) + (m.p[2] - w[2]).powi(2);
                    let dn = (n.p[0] - w[0]).powi(2) + (n.p[1] - w[1]).powi(2) + (n.p[2] - w[2]).powi(2);
                    dm.total_cmp(&dn)
                })
                .expect("a sample near the stamp");
            [q.theta / 360.0 * circ, q.v]
        })
        .collect()
}

/// A chameleon's granular hide as one periodic tile `tile` mm square: granules of three radii packed by dart
/// throwing, largest first, every land at least `land` mm; each a dome, white at its crown falling to black at its
/// rim, so the 0.5 iso-line stands at 0.86 of its radius. Seeded, so the tile is the same on every machine.
fn hide_tile(tile: f64, radii: [f64; 3], land: f64, seed: u64) -> String {
    // SplitMix64: consecutive draws are independent, so (x, y) pairs fill the square.
    let mut state = seed;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64) / ((1u64 << 53) as f64)
    };
    let mut placed: Vec<([f64; 2], f64)> = Vec::new();
    let wrap = |d: f64| d - tile * (d / tile).round();
    // Each throw takes the largest of the three radii its neighbours leave room for, so the big granules come
    // first and the small ones fill the gaps between them, as a chameleon's do.
    for _ in 0..60000 {
        let c = [next() * tile, next() * tile];
        let room = placed.iter().map(|(q, rq)| wrap(c[0] - q[0]).hypot(wrap(c[1] - q[1])) - rq - land).fold(f64::MAX, f64::min);
        if let Some(r) = radii.iter().copied().find(|r| *r <= room) {
            placed.push((c, r));
        }
    }
    let mut body = String::new();
    for (c, r) in &placed {
        // Each granule drawn at every wrap it reaches, so the tile is seamless.
        for dx in [-tile, 0.0, tile] {
            for dy in [-tile, 0.0, tile] {
                let (x, y) = (c[0] + dx, c[1] + dy);
                if x + r < 0.0 || x - r > tile || y + r < 0.0 || y - r > tile {
                    continue;
                }
                body.push_str(&format!("<circle cx=\"{x:.4}\" cy=\"{y:.4}\" r=\"{r:.4}\" fill=\"url(#g)\"/>"));
            }
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {tile} {tile}\"><defs><radialGradient id=\"g\"><stop offset=\"0\" stop-color=\"#000\"/><stop offset=\"0.55\" stop-color=\"#141414\"/><stop offset=\"0.86\" stop-color=\"#808080\"/><stop offset=\"1\" stop-color=\"#fff\"/></radialGradient></defs><rect width=\"{tile}\" height=\"{tile}\" fill=\"#fff\"/>{body}</svg>"
    )
}

fn svg_path(pts: &[[f64; 2]]) -> String {
    let body: Vec<String> = pts.iter().map(|c| format!("{:.3},{:.3}", c[0], c[1])).collect();
    format!("M{}Z", body.join(" L"))
}

/// The granules. The table's flanks cannot hold cast granules (a zero-draft table takes only steps down away from
/// the line), so they are cut at the bench: the lands between granules sunk into the cast table, the granules its
/// own surface left standing. The cheeks and walls face the pull and take cast granules in a clamped group, with
/// the lateral stripe's flat tubercles standing over them on the walls. Each is masked off the struck forms by an
/// SVG drawn in the chart, with a narrow polished halo.
fn granules(d: &mut RingDesign, a: &Atlas, boss: (f64, f64, f64, f64)) -> Result<()> {
    let ctx = d.field_context();
    let circ = 2.0 * PI * a.top;
    let page = |body: &str| format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {circ:.3} {:.3}\" preserveAspectRatio=\"none\">{body}</svg>", a.span);
    let halo = 2.0 * HALO_MM;
    let cut = |pts: &[[f64; 2]]| format!("<path d=\"{}\" fill=\"#fff\" stroke=\"#fff\" stroke-width=\"{halo:.3}\" stroke-linejoin=\"round\"/>", svg_path(pts));
    // The table's flanks: the whole table, less the outer crest and the boss, each with its halo.
    let crest = d.stamps.iter().find(|s| s.name == "Temporal crest, outer").context("no crest")?.clone();
    let (bt, bv, bl, bw) = boss;
    let bx = bt / 360.0 * circ;
    let table: Vec<&ringdesign_core::skin::Sample> = a.samples.iter().filter(|s| a.face(s) > 0.6).collect();
    ensure!(!table.is_empty(), "no table");
    let (t_lo, t_hi) = table.iter().fold((f64::MAX, f64::MIN), |m, s| (m.0.min(s.theta), m.1.max(s.theta)));
    let (v_lo, v_hi) = table.iter().fold((f64::MAX, f64::MIN), |m, s| (m.0.min(s.v), m.1.max(s.v)));
    // Inset from the table's edge round, so the grain never runs over onto a wall and combs.
    let inset = 0.6;
    let flank = format!(
        "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"1.2\" fill=\"#000\"/>{}<ellipse cx=\"{bx:.3}\" cy=\"{bv:.3}\" rx=\"{:.3}\" ry=\"{:.3}\" fill=\"#fff\"/>",
        t_lo / 360.0 * circ + inset,
        v_lo + inset,
        (t_hi - t_lo) / 360.0 * circ - 2.0 * inset,
        v_hi - v_lo - 2.0 * inset,
        cut(&chart_outline(d, a, &crest, circ)),
        0.5 * bl + HALO_MM,
        0.5 * bw + HALO_MM,
    );
    d.svgs.push(SvgAlpha { name: "Chamaeleo flanks".into(), svg: page(&flank), invert: false });
    // The cheeks and walls: everything, less each tail with its halo.
    let tails: String = d.stamps.iter().filter(|s| s.name.starts_with("Tail")).map(|s| cut(&chart_outline(d, a, s, circ))).collect();
    // Only where the wall stands square to the pull, near the head: the atlas read row by row into runs.
    let rows = 256usize;
    let mut runs = String::new();
    for row in 0..rows {
        let y = (row as f64 + 0.5) / rows as f64 * a.height as f64;
        let yy = (y as usize).min(a.height - 1);
        let mut start: Option<usize> = None;
        for x in 0..=a.width {
            let on = x < a.width && {
                let q = a.at(x, yy);
                q.n[2].abs() > SIDE_NZ && q.p[0].hypot(q.p[1]) > a.bore + 0.7 && (q.theta - 90.0).abs() < 0.5 * WALL_SPAN_DEG
            };
            match (on, start) {
                (true, None) => start = Some(x),
                (false, Some(x0)) => {
                    runs.push_str(&format!(
                        "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"#000\"/>",
                        x0 as f64 / a.width as f64 * circ,
                        row as f64 / rows as f64 * a.span,
                        (x - x0) as f64 / a.width as f64 * circ,
                        a.span / rows as f64 + 0.01
                    ));
                    start = None;
                }
                _ => {}
            }
        }
    }
    d.svgs.push(SvgAlpha { name: "Chamaeleo sides".into(), svg: page(&format!("{runs}{tails}")), invert: false });
    // The tiles: three radii of granule, and the stripe's flat tubercles.
    let tile = hide_tile(GRANULE_TILE, GRANULE_R, GRANULE_LAND, 7);
    d.svgs.push(SvgAlpha { name: "Chamaeleo granules".into(), svg: tile.clone(), invert: false });
    // The bench grain is finer and packed tighter: a graver, not the sand, sets its floor.
    d.svgs.push(SvgAlpha { name: "Chamaeleo grain".into(), svg: hide_tile(GRANULE_TILE, GRAIN_R, GRAIN_LAND, 11), invert: true });
    let tiled = |alpha: &str, height: f64| {
        let mut t = TilingLayer::default_for(alpha, &ctx);
        t.space = ChartSpace::Hide;
        t.repeats_around = (ctx.circumference_mm / GRANULE_TILE).round().max(1.0) as u32;
        t.rows = 6;
        t.v_center_mm = 0.0;
        t.v_span_mm = 6.0 * ctx.circumference_mm / t.repeats_around as f64;
        t.height_mm = height;
        t.feather_mm = 0.0;
        t.continuous = true;
        t
    };
    // The table's grain, at the bench.
    let mut e = LayerEntry::new("Table grain", Layer::Tiling(tiled("Chamaeleo grain", TABLE_GRAIN_MM)));
    e.blend = Blend::Subtract;
    e.bench_only = true;
    e.mask = Some("Chamaeleo flanks".into());
    d.layers.layers.push(e);
    // The cast hide on the cheeks and walls, wherever they stand square to the pull.
    let mut sides = LayerEntry::new("Granules, cheeks and walls", Layer::Tiling(tiled("Chamaeleo granules", GRANULE_MM)));
    sides.blend = Blend::Max;
    sides.mask = Some("Chamaeleo sides".into());
    sides.window = Window { enabled: true, theta_deg: 90.0, span_deg: WALL_SPAN_DEG, fade_deg: 4.0, invert: false, v_gate: Default::default() };
    let mut g = LayerEntry::new(
        "Hide, three granules",
        Layer::Group(GroupLayer { stack: LayerStack { layers: vec![sides] }, recipe: None, clamp: Some(SandClamp { resolution: [AW as u32, AH as u32], slack: 1.0 }) }),
    );
    g.blend = Blend::Max;
    g.window = Window { enabled: true, theta_deg: 90.0, span_deg: WALL_SPAN_DEG, fade_deg: 4.0, invert: false, v_gate: Default::default() };
    d.layers.layers.push(g);
    Ok(())
}

fn probe(d: &RingDesign) -> Result<()> {
    let a = Atlas::of(d, AW, AH)?;
    let hide = Hide::of(&a);
    eprintln!("top {:.3} bore {:.3} span {:.3} head {:.3} band {:.3}", a.top, a.bore, a.span, a.head_length_mm, a.band_width_mm);
    eprintln!("reach {:.3} folds {:?}", hide.reach(), hide.folds(&a, 12.0));
    for along in [-9.0, -8.0, -7.0, -5.0, -2.0, 0.0, 2.0, 5.0, 7.0, 8.0, 9.0] {
        let (t, v) = hide.crest_at(&a, along);
        eprintln!("along {along}: theta {t:.3} v {v:.3} p {:?}", hide.crest_point(&a, along));
    }
    let ctx = d.field_context();
    eprintln!("stretch {:?}", (0..=36).map(|k| (k * 10, (ctx.station_stretch(k as f64 * 10.0) * 100.0).round() / 100.0)).collect::<Vec<_>>());
    for deg in (20..=160).step_by(10) {
        let on: Vec<_> = a.samples.iter().filter(|q| q.p[2] > 0.0 && q.n[2] > SIDE_NZ && (q.theta - deg as f64).abs() < 0.2 && q.p[0].hypot(q.p[1]) > a.bore + 0.7).collect();
        let ac = on.iter().map(|q| hide.at(q).across).fold((f64::MAX, f64::MIN), |m, x| (m.0.min(x), m.1.max(x)));
        let r = on.iter().map(|q| q.p[0].hypot(q.p[1])).fold((f64::MAX, f64::MIN), |m, x| (m.0.min(x), m.1.max(x)));
        eprintln!("side at {deg}: across {ac:.2?} r {r:.2?}");
    }
    for name in ["table", "cheek", "wall", "rim", "shoulder", "palm"] {
        if let Some(m) = ringdesign_core::skin::region(&a, &hide, name) {
            let on: Vec<_> = a.samples.iter().filter(|s| m.data[s.i] > 0.5).collect();
            let ac = on.iter().map(|s| hide.at(s).across).fold((f64::MAX, f64::MIN), |m, x| (m.0.min(x), m.1.max(x)));
            let r = on.iter().map(|s| s.p[0].hypot(s.p[1])).fold((f64::MAX, f64::MIN), |m, x| (m.0.min(x), m.1.max(x)));
            eprintln!("region {name}: {} samples, across {ac:.2?}, r {r:.2?}", on.len());
        }
    }
    Ok(())
}

fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock(FACE)?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let (t_minus, _) = hide.crest_at(&a, -1.0);
    let (t_plus, _) = hide.crest_at(&a, 1.0);
    // A stamp's +x runs toward rising theta (measured on this stock); `along` runs whichever way the hide says.
    let along_per_x = if t_plus > t_minus { 1.0 } else { -1.0 };
    let (theta0, v0) = hide.crest_at(&a, 0.0);
    let mut placed = Placed { face_mm: [FACE.0, FACE.1], reach_mm: hide.reach(), folds_along_mm: hide.folds(&a, 12.0), ..Default::default() };

    // The head: two temporal crests stepping down away from the casque, and the casque, all on the parting line.
    // The crests are drawn from the head's centre; the casque from the occiput, so its dome crowns there. Each
    // frame is levelled on the parting line by itself.
    let frame = (theta0, v0, 0.0);
    let oc = hide.crest_at(&a, along_per_x * OCCIPUT);
    let occiput = (oc.0, oc.1, 0.0);
    let (w0, w1, wc) = HALF_W;
    // Each plate narrows toward the snout, so the head reads as a head and not a slab.
    let taper = |w: f64, x0: f64| move |x: f64| w * (0.72 + 0.28 * ((x - x0) / 3.0).clamp(0.0, 1.0));
    let outer = band_plate(CREST_FROM, CREST_TO, 0.9, 1.3, &taper(w0, CREST_FROM));
    let inner_from = CREST_FROM + 0.6;
    let inner = band_plate(inner_from, CREST_TO - 0.6, 0.8, 1.2, &taper(w1, inner_from));
    let casque_from = inner_from + 1.0;
    // The casque in plan: a narrow keel at the brow swelling to a broad, rounded helmet over the occiput.
    let casque = band_plate(casque_from, CREST_TO - 1.0, 1.6, 1.4, &|x| wc * (0.3 + 0.7 * ((x - casque_from) / 4.5).clamp(0.0, 1.0).powf(0.7)));
    let gable = |rise: f64| StampTop::Ridge { rise_mm: rise, from: [CREST_FROM, 0.0], to: [CREST_TO, 0.0], end_mm: rise };
    let mut crests = vec![
        struck("Temporal crest, outer", frame, outer, STEP_MM, 0, gable(0.12)),
        struck("Temporal crest, inner", frame, inner, STEP_MM, 1, gable(0.1)),
    ];
    let mut helmet = vec![struck("Casque", occiput, casque.into_iter().map(|p| [p[0] - OCCIPUT, p[1]]).collect(), CASQUE_MM, 2, match std::env::var("CHAM_CASQUE").as_deref() {
        Ok("ridge") => StampTop::Ridge { rise_mm: CASQUE_RISE.0, from: [casque_from - OCCIPUT, 0.0], to: [0.0, 0.0], end_mm: CASQUE_RISE.1 },
        Ok("flat") => StampTop::Flat,
        _ => StampTop::Dome { crown_mm: CASQUE_RISE.1 },
    })];
    // Off the head's centre the nearest section sample stands a hundredth off z = 0: walk the casque's station
    // across until its frame sits on the parting plane, or its narrow brow hangs off the line.
    let ctx = d.field_context();
    for _ in 0..4 {
        let z = helmet[0].frame(&d, &ctx).origin[2];
        let dz = a.point(helmet[0].theta_deg, helmet[0].v_mm + 0.1)[2] - a.point(helmet[0].theta_deg, helmet[0].v_mm)[2];
        if z.abs() < 1e-5 || dz.abs() < 1e-9 {
            break;
        }
        helmet[0].v_mm -= z * 0.1 / dz;
    }
    if std::env::var("CHAM_DEBUG").is_ok() {
        let f = helmet[0].frame(&d, &ctx);
        let g = crests[0].frame(&d, &ctx);
        eprintln!("casque frame origin {:?} x {:?} y {:?}; crest frame origin {:?} x {:?} y {:?}", f.origin, f.x, f.y, g.origin, g.x, g.y);
    }
    placed.levelled = level_on_line(&d, &mut crests) && level_on_line(&d, &mut helmet);
    placed.head_frame = [frame.0, frame.1, crests[0].rot_deg];
    let head: Vec<Stamp> = crests.into_iter().chain(helmet).collect();
    for s in &head {
        if let Err(e) = outline::check(&s.outline) {
            eprintln!("{}: {e}", s.name);
        }
    }
    if std::env::var("CHAM_DEBUG").is_ok() {
        for s in &head {
            if let Err(bad) = s.parting_monotone(&d) {
                eprintln!("{} not monotone at {} points, first {:?}", s.name, bad.len(), bad.iter().take(6).map(|i| s.outline[*i]).collect::<Vec<_>>());
            }
        }
    }
    d.stamps.extend(head);

    // The alexandrite on its boss at the snout's base: a top-level entry, so no clamp ever shaves its stock.
    let at = hide.crest_at(&a, along_per_x * STONE_ALONG);
    placed.stone_at = [at.0, at.1];
    let mut seat = SeatPadLayer {
        theta_deg: at.0,
        v_mm: at.1,
        style: SeatStyle::Boss,
        crown: 0.85,
        blend_mm: 0.4,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        mark_mm: 0.6,
        ..Default::default()
    };
    seat.fit_stone(alexandrite());
    seat.height_mm = BOSS_MM;
    placed.boss = json!({ "diameter_mm": seat.diameter_mm, "elong": seat.elong, "height_mm": seat.height_mm, "plan_pow": seat.plan_pow });
    let boss = (at.0, at.1, seat.diameter_mm * seat.elong + 2.0 * seat.blend_mm, seat.diameter_mm + 2.0 * seat.blend_mm);
    let mut e = LayerEntry::new("Alexandrite", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);

    dorsal(&mut d, &a, &hide, along_per_x, &mut placed)?;

    // The tail, curled on each cheek and standing along the pull.
    let tail = tail_outline();
    if let Err(e) = outline::check(&tail) {
        eprintln!("tail: {e}");
    }
    for (k, side) in [1.0f64, -1.0].into_iter().enumerate() {
        let s = cheek_stamp(&a, &format!("Tail, {}", if k == 0 { "fingertip" } else { "knuckle" }), [0.0, 11.9], side, &tail, TAIL_MM)?;
        placed.cheek.push(format!("{} at {:.2} deg, v {:.3}, {} points", s.name, s.theta_deg, s.v_mm, s.outline.len()));
        d.stamps.push(s);
    }
    if std::env::var("CHAM_NO_GRANULES").is_err() {
        granules(&mut d, &a, boss)?;
    }
    let _ = blockout;
    if let Ok(dir) = std::env::var("CHAM_DUMP") {
        for sv in &d.svgs {
            let r = sv.rasterize();
            let px: Vec<u8> = r.data.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8).collect();
            image::save_buffer(Path::new(&dir).join(format!("{}.png", sv.name)), &px, r.width as u32, r.height as u32, image::ColorType::L8)?;
        }
    }
    if let Ok(skip) = std::env::var("CHAM_SKIP") {
        let skip: Vec<&str> = skip.split(';').collect();
        d.stamps.retain(|s| !skip.iter().any(|k| s.name.starts_with(k)));
        d.layers.layers.retain(|e| !skip.iter().any(|k| e.name.starts_with(k)));
    }
    if std::env::var("CHAM_DEBUG").is_ok() {
        for s in &d.stamps {
            let bb = s.outline.iter().fold([f64::MAX, f64::MIN, f64::MAX, f64::MIN], |b, p| [b[0].min(p[0]), b[1].max(p[0]), b[2].min(p[1]), b[3].max(p[1])]);
            eprintln!("{}: {} points, area {:.2}, x {:.2}..{:.2} y {:.2}..{:.2}, at {:.2} deg v {:.3} rot {:.2}", s.name, s.outline.len(), outline::area(&s.outline), bb[0], bb[1], bb[2], bb[3], s.theta_deg, s.v_mm, s.rot_deg);
        }
        if std::env::var("CHAM_STOP").is_ok() {
            std::process::exit(0);
        }
    }
    let lib = mf::source_library(&d, &lib).into_owned();
    Ok((d, lib, placed))
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
            // A cabochon's dome is its whole depth; a faceted stone's metal-free part is its crown.
            let crown = if st.gem.form == ringdesign_core::gem::GemForm::Faceted { st.gem.crown_mm() } else { st.gem.depth_mm() };
            let (ra, rb, h) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03, crown - 0.03);
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
            if let (Err(bad), true) = (&r, std::env::var("CHAM_DEBUG").is_ok()) {
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
    let mut clamp_lib = lib.clone();
    let clamps = d.bake_clamps(&mut clamp_lib);
    let bites_ok = clamps.iter().all(|(_, r)| r.worst_mm <= 0.05);
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && margin >= -0.01
        && castable
        && bites_ok
        && coarse_release.obstructions.is_empty()
        && coarse_release.unresolved_rays == 0
        && fine.obstructions.is_empty()
        && fine.unresolved_rays == 0
        && monotone.iter().all(|(_, ok)| *ok)
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
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0100": release_line(coarse_release),
        "release_0100_obstructions": coarse_release.obstructions.len(),
        "release_0100_status_why": "Review, not Clear, with 0 obstructions: the factory table and bore walls carry under the sand's 3 deg draft (the bare stock reports the same). See the notes.",
        "release_0100_low_draft_area_mm2": coarse_release.low_draft_area_mm2,
        "release_0100_sand_findings": coarse_release.sand_findings.iter().map(|f| format!("{} at [{:.2}, {:.2}, {:.2}]", f.message, f.point[0], f.point[1], f.point[2])).collect::<Vec<_>>(),
        "release_0100_notes": coarse_release.notes,
        "release_0100_unresolved": coarse_release.unresolved_rays,
        "release_0075": release_line(&fine),
        "release_0075_obstructions": fine.obstructions.len(),
        "release_0075_obstruction_at": fine.obstructions.iter().map(|o| format!("[{:.2}, {:.2}, {:.2}] {:.3} mm deep", o.world[0], o.world[1], o.world[2], o.depth_mm)).collect::<Vec<_>>(),
        "release_0075_unresolved": fine.unresolved_rays,
        "clamp_bites_mm": clamps.iter().map(|(n, r)| json!({ "group": n, "worst_mm": r.worst_mm, "texels_cut": r.texels_cut })).collect::<Vec<_>>(),
        "parting_monotone": monotone,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "metal_inside_stones": inside,
        "stone_carats": stones.as_ref().map_or(0.0, |s| s.total_carats),
        "stone_warnings": warnings,
        "closest_stones": closest,
        "grams_925": built.report.metals.iter().find(|m| m.metal == "Silver 925").map_or(0.0, |m| m.grams),
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
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta/chamaeleo"));
    if args.iter().any(|a| a == "--probe") {
        return probe(&stock(FACE)?);
    }
    std::fs::create_dir_all(&out)?;
    println!("Chamaeleo");
    let bare = bare_study(FACE)?;
    println!("  bare 001 at {} x {}: {bare}", FACE.0, FACE.1);
    let (d, lib, placed) = author(blockout)?;
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: {draft_gates}");
    let (coarse_gates, coarse_pass, _) = gates(&d, &lib, coarse_params())?;
    println!("  384 x 192: pass {coarse_pass}");
    let (export_gates, export_pass, built) = if draft {
        (Value::Null, true, draft_built)
    } else {
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
            let file = if k == 0 { "reference-alexandrite.stl".to_string() } else { format!("reference-alexandrite-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Chamaeleo reference stone")?;
            materials.push(json!({ "mesh": file, "name": "Alexandrite", "tint": tint, "ior": 1.75, "dispersion": 0.015, "roughness": 0.06, "transmission": 0.6 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    let all = draft_pass && coarse_pass && export_pass && pattern_pass && cold != Some(false) && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": "chamaeleo",
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "sand": "Delft clay, 3.0 deg draft, 0.8 mm section, 0.30 mm detail",
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "bare_001": bare,
        "face_mm": [FACE.0, FACE.1],
        "placed": placed,
        "stamps": d.stamps.iter().map(|s| json!({ "name": s.name, "theta_deg": s.theta_deg, "v_mm": s.v_mm, "bench": s.bench, "cut": s.cut, "tier": s.tier, "along_pull": s.along_pull, "height_mm": s.height_mm, "finest_mm": dfm::stamp_finest_mm(&s.outline, d.draft.min_detail_mm) })).collect::<Vec<_>>(),
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
    ensure!(all || draft, "Chamaeleo failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
