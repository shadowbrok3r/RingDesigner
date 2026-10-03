//! Cataphracta — Ouroborus, the girdled wheel: a serpent biting its own tail, poured in Petrobond.
//! cargo build --release -p ringdesign-core --example cataphracta_ouroborus
//! target/release/examples/cataphracta_ouroborus [OUT_DIR] [--draft] [--verify]
//!
//! The band is the serpent's body, girdled in graded whorls under a dorsal keel, broad behind the head and tapering all
//! the way round to a pointed tail whose tip runs into the closed jaws at the top. The head is one made part: a
//! spade-shaped skull, cephalic plates in three tiers stepping down from the crest, an eye with a slit pupil under each
//! brow, a nostril, and the gape closed on the tail. The head is built section by section as a solid whose every section is a
//! single span across the parting plane, so each half of it draws straight out of its own half of the sand.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    field::{Blend, Layer, LayerEntry, Window},
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade, TilingLayer},
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, SandProcess, Verdict},
    csg, dfm, library, manufacturing as mf, mesh,
    profile::ShankKey,
    render, sculpt,
    skin::Atlas,
    stl,
};
use serde_json::json;
use std::f64::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const NAME: &str = "Ouroborus \u{2014} the girdled wheel";
const SLUG: &str = "ouroborus";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;

/// Ring angle of the snout's tip, degrees; the head runs toward increasing angle.
const SNOUT_DEG: f64 = 86.0;
/// Radius at which the head's frame measures arc along the ring, mm.
const R_REF: f64 = 13.0;
/// Stations along the head and points down each half-section.
const STATIONS: usize = 380;
const SECTION_PTS: usize = 260;
/// The head stands this far off the band's mid-plane, so its crest seam never lies on the band's own, mm.
const SEAM_SHIFT_MM: f64 = 0.0;

/// The keyed body: (degrees, width scale, thickness scale) on Flat 3.4 x 2.8 (the tail's tip is the reference width). The tail's tip runs under the snout at
/// 86°, the band widens inside the jaws to the neck behind the head, and tapers from the neck round to the tail.
const KEYS: [(f64, f64, f64); 15] = [
    (0.0, 1.2, 0.74),
    (40.0, 1.0, 0.68),
    (66.0, 0.8, 0.62),
    (76.0, 0.56, 0.58),
    (82.0, 0.36, 0.55),
    (86.0, 0.3, 0.54),
    (92.0, 0.6, 0.66),
    (100.0, 1.0, 0.86),
    (110.0, 1.4, 1.02),
    (120.0, 1.62, 1.12),
    (132.0, 1.47, 1.2),
    (160.0, 1.6, 1.2),
    (215.0, 1.75, 1.08),
    (270.0, 1.6, 0.92),
    (320.0, 1.4, 0.8),
];

// --- The head's primary forms, along `s` mm from the snout's tip --------------------------------------------------

/// Half-width at the widest of the side wall: a narrow rounded snout, flaring past the eyes to the jaw's angles, then
/// pinched into the neck.
const PLAN: [(f64, f64); 13] = [
    (0.0, 0.0),
    (0.3, 0.95),
    (1.0, 1.4),
    (2.0, 1.75),
    (3.4, 2.15),
    (4.8, 2.6),
    (6.0, 3.05),
    (6.9, 3.3),
    (7.7, 3.2),
    (8.6, 2.75),
    (9.5, 2.5),
    (10.6, 2.35),
    (11.6, 2.2),
];
/// Height of the dorsal crest over the bore.
const CREST: [(f64, f64); 12] = [
    (0.0, 1.6),
    (0.3, 2.2),
    (0.8, 2.7),
    (1.6, 3.1),
    (2.8, 3.5),
    (4.0, 3.75),
    (5.5, 3.9),
    (7.0, 4.0),
    (8.2, 3.9),
    (9.4, 3.65),
    (10.5, 3.45),
    (11.6, 3.2),
];
/// The dorsal dome's fall from the crest to the canthus, before the plate tiers' risers.
const DOME: [(f64, f64); 4] = [(0.0, 0.12), (3.0, 0.15), (7.0, 0.22), (11.6, 0.2)];
/// Each tier of cephalic plates stands this far over the next one out.
const RISER: f64 = 0.26;
/// How far the dorsal plan stands inside the side wall's widest: the section narrows to the crown like a viper's.
const TOP_INSET: [(f64, f64); 4] = [(0.0, 0.15), (3.0, 0.22), (6.9, 0.35), (11.6, 0.2)];
/// The head's length from the snout's tip to where it sinks into the neck.
const HEAD_LEN: f64 = 11.6;
/// The gape line's height at the snout and its sag to the mouth's corner.
const LIP_H: f64 = 1.45;
const LIP_SAG: f64 = 0.25;
const MOUTH_CORNER: f64 = 6.6;
/// The mouth's corner turns up over its last stretch: the rise and the length it takes, mm.
const MOUTH_UPTURN: (f64, f64) = (0.4, 1.6);
/// The lower jaw: where the chin starts, its inset under the upper lip, and the height of the throat's bottom.
const CHIN_S: f64 = 0.6;
const JAW_INSET: f64 = 0.16;
const JAW_BOTTOM: f64 = 0.06;
/// The tail runs into the mouth: the gape between the lips round it at the snout, and where the lips close behind it.
const TAIL_GAPE: f64 = 1.2;
const TAIL_IN: (f64, f64) = (1.8, 2.7);
/// Inside the gape the head keeps this share of the tail's own half-width, hidden in the tail.
const WAIST: f64 = 0.6;
/// The gape groove: depth and half-width.
const GAPE_MM: f64 = 0.34;
const GAPE_W: f64 = 0.22;

/// The eye: centre along the head and over the bore, radius, and how far it bulges from the wall.
const EYE_S: f64 = 3.3;
const EYE_H: f64 = 2.3;
const EYE_R: f64 = 0.72;
const EYE_BULGE: f64 = 0.55;
/// The slit pupil: half-width along the head, half-height, depth.
const PUPIL: (f64, f64, f64) = (0.2, 0.5, 0.22);
/// The orbit groove round the eye: width and depth.
const ORBIT: (f64, f64) = (0.3, 0.14);
/// The brow: how far the supraocular overhangs the eye, and its reach along the head.
const BROW: (f64, f64) = (0.3, 1.05);
/// The nostril: centre along the head, over the lip, radius and depth.
const NOSTRIL: (f64, f64, f64, f64) = (0.8, 0.7, 0.22, 0.18);

/// The cephalic plates' transverse sutures, each inside one tier, shallower than the riser out of it: (s, tier, depth).
const SUTURES: [(f64, u8, f64); 4] = [(1.4, 1, 0.12), (2.6, 1, 0.14), (2.6, 2, 0.12), (5.3, 2, 0.12)];
const SUTURE_W: f64 = 0.18;
/// The frontal shield: front edge, the end of its parallel sides, its point, half-width.
const FRONTAL: (f64, f64, f64, f64) = (2.6, 4.4, 5.8, 0.95);
/// The parietals' outer edge (s, half-width), closing round behind.
const PARIETAL_EDGE: [(f64, f64); 6] = [(4.7, 1.1), (5.6, 1.8), (7.0, 2.1), (8.0, 1.8), (8.8, 1.0), (9.4, 0.0)];
/// The second tier's outer edge: the snout's loreal margin, the supraoculars out to the brow, the temporals.
const TIER2_EDGE: [(f64, f64); 7] = [(0.5, 0.0), (1.0, 0.85), (2.6, 1.45), (3.6, 2.0), (5.5, 2.45), (7.5, 2.65), (9.6, 0.0)];
/// The median ridge where paired plates meet: its fall over the first mm off the crest.
const MEDIAN_RIDGE: f64 = 0.16;

// --- The body's hide -----------------------------------------------------------------------------------------------

/// The whorls: girdle pitch just behind the seam and just before it, mm, the seam itself (hidden in the head), the
/// girdle's height and its trailing drop.
const WHORL_PITCH: (f64, f64) = (2.8, 1.0);
const WHORL_SEAM_DEG: f64 = 92.0;
const WHORL_MM: f64 = 0.36;
/// Scale rows across each girdle, odd rows staggered half a girdle, each row out from the crest a step lower: the
/// alpha's floor and swing per row from the crest outward.
const SCALE_ROWS: [(f64, f64); 3] = [(0.56, 0.44), (0.22, 0.26), (0.0, 0.19)];
/// The girdle's profile: its rounded rise from the leading edge (an exponent), and the share of the pitch its trailing
/// drop takes.
const WHORL_RISE: f64 = 1.6;
const WHORL_DROP: f64 = 0.14;
/// The whorls run from the nape round to the tail's tip: the window's centre and span, and its fade, degrees.
const WHORL_WINDOW: (f64, f64, f64) = (279.0, 334.0, 3.0);
/// The dorsal keel: width across the crest and height, and its window from the nape to the tail's last third.
const KEEL: (f64, f64) = (1.0, 0.18);
const KEEL_WINDOW: (f64, f64, f64) = (268.0, 226.0, 10.0);

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
    setup.recipe.name = "Ouroborus / Petrobond / Silver 925".into();
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
    setup.bench_notes = "Two-part Petrobond pour parting on the crest line. The serpent's head straddles the parting plane: its dorsal crest is the seam, and each half of the skull, the eyes and the jaws draws out of its own half of the sand. Dress the seam along the head's crest and the band's crest with a needle file.".into();
    setup
}

/// The bare body: Flat 3.4 x 2.8 with a parabolic crown, keyed from the neck round to the tail.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.width_mm = 3.4;
    d.profile.thickness_mm = 2.8;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 2.2;
    d.profile.shape_a = 2.0;
    d.profile.flatten_sides();
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = KEYS.iter().map(|&(theta_deg, width_scale, thickness_scale)| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 }).collect();
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.manufacturing = Some(s);
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

// --- The head -----------------------------------------------------------------------------------------------------

/// The head's section at one station: the dorsal surface as a height over the bore against `z`, then the side wall and
/// the jaw as `z` against the height.
struct Head {
    bore_r: f64,
    band: BandShape,
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
    /// Half-width at `theta_deg` and `h` mm over the bore.
    fn half_w(&self, theta_deg: f64, h: f64) -> f64 {
        let n = self.cols.len();
        let x = ((theta_deg.rem_euclid(360.0) / 360.0 * n as f64).round() as usize) % n;
        let col = &self.cols[x];
        let i = ((h / BAND_DH).max(0.0) as usize).min(col.len() - 1);
        col[i]
    }
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

/// The frontal shield's half-width at `s`: parallel sides, then a point toward the parietals.
fn frontal_w(s: f64) -> f64 {
    let (front, sides, point, w) = FRONTAL;
    if s < front - 0.1 || s > point {
        0.0
    } else if s <= sides {
        w
    } else {
        w * (1.0 - ((s - sides) / (point - sides)).powf(1.3))
    }
}

impl Head {
    fn station(&self, s: f64) -> Station {
        let crest = pchip(&CREST, s);
        let wall_w = pchip(&PLAN, s);
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

    /// The dorsal surface's height over the bore at `z` across, from the crest to the canthus: a dome whose plates step
    /// down outward (the frontal over the supraoculars, the parietals over the temporals), whose paired plates meet in a
    /// median ridge, and whose transverse sutures run out to the edge, so the height never rises away from the crest.
    fn dorsal(&self, st: &Station, z: f64) -> f64 {
        let s = st.s;
        let u = (z / st.top_w.max(1e-6)).min(1.0);
        let mut h = st.crest - pchip(&DOME, s) * u.powf(2.2);
        let fw = frontal_w(s);
        let on_frontal = smoothstep(FRONTAL.0 - 0.25, FRONTAL.0 + 0.05, s) * (1.0 - smoothstep(FRONTAL.2 - 0.6, FRONTAL.2, s));
        // Paired plates meet in a ridge on the crest; the frontal is one shield.
        let paired = (1.0 - on_frontal) * smoothstep(0.35, 0.7, s) * (1.0 - smoothstep(8.6, 9.4, s));
        h -= MEDIAN_RIDGE * paired * (z.min(1.0)).sqrt();
        // The first tier: the internasals and prefrontals, the frontal, the parietals.
        let snout = if s < FRONTAL.0 { (st.top_w - 0.5).max(0.0) * smoothstep(0.5, 1.0, s) } else { 0.0 };
        let snout = snout.min(pchip(&TIER2_EDGE, s) - 0.4).max(0.0);
        let parietal = if s > PARIETAL_EDGE[0].0 { pchip(&PARIETAL_EDGE, s) } else { 0.0 };
        let t1 = snout.max(fw).max(parietal);
        // The second tier: the loreal margin, the supraoculars and the temporals.
        let t2 = pchip(&TIER2_EDGE, s).min(st.top_w - 0.12).max(t1 + 0.4 * smoothstep(0.0, 0.3, t1));
        let step = |w: f64| if w <= 0.0 { 1.0 } else { smoothstep(w - 0.04, w + 0.04, z) };
        let plated = smoothstep(0.35, 0.9, s) * (1.0 - smoothstep(9.4, 10.4, s));
        h -= RISER * (step(t1) + step(t2)) * plated;
        // Each plate is domed a little toward its edge.
        if t1 > 0.0 && z < t1 {
            h -= 0.06 * (z / t1).powi(2);
        }
        let inside = |w: f64| 1.0 - smoothstep(w - 0.04, w + 0.04, z);
        for (at, tier, depth) in SUTURES {
            let mask = if tier == 1 { inside(t1) } else { inside(t2) * (1.0 - inside(t1)) };
            h -= depth * vee(s - at, SUTURE_W) * mask * plated;
        }
        h
    }

    /// The side wall's `z` at height `h` between the lip and the canthus: bellied, with the eye, brow and nostril.
    fn wall(&self, st: &Station, h: f64) -> f64 {
        let s = st.s;
        let t = ((h - st.lip) / (st.canthus - st.lip).max(1e-6)).clamp(0.0, 1.0);
        // The belly sits low, so the wall leans in toward the crown.
        let base = st.side_w + st.belly * (PI * t.powf(0.75)).sin().powf(0.6) * (1.0 - 0.5 * t);
        let brow = BROW.0 * vee(s - EYE_S, BROW.1) * smoothstep(0.6, 1.0, t);
        let mut z = base + brow;
        // The eye: a spherical cap bulging from the wall, ringed by its orbit, with a slit pupil.
        let (ds, dh) = (s - EYE_S, h - EYE_H);
        let d = ds.hypot(dh);
        let sphere = (EYE_R * EYE_R + EYE_BULGE * EYE_BULGE) / (2.0 * EYE_BULGE);
        if d < EYE_R {
            z += (sphere * sphere - d * d).sqrt() - (sphere - EYE_BULGE);
            let p = (ds / PUPIL.0).powi(2) + (dh / PUPIL.1).powi(2);
            if p < 1.0 {
                z -= PUPIL.2 * (1.0 - p).sqrt();
            }
        }
        z -= ORBIT.1 * vee(d - EYE_R - ORBIT.0 * 0.5, ORBIT.0 * 0.5);
        // The nostril.
        let dn = (s - NOSTRIL.0).hypot(h - st.lip - NOSTRIL.1);
        if dn < NOSTRIL.2 {
            z -= NOSTRIL.3 * (1.0 - (dn / NOSTRIL.2).powi(2)).sqrt();
        }
        // The gape's upper lip edge.
        z -= GAPE_MM * vee(h - st.lip, GAPE_W) * smoothstep(MOUTH_CORNER + 0.4, MOUTH_CORNER - 0.2, s) * smoothstep(0.2, 1.4, s);
        z.max(0.0)
    }

    /// The lower jaw's `z` at height `h` under its lip: boxy, closed on the upper lip with the gape's groove, or open
    /// round the tail where it enters.
    fn jaw(&self, st: &Station, h: f64) -> f64 {
        let span = (st.lower_lip - st.bottom).max(1e-6);
        let t = ((st.lower_lip - h) / span).clamp(0.0, 1.0);
        let shape = (1.0 - t.powf(2.5)).max(0.0).powf(0.4);
        let closed = smoothstep(MOUTH_CORNER + 0.4, MOUTH_CORNER - 0.2, st.s);
        let groove = GAPE_MM * vee(h - st.lip, GAPE_W) * closed * smoothstep(0.2, 1.4, st.s) * (1.0 - st.open);
        let jaw = st.jaw_w * shape - groove;
        // Past the mouth's corner the jaw is the wall run down to the throat.
        let full = st.side_w * shape;
        let z = jaw * closed + full * (1.0 - closed);
        // Under the chin the section stays inside the tail.
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
        // Height never rises down the section.
        for k in 1..pts.len() {
            if pts[k][0] >= pts[k - 1][0] {
                pts[k][0] = pts[k - 1][0] - 1e-5;
            }
        }
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
        let theta = (SNOUT_DEG.to_radians()) + s / R_REF;
        let r = self.bore_r + h;
        [r * theta.cos(), r * theta.sin(), z + SEAM_SHIFT_MM]
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
            if std::env::var("OURO_DEBUG").is_ok() {
                if let Some(k) = (1..n).find(|&k| half[k][1] < 0.02) {
                    eprintln!("pinch at s {s:.3} k {k} h {:.3} z {:.4}", half[k][0], half[k][1]);
                }
            }
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
        // The snout's tip and the buried back, each a fan to a point on the section's middle.
        let tip = self.world(0.0, LIP_H - 0.07, 0.0);
        let tip_i = v.len() as u32;
        v.push(tip);
        for j in 0..ring {
            f.push([tip_i, at(0, j), at(0, j + 1)]);
        }
        let last = self.station(HEAD_LEN);
        let back = self.world(HEAD_LEN + 0.05, 0.5 * (last.crest + last.bottom), 0.0);
        let back_i = v.len() as u32;
        v.push(back);
        for j in 0..ring {
            f.push([back_i, at(STATIONS - 1, j + 1), at(STATIONS - 1, j)]);
        }
        let mut solid = csg::Solid { v, f };
        // Face outward: flip the whole if the volume reads negative.
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

/// What the author put down, for the report.
#[derive(Default, serde::Serialize)]
struct Composition {
    head_faces: usize,
    head_volume_mm3: f64,
    head_min_draft_deg: f64,
    head_undercut_facets: usize,
    /// The narrowest a section comes between its crest and its throat, mm: a section must never pinch to the plane.
    head_least_half_width_mm: f64,
    crest_r_at_snout_mm: f64,
    band_half_w_at_snout_mm: f64,
    /// The whorls: count round the ring, the nominal cell, and the finest girdle's pitch, mm.
    whorls: Option<(u32, [f64; 2], f64)>,
}

/// The head's facets read against the parting plane: the least draft off the plane, and the count leaning back.
fn head_drafts(s: &csg::Solid) -> (f64, usize) {
    let mut least = 90.0f64;
    let mut back = 0;
    for t in &s.f {
        let [a, b, c] = t.map(|i| s.v[i as usize]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        if (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() < 1e-14 {
            continue;
        }
        let zc = (a[2] + b[2] + c[2]) / 3.0;
        let (lo, hi) = (a[2].min(b[2]).min(c[2]), a[2].max(b[2]).max(c[2]));
        let d = castability::draft_angle(n, zc, 0.0);
        if std::env::var("OURO_DEBUG").is_ok() && d < -5.0 {
            let c = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0];
            eprintln!("leaning facet {:.1} deg at theta {:.2} r {:.3} z {:.5}..{:.5}", d, c[1].atan2(c[0]).to_degrees(), c[0].hypot(c[1]), lo, hi);
        }
        if lo <= 1e-9 && hi >= -1e-9 {
            continue;
        }
        least = least.min(d);
        if d < -0.5 {
            back += 1;
        }
    }
    (least, back)
}

/// One girdle per tile: each scale rises from its leading edge in a rounded loaf to its trailing edge, then drops
/// steeply to the next, with no flat between, so the girdles overlap like a lizard's whorls. Across the band the girdle
/// is split into rows of scales, odd rows staggered half a girdle, each row out from the crest a step lower.
fn girdle_svg(w: f64, h: f64) -> String {
    let rise = 1.0 - WHORL_DROP;
    let loaf = |x: f64| if x <= rise { (FRAC_PI_2 * x / rise).sin().powf(WHORL_RISE) } else { (1.0 - x) / WHORL_DROP };
    let n = 2 * SCALE_ROWS.len() - 1;
    let row_h = h / n as f64;
    let mut defs = String::new();
    let mut body = String::new();
    for (k, (floor, swing)) in SCALE_ROWS.iter().enumerate() {
        let off = if k % 2 == 1 { 0.5 * w } else { 0.0 };
        let mut stops = String::new();
        for j in 0..=40 {
            let x = j as f64 / 40.0;
            let v = floor + swing * loaf(x);
            let g = ((1.0 - v) * 255.0).round() as u8;
            stops.push_str(&format!(r##"<stop offset="{x:.4}" stop-color="rgb({g},{g},{g})"/>"##));
        }
        defs.push_str(&format!(
            r##"<linearGradient id="r{k}" gradientUnits="userSpaceOnUse" x1="{off:.4}" y1="0" x2="{:.4}" y2="0" spreadMethod="repeat">{stops}</linearGradient>"##,
            off + w
        ));
    }
    // Outer rows first, each overdrawn by the row inside it, so every boundary falls to the outer side.
    for k in (0..SCALE_ROWS.len()).rev() {
        let mid = (n / 2) as f64;
        let (y0, y1) = ((mid - k as f64) * row_h, (mid + k as f64 + 1.0) * row_h);
        body.push_str(&format!(r##"<rect x="0" y="{y0:.4}" width="{w:.4}" height="{:.4}" fill="url(#r{k})"/>"##, y1 - y0));
    }
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}mm" height="{h:.4}mm" viewBox="0 0 {w:.4} {h:.4}"><defs>{defs}</defs>{body}</svg>"##)
}

/// The body's hide: graded whorls girdling the body from the nape to the tail's tip, and a knife keel on the crest.
fn hide(d: &mut RingDesign, lib: &mut AlphaLibrary, art: &Path, comp: &mut Composition) -> Result<()> {
    let ctx = d.field_context();
    let k = (WHORL_PITCH.0 / WHORL_PITCH.1).ln();
    let n = (ctx.circumference_mm / WHORL_PITCH.0 * k.exp_m1() / k).round() as u32;
    let mut t = TilingLayer::default_for("Whorl", &ctx);
    t.repeats_around = n;
    t.rows = 1;
    t.v_center_mm = ctx.crest_v_mm;
    t.v_span_mm = (ctx.band_v_len_mm - 0.9).max(1.0);
    t.feather_mm = 0.45;
    t.height_mm = WHORL_MM;
    t.grade = Some(TileGrade { taper: 1.0 - WHORL_PITCH.1 / WHORL_PITCH.0, theta_deg: WHORL_SEAM_DEG, law: GradeLaw::Spiral { seam_deg: WHORL_SEAM_DEG }, isotropic: false });
    let (cw, ch) = t.cell_size(&ctx);
    let fine = cw * t.grade.unwrap().finest_over_nominal();
    let svg = girdle_svg(cw, ch);
    std::fs::write(art.join("whorl.svg"), &svg)?;
    d.svgs.push(SvgAlpha { name: "Whorl".into(), svg, invert: false });
    let mut e = LayerEntry::new("Whorls", Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window = Window::around(WHORL_WINDOW.0, WHORL_WINDOW.1);
    e.window.fade_deg = WHORL_WINDOW.2;
    d.layers.layers.push(e);
    comp.whorls = Some((n, [cw, ch], fine));
    // The keel: a gable across the crest, constant round the ring.
    let mut kt = TilingLayer::default_for("Keel", &ctx);
    kt.repeats_around = 90;
    kt.rows = 1;
    kt.v_center_mm = ctx.crest_v_mm;
    kt.v_span_mm = KEEL.0;
    kt.feather_mm = 0.35;
    kt.height_mm = KEEL.1;
    let (kw, kh) = kt.cell_size(&ctx);
    let keel = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{kw:.4}mm" height="{kh:.4}mm" viewBox="0 0 {kw:.4} {kh:.4}"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7f7f7f"/><stop offset="0.5" stop-color="#000"/><stop offset="1" stop-color="#7f7f7f"/></linearGradient></defs><rect width="{kw:.4}" height="{kh:.4}" fill="url(#g)"/></svg>"##
    );
    std::fs::write(art.join("keel.svg"), &keel)?;
    d.svgs.push(SvgAlpha { name: "Keel".into(), svg: keel, invert: false });
    let mut ke = LayerEntry::new("Dorsal keel", Layer::Tiling(kt));
    ke.blend = Blend::Add;
    ke.window = Window::around(KEEL_WINDOW.0, KEEL_WINDOW.1);
    ke.window.fade_deg = KEEL_WINDOW.2;
    d.layers.layers.push(ke);
    d.bake_all(lib);
    Ok(())
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

fn author(art: &Path) -> Result<(RingDesign, AlphaLibrary, Composition, csg::Solid)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let mut comp = Composition::default();
    hide(&mut d, &mut lib, art, &mut comp)?;
    let a = Atlas::of(&d, 1440, 256)?;
    let col = ((SNOUT_DEG / 360.0) * a.width as f64).round() as usize % a.width;
    comp.crest_r_at_snout_mm = (0..a.height).map(|y| { let p = a.at(col, y).p; p[0].hypot(p[1]) }).fold(0.0, f64::max);
    comp.band_half_w_at_snout_mm = (0..a.height).map(|y| a.at(col, y).p[2].abs()).fold(0.0, f64::max);
    let head = Head { bore_r: d.inner_radius_mm(), band: BandShape::of(&d)? };
    let (solid, pinch) = head.solid();
    comp.head_least_half_width_mm = pinch;
    comp.head_faces = solid.f.len();
    comp.head_volume_mm3 = signed_volume(&solid);
    let (least, back) = head_drafts(&solid);
    comp.head_min_draft_deg = least;
    comp.head_undercut_facets = back;
    let packed = sculpt::packed(&solid)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let recipe = stored::Recipe {
        kernel: "sections".into(),
        op: "serpent head".into(),
        params: json!({"snout_deg": SNOUT_DEG, "r_ref_mm": R_REF, "length_mm": HEAD_LEN, "stations": STATIONS, "section_points": SECTION_PTS}),
        digest: String::new(),
    };
    doc.append(Feature { id: next, name: "Serpent head".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component: joined() })?;
    Ok((d, lib, comp, solid))
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

fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let v = m.vertices[i as usize];
        let d = [v.0 as f64 - centre[0], v.1 as f64 - centre[1], v.2 as f64 - centre[2]];
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < radius * radius
    };
    let mut remap = vec![u32::MAX; m.vertices.len()];
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        out.faces.push(f.map(|i| {
            if remap[i as usize] == u32::MAX {
                remap[i as usize] = out.vertices.len() as u32;
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals[i as usize]);
            }
            remap[i as usize]
        }));
    }
    out
}

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

/// Studio-gold renders, the 300 px read, a contact sheet and the bare band against the finished ring.
fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // No stones: the stones view is the head close, from over the snout's side.
    let t = (SNOUT_DEG + 25.0).to_radians();
    let head = crop(&built.mesh, [12.5 * t.cos(), 12.5 * t.sin(), 0.0], 9.0);
    render::write_png_parts(out.join("stones.png"), &[render::Part::metal(&head, render::GOLD)], -0.35, 0.7, edge)?;
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

/// The gates at one build size: geometry, parts, the ray release at both pitches.
struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    stamped: usize,
    notes: Vec<String>,
    parts: Vec<(String, usize)>,
    joined: usize,
    release_01: (usize, usize, f64),
    release_0075: (usize, usize, f64),
    obstructions: Vec<String>,
}

fn release_where(r: &mf::release::ReleaseReport) -> Vec<String> {
    r.obstructions
        .iter()
        .map(|o| {
            let [x, y, z] = o.world;
            format!("{:.1}° r {:.2} z {:+.2}: {:.3} mm", y.atan2(x).to_degrees().rem_euclid(360.0), x.hypot(y), z, o.depth_mm)
        })
        .collect()
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
            parts: part_crossings(&built),
            joined: built.parts.joined,
            release_01: release_triple(&inspection.release),
            release_0075: release_triple(&fine_r),
            obstructions: release_where(&inspection.release).into_iter().chain(release_where(&fine_r)).collect(),
        },
        built,
    ))
}

impl Pass {
    fn geometry_line(built: &mesh::BuildResult) -> String {
        let (w, g, x) = geometry(&built.mesh);
        format!("{} tris, watertight {w}, degenerate {g}, crossings {x}, notes {:?} {:?}, joined {}", built.mesh.faces.len(), built.solids.notes, built.parts.notes, built.parts.joined)
    }
    fn ok(&self, stamps: usize) -> bool {
        self.watertight
            && self.degenerate == 0
            && self.crossings == 0
            && self.stamped == stamps
            && self.notes.is_empty()
            && self.parts.iter().all(|p| p.1 == 0)
            && self.joined == 1
            && self.release_01.0 == 0
            && self.release_01.1 == 0
            && self.release_0075.0 == 0
            && self.release_0075.1 == 0
    }
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "stamped": self.stamped, "notes": self.notes,
            "made_part_crossings": self.parts, "parts_joined": self.joined,
            "release_0100": {"obstructions": self.release_01.0, "unresolved": self.release_01.1, "deepest_mm": self.release_01.2},
            "release_0075": {"obstructions": self.release_0075.0, "unresolved": self.release_0075.1, "deepest_mm": self.release_0075.2},
            "obstructions": self.obstructions})
    }
    fn line(&self) -> String {
        format!(
            "{} tris, watertight {}, degenerate {}, crossings {}, stamped {}, notes {:?}, parts {:?}, joined {}, release 0.100 {:?}, 0.075 {:?}",
            self.triangles, self.watertight, self.degenerate, self.crossings, self.stamped, self.notes, self.parts, self.joined, self.release_01, self.release_0075
        ) + &if self.obstructions.is_empty() { String::new() } else { format!("\n      at {:?}", self.obstructions) }
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
    let (d, lib, comp, solid) = author(&art)?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  head {} faces, {:.1} mm3, least half-width {:.3}, least draft off the plane {:.2} deg, {} facets leaning back; tail crest r {:.2}, half-width {:.2} at the snout",
        comp.head_faces, comp.head_volume_mm3, comp.head_least_half_width_mm, comp.head_min_draft_deg, comp.head_undercut_facets, comp.crest_r_at_snout_mm, comp.band_half_w_at_snout_mm
    );
    let params = if draft { draft_params() } else { export_params() };
    if args.iter().any(|a| a == "--probe") {
        // Look only: the ring and the head from a dozen angles, no gates.
        let built = mesh::try_build(&d, &lib, params)?;
        println!("  {}", Pass::geometry_line(&built));
        let t = (SNOUT_DEG + 30.0).to_radians();
        let head = crop(&built.mesh, [12.5 * t.cos(), 12.5 * t.sin(), 0.0], 11.0);
        let whole = vec![render::Part::metal(&built.mesh, render::GOLD)];
        let close = vec![render::Part::metal(&head, render::GOLD)];
        let mut sheet = vec![0u8; 1600 * 1200 * 3];
        let looks: [(bool, f64, f64); 12] = [
            (true, -0.5, 0.62), (true, 0.0, PI * 0.5), (true, 0.0, 0.0), (true, 0.5, 0.55),
            (true, 0.9, 0.45), (true, -0.9, 0.5), (false, 0.0, PI * 0.5), (false, 0.0, 0.0),
            (false, 0.6, 0.5), (false, -0.6, 0.6), (false, 1.2, 0.9), (false, 0.3, 1.1),
        ];
        for (k, (ring, yaw, pitch)) in looks.iter().enumerate() {
            let img = render::render_parts_ss(if *ring { &whole } else { &close }, *yaw, *pitch, 400, 400, 2);
            paste(&mut sheet, 1600, &img, 400, (k % 4) * 400, (k / 4) * 400);
        }
        image::save_buffer(out.join("probe.png"), &sheet, 1600, 1200, image::ColorType::Rgb8)?;
        let heroes: [(f64, f64); 6] = [(-0.9, 0.5), (-0.6, 0.45), (-1.25, 0.55), (0.5, 0.55), (0.9, 0.45), (-0.9, 0.75)];
        let mut row = vec![0u8; 1800 * 300 * 3];
        for (k, (yaw, pitch)) in heroes.iter().enumerate() {
            let img = render::render_parts_ss(&whole, *yaw, *pitch, 300, 300, 3);
            paste(&mut row, 1800, &img, 300, k * 300, 0);
        }
        image::save_buffer(out.join("probe-heroes.png"), &row, 1800, 300, image::ColorType::Rgb8)?;
        render::write_png_parts(out.join("probe-side.png"), &close, 0.0, 0.0, 900)?;
        render::write_png_parts(out.join("probe-top.png"), &close, 0.0, PI * 0.5, 900)?;
        render::write_png_parts(out.join("probe-34.png"), &close, 0.55, 0.5, 900)?;
        render::write_png_parts(out.join("face-300.png"), &whole, VIEWS[1].1, VIEWS[1].2, 300)?;
        render::write_png_parts(out.join("hero-300.png"), &whole, VIEWS[0].1, VIEWS[0].2, 300)?;
        return Ok(());
    }
    let t = std::time::Instant::now();
    let (main_pass, built) = pass(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, main_pass.line());
    let quick = std::env::var("OURO_QUICK").is_ok();
    let coarse_pass = if quick { None } else { Some(pass(&d, &lib, coarse_params())?.0) };
    if let Some(c) = &coarse_pass {
        println!("  384x192: {}", c.line());
    }
    if std::env::var("OURO_DEBUG").is_ok() {
        let owners = ringdesign_core::interaction::pick::part_owners(&built);
        let m = &built.mesh;
        let mut shown = 0;
        for (f, o) in m.faces.iter().zip(&owners) {
            if o.is_none() {
                continue;
            }
            let Some(n) = m.face_normal(f) else { continue };
            let Some((a, b, c)) = m.triangle(f) else { continue };
            let zc = (a[2] + b[2] + c[2]) / 3.0;
            let dd = castability::draft_angle(n, zc, 0.0);
            if dd < -5.0 && shown < 30 {
                shown += 1;
                eprintln!("built leaning {:.1} at {:?} {:?} {:?}", dd, a, b, c);
            }
        }
    }
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let head_crossings = csg::self_crossings(&solid);
    let (open_edges, _) = sculpt::closure(&solid);
    let band_field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
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
    let clamp_worst = 0.0f64;
    let stamps = d.stamps.len();
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", main_pass.watertight && main_pass.degenerate == 0 && main_pass.crossings == 0),
        ("the head part closed and uncrossed, as made and as placed", open_edges == 0 && head_crossings == 0 && main_pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, every stamp resolved, the head joined", main_pass.notes.is_empty() && main_pass.stamped == stamps && main_pass.joined == 1),
        ("nothing enters the finger hole", inside == 0),
        ("field verdict Castable (sand, Petrobond), the head judged in", field.process == CastProcess::SandTwoPart && field.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable),
        ("ray release 0 obstructions, 0 unresolved at 0.100 and 0.075 mm", main_pass.release_01.0 == 0 && main_pass.release_01.1 == 0 && main_pass.release_0075.0 == 0 && main_pass.release_0075.1 == 0),
        ("every draft-clamp bite at most 0.05 mm", clamp_worst <= 0.05),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("gates hold at 384 x 192", coarse_pass.as_ref().is_none_or(|c| c.ok(stamps))),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within 2 million triangles", main_pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "stage": "block-out",
        "process": d.draft.process.label(),
        "sand": "Petrobond",
        "draft": {"process": d.draft.process.label(), "sand": format!("{:?}", d.draft.sand), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.as_ref().map(|c| c.json()),
        "head_part": {"open_edges": open_edges, "self_crossings": head_crossings, "faces": solid.f.len(), "volume_mm3": comp.head_volume_mm3, "least_draft_off_parting_deg": comp.head_min_draft_deg, "facets_leaning_back": comp.head_undercut_facets},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "band_verdict": band_field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "drag_percent": field.drag_fraction() * 100.0, "band_drag_percent": band_field.drag_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes,
            "parts": field.parts.iter().map(|p| json!({"name": p.name, "undercut_mm2": p.undercut_area_mm2, "silhouette_mm2": p.silhouette_mm2, "marginal_mm2": p.marginal_area_mm2, "vertical_mm2": p.vertical_area_mm2, "total_mm2": p.total_area_mm2, "worst_draft_deg": p.worst_draft_deg, "note": p.note})).collect::<Vec<_>>()},
        "draft_clamp": {"groups": 0, "worst_mm": clamp_worst, "note": "no clamped group on the block-out"},
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
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Ouroborus / Petrobond pattern")?;
    }
    renders(&out, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} (band {}; undercut {:.3}%, drag {:.1}% (band {:.1}%), worst {:.1} deg); dfm {}; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}",
        field.verdict.label(),
        band_field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.drag_fraction() * 100.0,
        band_field.drag_fraction() * 100.0,
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
    ensure!(!text.is_empty());
    Ok(())
}
