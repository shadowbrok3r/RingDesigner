//! Cataphracta — Chamaeleo, the casque: a chameleon's head sculpted as one part on a tall keyed body, its casque
//! rising above it, a turret eye standing out on each side, its tail coiled on each flank; an alexandrite on its back.
//! Lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_chamaeleo
//! target/release/examples/cataphracta_chamaeleo [OUT_DIR] [--draft] [--verify] [--blockout] [--probe]
#![recursion_limit = "256"]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle},
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh, outline,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smax, smin},
    setting::{SolidKind, Stamp, StampTop},
    skin::Atlas,
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const AW: usize = 2048;
const AH: usize = 768;
const BORE_MM: f64 = 18.6;
/// The lost-wax fill floor.
const MIN_SECTION_MM: f64 = 0.8;

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

/// The ring's sand set-up, kept so the two-part pull is still measured as a bonus; the ring is judged as lost wax.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.name = "Chamaeleo / Delft clay (bonus check) / Silver 925".into();
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Lost wax. The head is one sculpted part joined to the keyed body; the turret eyes' pupils and the \
        mouth are cast. After the pour: cut the alexandrite's seat in its boss to the measured stone and burnish it. \
        Polish the casque, the eyes and the coils; leave the hide satin."
        .into();
    s
}

/// The body: a flat band squared at the sides, rising under the head to a plinth about 9 mm tall whose side faces
/// are the chameleon's flanks, the palm the reference.
const PROFILE: (f64, f64) = (7.0, 3.0);
const KEYS: [(f64, f64, f64); 11] = [
    (0.0, 1.0, 0.8),
    (25.0, 1.04, 0.92),
    (45.0, 1.14, 1.5),
    (60.0, 1.25, 2.6),
    (70.0, 1.3, 3.0),
    (112.0, 1.3, 3.0),
    (124.0, 1.25, 2.6),
    (140.0, 1.14, 1.5),
    (160.0, 1.04, 0.92),
    (200.0, 1.0, 0.8),
    (340.0, 1.0, 0.8),
];

fn band() -> Result<RingDesign> {
    let mut d = RingDesign { name: "Chamaeleo \u{2014} the casque".into(), ..RingDesign::default() };
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = PROFILE.0;
    d.profile.thickness_mm = PROFILE.1;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = KEYS.iter().map(|&(theta_deg, width_scale, thickness_scale)| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 }).collect();
    d.build = export_params();
    let s = setup();
    d.draft.sand = s.recipe.sand;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d.manufacturing = Some(s);
    Ok(d)
}

fn release_line(r: &mf::release::ReleaseReport) -> String {
    let depth = r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max);
    format!("{:?}, {} obstructions (deepest {depth:.3} mm), {} unresolved", r.status, r.obstructions.len(), r.unresolved_rays)
}

/// The two-part pull at `params`, reported only: the inspection at 0.100 mm and the rays again at 0.075 mm.
fn pull(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(mf::Inspection, mf::release::ReleaseReport)> {
    let s = d.manufacturing.clone().unwrap_or_else(setup);
    let inspection = mf::inspect(d, lib, &s, params)?;
    let mut fine = s.clone();
    fine.sample_pitch_mm = 0.075;
    let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    Ok((inspection, release))
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

// --- The head -----------------------------------------------------------------------------------------------------
//
// Drawn in the head's own frame on the plinth's crown: `f` forward toward the snout (falling theta), `u` up from the
// crown, `z` across the band. The snout leads toward rising theta, so the hero camera meets the face. The skull is a long ellipsoid with a short snout; the casque a thin blade tilted up
// and back from the brow, its back cut steep; a turret cone on each side carrying a cast pinhole pupil; the mouth a
// groove along each jaw. The part reaches 0.8 mm into the plinth and is joined to it.

/// The head's station round the ring, degrees, and how far its frame stands under the crown, mm.
const HEAD_THETA: f64 = 90.0;
const SINK_MM: f64 = 0.8;
/// The eye turrets: the centre of each on the skull (f, u, |z|), its reach out, its base and tip radii.
const EYE: (P3, f64, f64) = ([2.4, 1.75, 2.15], 1.3, 0.18);
/// The head's granules: lattice pitch and height, in the head's unit sketch.
const GRAIN: (f64, f64) = (0.42, 0.17);
/// How much bigger the head is drawn than its unit sketch.
const HEAD_SCALE: f64 = 1.55;
const PUPIL_MM: f64 = 0.1;
/// The casque: an ellipsoid's centre and semi-axes and its tilt up and back.
const CASQUE: (P3, P3, f64) = ([-2.1, 2.55, 0.0], [3.0, 1.75, 1.3], 24.0);
/// Half the width of the casque's smooth ridge, unit sketch.
const CASQUE_RIDGE: f64 = 0.26;
/// The sculpt's grid step and face budget.
const STEP_MM: f64 = 0.06;
const FACES: usize = 360_000;

struct HeadFrame {
    o: P3,
    fwd: P3,
    up: P3,
}

impl HeadFrame {
    fn world(&self, q: P3) -> P3 {
        add(add(add(self.o, mul(self.fwd, q[0])), mul(self.up, q[1])), [0.0, 0.0, q[2]])
    }
    fn local(&self, p: P3) -> P3 {
        let d = [p[0] - self.o[0], p[1] - self.o[1], p[2] - self.o[2]];
        [d[0] * self.fwd[0] + d[1] * self.fwd[1], d[0] * self.up[0] + d[1] * self.up[1], d[2]]
    }
}

/// The capsule distance from `p` to the polyline `pts`, radius `r`.
fn tube(p: P3, pts: &[P3], r: f64) -> f64 {
    pts.windows(2).map(|w| round_cone(p, w[0], w[1], r, r)).fold(f64::MAX, f64::min)
}

/// The head's distance field in its own frame, drawn at unit size and scaled up by `HEAD_SCALE`: negative inside.
fn head_field(q: P3) -> f64 {
    let k = HEAD_SCALE;
    let f = head_shape([q[0] / k, q[1] / k, q[2] / k]) * k;
    // Into the plinth, no further.
    smax(f, -(q[1] + SINK_MM), 0.2)
}

fn head_shape(q: P3) -> f64 {
    let at = |c: P3| [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
    let skull = ellipsoid(at([0.0, 1.2, 0.0]), [4.4, 1.9, 2.5]);
    // The snout runs on 3 to 4 mm past the eyes, narrowing to a blunt end, its top sloping down from the brow.
    let snout = ellipsoid(at([4.4, 0.85, 0.0]), [2.1, 1.05, 1.45]);
    let jaw = ellipsoid(at([0.9, 0.2, 0.0]), [4.4, 1.35, 2.3]);
    let mut f = smin(smin(skull, snout, 1.1), jaw, 0.8);
    let helmet = casque(q);
    f = smin(f, helmet, 1.0);
    // Pebbled skin everywhere but the casque's narrow ridge and the eyes.
    let ridge = (q[2].abs() - CASQUE_RIDGE).max(-(q[0] - 1.0)).max(1.8 - q[1]);
    let eyes = [1.0f64, -1.0].map(|side| ellipsoid(at(eye_centre(side)), [EYE.1 + 0.25; 3]));
    let keep = ridge.min(eyes[0]).min(eyes[1]);
    if f.abs() < 0.5 && keep > 0.0 {
        f -= granules(q) * (keep / 0.2).clamp(0.0, 1.0);
    }
    // A turret each side: a dome beaded in rings like the skin, a smooth pupil boss at its crown with its pit,
    // looking forward and out.
    for side in [1.0, -1.0] {
        f = smin(f, turret(q, side), 0.35);
    }
    // The mouth: the chameleon's downturned grin, from the snout's tip back and down under the eye.
    for side in [1.0, -1.0] {
        let line = [[6.35, 0.62, side * 0.55], [5.3, 0.48, side * 1.15], [4.0, 0.34, side * 1.75], [2.6, 0.2, side * 2.15], [1.4, 0.02, side * 2.3], [0.9, -0.3, side * 2.3]];
        f = smax(f, -tube(q, &line, 0.24), 0.05);
    }
    f
}

fn eye_centre(side: f64) -> P3 {
    [EYE.0[0], EYE.0[1], side * EYE.0[2]]
}

/// The eye's axis: out from the head and turned forward.
fn eye_axis(side: f64) -> P3 {
    let (s, c) = 25f64.to_radians().sin_cos();
    let a = [s, 0.12, side * c];
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    [a[0] / l, a[1] / l, a[2] / l]
}

fn turret(q: P3, side: f64) -> f64 {
    let c = eye_centre(side);
    let r = EYE.1;
    let d = [q[0] - c[0], q[1] - c[1], q[2] - c[2]];
    let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let mut t = dist - r;
    if dist > r + 0.6 {
        return t;
    }
    let a = eye_axis(side);
    let cross = |u: P3, v: P3| [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let norm = |u: P3| {
        let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        [u[0] / l, u[1] / l, u[2] / l]
    };
    let b1 = norm(cross(a, [0.0, 1.0, 0.0]));
    let b2 = cross(a, b1);
    // The beads: rings of granules round the axis, smaller toward the crown.
    let mut beads = f64::MAX;
    for (k, phi) in [36.0f64, 52.0, 68.0, 84.0].into_iter().enumerate() {
        let (sp, cp) = phi.to_radians().sin_cos();
        let rb = EYE.2 * (0.8 + 0.07 * k as f64);
        let n = ((2.0 * PI * r * sp) / (2.0 * rb + 0.05)).floor().max(6.0) as usize;
        for i in 0..n {
            let w = 2.0 * PI * (i as f64 + 0.5 * (k % 2) as f64) / n as f64;
            let (sw, cw) = w.sin_cos();
            let p = [0, 1, 2].map(|j| c[j] + (r - 0.25 * rb) * (a[j] * cp + sp * (b1[j] * cw + b2[j] * sw)));
            let e = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt() - rb;
            beads = beads.min(e);
        }
    }
    t = smin(t, beads, 0.06);
    // The pupil boss and its pit.
    let boss_c = [0, 1, 2].map(|j| c[j] + a[j] * (r - 0.12));
    let boss = ((q[0] - boss_c[0]).powi(2) + (q[1] - boss_c[1]).powi(2) + (q[2] - boss_c[2]).powi(2)).sqrt() - 0.36;
    t = smin(t, boss, 0.12);
    let pit_c = [0, 1, 2].map(|j| c[j] + a[j] * (r + 0.3));
    let pit = ((q[0] - pit_c[0]).powi(2) + (q[1] - pit_c[1]).powi(2) + (q[2] - pit_c[2]).powi(2)).sqrt() - PUPIL_MM;
    smax(t, -pit, 0.03)
}

/// The casque: a helmet grown out of the skull, an ellipsoid tilted up and back so its top curves from between the
/// eyes over a peak at the back of the skull and rolls down to the neck; narrow at the brow, broad behind.
fn casque(q: P3) -> f64 {
    let (centre, radii, tilt) = (CASQUE.0, CASQUE.1, CASQUE.2);
    let d = [q[0] - centre[0], q[1] - centre[1], q[2]];
    let (s, c) = tilt.to_radians().sin_cos();
    let r = [d[0] * c - d[1] * s, d[0] * s + d[1] * c, d[2]];
    // Narrower toward the brow: the wedge in plan.
    let t = ((q[0] - (centre[0] - radii[0])) / (2.0 * radii[0])).clamp(0.0, 1.0);
    let squeeze = 1.0 - 0.55 * t;
    ellipsoid([r[0], r[1], r[2] / squeeze], radii) * squeeze.max(0.45)
}

/// Cast granules for the head's skin: a jittered lattice of domes, each its own size, as a raise of the surface.
fn granules(q: P3) -> f64 {
    let c = GRAIN.0;
    let cell = [(q[0] / c).floor() as i64, (q[1] / c).floor() as i64, (q[2] / c).floor() as i64];
    let mut best = 0.0f64;
    for i in -1..=1 {
        for j in -1..=1 {
            for k in -1..=1 {
                let n = [cell[0] + i, cell[1] + j, cell[2] + k];
                let h = |salt: u64| {
                    let mut z = (n[0] as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (n[1] as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (n[2] as u64).wrapping_mul(0x1656_67B1_9E37_79F9) ^ salt;
                    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
                };
                let p = [(n[0] as f64 + 0.25 + 0.5 * h(1)) * c, (n[1] as f64 + 0.25 + 0.5 * h(2)) * c, (n[2] as f64 + 0.25 + 0.5 * h(3)) * c];
                let r = c * (0.42 + 0.2 * h(4));
                let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt() / r;
                if d < 1.0 {
                    best = best.max((1.0 - d * d).powi(2));
                }
            }
        }
    }
    GRAIN.1 * best
}

/// Mesh the head and return it as a closed, uncrossed solid in world coordinates.
fn sculpt_head(frame: &HeadFrame) -> Result<csg::Solid> {
    let field = |p: P3| head_field(frame.local(p));
    let k = HEAD_SCALE;
    let corners: Vec<P3> = [-6.0 * k, 7.0 * k].iter().flat_map(|&f| [-1.2f64, 6.6 * k].into_iter().flat_map(move |u| [-4.8 * k, 4.8 * k].into_iter().map(move |z| [f, u, z]))).collect();
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for c in &corners {
        let w = frame.world(*c);
        for k in 0..3 {
            lo[k] = lo[k].min(w[k]);
            hi[k] = hi[k].max(w[k]);
        }
    }
    let mut raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &field);
    let unrelaxed = raw.clone();
    sculpt::relax(&mut raw, &field, 3);
    if !sculpt::crossing_sites(&raw).is_empty() {
        raw = unrelaxed;
    }
    let nets = sculpt::clean_decimate(&raw, FACES);
    let s = sculpt::settle(nets, &field, &|_| false);
    let (open, volume) = sculpt::closure(&s);
    ensure!(open == 0 && volume > 0.0, "the head does not close: {open} open edges");
    ensure!(csg::self_crossings(&s) == 0, "the head crosses itself");
    Ok(s)
}

// --- The tail -----------------------------------------------------------------------------------------------------
//
// Drawn on the flank as seen along the finger (x round the ring, y up from the axis): from the body's rear it runs
// forward along the flank and coils, tightening, under the head.

/// The coil's station (theta, radius from the axis), its outer and inner radius and turns; the run's start and the
/// stroke's width at the root and the tip.
const COIL: (f64, f64, f64, f64, f64) = (96.0, 13.5, 3.25, 0.5, 2.4);
const TAIL_ROOT: (f64, f64) = (58.0, 15.0);
const TAIL_W: (f64, f64) = (1.05, 0.42);
const TAIL_MM: f64 = 0.9;

fn polar(theta: f64, r: f64) -> [f64; 2] {
    let t = theta.to_radians();
    [r * t.cos(), r * t.sin()]
}

/// The tail's centreline: the run from the root, then the coil turning inward.
fn tail_line() -> Vec<[f64; 2]> {
    let (ct, cr, r_out, r_in, turns) = COIL;
    let c = polar(ct, cr);
    // The coil enters at its top, heading toward the snout (falling theta: +x at the top of the ring is falling x?).
    let up = [c[0] / cr, c[1] / cr];
    let entry = [c[0] + up[0] * r_out, c[1] + up[1] * r_out];
    let root = polar(TAIL_ROOT.0, TAIL_ROOT.1);
    let mid = polar(0.5 * (TAIL_ROOT.0 + ct) - 4.0, 16.4);
    let mut pts = Vec::new();
    // A quadratic run from the root through the flank's top into the coil's entry.
    let n = 400;
    for k in 0..n {
        let t = k as f64 / n as f64;
        let (a, b, cc) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
        pts.push([a * root[0] + b * mid[0] + cc * entry[0], a * root[1] + b * mid[1] + cc * entry[1]]);
    }
    // The coil turns the way the run arrives: toward falling theta at its top, then down and back under itself.
    let start = up[1].atan2(up[0]);
    let dir = 1.0;
    let span = turns * 2.0 * PI;
    let m = (span * r_out / 0.02).ceil() as usize;
    for k in 0..=m {
        let a = span * k as f64 / m as f64;
        let r = r_out + (r_in - r_out) * a / span;
        let phi = start + dir * a;
        pts.push([c[0] + r * phi.cos(), c[1] + r * phi.sin()]);
    }
    pts
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
        let k = ((a[0] - b[0]).hypot(a[1] - b[1]) / 0.16).ceil().max(1.0) as usize;
        for j in 0..k {
            let t = j as f64 / k as f64;
            fine.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    fine
}

/// The tail as a closed outline on the flank: the centreline offset by its tapering half width, ends rounded.
fn tail_outline() -> Vec<[f64; 2]> {
    let c = tail_line();
    let n = c.len();
    let mut len = vec![0.0; n];
    for i in 1..n {
        len[i] = len[i - 1] + (c[i][0] - c[i - 1][0]).hypot(c[i][1] - c[i - 1][1]);
    }
    let total = len[n - 1];
    let half = |i: usize| 0.5 * (TAIL_W.0 + (TAIL_W.1 - TAIL_W.0) * (len[i] / total).powf(0.8));
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
    let (t, m) = (c[n - 1], normal(n - 1));
    let h = half(n - 1);
    for k in 1..24 {
        let ang = (-m[1]).atan2(-m[0]) + PI * k as f64 / 24.0;
        dense.push([t[0] + h * ang.cos(), t[1] + h * ang.sin()]);
    }
    dense.extend((0..n).rev().map(|i| side(i, 1.0)));
    let (r0, m0) = (c[0], normal(0));
    let h0 = half(0);
    for k in 1..24 {
        let ang = m0[1].atan2(m0[0]) + PI * k as f64 / 24.0;
        dense.push([r0[0] + h0 * ang.cos(), r0[1] + h0 * ang.sin()]);
    }
    let mut out = thin(&dense, 0.12);
    if outline::area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// The nearest sample squarely on a side face to `(x, y)` seen along the finger, on the `side` of the band.
fn on_side(a: &Atlas, x: f64, y: f64, side: f64) -> Option<(f64, f64)> {
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && s.n[2] * side > 0.97)
        .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
        .filter(|(d, _)| *d < 0.25)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, s)| (s.theta, s.v))
}

/// A flank-drawn outline as a stamp standing along the pull at the side-face point nearest `centre`.
fn side_stamp(a: &Atlas, name: &str, centre: [f64; 2], side: f64, drawn: &[[f64; 2]], height: f64) -> Result<Stamp> {
    let at = on_side(a, centre[0], centre[1], side).with_context(|| format!("no side face at {centre:?}"))?;
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
        top: StampTop::Pillow { crown_mm: 0.3 },
    })
}

fn alexandrite() -> Gem {
    Gem { l_mm: 5.0, preview_tint: Some(ALEXANDRITE_TINT), ..Gem::calibrated(GemCut::Cushion, 4.0) }
}

/// The alexandrite's station on the back, behind the casque, degrees.
const STONE_THETA: f64 = 61.0;

/// What was made and where, for the report.
#[derive(Default, serde::Serialize)]
struct Placed {
    keys: Vec<[f64; 3]>,
    flank_height_mm: f64,
    head_frame: [f64; 6],
    head_faces: usize,
    head_volume_mm3: f64,
    head_top_over_crown_mm: f64,
    eye_reach_past_skull_mm: f64,
    stone_at: [f64; 2],
    tail: Vec<String>,
    coil_outer_diameter_mm: f64,
}

fn probe(d: &RingDesign) -> Result<()> {
    let a = Atlas::of(d, AW, AH)?;
    eprintln!("top {:.3} bore {:.3} span {:.3}", a.top, a.bore, a.span);
    for deg in (0..=180).step_by(10) {
        let f = (deg as f64).to_radians();
        let side: Vec<f64> = a.samples.iter().filter(|s| s.p[2] > 0.0 && s.n[2] > 0.97 && (s.p[0].atan2(s.p[1]) - (PI / 2.0 - f)).abs() < 0.01).map(|s| s.p[0].hypot(s.p[1])).collect();
        let r = side.iter().fold((f64::MAX, f64::MIN), |m, r| (m.0.min(*r), m.1.max(*r)));
        let crest = a.samples.iter().filter(|s| (s.theta - deg as f64).abs() < 0.2).map(|s| s.p[0].hypot(s.p[1])).fold(0.0, f64::max);
        eprintln!("  theta {deg}: side face r {r:.2?}, crest r {crest:.2}");
    }
    Ok(())
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed, csg::Solid)> {
    let mut d = band()?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let mut placed = Placed { keys: KEYS.iter().map(|k| [k.0, k.1, k.2]).collect(), ..Default::default() };
    placed.flank_height_mm = PROFILE.1 * 3.0;

    // The head's frame on the crown at its station.
    let crest = |theta: f64| a.samples.iter().filter(|s| (s.theta - theta).abs() < 0.2).map(|s| s.p[0].hypot(s.p[1])).fold(0.0, f64::max);
    let r = crest(HEAD_THETA) - SINK_MM * 0.0;
    let t = HEAD_THETA.to_radians();
    let frame = HeadFrame { o: [r * t.cos(), r * t.sin(), 0.0], fwd: [-t.sin(), t.cos(), 0.0], up: [t.cos(), t.sin(), 0.0] };
    placed.head_frame = [frame.o[0], frame.o[1], frame.o[2], HEAD_THETA, r, SINK_MM];
    let head = sculpt_head(&frame)?;
    placed.head_faces = head.f.len();
    placed.head_volume_mm3 = sculpt::closure(&head).1;
    placed.head_top_over_crown_mm = head.v.iter().map(|p| frame.local(*p)[1]).fold(f64::MIN, f64::max);
    placed.eye_reach_past_skull_mm = head.v.iter().map(|p| frame.local(*p)[2].abs()).fold(0.0, f64::max) - 2.55 * HEAD_SCALE;
    let packed = sculpt::packed(&head)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Keyed body".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "chameleon head".into(),
        params: json!({ "theta_deg": HEAD_THETA, "step_mm": STEP_MM, "faces": FACES, "casque": [CASQUE.0, CASQUE.1, [CASQUE.2, 0.0, 0.0]], "grain": [GRAIN.0, GRAIN.1], "eye": [EYE.0, EYE.1, EYE.2] }),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Head".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;

    // The alexandrite on its boss on the back, behind the casque.
    let ctx = d.field_context();
    let mut seat = SeatPadLayer {
        theta_deg: STONE_THETA,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::Boss,
        crown: 0.85,
        blend_mm: 0.5,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        mark_mm: 0.6,
        ..Default::default()
    };
    seat.fit_stone(alexandrite());
    seat.height_mm = 0.9;
    placed.stone_at = [STONE_THETA, ctx.crest_v_mm];
    let mut e = LayerEntry::new("Alexandrite", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);

    // The tail, coiled on each flank.
    let tail = tail_outline();
    if let Err(e) = outline::check(&tail) {
        eprintln!("tail: {e}");
    }
    placed.coil_outer_diameter_mm = 2.0 * COIL.2 + TAIL_W.0;
    let c = polar(COIL.0, COIL.1);
    for (k, side) in [1.0f64, -1.0].into_iter().enumerate() {
        let s = side_stamp(&a, &format!("Tail, {}", if k == 0 { "fingertip" } else { "knuckle" }), c, side, &tail, TAIL_MM)?;
        placed.tail.push(format!("{} at {:.2} deg, v {:.3}, {} points", s.name, s.theta_deg, s.v_mm, s.outline.len()));
        d.stamps.push(s);
    }
    let _ = blockout;
    if let Ok(skip) = std::env::var("CHAM_SKIP") {
        let skip: Vec<&str> = skip.split(';').collect();
        d.stamps.retain(|s| !skip.iter().any(|k| s.name.starts_with(k)));
        d.layers.layers.retain(|e| !skip.iter().any(|k| e.name.starts_with(k)));
    }
    let lib = mf::source_library(&d, &lib).into_owned();
    Ok((d, lib, placed, head))
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
    // Lost wax: fill and detail decide; the two-part pull, the clamp and the monotone rule are reported as a
    // bonus and gate nothing.
    let sand_pulls = bites_ok
        && coarse_release.obstructions.is_empty()
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
        && field.process == CastProcess::LostWax
        && field.thinnest_wall_mm >= MIN_SECTION_MM
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
        "sand_note": "Reported only: the ring is judged as lost wax (Logan, 2026-10-03). The release, clamp and monotone lines say whether it would also pull from Delft.",
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
        "process": field.process.label(),
        "sand_bonus_pulls_two_part": sand_pulls,
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders -----------------------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.5, 0.55),
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
    bare.cad = None;
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
        return probe(&band()?);
    }
    std::fs::create_dir_all(&out)?;
    println!("Chamaeleo");
    let (d, lib, placed, head) = author(blockout)?;
    println!("  head: {} faces, {:.1} mm3, top {:.2} mm over the crown", placed.head_faces, placed.head_volume_mm3, placed.head_top_over_crown_mm);
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
        "body": { "profile_mm": [PROFILE.0, PROFILE.1], "keys_theta_width_thickness": KEYS.iter().map(|k| [k.0, k.1, k.2]).collect::<Vec<_>>() },
        "head_part": { "faces": head.f.len(), "self_crossings": csg::self_crossings(&head), "open_edges": sculpt::closure(&head).0 },
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
