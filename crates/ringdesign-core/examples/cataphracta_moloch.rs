//! Cataphracta — Moloch, the thorn idol: a thorny devil wrapped round the finger, poured in Petrobond.
//! cargo build --release -p ringdesign-core --example cataphracta_moloch
//! target/release/examples/cataphracta_moloch [OUT_DIR] [--draft] [--verify] [--blockout]
//!
//! The false head (a knob on the nape) stands at the face, crowned by the largest thorn and flanked by two horns struck
//! along the pull out of the side faces. Behind the knob the back is a carpet of thorns: a crest row in a major-minor rhythm on the parting line, a row down each crown
//! flank struck along the pull, capillary grooves across the flanks, and on the side faces graded thorns over a granule
//! ground, with four clasping legs. The body tapers round the palm to a tail whose tip stops short of the knob.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, SandProcess, Verdict},
    csg, dfm,
    field::{Blend, FluteProfile, FlutesLayer, GroupLayer, Layer, LayerEntry, LayerStack, SIDE_FACE_MIN_DRAFT_DEG, SandClamp, SideFacePick, VGate},
    library, manufacturing as mf, mesh, outline,
    profile::ShankKey,
    render,
    reptile,
    setting::{self, RowPath, Stamp, StampRow, StampTop},
    skin::Atlas,
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const NAME: &str = "Moloch \u{2014} the thorn idol";
const SLUG: &str = "moloch";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The face: the false head's knob stands here.
const HEAD_DEG: f64 = 90.0;
/// Which way round the ring the tail's tip lies from the knob: -1 toward decreasing angle.
const SNOUT_DIR: f64 = -1.0;

/// The body: thickness-only keys. The knob on the nape is the tallest station; the hips swell again for the hind legs.
/// The knob rises steeply out of the tail tip ahead of it and falls gently down the body behind it.
const HUMP: [(f64, f64); 12] = [(40.0, 1.0), (68.0, 1.1), (90.0, 1.76), (110.0, 1.44), (135.0, 1.22), (165.0, 1.06), (200.0, 1.0), (270.0, 1.0), (305.0, 1.05), (332.0, 1.2), (358.0, 1.1), (20.0, 1.02)];

// The knob's crown thorn, the largest on the ring.
const CROWN_MM: f64 = 2.7;
const CROWN_APEX_MM: f64 = 2.2;
const THORN_TIP_MM: f64 = 0.3;

// The crest row behind the knob: a major thorn then three minors, graded down to the palm and held small to the tail.
const MAJOR_MM: (f64, f64) = (2.4, 1.0);
const MINOR_MM: (f64, f64) = (1.2, 0.62);
const MAJOR_APEX: f64 = 0.8;
const MINOR_APEX: f64 = 0.55;
/// Share of the path from the knob to the tail by which the grade is spent (about 250-270°).
const GRADE_SPENT: f64 = 0.55;
const CREST_GAP_MM: f64 = 0.35;

// Flank thorns, struck along the pull down each crown flank at every other crest position.
const FLANK_MM: f64 = 1.25;
const FLANK_APEX: f64 = 0.6;
const FLANK_OFF_MM: f64 = 2.85;
const FLANK_COUNT: u32 = 16;
const FLANK_TAPER: f64 = 0.45;
const FLANK_LAG_DEG: f64 = 6.0;

// The horns: rounded triangles on the side faces at the knob, their cones leaning toward the crest.
const HORN_W_MM: f64 = 3.4;
const HORN_H_MM: f64 = 3.2;
const HORN_ROUND_MM: f64 = 0.45;
const HORN_APEX_MM: f64 = 2.4;
const HORN_LEAN_MM: f64 = 0.55;
const HORN_TIP_MM: f64 = 0.4;

/// Clear crest between the knob's crown thorn and the tail's last thorn, degrees.
const TAIL_GAP_DEG: f64 = 20.0;

// Side thorns: cones struck along the pull down each side face, jittered in size and height on the face.
const SIDE_THORN_MM: f64 = 2.4;
const SIDE_THORN_MIN_MM: f64 = 0.9;
const SIDE_THORN_RISE: f64 = 0.55;
const SIDE_THORN_TAPER: f64 = 0.5;
const SIDE_EDGE_MM: f64 = 0.3;
const SIDE_GAP_MM: f64 = 0.45;
const SIDE_JITTER_MIN: f64 = 0.6;

// The granule ground on the side faces.
const GRANULE_H_MM: f64 = 0.32;
const GRANULE_LAND_MM: f64 = 0.45;

// Capillary grooves across the crown flanks, run out onto the side faces.
const GROOVES: u32 = 64;
const GROOVE_W_MM: f64 = 0.6;
const GROOVE_D_MM: f64 = 0.2;
/// Clear of the crest thorns' roots, mm of chart v either side of the crest.
const GROOVE_CLEAR_MM: f64 = 1.35;
/// Where they have faded out, short of the crown's edge, mm: carried over the edge onto the face they fold the surface.
const GROOVE_STOP_MM: f64 = 0.15;
/// The outer end's run-out: 0.2 mm over 0.5 mm rises 22° walking away from the crest, inside the edge's own 38°.
const GROOVE_FADE_MM: f64 = 0.5;

// The legs: domed capsules (limb, shin, three toes) on both side faces.
const FRONT_LEG_DEG: f64 = 140.0;
const HIND_LEG_DEG: f64 = 332.0;
const LIMB_W_MM: f64 = 1.2;
const TOE_W_MM: f64 = 0.6;
const TOE_MM: f64 = 1.25;
const LEG_H_MM: f64 = 0.45;
const LEG_DOME_MM: f64 = 0.7;
const LEG_CLEAR_MM: f64 = 0.45;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The Petrobond pour: parting on z = 0, a gate off the palm, a sprue below it.
fn setup() -> mf::Setup {
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe::sand(SandProcess::Petrobond);
    setup.recipe.name = "Moloch / Petrobond / Silver 925".into();
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(1.9, |m| m.shrink_pct);
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.flask.width_mm = 80.0;
    setup.flask.length_mm = 80.0;
    setup.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.5, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -33.0, 0.0], diameter_mm: 6.0 },
    ];
    setup.bench_notes = "Two-part Petrobond pour parting on the crest line. The crest thorns and the head straddle the parting plane; the horns, flank thorns, side thorns and legs stand along the pull. Dress the seam between the crest thorns with a needle file and keep every thorn's flat tip.".into();
    setup
}

/// The bare body: Flat 7.0 x 3.6 with a parabolic crown, thickness-only keys, the palm the reference.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.6;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 1.4;
    // A parabolic crown, not Flat's x^8 table: the table stands parallel to the pull and drags (22% of the surface).
    d.profile.shape_a = 2.0;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = HUMP.iter().map(|&(theta_deg, thickness_scale)| ShankKey { theta_deg, width_scale: 1.0, thickness_scale, crown_scale: 1.0 }).collect();
    d.shank.keys.sort_by(|a, b| a.theta_deg.total_cmp(&b.theta_deg));
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.manufacturing = Some(s);
    d
}

/// What the author put down, for the report.
#[derive(Default, serde::Serialize)]
struct Composition {
    side_faces_mm: Option<[[f64; 2]; 2]>,
    band_v_len_mm: f64,
    crest_v_mm: f64,
    granule_cell_mm: [f64; 2],
    granule_repeats: u32,
    crest_thorns: Vec<(f64, f64)>,
    flank_thorns: usize,
    horn_v_mm: [f64; 2],
    side_thorns: Vec<SideThorn>,
    leg_zones: Vec<(bool, f64, f64)>,
    tail_deg: f64,
    clamps: Vec<(String, usize, f64)>,
    monotone_failures: Vec<String>,
}

/// Each station's own side faces in chart v, low then high: rows whose normal lies within 11 deg of the pull.
fn station_faces(d: &RingDesign, width: usize) -> Result<(Atlas, Vec<[(f64, f64); 2]>)> {
    let a = Atlas::of(d, width, 512)?;
    let mut out = Vec::with_capacity(width);
    for x in 0..a.width {
        let (mut lo, mut hi) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
        for y in 0..a.height {
            let s = a.at(x, y);
            if s.n[2].abs() > 0.98 {
                let f = if s.n[2] < 0.0 { &mut lo } else { &mut hi };
                f[0] = f[0].min(s.v);
                f[1] = f[1].max(s.v);
            }
        }
        out.push([(lo[0], lo[1]), (hi[0], hi[1])]);
    }
    Ok((a, out))
}

fn col(a: &Atlas, theta: f64) -> usize {
    ((theta.rem_euclid(360.0) / 360.0 * a.width as f64).round() as usize) % a.width
}

fn radius_at(a: &Atlas, theta: f64, v: f64) -> f64 {
    let p = a.point(theta.rem_euclid(360.0), v);
    p[0].hypot(p[1])
}

/// Deterministic jitter in 0..1.
fn hash(k: usize, salt: u64) -> f64 {
    let mut x = (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 31;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// A side thorn's placement: ring angle, chart v, diameter.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct SideThorn {
    theta_deg: f64,
    v_mm: f64,
    diameter_mm: f64,
}

/// Graded cones down one side face from `start_deg` (the first thorn's near edge) to `end_deg`, going `dir`, each on its
/// own station's face, jittered in size and in height on the face so no two neighbours are alike.
fn side_row(a: &Atlas, faces: &[[(f64, f64); 2]], high: bool, dir: f64, start_deg: f64, end_deg: f64) -> Vec<SideThorn> {
    let face = |theta: f64| faces[col(a, theta)][usize::from(high)];
    let span = (end_deg - start_deg).abs().max(1e-9);
    let size_at = |theta: f64| {
        let s = ((theta - start_deg).abs() / span).clamp(0.0, 1.0);
        let graded = SIDE_THORN_MM * (1.0 - SIDE_THORN_TAPER * 0.5 * (1.0 - (PI * s).cos()));
        let (lo, hi) = face(theta);
        graded.min(hi - lo - 2.0 * SIDE_EDGE_MM)
    };
    let salt = if high { 7 } else { 3 };
    let mut out: Vec<SideThorn> = Vec::new();
    let mut edge = start_deg - dir * (SIDE_GAP_MM / radius_at(a, start_deg, 0.5 * (face(start_deg).0 + face(start_deg).1))).to_degrees();
    loop {
        let mut t = edge;
        let mut placed = None;
        let fac = SIDE_JITTER_MIN + (1.0 - SIDE_JITTER_MIN) * hash(out.len(), salt);
        for _ in 0..60 {
            let dd = (size_at(t) * fac).max(SIDE_THORN_MIN_MM.min(size_at(t)));
            let (lo, hi) = face(t);
            let r = radius_at(a, t, 0.5 * (lo + hi));
            let want = edge + dir * ((SIDE_GAP_MM + 0.5 * dd) / r).to_degrees();
            if (want - t).abs() < 1e-5 {
                placed = Some((t, dd, r));
                break;
            }
            t = want;
        }
        let Some((t, dd, r)) = placed else { break };
        if dir * (t - end_deg) + (0.5 * dd / r).to_degrees() > 0.0 || dd < SIDE_THORN_MIN_MM {
            break;
        }
        let (lo, hi) = face(t);
        let room = (0.5 * (hi - lo) - 0.5 * dd - SIDE_EDGE_MM).max(0.0);
        let v = 0.5 * (lo + hi) + room * (2.0 * hash(out.len(), salt + 1) - 1.0);
        out.push(SideThorn { theta_deg: t.rem_euclid(360.0), v_mm: v, diameter_mm: dd });
        edge = t + dir * (0.5 * dd / r).to_degrees();
    }
    out
}

/// Distance from `p` to a capsule from `a` (radius `ra`) to `b` (radius `rb`), negative inside.
fn capsule(p: [f64; 2], a: [f64; 2], b: [f64; 2], ra: f64, rb: f64) -> f64 {
    let (bx, by) = (b[0] - a[0], b[1] - a[1]);
    let l2 = (bx * bx + by * by).max(1e-12);
    let t = (((p[0] - a[0]) * bx + (p[1] - a[1]) * by) / l2).clamp(0.0, 1.0);
    let (dx, dy) = (p[0] - a[0] - bx * t, p[1] - a[1] - by * t);
    dx.hypot(dy) - (ra + (rb - ra) * t)
}

fn traced(f: &dyn Fn([f64; 2]) -> f64, lo: [f64; 2], hi: [f64; 2], cell: f64) -> Vec<[f64; 2]> {
    let nx = ((hi[0] - lo[0]) / cell).ceil() as usize + 3;
    let ny = ((hi[1] - lo[1]) / cell).ceil() as usize + 3;
    let at = |i: usize, j: usize| [lo[0] + (i as f64 - 1.0) * cell, lo[1] + (j as f64 - 1.0) * cell];
    let val: Vec<f64> = (0..ny).flat_map(|j| (0..nx).map(move |i| (i, j))).map(|(i, j)| {
        if i == 0 || j == 0 || i == nx - 1 || j == ny - 1 { 1.0 } else { f(at(i, j)) }
    }).collect();
    let v = |i: usize, j: usize| val[j * nx + i];
    // Edge keys: horizontal (i, j, 0) from (i, j) to (i+1, j); vertical (i, j, 1) from (i, j) to (i, j+1).
    let point = |k: (usize, usize, u8)| {
        let (i, j, o) = k;
        let (i2, j2) = if o == 0 { (i + 1, j) } else { (i, j + 1) };
        let (a, b) = (v(i, j), v(i2, j2));
        let t = (a / (a - b)).clamp(0.0, 1.0);
        let (p, q) = (at(i, j), at(i2, j2));
        [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
    };
    let mut next: std::collections::HashMap<(usize, usize, u8), (usize, usize, u8)> = Default::default();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let c = [v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)];
            let inside = c.map(|x| x < 0.0);
            let e = [(i, j, 0u8), (i + 1, j, 1u8), (i, j + 1, 0u8), (i, j, 1u8)];
            // Walk the cell's corners counter-clockwise; an edge leaving the inside starts a segment, entering ends it.
            let mut outs = Vec::new();
            let mut ins = Vec::new();
            for k in 0..4 {
                let (a, b) = (inside[k], inside[(k + 1) % 4]);
                if a && !b { outs.push(e[k]); }
                if !a && b { ins.push(e[k]); }
            }
            // Inside on the left: a segment runs from where the boundary leaves the inside... pair each exit with the next entry.
            if outs.len() == 1 {
                next.insert(ins[0], outs[0]);
            } else if outs.len() == 2 {
                let centre = c.iter().sum::<f64>() * 0.25 < 0.0;
                // Corners 0 and 2 inside, or 1 and 3: join through the centre when it is inside.
                let (o0, o1, i0, i1) = (outs[0], outs[1], ins[0], ins[1]);
                if centre { next.insert(i0, o1); next.insert(i1, o0); } else { next.insert(i0, o0); next.insert(i1, o1); }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut best: Vec<[f64; 2]> = Vec::new();
    let mut starts: Vec<_> = next.keys().copied().collect();
    starts.sort_unstable();
    for start in starts {
        if seen.contains(&start) { continue; }
        let mut lp = Vec::new();
        let mut k = start;
        while seen.insert(k) {
            lp.push(point(k));
            match next.get(&k) { Some(n) => k = *n, None => break }
        }
        if lp.len() > best.len() { best = lp; }
    }
    // Resample at 0.08 mm, then two passes of light smoothing.
    let n = best.len();
    let mut dense = Vec::new();
    let total: f64 = (0..n).map(|i| { let (a, b) = (best[i], best[(i + 1) % n]); (a[0] - b[0]).hypot(a[1] - b[1]) }).sum();
    let m = (total / 0.08).ceil().max(8.0) as usize;
    let step = total / m as f64;
    let (mut seg, mut acc) = (0usize, 0.0);
    for k in 0..m {
        let target = k as f64 * step;
        loop {
            let (a, b) = (best[seg % n], best[(seg + 1) % n]);
            let l = (a[0] - b[0]).hypot(a[1] - b[1]);
            if acc + l >= target || seg > 2 * n {
                let t = if l > 0.0 { (target - acc) / l } else { 0.0 };
                dense.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
                break;
            }
            acc += l;
            seg += 1;
        }
    }
    for _ in 0..2 {
        let c = dense.clone();
        let m = c.len();
        for i in 0..m {
            let (p, q, r) = (c[(i + m - 1) % m], c[i], c[(i + 1) % m]);
            dense[i] = [0.25 * p[0] + 0.5 * q[0] + 0.25 * r[0], 0.25 * p[1] + 0.5 * q[1] + 0.25 * r[1]];
        }
    }
    if outline::area(&dense) < 0.0 {
        dense.reverse();
    }
    dense
}

/// A leg's skeleton on a face `h` mm tall, (fwd, out) from the face's middle, and its toes' spread.
fn leg_skeleton(h: f64, hind: bool) -> (Vec<[f64; 2]>, f64) {
    let top = 0.5 * h - 0.5 * LIMB_W_MM - 0.3;
    let low = -0.5 * h + 0.5 * LIMB_W_MM + 0.65;
    if hind {
        // The thigh back and down, the shin kicked back, toes trailing.
        (vec![[1.1, top], [-0.4, 0.5 * (top + low) + 0.2], [-2.3, low]], 34.0)
    } else {
        // The arm forward and down, the hand reaching ahead.
        (vec![[-1.1, top], [0.5, 0.5 * (top + low) + 0.1], [2.1, low]], 32.0)
    }
}

/// A leg's parts: each a capsule from, to, radii, its name and its dome's crown.
fn leg_parts(h: f64, hind: bool) -> Vec<([f64; 2], [f64; 2], f64, f64, &'static str, f64)> {
    let (skel, spread) = leg_skeleton(h, hind);
    let hw = 0.5 * LIMB_W_MM;
    let wrist = skel[2];
    let heading = (skel[2][1] - skel[1][1]).atan2(skel[2][0] - skel[1][0]);
    let mut parts = vec![
        (skel[0], skel[1], hw * 1.12, hw, "limb", LEG_DOME_MM),
        (skel[1], skel[2], hw, hw * 0.85, "shin", LEG_DOME_MM * 0.9),
    ];
    for (k, name) in ["toe 1", "toe 2", "toe 3"].into_iter().enumerate() {
        let ang = heading + (1.0 - k as f64) * spread.to_radians();
        let tip = [wrist[0] + ang.cos() * (hw * 0.6 + TOE_MM), wrist[1] + ang.sin() * (hw * 0.6 + TOE_MM)];
        // Each toe starts a little way out along its own line: three capsules from one point meet degenerately.
        let root = [wrist[0] + ang.cos() * hw * 0.3, wrist[1] + ang.sin() * hw * 0.3];
        parts.push((root, tip, 0.5 * TOE_W_MM * (1.1 + 0.03 * k as f64), 0.5 * TOE_W_MM * 0.85, name, LEG_DOME_MM * (0.58 + 0.03 * k as f64)));
    }
    parts
}

fn stamp(name: &str, outline: Vec<[f64; 2]>, top: StampTop) -> Stamp {
    Stamp {
        name: name.into(),
        theta_deg: 0.0,
        v_mm: 0.0,
        rot_deg: 0.0,
        outline,
        height_mm: 0.10,
        sink_mm: 0.3,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        tier: 0,
        top,
        fine_cap: false,
    }
}

/// One stamp on the parting line at `theta`, named `name`: a row of one, so `v` is solved on the line.
fn on_crest(d: &RingDesign, name: &str, theta: f64, outline: Vec<[f64; 2]>, top: StampTop) -> Result<Stamp> {
    let proto = Stamp { top, ..stamp(name, outline, StampTop::Flat) };
    let mut s = setting::stamp_row(d, &StampRow { stamp: proto, path: RowPath::PartingLine, from_deg: theta, to_deg: theta + 1e-3, count: 1, taper: 0.0, fold_clear_mm: 0.0, mirror_shoulders: false })
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no {name} on the crest at {theta:.1}°"))?;
    s.name = name.into();
    Ok(s)
}

/// The crest behind the knob to the tail: a major thorn then three minors, each graded by its share of the path.
fn crest_row(a: &Atlas, crest_v: f64, from_edge_deg: f64, tail_deg: f64) -> Vec<(f64, f64, bool)> {
    let dir = (tail_deg - from_edge_deg).signum();
    let span = (tail_deg - from_edge_deg).abs();
    let grade = |theta: f64| {
        let s = (((theta - from_edge_deg).abs() / span) / GRADE_SPENT).clamp(0.0, 1.0);
        0.5 - 0.5 * (PI * s).cos()
    };
    let size = |theta: f64, major: bool| {
        let (big, small) = if major { MAJOR_MM } else { MINOR_MM };
        big + (small - big) * grade(theta)
    };
    let mut out = Vec::new();
    let mut edge = from_edge_deg;
    loop {
        let major = out.len() % 4 == 0;
        let mut t = edge;
        for _ in 0..60 {
            let r = radius_at(a, t, crest_v);
            let want = edge + dir * ((CREST_GAP_MM + 0.5 * size(t, major)) / r).to_degrees();
            if (want - t).abs() < 1e-5 {
                break;
            }
            t = want;
        }
        let dd = size(t, major);
        let r = radius_at(a, t, crest_v);
        if dir * (t - tail_deg) + (0.5 * dd / r).to_degrees() > 0.0 {
            break;
        }
        out.push((t, dd, major));
        edge = t + dir * (0.5 * dd / r).to_degrees();
    }
    out
}

/// The granule ground on the side faces, fitted to the reference faces and held to the draft rule in a clamped group.
fn granules(d: &mut RingDesign, ctx: &ringdesign_core::FieldContext, art: &Path, comp: &mut Composition) -> Result<()> {
    let mut t = TilingLayer::default_for("Granule ground", ctx);
    ensure!(t.fit_to_side_faces(ctx, SIDE_FACE_MIN_DRAFT_DEG), "no side faces to fit");
    t.height_mm = GRANULE_H_MM;
    t.feather_mm = 0.0;
    let (cw, ch) = t.cell_size(ctx);
    comp.granule_cell_mm = [cw, ch];
    comp.granule_repeats = t.repeats_around;
    let svg = reptile::svg::granules(&reptile::svg::Params::new(cw, ch, GRANULE_LAND_MM, 1.0));
    std::fs::write(art.join("granule-ground.svg"), &svg)?;
    d.svgs.push(SvgAlpha { name: "Granule ground".into(), svg, invert: false });
    let mut e = LayerEntry::new("Granule ground", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    let mut group = LayerStack::default();
    group.layers.push(e);
    let mut g = LayerEntry::new("Side hide", Layer::Group(GroupLayer { stack: group, recipe: None, clamp: Some(SandClamp::default()) }));
    g.blend = Blend::Max;
    d.layers.layers.push(g);
    Ok(())
}

/// Capillary grooves across each crown flank, from clear of the crest thorns' roots to just short of the crown's edge.
fn grooves(d: &mut RingDesign, ctx: &ringdesign_core::FieldContext) {
    let Some(faces) = ctx.side_faces_std() else { return };
    let (Some(low), Some(high)) = (faces.low, faces.high) else { return };
    for (side, inner, outer) in [("fingertip", ctx.crest_v_mm - GROOVE_CLEAR_MM, low.1 + GROOVE_STOP_MM), ("knuckle", ctx.crest_v_mm + GROOVE_CLEAR_MM, high.0 - GROOVE_STOP_MM)] {
        let fade = GROOVE_FADE_MM;
        let span = (inner - outer).abs() - 2.0 * fade;
        let mut e = LayerEntry::new(
            format!("Capillary grooves, {side} flank"),
            Layer::Flutes(FlutesLayer { count: GROOVES, profile: FluteProfile::Vee, width_mm: GROOVE_W_MM, height_mm: GROOVE_D_MM, lean: 0.0, along: false }),
        );
        e.blend = Blend::Subtract;
        e.window.v_gate = VGate::Band { center_mm: 0.5 * (inner + outer), span_mm: span, fade_mm: fade };
        d.layers.layers.push(e);
    }
}

fn author(art: &Path, blockout: bool) -> Result<(RingDesign, AlphaLibrary, Composition)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let ctx = d.field_context();
    let mut comp = Composition { band_v_len_mm: ctx.band_v_len_mm, crest_v_mm: ctx.crest_v_mm, ..Composition::default() };
    let faces_std = ctx.side_faces_std();
    comp.side_faces_mm = faces_std.and_then(|f| Some([f.low.map(|p| [p.0, p.1])?, f.high.map(|p| [p.0, p.1])?]));
    let (atlas, faces) = station_faces(&band(), 1440)?;
    let crest_r = |theta: f64| radius_at(&atlas, theta, ctx.crest_v_mm);
    let tail_deg = HEAD_DEG - SNOUT_DIR * (360.0 - TAIL_GAP_DEG);
    comp.tail_deg = tail_deg;

    // 1. Ground: granules on the side faces and capillary grooves across the crown flanks.
    granules(&mut d, &ctx, art, &mut comp)?;
    grooves(&mut d, &ctx);
    let _ = blockout;

    // 2. The knob's crown thorn, the tallest point on the ring.
    d.stamps.push(on_crest(&d, "Thorn, crown", HEAD_DEG, outline::circle(CROWN_MM), StampTop::Cone { apex_mm: CROWN_APEX_MM, at: [0.0, 0.0], tip_mm: THORN_TIP_MM })?);

    // 4. The crest row: majors and minors from behind the crown thorn round the body to the tail.
    let start = HEAD_DEG - SNOUT_DIR * ((0.5 * CROWN_MM + CREST_GAP_MM) / crest_r(HEAD_DEG)).to_degrees();
    for (k, (theta, dd, major)) in crest_row(&atlas, ctx.crest_v_mm, start, tail_deg).into_iter().enumerate() {
        let apex = dd * if major { MAJOR_APEX } else { MINOR_APEX };
        let name = if major { format!("Thorn, crest major {}", k / 4 + 1) } else { format!("Thorn, crest minor {}", k - k / 4) };
        d.stamps.push(on_crest(&d, &name, theta.rem_euclid(360.0), outline::circle(dd), StampTop::Cone { apex_mm: apex, at: [0.0, 0.0], tip_mm: THORN_TIP_MM })?);
        comp.crest_thorns.push((theta.rem_euclid(360.0), dd));
    }

    // 5. Flank thorns down each crown flank, struck along the pull where the crown already leans toward it.
    for (sign, side) in [(-1.0, "fingertip"), (1.0, "knuckle")] {
        let proto = Stamp {
            top: StampTop::Cone { apex_mm: FLANK_APEX * FLANK_MM, at: [0.0, 0.0], tip_mm: THORN_TIP_MM },
            along_pull: true,
            ..stamp(&format!("Thorn, flank {side}"), outline::circle(FLANK_MM), StampTop::Flat)
        };
        let row = setting::stamp_row(
            &d,
            &StampRow { stamp: proto, path: RowPath::ChartV { v_mm: ctx.crest_v_mm + sign * FLANK_OFF_MM }, from_deg: HEAD_DEG - SNOUT_DIR * FLANK_LAG_DEG, to_deg: tail_deg, count: FLANK_COUNT, taper: FLANK_TAPER, fold_clear_mm: 0.0, mirror_shoulders: false },
        );
        comp.flank_thorns += row.len();
        d.stamps.extend(row);
    }

    // 6. The horns: one per side face at the knob, standing along the pull, leaning toward the crest.
    let ef = faces[col(&atlas, HEAD_DEG)];
    let hv = [0.5 * (ef[0].0 + ef[0].1), 0.5 * (ef[1].0 + ef[1].1)];
    comp.horn_v_mm = hv;
    for (k, (side, v)) in [("fingertip", hv[0]), ("knuckle", hv[1])].into_iter().enumerate() {
        // The outline's x runs radially outward on either face.
        let mut s = stamp(
            &format!("Horn, {side}"),
            outline::rounded_triangle(HORN_W_MM, HORN_H_MM, HORN_ROUND_MM),
            StampTop::Cone { apex_mm: HORN_APEX_MM, at: [HORN_LEAN_MM, 0.0], tip_mm: HORN_TIP_MM },
        );
        s.theta_deg = HEAD_DEG;
        s.v_mm = v;
        s.rot_deg = if k == 0 { 90.0 } else { -90.0 };
        s.along_pull = true;
        d.stamps.push(s);
    }

    // 7. The legs: one per face at the front and hind stations, each a chain of domed capsules.
    let mut leg_zones: Vec<(bool, f64, f64)> = Vec::new();
    for (theta, hind, pair) in [(FRONT_LEG_DEG, false, "front"), (HIND_LEG_DEG, true, "hind")] {
        let f = faces[col(&atlas, theta)];
        for (k, (high, side)) in [(false, "fingertip"), (true, "knuckle")].into_iter().enumerate() {
            let (lo, hi) = f[k];
            let mid_v = 0.5 * (lo + hi);
            let r = radius_at(&atlas, theta, mid_v);
            // Frame: x runs with the ring angle, y with chart v, on either face; the head lies toward SNOUT_DIR.
            let ysign = if high { -1.0 } else { 1.0 };
            let to_frame = |p: [f64; 2]| [SNOUT_DIR * p[0], ysign * p[1]];
            let (mut lo_x, mut hi_x) = (f64::MAX, f64::MIN);
            for (n, part) in leg_parts(hi - lo, hind).into_iter().enumerate() {
                let (a, b) = (to_frame(part.0), to_frame(part.1));
                let m = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
                let (ra, rb) = (part.2, part.3);
                let f = |q: [f64; 2]| capsule([q[0] + m[0], q[1] + m[1]], a, b, ra, rb);
                let ext = ra.max(rb) + 0.3;
                let lo_q = [a[0].min(b[0]) - m[0] - ext, a[1].min(b[1]) - m[1] - ext];
                let hi_q = [a[0].max(b[0]) - m[0] + ext, a[1].max(b[1]) - m[1] + ext];
                let o = traced(&f, lo_q, hi_q, 0.015);
                outline::check(&o).map_err(|e| anyhow::anyhow!("leg {pair} {side} part {n}: {e}"))?;
                for q in &o {
                    lo_x = lo_x.min(q[0] + m[0]);
                    hi_x = hi_x.max(q[0] + m[0]);
                }
                let mut s = stamp(&format!("Leg, {pair} {side} {}", part.4), o, StampTop::Dome { crown_mm: part.5 });
                s.theta_deg = theta + (m[0] / r).to_degrees();
                s.v_mm = mid_v + m[1];
                s.height_mm = LEG_H_MM;
                s.along_pull = true;
                d.stamps.push(s);
            }
            leg_zones.push((high, theta + ((lo_x - LEG_CLEAR_MM) / r).to_degrees(), theta + ((hi_x + LEG_CLEAR_MM) / r).to_degrees()));
        }
    }
    comp.leg_zones = leg_zones.clone();

    // 8. Side thorns down both faces, from behind the horns round the body to the tail, clear of the legs.
    let horn_half_deg = ((0.5 * HORN_W_MM + 0.1) / radius_at(&atlas, HEAD_DEG, hv[0])).to_degrees();
    for (high, side) in [(false, "fingertip"), (true, "knuckle")] {
        let mut row = side_row(&atlas, &faces, high, -SNOUT_DIR, HEAD_DEG - SNOUT_DIR * horn_half_deg, tail_deg);
        row.retain(|t| {
            let half = (0.5 * t.diameter_mm / radius_at(&atlas, t.theta_deg, t.v_mm)).to_degrees();
            !leg_zones.iter().any(|&(h, a, b)| {
                let th = if t.theta_deg < a - 180.0 { t.theta_deg + 360.0 } else if t.theta_deg > b + 180.0 { t.theta_deg - 360.0 } else { t.theta_deg };
                h == high && th + half > a && th - half < b
            })
        });
        for (k, t) in row.iter().enumerate() {
            let mut s = stamp(&format!("Side thorn, {side} {}", k + 1), outline::circle(t.diameter_mm), StampTop::Cone { apex_mm: SIDE_THORN_RISE * t.diameter_mm, at: [0.0, 0.0], tip_mm: THORN_TIP_MM });
            s.theta_deg = t.theta_deg;
            s.v_mm = t.v_mm;
            s.along_pull = true;
            d.stamps.push(s);
        }
        comp.side_thorns.extend(row);
    }

    for s in &d.stamps {
        if let Err(bad) = s.parting_monotone(&d) {
            if std::env::var("MOLOCH_DEBUG").is_ok() {
                println!("    monotone {}: {} of {} points", s.name, bad.len(), s.outline.len());
            }
            comp.monotone_failures.push(s.name.clone());
        }
    }
    d.bake_all(&mut lib);
    comp.clamps = d.bake_clamps(&mut lib).into_iter().map(|(n, r)| (n, r.texels_cut, r.worst_mm)).collect();
    Ok((d, lib, comp))
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// Every stamp's solid, checked closed and uncrossed.
fn made_solids(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Vec<serde_json::Value>> {
    let unstruck = solid_of(&mesh::try_build(&setting::without_solids(d), lib, params)?.mesh);
    let ctx = d.field_context();
    let mut out = Vec::new();
    for s in &d.stamps {
        let part = s.solid(&s.frame(d, &ctx), &unstruck).map_err(anyhow::Error::msg)?;
        let c = part.check(true);
        out.push(json!({"name": s.name, "self_crossings": c.self_crossings, "zero_area_faces": c.zero_area_faces, "open_edges": c.open_edges, "repeated_edges": c.repeated_edges}));
    }
    Ok(out)
}

/// Every vertex nearer the finger axis than the bore allows.
fn bore_intrusion(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    let mut least = f64::MAX;
    let mut inside = 0;
    for v in &m.vertices {
        let r = (v.0 as f64).hypot(v.1 as f64);
        least = least.min(r);
        inside += usize::from(r < bore - 0.01);
    }
    (least, inside)
}

/// The camera for each named view: yaw about the head's axis, pitch toward the finger's.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.0, 0.9),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.6),
    ("reverse", PI - 0.5, 0.35),
];

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

/// Studio-gold renders, the 300 px read, a contact sheet and the bare hump against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The stones view on a ring without stones: the false head close, from above the knuckle side.
    render::write_png_parts(out.join("stones.png"), &parts, -0.35, 0.95, edge)?;
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let fin_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut pair = vec![0u8; edge * 2 * edge * 3];
    paste(&mut pair, edge * 2, &bare_img, edge, 0, 0);
    paste(&mut pair, edge * 2, &fin_img, edge, edge, 0);
    image::save_buffer(out.join("bare-vs-finished.png"), &pair, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    render::write_png_parts(out.join("face-300.png"), &parts, VIEWS[1].1, VIEWS[1].2, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    let mut sheet = vec![0u8; 900 * 600 * 3];
    for (k, (_, yaw, pitch)) in VIEWS.iter().enumerate() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3);
        paste(&mut sheet, 900, &img, 300, (k % 3) * 300, (k / 3) * 300);
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 900, 600, image::ColorType::Rgb8)?;
    Ok(())
}

/// The gates at one build size: geometry, stamps, bore, the ray release at both pitches.
struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    stamped: usize,
    notes: Vec<String>,
    release_01: (usize, usize, f64),
    release_0075: (usize, usize, f64),
    sand_findings: usize,
    obstructions: Vec<String>,
}

fn release_where(r: &mf::release::ReleaseReport) -> Vec<String> {
    r.obstructions.iter().map(|o| {
        let [x, y, z] = o.world;
        format!("{:.1}° r {:.2} z {:+.2}: {:.3} mm", y.atan2(x).to_degrees().rem_euclid(360.0), x.hypot(y), z, o.depth_mm)
    }).collect()
}

fn release_triple(r: &mf::release::ReleaseReport) -> (usize, usize, f64) {
    (r.obstructions.len(), r.unresolved_rays, r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max))
}

fn pass(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Pass, mesh::BuildResult)> {
    let built = mesh::try_build(d, lib, params)?;
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let setup = d.manufacturing.clone().unwrap();
    let inspection = mf::inspect(d, lib, &setup, params)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let fine_r = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let mut notes = built.solids.notes.clone();
    notes.extend(built.parts.notes.iter().cloned());
    Ok((
        Pass {
            triangles: built.mesh.faces.len(),
            watertight,
            degenerate,
            crossings,
            stamped: built.solids.stamped,
            notes,
            release_01: release_triple(&inspection.release),
            release_0075: release_triple(&fine_r),
            sand_findings: inspection.release.sand_findings.len(),
            obstructions: release_where(&inspection.release).into_iter().chain(release_where(&fine_r)).collect(),
        },
        built,
    ))
}

impl Pass {
    fn ok(&self, stamps: usize) -> bool {
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.stamped == stamps && self.notes.is_empty() && self.release_01.0 == 0 && self.release_01.1 == 0 && self.release_0075.0 == 0 && self.release_0075.1 == 0
    }
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "stamped": self.stamped, "notes": self.notes,
            "release_0100": {"obstructions": self.release_01.0, "unresolved": self.release_01.1, "deepest_mm": self.release_01.2},
            "release_0075": {"obstructions": self.release_0075.0, "unresolved": self.release_0075.1, "deepest_mm": self.release_0075.2},
            "sand_findings": self.sand_findings, "obstructions": self.obstructions})
    }
    fn line(&self) -> String {
        format!(
            "{} tris, watertight {}, degenerate {}, crossings {}, stamped {}, notes {:?}, release 0.100 {:?}, 0.075 {:?}",
            self.triangles, self.watertight, self.degenerate, self.crossings, self.stamped, self.notes, self.release_01, self.release_0075
        ) + &if self.obstructions.is_empty() { String::new() } else { format!("\n      at {:?}", self.obstructions) }
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta").join(SLUG));
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/cataphracta/art").join(SLUG);
    std::fs::create_dir_all(&out)?;
    std::fs::create_dir_all(&art)?;
    println!("{NAME}");
    let started = std::time::Instant::now();
    if let Ok(skip) = std::env::var("MOLOCH_SKIP") {
        // Bisect a fault: build with the named stamp families or layers left out.
        let (d, lib, _) = author(&art, blockout)?;
        let mut d = d;
        for key in skip.split(',') {
            d.stamps.retain(|s| !s.name.starts_with(key));
            d.layers.layers.retain(|e| !e.name.starts_with(key));
        }
        let built = mesh::try_build(&d, &lib, draft_params())?;
        let (w, g, x) = geometry(&built.mesh);
        println!("  without {skip}: watertight {w}, degenerate {g}, crossings {x}, notes {:?}", built.solids.notes);
        return Ok(());
    }
    if std::env::var("MOLOCH_PROBE").is_ok() {
        let lib = AlphaLibrary::builtin();
        for (label, sa, crown) in [("a 8", 8.0, 1.4), ("a 2", 2.0, 1.4), ("a 3", 3.0, 1.4), ("a 2 crown 1.2", 2.0, 1.2), ("a 1.6", 1.6, 1.4)] {
            let mut b = band();
            b.profile.shape_a = sa;
            b.profile.crown_mm = crown;
            let f = castability::analyze_field(&b, &lib, &b.draft, 256, 128);
            println!("  {label}: {} marginal {:.1} vertical {:.1} of {:.1} mm2 = {:.1}%, faces {:?}", f.verdict.label(), f.marginal_area_mm2, f.vertical_area_mm2, f.total_area_mm2, f.drag_fraction() * 100.0, b.field_context().side_faces_std());
        }
        return Ok(());
    }
    let (d, lib, comp) = author(&art, blockout)?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  faces {:?} of {:.2} mm, crest v {:.2}; granule cell {:.2} x {:.2} x{}; {} crest thorns, {} flank, {} side; horns at v {:.2} / {:.2}; tail {:.1}; clamps {:?}; monotone failures {:?}",
        comp.side_faces_mm, comp.band_v_len_mm, comp.crest_v_mm, comp.granule_cell_mm[0], comp.granule_cell_mm[1], comp.granule_repeats, comp.crest_thorns.len(), comp.flank_thorns, comp.side_thorns.len(), comp.horn_v_mm[0], comp.horn_v_mm[1], comp.tail_deg, comp.clamps, comp.monotone_failures
    );
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let (main_pass, built) = pass(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, main_pass.line());
    let (coarse_pass, _) = pass(&d, &lib, coarse_params())?;
    println!("  384x192: {}", coarse_pass.line());
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let solids = made_solids(&d, &lib, params)?;
    for g in solids.iter().filter(|g| !(g["self_crossings"] == 0 && g["open_edges"] == 0 && g["repeated_edges"] == 0 && g["zero_area_faces"] == 0)) {
        println!("    solid: {g}");
    }
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    for share in castability::attribute_drag(&d, &lib, &field) {
        println!("    drag: {} marginal {:.2} vertical {:.2} mm2", share.layer, share.marginal_mm2, share.vertical_mm2);
    }
    println!("    drag total {:.1} mm2: marginal {:.1}, vertical {:.1}", field.total_area_mm2, field.marginal_area_mm2, field.vertical_area_mm2);
    let findings = dfm::findings_in(&d, &lib);
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
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
    let clamp_worst = comp.clamps.iter().map(|c| c.2).fold(0.0, f64::max);
    let stamps = d.stamps.len();
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", main_pass.watertight && main_pass.degenerate == 0 && main_pass.crossings == 0),
        ("every stamp solid closed without crossings", solids.iter().all(|g| g["self_crossings"] == 0 && g["open_edges"] == 0 && g["repeated_edges"] == 0 && g["zero_area_faces"] == 0)),
        ("solids notes empty, every stamp resolved, every stamp parting-monotone", main_pass.notes.is_empty() && main_pass.stamped == stamps && comp.monotone_failures.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("field verdict Castable (sand, Petrobond)", field.process == CastProcess::SandTwoPart && field.verdict == Verdict::Castable),
        ("ray release 0 obstructions, 0 unresolved at 0.100 and 0.075 mm", main_pass.release_01.0 == 0 && main_pass.release_01.1 == 0 && main_pass.release_0075.0 == 0 && main_pass.release_0075.1 == 0),
        ("every draft-clamp bite at most 0.05 mm", clamp_worst <= 0.05),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("gates hold at 384 x 192", coarse_pass.ok(stamps)),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within 2 million triangles", main_pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "sand": "Petrobond",
        "draft": {"process": d.draft.process.label(), "sand": format!("{:?}", d.draft.sand), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.json(),
        "made_solids": solids,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes},
        "draft_clamp": comp.clamps.iter().map(|c| json!({"group": c.0, "texels_cut": c.1, "worst_mm": c.2})).collect::<Vec<_>>(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "composition": comp,
        "design": {"bytes": text.len(), "stamps": stamps},
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let name = if draft { "report-draft.json" } else { "report.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Moloch / Petrobond pattern")?;
    }
    if std::env::var("MOLOCH_VIEWS").is_ok() {
        let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
        let mut sheet = vec![0u8; 1200 * 900 * 3];
        for (k, (yaw, pitch)) in [(-0.65, 0.6), (-0.3, 0.8), (0.0, 0.9), (0.3, 0.8), (-0.9, 0.9), (-0.4, 1.1), (0.4, 1.1), (0.65, 0.6), (-1.2, 0.7), (1.2, 0.7), (-0.2, 0.5), (0.2, 0.5)].iter().enumerate() {
            let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 2);
            paste(&mut sheet, 1200, &img, 300, (k % 4) * 300, (k / 4) * 300);
        }
        image::save_buffer(out.join("views-probe.png"), &sheet, 1200, 900, image::ColorType::Rgb8)?;
    }
    renders(&out, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.3}% at {:.1} deg); dfm {}; clamp worst {clamp_worst:.3} mm; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        findings.len(),
        d.inner_radius_mm()
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in &field.notes {
        println!("    field: {n}");
    }
    for (g, p) in &gates {
        println!("  {} {g}", if *p { "pass" } else { "FAIL" });
    }
    Ok(())
}
