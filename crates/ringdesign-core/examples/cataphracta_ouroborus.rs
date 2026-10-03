//! Cataphracta — Ouroborus, the girdled wheel: *Ouroborus cataphractus*, the armadillo girdled lizard, curled into a
//! ring with its own tail in its jaws. Lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_ouroborus
//! target/release/examples/cataphracta_ouroborus [OUT_DIR] [--draft] [--verify] [--probe]
//!
//! The band is the lizard's body: a broad, flat back girdled in graded, overlapping whorls from the nape round to a
//! tapering tail whose tip runs into the jaws at the top. The head is one made part, built section by section: a broad,
//! flat, triangular skull about 1.4 times as long as wide, plated, with an eye bulging from each side under its brow,
//! an ear opening behind the jaw, a fringe of spiny occipital scales flaring back over the neck, and the gape closed
//! on the tail. Four short legs lie tucked against the flanks, forelegs behind the head and hind legs at the hips, each
//! a made part: a height field over the side face, upper limb, lower limb and splayed clawed toes.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    field::{Blend, Layer, LayerEntry, SideFacePick, VGate, Window},
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade, TilingLayer},
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, SandProcess, Verdict},
    csg, dfm, library, manufacturing as mf, mesh,
    profile::ShankKey,
    reptile,
    render, sculpt,
    skin::Atlas,
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const NAME: &str = "Ouroborus \u{2014} the girdled wheel";
const SLUG: &str = "ouroborus";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Lost wax: the investment fill floor Logan set for this collection, mm.
const MIN_SECTION_MM: f64 = 0.8;

/// Ring angle of the snout's tip, degrees; the head runs toward increasing angle, so the face camera at 90° looks
/// straight down on the middle of the skull.
const SNOUT_DEG: f64 = 68.0;
/// Radius at which the head's frame measures arc along the ring, mm.
const R_REF: f64 = 13.0;
/// Stations along the head and points down each half-section.
const STATIONS: usize = 240;
const SECTION_PTS: usize = 160;

/// The keyed body: (degrees, width scale, thickness scale, crown scale) on Flat 3.4 x 2.8 (the tail's tip is the
/// reference width). The tail's tip runs into the jaws at 68° and is narrowest just inside them, the band widens
/// under the head to the neck, holds broad and flat through the body, and tapers from the hips round to the tail.
const KEYS: [(f64, f64, f64, f64); 15] = [
    (0.0, 0.88, 0.74, 0.8),
    (40.0, 0.66, 0.62, 0.65),
    (58.0, 0.48, 0.54, 0.55),
    (68.0, 0.34, 0.48, 0.5),
    (74.0, 0.3, 0.44, 0.5),
    (86.0, 0.55, 0.6, 0.6),
    (100.0, 1.0, 0.88, 0.8),
    (112.0, 1.12, 1.1, 1.0),
    (125.0, 1.15, 1.12, 1.0),
    (148.0, 1.4, 1.12, 1.0),
    (190.0, 1.5, 1.1, 1.0),
    (228.0, 1.42, 1.04, 1.0),
    (262.0, 1.3, 0.96, 0.95),
    (300.0, 1.15, 0.88, 0.9),
    (332.0, 1.0, 0.8, 0.85),
];

// --- The head's primary forms, along `s` mm from the snout's tip --------------------------------------------------

/// Half-width at the widest of the side wall: a rounded snout, straight sides flaring to the angle of the jaws, then
/// in to the neck. The skull is about 10.5 mm long and 7.4 mm across at the jaws.
const PLAN: [(f64, f64); 13] = [
    (0.0, 0.0),
    (0.25, 0.7),
    (0.7, 1.12),
    (1.6, 1.62),
    (3.0, 2.25),
    (4.6, 2.85),
    (6.2, 3.3),
    (7.6, 3.62),
    (8.5, 3.7),
    (9.3, 3.45),
    (10.0, 2.85),
    (10.6, 2.45),
    (11.2, 2.15),
];
/// Height of the dorsal crest over the bore: low at the snout, flat over the skull.
const CREST: [(f64, f64); 11] = [
    (0.0, 1.95),
    (0.3, 2.35),
    (0.8, 2.7),
    (1.8, 3.05),
    (3.2, 3.35),
    (4.8, 3.55),
    (6.6, 3.65),
    (8.4, 3.68),
    (9.6, 3.5),
    (10.4, 3.32),
    (11.2, 3.1),
];
/// The dorsal fall from the crest to the canthus: a flat skull rounding over at its edges.
const DOME: [(f64, f64); 4] = [(0.0, 0.35), (3.0, 0.45), (7.0, 0.55), (11.2, 0.5)];
/// The cephalic shields' sutures in plan, (s, z) segments on the +z half: rostral and internasals, the frontonasal,
/// the prefrontals, the long frontal between the supraoculars, the frontoparietals, the interparietal flanked by the
/// parietals, and the occipital row ahead of the spiny fringe. Paired shields meet on a median suture.
const SUTURES: [[(f64, f64); 2]; 18] = [
    [(0.8, 0.0), (1.0, 1.1)],
    [(2.2, 0.0), (2.3, 1.75)],
    [(3.3, 0.0), (3.3, 1.0)],
    [(3.3, 1.0), (3.1, 2.1)],
    [(3.3, 1.0), (5.4, 0.75)],
    [(5.4, 0.75), (6.0, 0.0)],
    [(3.1, 2.1), (4.4, 2.6)],
    [(4.4, 2.6), (5.8, 2.5)],
    [(5.8, 2.5), (5.4, 0.75)],
    [(5.4, 0.75), (6.6, 1.6)],
    [(6.6, 1.6), (5.8, 2.5)],
    [(6.0, 0.0), (6.8, 0.75)],
    [(6.8, 0.75), (8.3, 0.6)],
    [(8.3, 0.6), (8.8, 0.0)],
    [(6.6, 1.6), (6.8, 0.75)],
    [(6.6, 1.6), (8.6, 2.35)],
    [(8.6, 2.35), (8.3, 0.6)],
    [(1.0, 0.0), (3.3, 0.0)],
];
/// A suture's depth and half-width, mm.
const SUTURE: (f64, f64) = (0.22, 0.13);
/// How far the dorsal plan stands inside the side wall's widest.
const TOP_INSET: [(f64, f64); 4] = [(0.0, 0.1), (3.0, 0.14), (7.0, 0.2), (11.2, 0.15)];
/// The head's length from the snout's tip to where it sinks into the neck, and where that burial starts.
const HEAD_LEN: f64 = 12.2;
const BURY_FROM: f64 = 10.0;
/// The spiny occipital fringe on the plan: first spine's root, pitch, and each spine's reach, mm. Each spine rises
/// slowly toward its point and drops back steeply behind it, so the points rake back over the neck.
const SPINES: (f64, f64, [f64; 4]) = (7.2, 0.8, [0.38, 0.55, 0.62, 0.5]);
/// The gape line's height at the snout and its sag to the mouth's corner.
const LIP_H: f64 = 1.85;
const LIP_SAG: f64 = 0.22;
const MOUTH_CORNER: f64 = 7.4;
/// The mouth's corner turns up over its last stretch: the rise and the length it takes, mm.
const MOUTH_UPTURN: (f64, f64) = (0.35, 1.5);
/// The lower jaw: where the chin starts, its inset under the upper lip, and the height of the throat's bottom.
const CHIN_S: f64 = 0.55;
const JAW_INSET: f64 = 0.16;
const JAW_BOTTOM: f64 = 0.06;
/// The tail runs into the mouth: the gape between the lips round it at the snout, and where the lips close behind it.
const TAIL_GAPE: f64 = 1.15;
const TAIL_IN: (f64, f64) = (2.0, 3.0);
/// Inside the gape the head keeps this share of the tail's own half-width, hidden in the tail.
const WAIST: f64 = 0.6;
/// The gape groove: depth and half-width.
const GAPE_MM: f64 = 0.45;
const GAPE_W: f64 = 0.24;

/// The eye: centre along the head and over the bore, radius, and how far it bulges past the wall.
const EYE_S: f64 = 4.4;
const EYE_H: f64 = 2.7;
const EYE_R: f64 = 0.95;
const EYE_BULGE: f64 = 0.85;
/// The round pupil: radius and depth.
const PUPIL: (f64, f64) = (0.34, 0.2);
/// The orbit groove round the eye: width and depth.
const ORBIT: (f64, f64) = (0.28, 0.12);
/// The brow: how far the supraocular overhangs the eye, and its reach along the head.
const BROW: (f64, f64) = (0.4, 1.25);
/// The nostril: centre along the head, over the lip, radius and depth.
const NOSTRIL: (f64, f64, f64, f64) = (0.75, 0.55, 0.2, 0.16);
/// The ear opening behind the jaw: centre along the head and over the bore, half-length, half-height, depth.
const EAR: (f64, f64, f64, f64, f64) = (8.7, 2.2, 0.3, 0.45, 0.2);

// --- The legs ------------------------------------------------------------------------------------------------------

/// One limb bone or toe of a leg: from `a` to `b` in (mm tailward of the leg's root at its own radius, mm over the
/// bore), half-width `r` and height over the flank `h` at each end.
#[derive(Clone, Copy)]
struct Bone {
    a: [f64; 2],
    b: [f64; 2],
    r: (f64, f64),
    h: (f64, f64),
}

const fn bone(a: [f64; 2], b: [f64; 2], r: (f64, f64), h: (f64, f64)) -> Bone {
    Bone { a, b, r, h }
}

/// A foreleg tucked back along the flank: a fat upper arm running back from the shoulder to the elbow, the forearm
/// bent sharply down to the wrist by the bore, four splayed, clawed toes raking tailward.
const FORELEG: [Bone; 7] = [
    bone([0.0, 2.05], [1.7, 1.8], (0.95, 0.66), (1.4, 1.3)),
    bone([1.7, 1.8], [2.6, 0.8], (0.6, 0.48), (1.25, 1.1)),
    bone([2.6, 0.8], [2.9, 0.8], (0.5, 0.48), (1.05, 1.0)),
    bone([2.9, 0.8], [3.75, 1.75], (0.3, 0.2), (1.1, 0.65)),
    bone([2.9, 0.8], [4.35, 1.3], (0.3, 0.2), (1.1, 0.65)),
    bone([2.9, 0.8], [4.45, 0.62], (0.28, 0.2), (1.05, 0.6)),
    bone([2.9, 0.8], [3.95, 0.3], (0.26, 0.19), (1.0, 0.55)),
];
/// A hind leg folded along the tail: the thigh back from the hip to the knee, the shin bent down to the ankle, four
/// long toes.
const HINDLEG: [Bone; 7] = [
    bone([0.0, 1.85], [1.9, 1.5], (1.0, 0.7), (1.6, 1.45)),
    bone([1.9, 1.5], [2.9, 0.72], (0.58, 0.46), (1.35, 1.15)),
    bone([2.9, 0.72], [3.2, 0.72], (0.48, 0.46), (1.25, 1.15)),
    bone([3.2, 0.72], [4.2, 1.65], (0.3, 0.2), (1.0, 0.6)),
    bone([3.2, 0.72], [4.85, 1.2], (0.3, 0.2), (1.0, 0.6)),
    bone([3.2, 0.72], [4.95, 0.55], (0.28, 0.2), (1.0, 0.6)),
    bone([3.2, 0.72], [4.3, 0.28], (0.26, 0.19), (0.95, 0.55)),
];
/// The legs' roots round the ring, degrees: the shoulders just behind the head, the hips where the tail begins.
const FORE_DEG: f64 = 121.0;
const HIND_DEG: f64 = 226.0;
/// The legs' height-field grid pitch, mm, and the margin of buried slab kept round each footprint.
const LEG_GRID: f64 = 0.06;
const LEG_MARGIN: f64 = 0.2;

// --- The body's hide -----------------------------------------------------------------------------------------------

/// The whorls: girdle pitch at the nape and at the tail's tip, mm, the grade's seam (hidden in the jaws), the
/// girdle's height.
const WHORL_PITCH: (f64, f64) = (2.6, 1.0);
const WHORL_SEAM_DEG: f64 = SNOUT_DEG + 4.0;
const WHORL_MM: f64 = 0.6;
/// How far each girdle's free edge bows tailward at the crest, as a share of the pitch.
const WHORL_BOW: f64 = 0.32;
/// Each girdle's free edge is toothed into this many spiny scale points across the crown, each reaching this share of
/// the pitch.
const TEETH: f64 = 5.0;
const TOOTH: f64 = 0.28;
/// The whorls run from the nape round to the tail's tip: the window's centre and span, and its fade, degrees.
const WHORL_WINDOW: (f64, f64, f64) = (268.0, 298.0, 8.0);
/// The flank spines on the side faces, one per girdle on its trailing edge: height, mm.
const SPINE_MM: f64 = 0.4;
/// The girdles' continuation down the flanks, lower than on the back so the legs stand clear of it, mm.
const FLANK_MM: f64 = 0.35;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The investment set-up: lost wax in silver, a sprue off the palm.
fn setup(d: &RingDesign) -> mf::Setup {
    let mut setup = mf::Setup::from_design(d);
    setup.recipe.name = "Ouroborus / investment / Silver 925".into();
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(1.9, |m| m.shrink_pct);
    setup.sample_pitch_mm = 0.1;
    setup.channels = vec![mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -10.8, 0.0], end: [0.0, -22.0, 0.0], diameter_mm: 3.0 }];
    setup.bench_notes = "Invest the whole ring with the head and the four legs, sprued at the palm. Clean the gape round the tail and the toes with a fine graver.".into();
    setup
}

/// The same pour judged as Petrobond sand, for the report's bonus line only.
fn sand_setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::Petrobond);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s
}

/// The bare body: Flat 3.4 x 2.8 with a low crown, keyed from the neck round to the tail.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.width_mm = 3.4;
    d.profile.thickness_mm = 2.8;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 1.0;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = KEYS.iter().map(|&(theta_deg, width_scale, thickness_scale, crown_scale)| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale }).collect();
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d.manufacturing = Some(setup(&d));
    d
}

// --- Helpers ------------------------------------------------------------------------------------------------------

/// Monotone cubic through `knots` at `x`, clamped to their range.
fn pchip(knots: &[(f64, f64)], x: f64) -> f64 {
    let n = knots.len();
    let x = x.clamp(knots[0].0, knots[n - 1].0);
    let h: Vec<f64> = (0..n - 1).map(|i| knots[i + 1].0 - knots[i].0).collect();
    let delta: Vec<f64> = (0..n - 1).map(|i| (knots[i + 1].1 - knots[i].1) / h[i]).collect();
    let mut m = vec![0.0; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        if delta[i - 1] * delta[i] > 0.0 {
            let (w1, w2) = (2.0 * h[i] + h[i - 1], h[i] + 2.0 * h[i - 1]);
            m[i] = (w1 + w2) / (w1 / delta[i - 1] + w2 / delta[i]);
        }
    }
    let i = (0..n - 1).find(|&i| x <= knots[i + 1].0).unwrap_or(n - 2);
    let t = (x - knots[i].0) / h[i];
    let (t2, t3) = (t * t, t * t * t);
    knots[i].1 * (2.0 * t3 - 3.0 * t2 + 1.0) + h[i] * m[i] * (t3 - 2.0 * t2 + t) + knots[i + 1].1 * (3.0 * t2 - 2.0 * t3) + h[i] * m[i + 1] * (t3 - t2)
}

fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A rounded V: 1 at the centre, 0 past `w`.
fn vee(x: f64, w: f64) -> f64 {
    let t = (x.abs() / w).min(1.0);
    (1.0 - t * t).powi(2)
}

/// The bare band's half-width along the finger at each height over the bore, column by column round the ring.
struct BandShape {
    cols: Vec<Vec<f64>>,
}

/// Height step of the band's half-width table, mm.
const BAND_DH: f64 = 0.02;

impl BandShape {
    fn of(d: &RingDesign) -> Result<Self> {
        let a = Atlas::of(d, 1440, 384)?;
        let bore = d.inner_radius_mm();
        let bins = (6.0 / BAND_DH) as usize;
        let cols = (0..a.width)
            .map(|x| {
                let mut w = vec![0.0f64; bins];
                for y in 0..a.height {
                    let p = a.at(x, y).p;
                    let h = p[0].hypot(p[1]) - bore;
                    let top = ((h / BAND_DH).floor().max(0.0) as usize).min(bins - 1);
                    for v in &mut w[..=top] {
                        *v = v.max(p[2].abs());
                    }
                }
                w
            })
            .collect();
        Ok(Self { cols })
    }
    /// Half-width at `theta_deg` and `h` mm over the bore, interpolated round the ring and up the height.
    fn half_w(&self, theta_deg: f64, h: f64) -> f64 {
        let n = self.cols.len();
        let x = theta_deg.rem_euclid(360.0) / 360.0 * n as f64;
        let (x0, tx) = (x.floor() as usize % n, x.fract());
        let x1 = (x0 + 1) % n;
        let y = (h / BAND_DH).max(0.0);
        let len = self.cols[0].len();
        let (y0, ty) = ((y.floor() as usize).min(len - 1), y.fract());
        let y1 = (y0 + 1).min(len - 1);
        let at = |c: usize| self.cols[c][y0] * (1.0 - ty) + self.cols[c][y1] * ty;
        at(x0) * (1.0 - tx) + at(x1) * tx
    }
    /// The crest's height over the bore at `theta_deg`.
    fn crest(&self, theta_deg: f64) -> f64 {
        let n = self.cols.len();
        let x = ((theta_deg.rem_euclid(360.0) / 360.0 * n as f64).round() as usize) % n;
        let col = &self.cols[x];
        col.iter().rposition(|&w| w > 1e-6).map_or(0.0, |i| (i as f64 + 1.0) * BAND_DH)
    }
}

// --- The head -----------------------------------------------------------------------------------------------------

/// The head's section at one station: the dorsal surface as a height over the bore against `z`, then the side wall and
/// the jaw as `z` against the height.
struct Head<'a> {
    bore_r: f64,
    band: &'a BandShape,
}

/// One station's section frame.
struct Station {
    s: f64,
    crest: f64,
    canthus: f64,
    /// The dorsal plan's half-width at the canthus, brow included.
    top_w: f64,
    /// The side wall's half-width at the lip and the canthus, and how far its belly stands past that.
    side_w: f64,
    belly: f64,
    lip: f64,
    /// How far the mouth gapes round the tail, 0 to 1, and the lower lip's height.
    open: f64,
    lower_lip: f64,
    jaw_w: f64,
    /// Where the section closes under the jaw.
    bottom: f64,
    theta_deg: f64,
}

/// The occipital fringe's reach past the plan at `s`: each spine rises over most of its pitch and drops behind.
fn spines(s: f64) -> f64 {
    let (start, pitch, reach) = SPINES;
    let k = (s - start) / pitch;
    if k < 0.0 || k >= reach.len() as f64 {
        return 0.0;
    }
    let (i, f) = (k.floor() as usize, k.fract());
    let up = 0.78;
    let shape = if f < up { (f / up).powf(1.7) } else { 1.0 - smoothstep(up, 1.0, f) };
    reach[i] * shape
}

impl Head<'_> {
    fn station(&self, s: f64) -> Station {
        let crest = pchip(&CREST, s);
        let wall_w = pchip(&PLAN, s) + spines(s);
        let belly = pchip(&TOP_INSET, s).min(0.3 * wall_w);
        let side_w = wall_w - belly;
        let top_w = side_w + BROW.0 * vee(s - EYE_S, BROW.1);
        let lip = LIP_H - LIP_SAG * (s / MOUTH_CORNER).min(1.0).powi(2) + MOUTH_UPTURN.0 * smoothstep(MOUTH_CORNER - MOUTH_UPTURN.1, MOUTH_CORNER, s).powi(2);
        let chin = smoothstep(CHIN_S, CHIN_S + 0.9, s).sqrt();
        let jaw_w = (wall_w - JAW_INSET.min(0.1 * wall_w)) * chin;
        let open = 1.0 - smoothstep(TAIL_IN.0, TAIL_IN.1, s);
        let lower_lip = lip - TAIL_GAPE * open;
        let bottom = (lower_lip - 0.1) + (JAW_BOTTOM - lower_lip + 0.1) * chin;
        let theta_deg = SNOUT_DEG + (s / R_REF).to_degrees();
        let mut st = Station { s, crest, canthus: crest, top_w, side_w, belly, lip, open, lower_lip, jaw_w, bottom, theta_deg };
        st.canthus = self.dorsal(&st, top_w);
        st
    }

    /// The dorsal surface's height over the bore at `z` across, from the crest to the canthus: a flat skull whose
    /// plates step down outward (the frontal and parietals over the supraoculars and temporals), paired plates meeting
    /// in a low median ridge.
    fn dorsal(&self, st: &Station, z: f64) -> f64 {
        let s = st.s;
        let u = (z / st.top_w.max(1e-6)).min(1.0);
        let mut h = st.crest - pchip(&DOME, s).min(0.5 * (st.crest - st.lip)) * u.powf(3.2);
        // Each shield is domed a little between its sutures, which are cut as rounded V grooves.
        let mut near = f64::MAX;
        for [a, b] in SUTURES {
            let ab = (b.0 - a.0, b.1 - a.1);
            let t = (((s - a.0) * ab.0 + (z - a.1) * ab.1) / (ab.0 * ab.0 + ab.1 * ab.1)).clamp(0.0, 1.0);
            near = near.min((a.0 + ab.0 * t - s).hypot(a.1 + ab.1 * t - z));
        }
        let fade = 1.0 - smoothstep(st.top_w - 0.35, st.top_w - 0.1, z);
        h -= SUTURE.0 * vee(near, SUTURE.1) * fade;
        h
    }

    /// The side wall's `z` at height `h` between the lip and the canthus: bellied, with the eye, brow, ear and nostril.
    fn wall(&self, st: &Station, h: f64) -> f64 {
        let s = st.s;
        let t = ((h - st.lip) / (st.canthus - st.lip).max(1e-6)).clamp(0.0, 1.0);
        let base = st.side_w + st.belly * (PI * t.powf(0.75)).sin().powf(0.6) * (1.0 - 0.5 * t);
        let brow = BROW.0 * vee(s - EYE_S, BROW.1) * smoothstep(0.6, 1.0, t);
        let mut z = base + brow;
        // The eye: a spherical cap bulging past the wall, ringed by its orbit, with a round pupil.
        let (ds, dh) = (s - EYE_S, h - EYE_H);
        let d = ds.hypot(dh);
        let sphere = (EYE_R * EYE_R + EYE_BULGE * EYE_BULGE) / (2.0 * EYE_BULGE);
        if d < EYE_R {
            z += (sphere * sphere - d * d).sqrt() - (sphere - EYE_BULGE);
            if d < PUPIL.0 {
                z -= PUPIL.1 * (1.0 - (d / PUPIL.0).powi(2)).sqrt();
            }
        }
        z -= ORBIT.1 * vee(d - EYE_R - ORBIT.0 * 0.5, ORBIT.0 * 0.5);
        // The ear opening: an oval pit behind the jaw.
        let e = ((s - EAR.0) / EAR.2).powi(2) + ((h - EAR.1) / EAR.3).powi(2);
        if e < 1.0 {
            z -= EAR.4 * (1.0 - e).sqrt();
        }
        // The nostril.
        let dn = (s - NOSTRIL.0).hypot(h - st.lip - NOSTRIL.1);
        if dn < NOSTRIL.2 {
            z -= NOSTRIL.3 * (1.0 - (dn / NOSTRIL.2).powi(2)).sqrt();
        }
        // The gape's upper lip edge.
        z -= GAPE_MM * vee(h - st.lip, GAPE_W) * smoothstep(MOUTH_CORNER + 0.4, MOUTH_CORNER - 0.2, s) * smoothstep(0.2, 1.4, s);
        z.max(0.0)
    }

    /// The lower jaw's `z` at height `h` under its lip: closed on the upper lip with the gape's groove, or open round
    /// the tail where it enters.
    fn jaw(&self, st: &Station, h: f64) -> f64 {
        let span = (st.lower_lip - st.bottom).max(1e-6);
        let t = ((st.lower_lip - h) / span).clamp(0.0, 1.0);
        let shape = (1.0 - t.powf(2.5)).max(0.0).powf(0.4);
        let closed = smoothstep(MOUTH_CORNER + 0.4, MOUTH_CORNER - 0.2, st.s);
        let groove = GAPE_MM * vee(h - st.lip, GAPE_W) * closed * smoothstep(0.2, 1.4, st.s) * (1.0 - st.open);
        let jaw = st.jaw_w * shape - groove;
        let full = st.side_w * shape;
        let z = jaw * closed + full * (1.0 - closed);
        z.max(self.waist(st, h)).max(0.0)
    }

    /// Inside the gape: a share of the tail's half-width, so the tail shows between the lips.
    fn waist(&self, st: &Station, h: f64) -> f64 {
        (WAIST * self.band.half_w(st.theta_deg, h)).max(0.05)
    }

    /// The half-section at `s` as (height over bore, z) from the crest to the throat, densely, height never rising.
    fn half_section(&self, s: f64) -> Vec<[f64; 2]> {
        let st = self.station(s);
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(900);
        const TOP: usize = 300;
        for k in 0..=TOP {
            let z = st.top_w * k as f64 / TOP as f64;
            pts.push([self.dorsal(&st, z), z]);
        }
        let side_n = 300;
        let top_end = pts.last().unwrap()[0];
        for k in 1..=side_n {
            let h = top_end + (st.lip - top_end) * k as f64 / side_n as f64;
            pts.push([h, self.wall(&st, h)]);
        }
        if st.lip - st.lower_lip > 1e-3 {
            let gape_n = 120;
            for k in 1..=gape_n {
                let h = st.lip + (st.lower_lip - st.lip) * k as f64 / gape_n as f64;
                pts.push([h, self.waist(&st, h)]);
            }
        }
        let jaw_n = 200;
        for k in 1..=jaw_n {
            let h = st.lower_lip + (st.bottom - st.lower_lip) * k as f64 / jaw_n as f64;
            pts.push([h, self.jaw(&st, h)]);
        }
        // Behind the skull the head sinks into the neck: the section shrinks inside the band's own.
        let bury = smoothstep(BURY_FROM, HEAD_LEN, s);
        if bury > 0.0 {
            let top = self.band.crest(st.theta_deg) - 0.12;
            for p in &mut pts {
                let h = p[0].min(top);
                let z = p[1].min((self.band.half_w(st.theta_deg, h) - 0.1).max(0.05));
                p[0] += (h - p[0]) * bury;
                p[1] += (z - p[1]) * bury;
            }
        }
        pts.dedup_by(|b, a| (b[0] - a[0]).hypot(b[1] - a[1]) < 1e-7);
        pts
    }

    /// `n + 1` points evenly spaced by arc length down the half-section.
    fn resampled(&self, s: f64, n: usize) -> Vec<[f64; 2]> {
        let dense = self.half_section(s);
        let mut acc = vec![0.0];
        for w in dense.windows(2) {
            let l = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
            acc.push(acc.last().unwrap() + l);
        }
        let total = *acc.last().unwrap();
        let mut out = Vec::with_capacity(n + 1);
        let mut j = 0;
        for k in 0..=n {
            let target = total * k as f64 / n as f64;
            while j + 2 < acc.len() && acc[j + 1] < target {
                j += 1;
            }
            let t = ((target - acc[j]) / (acc[j + 1] - acc[j]).max(1e-12)).clamp(0.0, 1.0);
            out.push([dense[j][0] + (dense[j + 1][0] - dense[j][0]) * t, dense[j][1] + (dense[j + 1][1] - dense[j][1]) * t]);
        }
        out[0][1] = 0.0;
        out[n][1] = 0.0;
        out
    }

    fn world(&self, s: f64, h: f64, z: f64) -> P3 {
        let theta = SNOUT_DEG.to_radians() + s / R_REF;
        let r = self.bore_r + h;
        [r * theta.cos(), r * theta.sin(), z]
    }

    /// The closed head: stations from the snout to the neck, each a loop of mirrored half-sections, capped at both ends.
    fn solid(&self) -> (csg::Solid, f64) {
        let n = SECTION_PTS;
        let mut pinch = f64::MAX;
        let ring = 2 * n;
        let mut v: Vec<P3> = Vec::new();
        let mut f: Vec<[u32; 3]> = Vec::new();
        let s0 = 0.03;
        for i in 0..STATIONS {
            let s = s0 + (HEAD_LEN - s0) * i as f64 / (STATIONS - 1) as f64;
            let half = self.resampled(s, n);
            pinch = half[1..n].iter().map(|p| p[1]).fold(pinch, f64::min);
            for p in &half {
                v.push(self.world(s, p[0], p[1]));
            }
            for p in half[1..n].iter().rev() {
                v.push(self.world(s, p[0], -p[1]));
            }
        }
        let at = |i: usize, j: usize| (i * ring + j % ring) as u32;
        for i in 0..STATIONS - 1 {
            for j in 0..ring {
                f.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
                f.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
            }
        }
        let tip = self.world(0.0, LIP_H - 0.07, 0.0);
        let tip_i = v.len() as u32;
        v.push(tip);
        for j in 0..ring {
            f.push([tip_i, at(0, j), at(0, j + 1)]);
        }
        let last = self.half_section(HEAD_LEN);
        let (hi, lo) = (last[0][0], last.last().unwrap()[0]);
        let back = self.world(HEAD_LEN + 0.03, 0.5 * (hi + lo), 0.0);
        let back_i = v.len() as u32;
        v.push(back);
        for j in 0..ring {
            f.push([back_i, at(STATIONS - 1, j + 1), at(STATIONS - 1, j)]);
        }
        let mut solid = csg::Solid { v, f };
        if signed_volume(&solid) < 0.0 {
            for t in &mut solid.f {
                t.swap(1, 2);
            }
        }
        (solid, pinch)
    }
}

fn signed_volume(s: &csg::Solid) -> f64 {
    s.f.iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| s.v[i as usize]);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0
        })
        .sum()
}

// --- The legs -----------------------------------------------------------------------------------------------------

/// One leg on one flank: a height field standing off the side face, built over a grid of cells round its footprint,
/// with a slab buried under the flank round the footprint so the part closes inside the band.
struct Leg<'a> {
    bones: &'a [Bone],
    root_deg: f64,
    /// +1 for the flank at +z, -1 for the flank at -z.
    side: f64,
    bore_r: f64,
    band: &'a BandShape,
}

impl Leg<'_> {
    /// Height over the flank at (s, h), and the distance outside the nearest bone's outline, mm.
    fn field(&self, p: [f64; 2]) -> (f64, f64) {
        let mut top = 0.0f64;
        let mut out = f64::MAX;
        for b in self.bones {
            let ab = [b.b[0] - b.a[0], b.b[1] - b.a[1]];
            let l2 = ab[0] * ab[0] + ab[1] * ab[1];
            let t = (((p[0] - b.a[0]) * ab[0] + (p[1] - b.a[1]) * ab[1]) / l2).clamp(0.0, 1.0);
            let q = [b.a[0] + ab[0] * t - p[0], b.a[1] + ab[1] * t - p[1]];
            let d = q[0].hypot(q[1]);
            let r = b.r.0 + (b.r.1 - b.r.0) * t;
            let h = b.h.0 + (b.h.1 - b.h.0) * t;
            out = out.min(d - r);
            if d < r {
                top = top.max((h + FLANK_MM + 0.1) * (1.0 - (d / r).powi(2)).powf(0.6));
            }
        }
        (top, out)
    }

    fn theta_deg(&self, p: [f64; 2]) -> f64 {
        self.root_deg + (p[0] / (self.bore_r + p[1])).to_degrees()
    }

    fn world(&self, p: [f64; 2], z: f64) -> P3 {
        let th = self.theta_deg(p).to_radians();
        let r = self.bore_r + p[1];
        [r * th.cos(), r * th.sin(), self.side * z]
    }

    fn solid(&self) -> csg::Solid {
        let (mut s0, mut s1, mut h0, mut h1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for b in self.bones {
            for (q, r) in [(b.a, b.r.0), (b.b, b.r.1)] {
                s0 = s0.min(q[0] - r);
                s1 = s1.max(q[0] + r);
                h0 = h0.min(q[1] - r);
                h1 = h1.max(q[1] + r);
            }
        }
        let m = LEG_MARGIN + LEG_GRID;
        let (s0, s1, h0, h1) = (s0 - m, s1 + m, (h0 - m).max(0.08), h1 + m);
        let (ni, nj) = (((s1 - s0) / LEG_GRID).ceil() as usize, ((h1 - h0) / LEG_GRID).ceil() as usize);
        let node = |i: usize, j: usize| [s0 + i as f64 * LEG_GRID, h0 + j as f64 * LEG_GRID];
        let fields: Vec<(f64, f64)> = (0..=nj).flat_map(|j| (0..=ni).map(move |i| (i, j))).map(|(i, j)| self.field(node(i, j))).collect();
        let fat = |i: usize, j: usize| fields[j * (ni + 1) + i];
        let inc: Vec<bool> = (0..nj).flat_map(|j| (0..ni).map(move |i| (i, j))).map(|(i, j)| [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)].iter().any(|&(a, b)| fat(a, b).1 < LEG_MARGIN)).collect();
        let mut inc = inc;
        // No two cells may touch only at a corner: fill one side of every such pair so the walls stay manifold.
        loop {
            let mut changed = false;
            for j in 0..nj.saturating_sub(1) {
                for i in 0..ni.saturating_sub(1) {
                    let (a, b, c, e) = (inc[j * ni + i], inc[j * ni + i + 1], inc[(j + 1) * ni + i], inc[(j + 1) * ni + i + 1]);
                    if (a && e && !b && !c) || (b && c && !a && !e) {
                        inc[j * ni + i + 1] = true;
                        inc[(j + 1) * ni + i] = true;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let cell = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < ni && (j as usize) < nj && inc[j as usize * ni + i as usize];
        let mut index = vec![u32::MAX; (ni + 1) * (nj + 1)];
        let mut v: Vec<P3> = Vec::new();
        let mut f: Vec<[u32; 3]> = Vec::new();
        for j in 0..nj {
            for i in 0..ni {
                if !inc[j * ni + i] {
                    continue;
                }
                for (a, b) in [(i, j), (i + 1, j), (i, j + 1), (i + 1, j + 1)] {
                    let k = b * (ni + 1) + a;
                    if index[k] == u32::MAX {
                        let p = node(a, b);
                        let base = self.band.half_w(self.theta_deg(p), p[1]);
                        index[k] = v.len() as u32;
                        v.push(self.world(p, base - 0.1 + fat(a, b).0));
                        v.push(self.world(p, base - 0.32));
                    }
                }
            }
        }
        let top = |a: usize, b: usize| index[b * (ni + 1) + a];
        let bot = |a: usize, b: usize| index[b * (ni + 1) + a] + 1;
        for j in 0..nj {
            for i in 0..ni {
                if !inc[j * ni + i] {
                    continue;
                }
                f.push([top(i, j), top(i + 1, j), top(i + 1, j + 1)]);
                f.push([top(i, j), top(i + 1, j + 1), top(i, j + 1)]);
                f.push([bot(i, j), bot(i + 1, j + 1), bot(i + 1, j)]);
                f.push([bot(i, j), bot(i, j + 1), bot(i + 1, j + 1)]);
                let (ii, jj) = (i as isize, j as isize);
                // Walls along every cell edge that borders an excluded cell, wound outward.
                if !cell(ii, jj - 1) {
                    f.push([top(i, j), bot(i, j), bot(i + 1, j)]);
                    f.push([top(i, j), bot(i + 1, j), top(i + 1, j)]);
                }
                if !cell(ii, jj + 1) {
                    f.push([top(i + 1, j + 1), bot(i + 1, j + 1), bot(i, j + 1)]);
                    f.push([top(i + 1, j + 1), bot(i, j + 1), top(i, j + 1)]);
                }
                if !cell(ii - 1, jj) {
                    f.push([top(i, j + 1), bot(i, j + 1), bot(i, j)]);
                    f.push([top(i, j + 1), bot(i, j), top(i, j)]);
                }
                if !cell(ii + 1, jj) {
                    f.push([top(i + 1, j), bot(i + 1, j), bot(i + 1, j + 1)]);
                    f.push([top(i + 1, j), bot(i + 1, j + 1), top(i + 1, j + 1)]);
                }
            }
        }
        let mut solid = csg::Solid { v, f };
        if signed_volume(&solid) < 0.0 {
            for t in &mut solid.f {
                t.swap(1, 2);
            }
        }
        solid
    }
}

/// What the author put down, for the report.
#[derive(Default, serde::Serialize)]
struct Composition {
    head_faces: usize,
    head_volume_mm3: f64,
    /// The narrowest a section comes between its crest and its throat, mm: a section must never pinch to the plane.
    head_least_half_width_mm: f64,
    /// The skull's length to the back of the occipital fringe against its width across the jaws, mm, and the ratio.
    head_length_mm: f64,
    head_width_mm: f64,
    head_ratio: f64,
    leg_faces: Vec<usize>,
    /// The whorls: count round the ring, the nominal cell, and the finest girdle's pitch, mm.
    whorls: Option<(u32, [f64; 2], f64)>,
}

/// Two girdles per tile. Each is a plate whose free edge bows tailward in a U with its apex on the crest, rising in a
/// rounded loaf to that edge and dropping onto the next girdle, which it overlaps. The edge is toothed into a row of
/// spiny scale points, the points of one girdle offset half a scale from the next.
fn girdle_svg(w2: f64, h: f64) -> String {
    let w = 0.5 * w2;
    let bow = WHORL_BOW * w;
    let n = 160;
    // The free edge: the U, toothed into a row of spiny scale points, offset half a scale on alternate girdles.
    let edge = |y: f64, odd: bool| {
        let f = (y / h * TEETH + if odd { 0.5 } else { 0.0 }).fract();
        w - bow * ((y - 0.5 * h) / (0.5 * h)).powi(2) - TOOTH * w * (2.0 * (f - 0.5).abs()).powf(0.8)
    };
    let mut defs = String::new();
    let mut plates = String::new();
    for (k, g) in (-1i32..=2).enumerate() {
        let dx = g as f64 * w;
        let (x1, x2) = (dx - bow, dx + w);
        let mut stops = String::new();
        for j in 0..=20 {
            let t = j as f64 / 20.0;
            let v = 0.52 + 0.48 * (t * PI * 0.5).sin().powf(0.7);
            let c = ((1.0 - v) * 255.0).round() as u8;
            stops.push_str(&format!(r##"<stop offset="{t:.3}" stop-color="rgb({c},{c},{c})"/>"##));
        }
        defs.push_str(&format!(r##"<linearGradient id="p{k}" gradientUnits="userSpaceOnUse" x1="{x1:.4}" y1="0" x2="{x2:.4}" y2="0">{stops}</linearGradient>"##));
        let (odd, prev) = (g.rem_euclid(2) == 1, g.rem_euclid(2) == 0);
        let mut pts = String::new();
        for j in 0..=n {
            let y = h * j as f64 / n as f64;
            pts.push_str(&format!("{:.4},{:.4} ", dx + edge(y, odd), y));
        }
        for j in (0..=n).rev() {
            let y = h * j as f64 / n as f64;
            pts.push_str(&format!("{:.4},{:.4} ", dx + edge(y, prev) - w, y));
        }
        plates.push_str(&format!(r##"<polygon points="{pts}" fill="url(#p{k})"/>"##));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w2:.4}mm" height="{h:.4}mm" viewBox="0 0 {w2:.4} {h:.4}"><defs>{defs}<filter id="round" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="0.07"/></filter></defs><rect width="{w2:.4}" height="{h:.4}" fill="rgb(122,122,122)"/><g filter="url(#round)">{plates}</g></svg>"##
    )
}

/// The body's hide: graded whorls girdling the back from the nape to the tail's tip, and a spine on each girdle's
/// trailing edge down both flanks.
fn hide(d: &mut RingDesign, lib: &mut AlphaLibrary, art: &Path, comp: &mut Composition) -> Result<()> {
    let ctx = d.field_context();
    let k = (WHORL_PITCH.0 / WHORL_PITCH.1).ln();
    let n = 2 * (0.5 * ctx.circumference_mm / WHORL_PITCH.0 * k.exp_m1() / k).round() as u32;
    let grade = TileGrade { taper: 1.0 - WHORL_PITCH.1 / WHORL_PITCH.0, theta_deg: WHORL_SEAM_DEG, law: GradeLaw::Spiral { seam_deg: WHORL_SEAM_DEG }, isotropic: false };
    let mut t = TilingLayer::default_for("Whorl", &ctx);
    t.repeats_around = n / 2;
    t.rows = 1;
    t.v_center_mm = ctx.crest_v_mm;
    t.v_span_mm = (ctx.band_v_len_mm - 0.6).max(1.0);
    t.feather_mm = 0.3;
    t.height_mm = WHORL_MM;
    t.grade = Some(grade);
    let (cw, ch) = t.cell_size(&ctx);
    let fine = 0.5 * cw * grade.finest_over_nominal();
    let svg = girdle_svg(cw, ch);
    std::fs::write(art.join("whorl.svg"), &svg)?;
    d.svgs.push(SvgAlpha { name: "Whorl".into(), svg, invert: false });
    let mut e = LayerEntry::new("Whorls", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window = Window::around(WHORL_WINDOW.0, WHORL_WINDOW.1);
    e.window.fade_deg = WHORL_WINDOW.2;
    let sf = ctx.side_faces_std();
    let crown = sf.and_then(|f| Some((f.low?.1, f.high?.0))).unwrap_or((0.0, ctx.band_v_len_mm));
    e.window.v_gate = VGate::Band { center_mm: 0.5 * (crown.0 + crown.1), span_mm: (crown.1 - crown.0).max(0.5), fade_mm: 0.35 };
    d.layers.layers.push(e.clone());
    // The girdles run on down the flanks, lower.
    if let Layer::Tiling(ft) = &mut e.layer {
        ft.height_mm = FLANK_MM;
    }
    e.name = "Flank whorls".into();
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    comp.whorls = Some((n, [cw, ch], fine));
    // The flank spines: the same lattice down the side faces, one blunt spine per girdle raking tailward.
    let mut sp = TilingLayer::default_for("Whorl spine", &ctx);
    if sp.fit_to_side_faces(&ctx, 0.0) {
        sp.repeats_around = n;
        sp.height_mm = SPINE_MM;
        sp.grade = Some(grade);
        let (sw, sh) = sp.cell_size(&ctx);
        let spine = reptile::svg::whorl_spine(&reptile::svg::Params::new(sw, sh, 0.25, 0.6));
        std::fs::write(art.join("whorl-spine.svg"), &spine)?;
        d.svgs.push(SvgAlpha { name: "Whorl spine".into(), svg: spine, invert: false });
        let mut se = LayerEntry::new("Whorl spines", Layer::Tiling(sp));
        se.blend = Blend::Max;
        se.window = Window::around(WHORL_WINDOW.0, WHORL_WINDOW.1);
        se.window.fade_deg = WHORL_WINDOW.2;
        se.window.v_gate = VGate::SideFaces(SideFacePick::Both);
        d.layers.layers.push(se);
    }
    d.bake_all(lib);
    Ok(())
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

fn stored_part(doc: &mut Document, name: &str, op: &str, params: serde_json::Value, solid: &csg::Solid) -> Result<()> {
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe { kernel: "sections".into(), op: op.into(), params, digest: String::new() };
    let mesh = sculpt::packed(solid).map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
    doc.append(Feature { id: next, name: name.into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh }, component: joined() })?;
    Ok(())
}

/// The made parts' names, in the order they are appended.
const PARTS: [&str; 5] = ["Head", "Foreleg, near", "Foreleg, far", "Hind leg, near", "Hind leg, far"];

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, Composition, Vec<csg::Solid>)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let mut comp = Composition::default();
    hide(&mut d, &mut lib, art, &mut comp)?;
    let shape = BandShape::of(&d)?;
    let bore_r = d.inner_radius_mm();
    let head = Head { bore_r, band: &shape };
    let (hs, pinch) = head.solid();
    comp.head_least_half_width_mm = pinch;
    comp.head_faces = hs.f.len();
    comp.head_volume_mm3 = signed_volume(&hs);
    comp.head_length_mm = SPINES.0 + SPINES.1 * SPINES.2.len() as f64;
    comp.head_width_mm = 2.0 * PLAN.iter().map(|p| p.1).fold(0.0, f64::max);
    comp.head_ratio = comp.head_length_mm / comp.head_width_mm;
    let mut solids = vec![hs];
    for (bones, root) in [(&FORELEG[..], FORE_DEG), (&HINDLEG[..], HIND_DEG)] {
        for side in [1.0, -1.0] {
            let leg = Leg { bones, root_deg: root, side, bore_r, band: &shape };
            let s = leg.solid();
            comp.leg_faces.push(s.f.len());
            solids.push(s);
        }
    }
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    stored_part(doc, PARTS[0], "lizard head", json!({"snout_deg": SNOUT_DEG, "r_ref_mm": R_REF, "length_mm": HEAD_LEN, "stations": STATIONS, "section_points": SECTION_PTS}), &solids[0])?;
    for (k, name) in PARTS[1..].iter().enumerate() {
        let root = if k < 2 { FORE_DEG } else { HIND_DEG };
        stored_part(doc, name, "lizard leg", json!({"root_deg": root, "side": if k % 2 == 0 { 1 } else { -1 }, "grid_mm": LEG_GRID}), &solids[k + 1])?;
    }
    Ok((d, lib, comp, solids))
}

// --- Gates --------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// Every made part's self-crossings, as placed.
fn part_crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
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
        .collect()
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

/// The camera for each named view: yaw about the finger axis, pitch from it toward the head.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", -0.6, 0.45),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.55, 0.55),
    ("reverse", PI - 0.5, 0.35),
];

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

/// Studio-gold renders, the 300 px read, a contact sheet, framed close-ups and the bare band against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // No stones: the stones view is the head close, framed on the whole mesh, from over the snout's side.
    let mid = SNOUT_DEG + (0.5 * HEAD_LEN / R_REF).to_degrees();
    let t = mid.to_radians();
    let centre = [12.0 * t.cos(), 12.0 * t.sin(), 0.0];
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(mid) - 0.5, 0.75, render::Framing::new(centre, 8.5), edge)?;
    render::write_png_framed(out.join("head-top.png"), &parts, render::yaw_facing(mid), PI * 0.5, render::Framing::new(centre, 8.5), edge)?;
    let fl = (FORE_DEG + 10.0).to_radians();
    render::write_png_framed(out.join("foreleg.png"), &parts, 0.0, 0.12, render::Framing::new([11.0 * fl.cos(), 11.0 * fl.sin(), 0.0], 6.5), edge)?;
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

/// The gates at one build size: geometry, parts, and the sand pull for the bonus line.
struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    stamped: usize,
    notes: Vec<String>,
    parts: Vec<(String, usize)>,
    joined: usize,
    sand_release_01: (usize, usize, f64),
}

fn release_triple(r: &mf::release::ReleaseReport) -> (usize, usize, f64) {
    (r.obstructions.len(), r.unresolved_rays, r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max))
}

fn pass(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, sand: bool) -> Result<(Pass, mesh::BuildResult)> {
    let built = mesh::try_build(d, lib, params)?;
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let sand_release_01 = if sand { release_triple(&mf::inspect(d, lib, &sand_setup(), params)?.release) } else { (0, 0, 0.0) };
    let mut notes = built.solids.notes.clone();
    notes.extend(built.parts.notes.iter().cloned());
    Ok((
        Pass { triangles: built.mesh.faces.len(), watertight, degenerate, crossings, stamped: built.solids.stamped, notes, parts: part_crossings(&built), joined: built.parts.joined, sand_release_01 },
        built,
    ))
}

impl Pass {
    fn ok(&self, stamps: usize) -> bool {
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.stamped == stamps && self.notes.is_empty() && self.parts.iter().all(|p| p.1 == 0) && self.joined == PARTS.len()
    }
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "stamped": self.stamped, "notes": self.notes,
            "made_part_crossings": self.parts, "parts_joined": self.joined})
    }
    fn line(&self) -> String {
        format!(
            "{} tris, watertight {}, degenerate {}, crossings {}, stamped {}, notes {:?}, parts {:?}, joined {}",
            self.triangles, self.watertight, self.degenerate, self.crossings, self.stamped, self.notes, self.parts, self.joined
        )
    }
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    let started = std::time::Instant::now();
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/cataphracta/art").join(SLUG);
    std::fs::create_dir_all(&art)?;
    let (d, lib, comp, solids) = author(&art)?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  head {} faces, {:.1} mm3, least half-width {:.3}, {:.1} x {:.1} mm (ratio {:.2}); legs {:?} faces",
        comp.head_faces, comp.head_volume_mm3, comp.head_least_half_width_mm, comp.head_length_mm, comp.head_width_mm, comp.head_ratio, comp.leg_faces
    );
    let params = if draft { draft_params() } else { export_params() };
    if args.iter().any(|a| a == "--probe") {
        let built = mesh::try_build(&d, &lib, params)?;
        let (w, g, x) = geometry(&built.mesh);
        println!("  {} tris, watertight {w}, degenerate {g}, crossings {x}, notes {:?} {:?}, joined {}", built.mesh.faces.len(), built.solids.notes, built.parts.notes, built.parts.joined);
        renders(&out, &lib, &built, 1000)?;
        return Ok(());
    }
    let t = std::time::Instant::now();
    let (main_pass, built) = pass(&d, &lib, params, true)?;
    let build_s = t.elapsed().as_secs_f64();
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, main_pass.line());
    let coarse_pass = if std::env::var("OURO_QUICK").is_ok() { None } else { Some(pass(&d, &lib, coarse_params(), false)?.0) };
    if let Some(c) = &coarse_pass {
        println!("  384x192: {}", c.line());
    }
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let made: Vec<(usize, usize)> = solids.iter().map(|s| (sculpt::closure(s).0, csg::self_crossings(s))).collect();
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let setup = d.manufacturing.clone().unwrap();
    let prepared = mf::prepare(&d, &lib, &setup, params)?;
    let (pw, pd, px) = geometry(&prepared.mesh);
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
    let stamps = d.stamps.len();
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", main_pass.watertight && main_pass.degenerate == 0 && main_pass.crossings == 0),
        ("every made part closed and uncrossed, as made and as placed", made.iter().all(|&(o, x)| o == 0 && x == 0) && main_pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, every stamp resolved, all five parts joined", main_pass.notes.is_empty() && main_pass.stamped == stamps && main_pass.joined == PARTS.len()),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax verdict Castable, the parts judged in, thinnest wall at or above the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("gates hold at 384 x 192", coarse_pass.as_ref().is_none_or(|c| c.ok(stamps))),
        ("investment pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within 2 million triangles", main_pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "stage": std::env::var("OURO_STAGE").unwrap_or_else(|_| "block-out".into()),
        "process": d.draft.process.label(),
        "process_note": "Lost wax, per Logan's rule of 2026-10-03: 0.8 mm minimum section, no pull rule. The sand pull is recorded below as a bonus only.",
        "draft": {"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.as_ref().map(|c| c.json()),
        "made_parts": PARTS.iter().zip(&made).zip(&solids).map(|((n, (o, x)), s)| json!({"name": n, "open_edges": o, "self_crossings": x, "faces": s.f.len(), "volume_mm3": signed_volume(s)})).collect::<Vec<_>>(),
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "process": field.process.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "undercut_percent": field.undercut_fraction() * 100.0, "notes": field.notes,
            "parts": field.parts.iter().map(|p| json!({"name": p.name, "total_mm2": p.total_area_mm2, "note": p.note})).collect::<Vec<_>>()},
        "release": {"applies": false, "note": "No pull rule in lost wax.", "bonus_petrobond_0100": {"obstructions": main_pass.sand_release_01.0, "unresolved": main_pass.sand_release_01.1, "deepest_mm": main_pass.sand_release_01.2}},
        "draft_clamp": {"groups": 0, "note": "lost wax: no clamp"},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len()},
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
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Ouroborus / investment pattern")?;
    }
    renders(&out, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} (thinnest {:.2} mm); dfm {}; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}; sand bonus {:?}",
        field.verdict.label(),
        field.thinnest_wall_mm,
        findings.len(),
        d.inner_radius_mm(),
        main_pass.sand_release_01
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
    ensure!(!text.is_empty());
    Ok(())
}
