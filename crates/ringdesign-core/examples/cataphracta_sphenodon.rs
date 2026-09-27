//! Cataphracta — Sphenodon, the parietal: a tuatara lying round the band, its head on the face with the peridot set on the crown of its skull, its crest a
//! serrated fin from nape to tail tip, cast in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_sphenodon
//! target/release/examples/cataphracta_sphenodon [OUT_DIR] [--draft] [--verify] [--block-out]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window},
    gem::{Gem, GemCut},
    interaction::bvh::Bvh,
    manufacturing as mf, mesh,
    profile::ShankKey,
    render::{self, Part},
    setting::SolidKind,
    skin::{self, Atlas, Hide, HidePoint},
    stl,
};
use serde_json::{Value, json};
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
    time::Instant,
};

const BORE_MM: f64 = 18.6;
const STONE_DEG: f64 = 90.0;
/// The investment's fill floor, mm (Logan: 0.8 for this ring), and the lost-wax detail floor.
const MIN_SECTION_MM: f64 = 0.8;
const HERO: (f64, f64) = (0.55, 0.95);
/// The painted atlas the animal is modelled on.
const AW: usize = 2048;
const AH: usize = 640;
/// The alpha's full scale, mm: the tallest the relief may stand.
const TOP_MM: f64 = 3.4;

/// Along the animal from the snout tip, mm: where the stone sits on the skull (the parietal), and how far short of the
/// snout the tail tip stops.
const STONE_T: f64 = 9.8;
/// Along the animal: the eyes' centres, ahead of the stone.
const EYE_T: f64 = 7.0;
const EYE_X: f64 = 2.2;
const GAP_MM: f64 = 9.0;
/// The seat: the mound's top over the band.
const SEAT_MM: f64 = 2.6;

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

/// Polynomial smooth maximum, `k` mm of blend.
fn smax(a: f64, b: f64, k: f64) -> f64 {
    if k < 1e-6 {
        return a.max(b);
    }
    let h = (0.5 + 0.5 * (a - b) / k).clamp(0.0, 1.0);
    b + (a - b) * h + k * h * (1.0 - h)
}

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

/// The body's half-width in plan, mm, along the animal from the snout tip: the blunt beak, the wedge of the skull widest
/// behind the eyes, the neck, the barrel, the pelvis and the long tail tapering to a point.
const WIDTH: [(f64, f64); 19] = [
    (0.0, 1.5),
    (0.6, 1.95),
    (1.5, 2.3),
    (3.5, 2.85),
    (6.0, 3.2),
    (9.0, 3.25),
    (11.5, 3.1),
    (13.0, 2.7),
    (14.5, 2.3),
    (16.5, 2.45),
    (19.0, 2.75),
    (23.0, 2.85),
    (27.0, 2.75),
    (30.0, 2.45),
    (33.0, 2.2),
    (45.0, 1.85),
    (60.0, 1.25),
    (74.0, 0.6),
    (80.0, 0.25),
];
/// The body's height over the band on its midline, mm.
const HEIGHT: [(f64, f64); 17] = [
    (0.0, 0.55),
    (0.6, 0.95),
    (2.0, 1.15),
    (4.0, 1.45),
    (6.5, 1.7),
    (9.5, 1.85),
    (12.0, 1.75),
    (13.5, 1.4),
    (15.0, 1.25),
    (19.0, 1.4),
    (24.0, 1.45),
    (29.0, 1.3),
    (33.0, 1.2),
    (45.0, 1.05),
    (60.0, 0.8),
    (74.0, 0.45),
    (80.0, 0.2),
];
/// The crest's height over the body, mm: the comb tallest at the nape and over the shoulders, lower down the back, a
/// low saddle at the hips, then a low saw down the tail to the tip, one spine line from nape to tip.
const CREST: [(f64, f64); 16] = [
    (12.9, 0.0),
    (13.8, 0.8),
    (15.0, 1.2),
    (17.0, 1.2),
    (19.5, 1.15),
    (21.5, 0.85),
    (24.0, 0.65),
    (27.0, 0.6),
    (28.5, 0.4),
    (29.5, 0.25),
    (32.0, 0.25),
    (33.2, 0.45),
    (36.0, 0.5),
    (50.0, 0.42),
    (65.0, 0.3),
    (80.0, 0.15),
];
/// The crest's half-thickness at its foot, mm; its section is a half ellipse, so every spine tip is rounded.
const CREST_FOOT_MM: f64 = 0.6;
/// The share of a spine's height the web between spines keeps.
const CREST_WEB: f64 = 0.25;

/// `(1 - u^2)^p` inside the unit, 0 outside: the section of a raised form.
fn dome(u: f64, p: f64) -> f64 {
    let q = 1.0 - u * u;
    if q <= 0.0 { 0.0 } else { q.powf(p) }
}

/// One leg on one side: a chain of joints in (along the animal, out from the midline) mm, each with its tube radius and how
/// high the tube's floor stands over the band, then the foot's heading and five toe lengths.
struct Leg {
    joints: [(f64, f64, f64, f64); 3],
    heading: (f64, f64),
    toes: [f64; 5],
}

/// The painted animal.
struct Tuatara {
    /// The band's rim at the face: where the crown has turned to face the side, mm across from the parting line.
    rim: f64,
    /// Along the animal from the snout tip to the tail's tip, mm.
    end: f64,
    /// Along the parting line: the stone's place is 0; half the way round.
    reach: f64,
    legs: Vec<Leg>,
    /// The crest's spine index and place within it along the animal, tabulated every `PHASE_STEP` mm.
    phase: Vec<f64>,
    /// The tail's scale rings along the animal, tabulated the same way.
    rings: Vec<f64>,
}

const PHASE_STEP: f64 = 0.01;
/// Which side face the tail's tip curls down onto: +1 is +Z.
const CURL_SIDE: f64 = 1.0;

impl Tuatara {
    fn new(rim: f64, reach: f64) -> Self {
        let end = 2.0 * reach - GAP_MM;
        let front = Leg { joints: [(18.3, 1.9, 0.68, 0.95), (20.3, rim - 0.1, 0.56, 0.45), (18.2, rim + 0.35, 0.44, 0.1)], heading: (-0.9, 0.44), toes: [0.55, 0.95, 1.2, 1.1, 0.85] };
        let hind = Leg { joints: [(27.6, 1.9, 0.8, 0.9), (25.8, rim, 0.64, 0.45), (28.6, rim + 0.35, 0.48, 0.1)], heading: (0.9, 0.44), toes: [0.9, 1.2, 1.35, 1.0, 0.6] };
        let n = (end / PHASE_STEP) as usize + 2;
        let mut me = Self { rim, end, reach, legs: vec![front, hind], phase: vec![0.0; n], rings: vec![0.0; n] };
        // Spines about 1.45 mm apart on the back, closing to 0.8 mm at the tail's tip; the tail's scale rings about as long
        // as they are wide, 0.9 mm at its root and 0.35 mm at its tip.
        for k in 1..n {
            let t = k as f64 * PHASE_STEP;
            let pitch = pchip(&[(12.0, 1.4), (30.0, 1.5), (40.0, 1.15), (80.0, 0.8)], me.table_t(t));
            me.phase[k] = me.phase[k - 1] + PHASE_STEP / pitch;
            me.rings[k] = me.rings[k - 1] + PHASE_STEP / me.ring_pitch(t);
        }
        me
    }

    /// The knot tables run to 80 mm along the animal: a longer or shorter tail stretches their tail.
    fn table_t(&self, t: f64) -> f64 {
        if t > 34.0 { 34.0 + (t - 34.0) * (80.0 - 34.0) / (self.end - 34.0) } else { t }
    }

    fn ring_pitch(&self, t: f64) -> f64 {
        0.35 + 0.55 * ((self.width(t) - 0.25) / (2.2 - 0.25)).clamp(0.0, 1.0)
    }

    /// Along the animal from the snout tip, for a point `along` mm from the stone round the parting line.
    fn t_of(&self, along: f64) -> f64 {
        (along + STONE_T).rem_euclid(2.0 * self.reach)
    }

    /// The tail's line off the midline, mm: none on the body, a slow S down the tail, then the tip curling over the rim
    /// and down onto the side face.
    fn centre(&self, t: f64) -> f64 {
        if t < 32.0 {
            return 0.0;
        }
        let amp = 0.25 + 0.9 * smoothstep(40.0, self.end - 8.0, t);
        let c = amp * smoothstep(32.0, 38.0, t) * (2.0 * PI * (t - 32.0) / 28.0).sin();
        let room = (self.rim - 0.35 - self.width(t)).max(0.0);
        let c = c.clamp(-room, room);
        let curl = smoothstep(self.end - 7.5, self.end - 0.8, t);
        c + (CURL_SIDE * (self.rim + 0.45) - c) * curl
    }

    fn width(&self, t: f64) -> f64 {
        if t < 0.0 || t > self.end {
            return 0.0;
        }
        let mut w = pchip(&WIDTH, self.table_t(t));
        // Rounded ends: the blunt beak and the tail's point.
        let snout = 0.7;
        if t < snout {
            w *= (1.0 - ((snout - t) / snout).powi(2)).max(0.0).sqrt();
        }
        let cap = 0.4;
        if t > self.end - cap {
            w *= (1.0 - ((t - (self.end - cap)) / cap).powi(2)).max(0.0).sqrt();
        }
        w
    }

    fn height(&self, t: f64) -> f64 {
        pchip(&HEIGHT, self.table_t(t)) * (0.35 + 0.65 * smoothstep(0.0, 0.6, t))
    }

    fn crest(&self, t: f64) -> f64 {
        let tt = self.table_t(t);
        if tt < CREST[0].0 {
            return 0.0;
        }
        pchip(&CREST, tt) * (1.0 - smoothstep(self.end - 1.5, self.end - 0.3, t))
    }

    /// The trunk (head, neck, body and tail) as a raised figure: domed across, a crisp fall at its outline. Returns the
    /// height and the share across (0 on the midline, 1 at the outline).
    fn trunk(&self, t: f64, x: f64) -> (f64, f64) {
        let w = self.width(t);
        if w <= 1e-3 {
            return (0.0, 2.0);
        }
        let u = (x - self.centre(t)) / w;
        if u.abs() >= 1.0 {
            return (0.0, u.abs());
        }
        (self.height(t) * dome(u, 0.45), u.abs())
    }

    /// The comb on the midline: backswept spines with rounded tips standing on a continuous web, a half-ellipse in
    /// section.
    fn fin(&self, t: f64, x: f64) -> f64 {
        let f = self.crest(t);
        if f <= 0.0 {
            return 0.0;
        }
        let foot = CREST_FOOT_MM.min(0.5 * self.width(t));
        let d = (x - self.centre(t)).abs();
        if d >= foot {
            return 0.0;
        }
        let k = ((t / PHASE_STEP) as usize).min(self.phase.len() - 1);
        let s = self.phase[k].fract();
        let pitch = 1.0 / ((self.phase[(k + 1).min(self.phase.len() - 1)] - self.phase[k]) / PHASE_STEP).max(1e-6);
        // Backswept: the spine's front edge is long, its back edge short; the tip is a parabola, so it is round.
        let peak = 0.62;
        let (off, half) = if s < peak { ((peak - s) * pitch, peak * pitch) } else { ((s - peak) * pitch, (1.0 - peak) * pitch) };
        // A triangle whose tip is rounded over 0.2 mm.
        let rho = 0.12;
        let tooth = 1.0 - ((off * off + rho * rho).sqrt() - rho) / (half - rho).max(0.1);
        let top = f * (CREST_WEB + (1.0 - CREST_WEB) * tooth.max(0.0));
        top * dome(d / foot, 0.62)
    }

    /// The head's features on the skull: the beak, the mouth line, nostrils, and the eyes under their heavy lids.
    fn head(&self, t: f64, x: f64, base: f64) -> f64 {
        if t > 14.0 || base <= 0.0 {
            return base;
        }
        let ax = x.abs();
        let w = self.width(t).max(0.3);
        let g = |d: f64, s: f64| (-(d / s).powi(2)).exp();
        let mut h = base;
        // The beak: the upper jaw's tip swells over the lower and hooks down at the front.
        h += 0.4 * g(t - 0.75, 0.45) * g(x, 0.85);
        // The mouth line: a groove down each side of the head from the beak back to below the eye, hooking down at the
        // front under the beak.
        let line = (ax - 0.72 * w).abs();
        h -= 0.3 * smoothstep(0.6, 2.2, t) * (1.0 - smoothstep(8.2, 9.2, t)) * g(line, 0.11);
        // Nostrils, on the beak's shoulders.
        h -= 0.22 * g(((t - 1.2).powi(2) + (ax - 0.65).powi(2)).sqrt(), 0.13);
        // The eyes: a domed ball in a narrow socket with a vertical slit pupil, and over it a heavy lid fold on the
        // midline side.
        let (et, ex, er) = (EYE_T, EYE_X, 0.85);
        let r = ((t - et).powi(2) + (ax - ex).powi(2)).sqrt();
        let medial = smoothstep(-0.2, 0.5, (ex - ax) / er);
        let lid = 0.42 * g(r - er - 0.1, 0.27) * medial;
        let ball = 0.6 * dome(r / er, 0.5);
        let socket = -0.16 * g(r - er - 0.02, 0.12);
        let slit = -0.25 * (1.0 - smoothstep(0.06, 0.12, (t - et).abs())) * (1.0 - smoothstep(0.35, 0.5, (ax - ex).abs()));
        let eye = (ball + socket).max(lid) + slit * dome(r / er, 0.5).min(1.0).powf(0.2);
        h += eye;
        // The brow ridge runs on from the lid toward the stone.
        let (br, along) = seg_dist((t, ax), (et - 0.4, ex - er - 0.1), (et + 1.4, 1.35));
        h += 0.22 * (4.0 * along * (1.0 - along)).sqrt() * g(br, 0.22) * smoothstep(er - 0.1, er + 0.2, r);
        h
    }

    /// The legs on both sides: tapering tubes from shoulder and hip over the rounded rim onto the side faces, and splayed
    /// toes gripping them. Returns the height, or 0.
    fn legs(&self, t: f64, x: f64, limit: f64) -> f64 {
        let ax = x.abs();
        let mut best: f64 = 0.0;
        for leg in &self.legs {
            let j = leg.joints;
            for s in 0..2 {
                let (p, q) = ((j[s].0, j[s].1), (j[s + 1].0, j[s + 1].1));
                let (d, f) = seg_dist((t, ax), p, q);
                let r = (j[s].2 + (j[s + 1].2 - j[s].2) * f) * (1.0 + 0.16 * (PI * f).sin());
                if d < r {
                    let floor = j[s].3 + (j[s + 1].3 - j[s].3) * f;
                    best = best.max(floor * dome(d / r, 0.35) + 1.1 * r * dome(d / r, 0.7));
                }
            }
            // The foot: five toes fanned round the heading, each a tapering tube with a blunt claw.
            let (w0, w1) = (j[2].0, j[2].1);
            let base_ang = leg.heading.1.atan2(leg.heading.0);
            for (k, len) in leg.toes.iter().enumerate() {
                let ang = base_ang + (k as f64 - 2.0) * 0.45;
                let tip = (w0 + len * ang.cos(), w1 + len * ang.sin());
                let (d, f) = seg_dist((t, ax), (w0, w1), tip);
                let r = 0.3 * (1.0 - 0.3 * f) + 0.04 * (PI * f * 2.0).sin().max(0.0);
                if d < r {
                    best = best.max(0.4 * (1.0 - 0.25 * f) * dome(d / r, 1.0));
                }
            }
            let d = ((t - w0).powi(2) + (ax - w1).powi(2)).sqrt();
            best = best.max(0.42 * dome(d / 0.5, 1.0));
        }
        // Nothing past the side face's bore edge.
        best * (1.0 - smoothstep(limit - 0.5, limit - 0.2, ax))
    }

    /// The skin: on the body, fine granules with larger tubercles scattered down the flanks; on the head, granules finer
    /// still; on the tail, rings of domed squarish scales, staggered ring to ring, shrinking with the tail.
    fn skin(&self, t: f64, x: f64, along: f64, share: f64) -> f64 {
        let head = 1.0 - smoothstep(12.0, 14.0, t);
        let tail = smoothstep(31.0, 35.0, t);
        let mut body = 0.0;
        if tail < 1.0 {
            let cell = 0.46 * (1.0 - 0.4 * head);
            let (f1, f2, _, _) = voronoi(along, x, cell, 11);
            let granule = (0.09 - 0.04 * head) * smoothstep(0.0, 0.3 * cell, f2 - f1) * (0.55 + 0.45 * (1.0 - f1 / (0.8 * cell)).max(0.0));
            let (g1, _, id, _) = voronoi(along, x, 1.25, 23);
            let pick = skin::hash(id.0, id.1 + 7);
            let size = 0.32 + 0.16 * skin::hash(id.0 + 3, id.1);
            let flank = smoothstep(0.3, 0.45, share) * (1.0 - smoothstep(0.82, 0.95, share)) * (1.0 - head);
            let tubercle = if pick > 0.35 { (0.22 + 0.14 * skin::hash(id.1, id.0)) * dome(g1 / size, 0.75) * flank } else { 0.0 };
            body = granule.max(tubercle);
        }
        let mut scales = 0.0;
        if tail > 0.0 {
            let w = self.width(t).max(0.2);
            let u = ((x - self.centre(t)) / w).clamp(-1.0, 1.0);
            let k = ((t / PHASE_STEP) as usize).min(self.rings.len() - 1);
            // Rings bow back along the flanks.
            let ring = self.rings[k] - 0.35 * u * u;
            let row = ring.floor();
            let fr = ring - row;
            let p = self.ring_pitch(t);
            let n = (2.0 * w / p).round().max(2.0);
            let c = (u + 1.0) * 0.5 * n + 0.5 * (row.rem_euclid(2.0));
            let fc = c - c.floor();
            let amp = 0.14 + 0.1 * ((p - 0.35) / 0.55);
            scales = amp * dome(2.0 * fr - 1.0, 0.55) * dome(2.0 * fc - 1.0, 0.55);
        }
        body * (1.0 - tail) + scales * tail
    }

    /// The whole relief at one hide point, mm over the band.
    fn relief(&self, hp: HidePoint) -> f64 {
        let t = self.t_of(hp.along);
        let x = hp.across;
        let limit = hp.rim + hp.wall;
        let (body, share) = self.trunk(t, x);
        let body = self.head(t, x, body);
        let legs = self.legs(t, x, limit);
        let inside = smoothstep(0.0, 0.1, body.max(legs));
        let skin = if inside > 0.0 { self.skin(t, x, hp.along, share.min(1.0)) * inside } else { 0.0 };
        // The legs fair into the body; off it they stand alone, with no step at their outline.
        let form = if legs > 0.0 { smax(body, legs, 0.25 * smoothstep(0.0, 0.2, body)) } else { body };
        let form = if form > 0.0 { form + skin } else { 0.0 };
        (form + self.fin(t, x)).clamp(0.0, TOP_MM)
    }
}

/// Distance from `p` to the segment `a`–`b`, and how far along it the nearest point is.
fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = (dx * dx + dy * dy).max(1e-12);
    let f = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0);
    (((p.0 - a.0 - f * dx).powi(2) + (p.1 - a.1 - f * dy).powi(2)).sqrt(), f)
}

/// Jittered cells of about `g` mm: the nearest and second-nearest seed distances, the nearest seed's cell, and its seed.
fn voronoi(u: f64, v: f64, g: f64, salt: i64) -> (f64, f64, (i64, i64), (f64, f64)) {
    let (i0, j0) = ((u / g).floor() as i64, (v / g).floor() as i64);
    let (mut f1, mut f2, mut id, mut seed) = (f64::MAX, f64::MAX, (0, 0), (0.0, 0.0));
    for di in -1..=1 {
        for dj in -1..=1 {
            let (i, j) = (i0 + di, j0 + dj);
            let px = (i as f64 + 0.15 + 0.7 * skin::hash(i, j * 31 + salt)) * g;
            let py = (j as f64 + 0.15 + 0.7 * skin::hash(i * 17 + salt, j)) * g;
            let d = ((u - px).powi(2) + (v - py).powi(2)).sqrt();
            if d < f1 {
                f2 = f1;
                f1 = d;
                id = (i, j);
                seed = (px, py);
            } else if d < f2 {
                f2 = d;
            }
        }
    }
    (f1, f2, id, seed)
}

/// The Flat band in lost wax: thickness keyed a touch deeper under the head, lighter at the palm.
fn band() -> RingDesign {
    let mut d = RingDesign { name: "Sphenodon \u{2014} the parietal".into(), ..RingDesign::default() };
    d.profile.width_mm = 7.5;
    d.profile.thickness_mm = 2.5;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.crown_mm = 0.8;
    d.profile.comfort_fit_mm = 0.15;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).expect("bore");
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale: 1.0, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(0.0, 1.0), key(90.0, 1.05), key(180.0, 1.0), key(270.0, 0.9)];
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = format!("{} / investment / Silver 925", d.name);
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").unwrap().shrink_pct;
    setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, wax and measured trials.".into();
    setup.sample_pitch_mm = 0.1;
    setup.auto_parting = false;
    setup.parting_mm = 0.0;
    setup.bench_notes = "Lost wax. A tuatara lies round the band: its head on the face with a 3.0 mm round peridot set flush in a mound on the crown \
        of the skull, ringed with beads; its crest a serrated fin from the nape to the tail tip. Sprue from the palm. At the bench: drill from \
        the raised dot and cut the flush seat, set the peridot; clean the fin's spines and the toes with a fine graver; polish the ground, \
        leave the skin satin."
        .into();
    d.manufacturing = Some(setup);
    d
}

fn peridot() -> Gem {
    Gem { preview_tint: Some([0.50, 0.78, 0.12]), ..Gem::calibrated(GemCut::Round, 3.0) }
}

fn seat() -> SeatPadLayer {
    let mut s = SeatPadLayer { theta_deg: STONE_DEG, v_mm: 0.0, style: SeatStyle::GypsyMound, crown: 1.0, blend_mm: 0.35, solid: SolidKind::Flush, through: true, ..SeatPadLayer::default() };
    s.fit_stone(peridot());
    s.crown = 0.55;
    s.height_mm = SEAT_MM;
    s.mark_mm = 0.6;
    s
}

struct Layers {
    names: Vec<String>,
    end: f64,
    rim: f64,
    wall: f64,
    seat_r: f64,
}

/// The design: the band, the painted tuatara, and the parietal seat.
fn design(_block_out: bool) -> Result<(RingDesign, AlphaLibrary, Layers)> {
    let mut d = band();
    let ctx = d.field_context();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let col = AW / 4;
    let (rim, wall) = (0.5 * (hide.rim[col][0] + hide.rim[col][1]), 0.5 * (hide.wall[col][0] + hide.wall[col][1]));
    let s = seat();
    let seat_r = 0.5 * s.diameter_mm;
    println!("  hide: reach {:.2} mm, rim {rim:.2} mm, wall {wall:.2} mm at the face; seat {:.2} mm", hide.reach(), s.diameter_mm);
    let animal = Tuatara::new(rim, hide.reach());
    let end = animal.end;
    let alpha = a.paint("Tuatara", |smp| animal.relief(hide.at(smp)) / TOP_MM);
    let tuatara = ringdesign_core::Alpha::from_png16("Tuatara", &alpha.to_png16()?)?;
    let e = skin::hide_layer(&d, "Tuatara", TOP_MM, Window::default());
    d.layers.layers.push(e);

    let mut s = s;
    s.v_mm = ctx.crest_v_mm;
    let mut e = LayerEntry::new("Parietal peridot", Layer::SeatPad(s));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    if let Ok(mute) = std::env::var("SPHENODON_MUTE") {
        d.layers.layers.retain(|e| !mute.split(',').any(|m| m == e.name));
    }
    let mut lib = AlphaLibrary::default();
    lib.insert(tuatara);
    d.bake_all(&mut lib);
    let names = d.layers.layers.iter().map(|e| e.name.clone()).collect();
    Ok((d, lib, Layers { names, rim, wall, seat_r, end }))
}

fn release_json(r: &mf::release::ReleaseReport) -> Value {
    json!({
        "status": format!("{:?}", r.status),
        "obstructions": r.obstructions.len(),
        "unresolved_rays": r.unresolved_rays,
    })
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Which part of the animal a point of the finished ring belongs to, for naming its thin sections.
fn feature_at(p: [f64; 3], d: &RingDesign, end: f64) -> &'static str {
    let theta = p[1].atan2(p[0]).to_degrees().rem_euclid(360.0);
    let ctx = d.field_context();
    let along = ((theta - STONE_DEG + 540.0).rem_euclid(360.0) - 180.0).to_radians() * ctx.crest_radius_mm;
    let t = (along + STONE_T).rem_euclid(2.0 * PI * ctx.crest_radius_mm);
    let z = p[2].abs();
    if t < 14.5 {
        return if (t - STONE_T).hypot(p[2]) < 3.0 { "the parietal seat" } else { "the head: beak, eyes, lids, brows, nostrils" };
    }
    if z > 2.0 && ((15.5..23.0).contains(&t) || (23.5..31.5).contains(&t)) {
        return "the legs and toes";
    }
    if t > end - 8.0 {
        return "the tail's tip";
    }
    if t < 33.0 && z < CREST_FOOT_MM + 0.1 {
        return "the crest's spines";
    }
    if t >= 31.0 {
        return "the tail: scale rings and its saw";
    }
    "the body's skin: granules and tubercles"
}

/// What the bench does with each kind of thin section, with its measured thinnest.
fn treatment(feature: &str, thinnest: f64) -> String {
    let what = match feature {
        "the crest's spines" => "The comb: a half-ellipse blade 1.2 mm at its foot with parabolic, rounded spine tips; the tips fill from the blade in investment. Clean with a fine graver; do not polish them round.",
        "the legs and toes" => "Legs and toes: tubes 0.5 mm and up standing on the band and its rim; they fill from the band. Keep the claws; satin brush only.",
        "the parietal seat" => "The mound's lip round the flush seat: fills from the head; burnished over the girdle when the peridot is set.",
        "the head: beak, eyes, lids, brows, nostrils" => "Beak, eyes, lids, brows and the mouth line: relief on a 1.8 mm skull; they fill from the head. Re-cut the slit pupils with a knife graver if the investment softens them.",
        "the tail's tip" => "The tail's tip where it curls over the rim onto the side face: a 0.5 mm point on the band; fills from the band. Leave as cast.",
        "the tail: scale rings and its saw" => "The tail's domed scale rings and its low saw: relief on the band; fill from the tail. Satin brush.",
        _ => "Skin granules and tubercles: 0.05 to 0.36 mm relief on the body; fill from the body. Leave satin.",
    };
    format!("{what} Thinnest single-ray section read here: {thinnest:.3} mm, a relief read across its own flank where it rises off the metal beneath, not a free-standing wire.")
}

/// The land-width census: every face of the finished ring read by one ray along its inward normal (as `dfm::part_sections`
/// reads a part), the area under the fill floor grouped by the feature it belongs to, each with its bench treatment.
fn land_widths(built: &mesh::BuildResult, d: &RingDesign, end: f64) -> Value {
    let m = &built.mesh;
    let solid = solid_of(m);
    let (min_all, under_all) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    let bvh = Bvh::build(m);
    const IN: f64 = 1e-4;
    let mut groups: Vec<(&'static str, f64, f64, usize)> = Vec::new();
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let e1: [f64; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let e2: [f64; 3] = std::array::from_fn(|k| c[k] - a[k]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !(twice > 1e-14) {
            continue;
        }
        let inward = n.map(|x| -x / twice);
        let cen: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let o: [f64; 3] = std::array::from_fn(|k| cen[k] + IN * inward[k]);
        let Some((_, t)) = bvh.ray(m, o, inward) else { continue };
        let section = t + IN;
        if section < MIN_SECTION_MM {
            let name = feature_at(cen, d, end);
            if std::env::var_os("SPHENODON_DIAG").is_some() && section < 0.3 {
                let th = cen[1].atan2(cen[0]).to_degrees().rem_euclid(360.0);
                eprintln!("thin {section:.3} at theta {th:.2} z {:.2} r {:.2} n [{:.2} {:.2} {:.2}] {name}", cen[2], cen[0].hypot(cen[1]), inward[0], inward[1], inward[2]);
            }
            match groups.iter_mut().find(|g| g.0 == name) {
                Some(g) => {
                    g.1 += 0.5 * twice;
                    g.2 = g.2.min(section);
                    g.3 += 1;
                }
                None => groups.push((name, 0.5 * twice, section, 1)),
            }
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    let named = groups.iter().map(|g| g.1).sum::<f64>();
    json!({
        "floor_mm": MIN_SECTION_MM,
        "method": "dfm::part_sections over the whole finished ring (no made parts: the animal is painted relief), then the same one-ray read per face grouped by the feature it lies in",
        "thinnest_mm": min_all,
        "area_under_floor_mm2": under_all,
        "area_named_mm2": named,
        "all_named": (named - under_all).abs() < 1e-3 * under_all.max(1.0) + 1e-6,
        "removed": [],
        "by_feature": groups.iter().map(|g| json!({ "feature": g.0, "area_mm2": g.1, "thinnest_mm": g.2, "faces": g.3, "bench": treatment(g.0, g.2) })).collect::<Vec<_>>(),
    })
}

/// Every gate at one build size, as JSON, with whether they all passed.
struct Gated {
    json: Value,
    passed: bool,
    built: mesh::BuildResult,
    pattern: mesh::BuildResult,
    inspection: mf::Inspection,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str, end: f64) -> Result<Gated> {
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
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let triangles = built.mesh.faces.len();
    let lands = land_widths(&built, d, end);
    let lands_ok = lands["all_named"].as_bool().unwrap_or(false);
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && degenerate == 0),
        ("0 self-crossings on the ring (no made parts)", crossings == 0),
        ("solids notes empty, every stamp resolved", built.solids.notes.is_empty() && built.parts.notes.is_empty() && built.solids.stamped == d.stamps.len()),
        ("nothing inside the finger hole", inside == 0),
        ("field Castable under lost wax at the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && d.draft.min_section_mm >= MIN_SECTION_MM),
        ("land widths: every section under 0.8 mm named with its bench treatment", lands_ok),
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
    println!("  field {} (thinnest wall {:.2} mm; two-part undercut {:.2} mm2) {:?}", field.verdict.label(), field.thinnest_wall_mm, field.undercut_area_mm2, field.notes);
    println!("  lands: thinnest {:.3} mm, {:.2} mm2 under the floor", lands["thinnest_mm"].as_f64().unwrap_or(0.0), lands["area_under_floor_mm2"].as_f64().unwrap_or(0.0));
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    let json = json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": triangles, "ms": build_ms },
        "process": format!("{:?}", d.draft.process),
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3 },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "resolved": built.solids.resolved, "notes": built.solids.notes, "parts_notes": built.parts.notes, "made_parts": 0 },
        "finger_hole": { "bore_radius_mm": bore, "vertices_inside": inside, "nearest_margin_mm": nearest },
        "field": { "verdict": field.verdict.label(), "process": format!("{:?}", field.process), "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes },
        "land_widths": lands,
        "two_part_numbers": {
            "note": "Lost wax: these measure a two-part sand pull and are reported, not gated.",
            "undercut_area_mm2": field.undercut_area_mm2,
            "marginal_area_mm2": field.marginal_area_mm2,
            "vertical_area_mm2": field.vertical_area_mm2,
            "total_area_mm2": field.total_area_mm2,
            "worst_draft_deg": field.worst_draft_deg,
            "release_0100": release_json(r),
            "release_0075": release_json(&release_fine),
        },
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
    // The stone close-up: over the head, from the snout's side.
    render::write_png_parts(out.join("stones.png"), &parts, -0.35, 1.2, edge)?;
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
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" }, layers.end)?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320", layers.end)?;
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
        Layer::SeatPad(s) => Some(json!({ "theta_deg": s.theta_deg, "v_mm": s.v_mm, "diameter_mm": s.diameter_mm, "height_mm": s.height_mm, "crown": s.crown, "blend_mm": s.blend_mm, "style": format!("{:?}", s.style) })),
        _ => None,
    });
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": "sphenodon",
        "process": "lost wax",
        "stage": if block_out { "block-out" } else { "full" },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "layers": layers.names,
        "seat": stone,
        "animal": { "atlas": [AW, AH], "top_mm": TOP_MM, "stone_t_mm": STONE_T, "gap_mm": GAP_MM, "tail_end_mm": layers.end, "eye_t_mm": EYE_T, "rim_mm": layers.rim, "wall_mm": layers.wall, "seat_radius_mm": layers.seat_r, "width_knots": WIDTH, "height_knots": HEIGHT, "crest_knots": CREST },
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
