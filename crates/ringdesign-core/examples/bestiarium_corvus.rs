//! Bestiarium — Corvus, Huginn and Muninn: Odin's two ravens riding the arms of a bypass, necks crossed under an onyx, each head sent outward along the other's back, cast in Delft sand.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_corvus
//! target/release/examples/bestiarium_corvus [OUT_DIR] [--draft] [--verify] [--preview]
#![recursion_limit = "256"]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, LayerEntry, ProfileStyle, RingDesign, RingSize, ShankKind, Window,
    castability::{self, SandProcess},
    csg, dfm,
    field::{Layer, SeatPadLayer, SeatStyle},
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh, outline, render,
    setting::{self, SolidKind, Stamp, StampTop},
    skin::{self, Atlas, Hide},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn stamp_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The bypass LowDome: two arms passing over the top, each the band's own width, parted on the plane its crest is held to.
fn band() -> RingDesign {
    let mut d = RingDesign { name: "Corvus \u{2014} Huginn and Muninn".into(), ..RingDesign::default() };
    d.size = RingSize::from_diameter_mm(18.6);
    d.profile.width_mm = 6.0;
    d.profile.thickness_mm = 3.0;
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.comfort_fit_mm = 0.0;
    d.shank.kind = ShankKind::Bypass;
    d.shank.amount = 1.0;
    SandProcess::DelftClay.apply(&mut d.draft);
    d.draft.auto_parting = true;
    d.draft.parting_z_mm = 0.0;
    d
}

fn onyx() -> Gem {
    Gem { l_mm: 8.0, preview_tint: Some([0.015, 0.015, 0.02]), ..Gem::cabochon(GemCut::Oval, 6.0) }
}

/// Height of the gypsy stock under the collet, mm.
const SEAT_MM: f64 = 0.25;
/// How far the onyx's girdle sits below the stock's top, mm.
const SET_DEPTH_MM: f64 = 0.3;

/// The onyx on low gypsy stock at the crossing, in a made collet.
fn seat(d: &mut RingDesign) -> Result<()> {
    let v = setting::crest_v(d, 90.0).ok_or_else(|| anyhow::anyhow!("No crest at the top"))?;
    let mut s = SeatPadLayer {
        theta_deg: 90.0,
        v_mm: v,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        metal_true: true,
        solid: SolidKind::Bezel,
        mark_mm: 0.8,
        ..Default::default()
    };
    s.fit_stone(onyx());
    s.height_mm = SEAT_MM;
    s.set_depth_mm = Some(SET_DEPTH_MM);
    d.layers.layers.push(LayerEntry::new("Onyx, made collet", Layer::SeatPad(s)));
    Ok(())
}

type P2 = [f64; 2];

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth minimum of `a` and `b`, blended over `r`.
fn soft_min(a: f64, b: f64, r: f64) -> f64 {
    let h = (r - (a - b).abs()).max(0.0) / r;
    a.min(b) - 0.25 * r * h * h
}

fn smoother(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// An open polyline resampled at equal steps no longer than `step`, both ends kept.
fn run(pts: &[P2], step: f64) -> Vec<P2> {
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum[cum.len() - 1] + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = cum[cum.len() - 1];
    let m = ((total / step).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(m + 1);
    let mut j = 0;
    for k in 0..=m {
        let s = total * k as f64 / m as f64;
        while j + 2 < pts.len() && cum[j + 1] < s {
            j += 1;
        }
        let f = ((s - cum[j]) / (cum[j + 1] - cum[j]).max(1e-12)).clamp(0.0, 1.0);
        out.push([pts[j][0] + (pts[j + 1][0] - pts[j][0]) * f, pts[j][1] + (pts[j + 1][1] - pts[j][1]) * f]);
    }
    out
}

/// Counter-clockwise outline between `top(x)` and `bottom(x)` over `x0..x1`, corners kept, edges under 0.1 mm.
fn banded(x0: f64, x1: f64, top: impl Fn(f64) -> f64, bottom: impl Fn(f64) -> f64) -> Vec<P2> {
    const STEP: f64 = 0.09;
    let n = (((x1 - x0) / 0.01).ceil() as usize).max(16);
    let xs: Vec<f64> = (0..=n).map(|k| x0 + (x1 - x0) * k as f64 / n as f64).collect();
    let low: Vec<P2> = xs.iter().map(|&x| [x, bottom(x)]).collect();
    let high: Vec<P2> = xs.iter().rev().map(|&x| [x, top(x)]).collect();
    let mut out = run(&low, STEP);
    out.pop();
    let (b1, t1) = (bottom(x1), top(x1));
    if t1 - b1 > 1e-6 {
        out.extend(run(&[[x1, b1], [x1, t1]], STEP));
        out.pop();
    }
    out.extend(run(&high, STEP));
    out.pop();
    let (b0, t0) = (bottom(x0), top(x0));
    if t0 - b0 > 1e-6 {
        out.extend(run(&[[x0, t0], [x0, b0]], STEP));
        out.pop();
    }
    out
}

/// Huginn rides the high-z arm with his body on the low-angle shoulder; Muninn mirrors him.
const BIRDS: [(&str, f64); 2] = [("Huginn", 1.0), ("Muninn", -1.0)];

/// A hide sample seen from one raven: along its body from the top (its head at negative `o`), across the crest, and its side's rim.
#[derive(Clone, Copy)]
struct On {
    o: f64,
    w: f64,
    rim: f64,
}

fn on_bird(p: skin::HidePoint, sign: f64, palm: f64) -> On {
    let raw = -sign * p.along;
    On { o: if raw < -0.5 * palm { raw + 2.0 * palm } else { raw }, w: p.across, rim: p.rim }
}

/// Per atlas column, the hide's `across` at the section's exact parting-plane crossing.
fn zero_across(a: &Atlas, hide: &Hide) -> Vec<f64> {
    (0..a.width)
        .map(|x| {
            let c = hide.crest[x];
            let at = |y: usize| (a.at(x, y).p[2], hide.across[y * a.width + x]);
            for (y0, y1) in [(c.saturating_sub(1), c), (c, (c + 1).min(a.height - 1))] {
                let ((z0, w0), (z1, w1)) = (at(y0), at(y1));
                if y0 != y1 && z0 * z1 <= 0.0 && z0 != z1 {
                    return w0 + (w1 - w0) * z0 / (z0 - z1);
                }
            }
            0.0
        })
        .collect()
}

/// Per atlas column, how much further out the high-z side of the bare crest stands than the low side 0.4 mm off it: in radius, and in its normal's radial share.
struct Lean {
    r: Vec<f64>,
    n: Vec<f64>,
}

impl Lean {
    fn of(a: &Atlas, hide: &Hide, zero: &[f64]) -> Self {
        let (r, n) = (0..a.width)
            .map(|x| {
                let at = |w: f64| {
                    let y = (0..a.height).min_by(|p, q| (hide.across[*p * a.width + x] - zero[x] - w).abs().total_cmp(&(hide.across[*q * a.width + x] - zero[x] - w).abs())).unwrap_or(0);
                    let s = a.at(x, y);
                    let rad = s.p[0].hypot(s.p[1]).max(1e-9);
                    (rad, (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / rad)
                };
                let ((rp, np), (rm, nm)) = (at(0.4), at(-0.4));
                (rp - rm, np - nm)
            })
            .unzip();
        Self { r, n }
    }

    /// Height to take off at `w` so relief `h` tall at the crest of column `x` stands a share `k` nearer level either side of it.
    fn level(&self, x: usize, h: f64, w: f64, k: f64) -> f64 {
        let d = self.r[x] + h * self.n[x];
        if d * w <= 0.0 {
            return 0.0;
        }
        k * d.abs() * (w.abs() / 0.4).min(1.5).powi(2)
    }
}

/// Share of the bare crest's lean the beaks take out.
const LEVEL: f64 = 0.35;
/// Share of the bare crest's lean the skulls take out.
const SKULL_LEVEL: f64 = 0.15;

/// A sample in hide millimetres with `across` measured from the exact parting line.
fn hide_at(hide: &Hide, zero: &[f64], s: &skin::Sample) -> skin::HidePoint {
    let mut p = hide.at(s);
    p.across -= zero[s.i % zero.len()];
    p
}

/// Running median of each column's rim and wall over 61 columns, then a mean over 17, wrapping round the ring.
fn steady(hide: &mut Hide) {
    let n = hide.rim.len();
    let window = |v: &[[f64; 2]], x: usize, side: usize, half: usize, median: bool| {
        let mut w: Vec<f64> = (0..=2 * half).map(|k| v[(x + n + k - half) % n][side]).collect();
        if median {
            w.sort_by(f64::total_cmp);
            w[half]
        } else {
            w.iter().sum::<f64>() / w.len() as f64
        }
    };
    for field in [&mut hide.rim, &mut hide.wall] {
        let raw = field.clone();
        let med: Vec<[f64; 2]> = (0..n).map(|x| [0, 1].map(|side| window(&raw, x, side, 30, true))).collect();
        *field = (0..n).map(|x| [0, 1].map(|side| window(&med, x, side, 8, false))).collect();
    }
}

/// Half-width of the flat every crest relief keeps across the parting line, mm.
const CREST_FLAT: f64 = 0.3;
/// Half-width of a head's own crest flat, mm.
const HEAD_FLAT: f64 = 0.45;

/// A raven's head in crest millimetres: nape clear of the collet, rounded skull with a level crown, forehead step, beak sent away from the stone.
#[derive(Clone, Copy)]
struct HeadSpec {
    /// Skull's rear, mm along the crest from the top.
    nape: f64,
    skull: f64,
    /// Where the struck beak starts, mm forward of the skull's rear.
    root: f64,
    beak: f64,
    crown: f64,
    lore_h: f64,
    tip_h: f64,
    /// Painted beak half-width at its root.
    beak_w: f64,
}

const HEAD: HeadSpec = HeadSpec { nape: 5.2, skull: 5.6, root: 5.95, beak: 6.0, crown: 2.9, lore_h: 2.35, tip_h: 1.25, beak_w: 1.05 };
/// Length of the hooked drop at the beak's tip, mm.
const HOOK: f64 = 0.6;

/// Height of the flat each head stands on, mm: the plumage's own height beside it.
const HEAD_FLOOR: f64 = 0.85;
/// Share of the beak's height over the floor at which the gape runs.
const GAPE: f64 = 0.4;
/// Where the gape starts, mm forward of the skull's rear: under the eye's front corner.
const RICTUS_U: f64 = 4.3;
/// Eye socket depth below the crown's edge, mm.
const SOCKET_MM: f64 = 0.32;
/// The eye's distance forward of the skull's rear, mm.
const EYE_U: f64 = 3.9;

impl HeadSpec {
    /// Share of the crown height along the crest `u` mm forward of the skull's rear: a rounded nape, a level crown, a steep forehead.
    fn skull_along(&self, u: f64) -> f64 {
        let s = (2.0 * u / self.skull - 1.0).clamp(-1.0, 1.0);
        if s < 0.0 { 1.0 - (-s).powi(3) } else { 1.0 - s.powi(6) }
    }
    /// Skull half-width `u` mm forward of its rear, before the side's own room: an egg, widest behind the eye, drawn into the beak.
    fn skull_half(&self, u: f64) -> f64 {
        const WIDEST: f64 = 2.8;
        if u <= WIDEST {
            let t = 1.0 - u / WIDEST;
            1.25 + 0.85 * (1.0 - t * t).max(0.0).sqrt()
        } else {
            self.beak_w + (2.1 - self.beak_w) * (1.0 - smoother(WIDEST, self.skull + 0.2, u))
        }
    }
    /// Painted beak half-width `v` mm forward of its root.
    fn beak_half(&self, v: f64) -> f64 {
        let t = (v / self.beak).clamp(0.0, 1.0);
        (self.beak_w * (1.0 - t.powf(1.8)).max(0.0).sqrt()).max(0.32)
    }
    /// Culmen height `v` mm forward of the beak's root: level off the forehead, arching down to the tip, then hooked into the floor.
    fn beak_line(&self, v: f64) -> f64 {
        if v <= self.beak {
            let t = (v / self.beak).clamp(0.0, 1.0);
            return self.tip_h + (self.lore_h - self.tip_h) * (1.0 - t.powf(2.4));
        }
        let k = ((v - self.beak) / HOOK).min(1.0);
        self.tip_h * (1.0 - k * k).max(0.0).sqrt()
    }
    /// Depth of the terrace under the brow at `u`, mm: opening behind the eye and running forward to the beak.
    fn socket(&self, u: f64) -> f64 {
        SOCKET_MM * smoother(EYE_U - 3.1, EYE_U - 1.3, u)
    }
    /// Head relief, mm, `u` forward of the skull's rear and `x` off the crest on a side with `room`, and the head's half-width there.
    fn head(&self, u: f64, x: f64, room: f64) -> (f64, f64) {
        if !(0.0..=self.root + self.beak + HOOK).contains(&u) {
            return (0.0, 0.0);
        }
        let xe = (x - HEAD_FLAT).max(0.0);
        let (mut h, mut half): (f64, f64) = (0.0, 0.0);
        if u <= self.skull {
            let wide = soft_min(self.skull_half(u), room, 0.4);
            let we = (wide - HEAD_FLAT).max(0.15);
            let a = self.skull_along(u);
            let dome = |e: f64| self.crown * (a - (e / we).powi(2)).max(0.0).sqrt();
            let xs = 0.35 * we;
            let cap = if xe > xs { dome(xs) - self.socket(u) * smoother(0.0, 0.25, xe - xs) - 0.3 * (xe - xs) } else { f64::MAX };
            h = soft_min(dome(xe), cap, 0.3);
            half = wide;
        }
        if u >= self.skull - 0.6 {
            let v = (u - self.root).max(0.0);
            let wide = self.beak_half(v).min(room);
            let flat = CREST_FLAT.min(wide - 0.02);
            let fall = ((x - flat).max(0.0) / (wide - flat).max(0.02)).min(1.0);
            h = h.max(self.beak_line(v) * (1.0 - fall * fall).sqrt());
            half = half.max(wide);
        }
        (h.max(HEAD_FLOOR * (1.0 - smooth(half, half + 0.12, x))), half)
    }
    fn head_mm(&self, u: f64, x: f64, room: f64) -> f64 {
        self.head(u, x, room).0
    }
    /// Height of the gape line `u` mm forward of the skull's rear: level from the mouth's corner, then a share of the beak's height over the floor.
    fn gape_h(&self, u: f64) -> f64 {
        let v = (u - self.root).max(0.0);
        HEAD_FLOOR + GAPE * (self.beak_line(v.min(self.beak)) - HEAD_FLOOR)
    }
    /// The gape line's distance off the crest `u` mm forward of the skull's rear on a side with `room`.
    fn gape_x(&self, u: f64, room: f64) -> f64 {
        let (target, half) = (self.gape_h(u), self.head(u, 0.0, room).1);
        let (mut lo, mut hi) = (CREST_FLAT, half);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if self.head_mm(u, mid, room) > target { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    }
    /// The eye's distance off the crest on a side with `room`: on its socket's terrace.
    fn eye_x(&self, room: f64) -> f64 {
        let we = (soft_min(self.skull_half(EYE_U), room, 0.4) - HEAD_FLAT).max(0.15);
        HEAD_FLAT + 0.35 * we + 0.4
    }
}

/// Crown a side of the section offers a head, mm off the crest: its rim and a little way onto the fillet.
fn room(rim: f64) -> f64 {
    rim + ROOM_PAST_RIM
}

/// How far past its rim a side lets a head run over the fillet, mm.
const ROOM_PAST_RIM: f64 = 1.3;

/// Height of head relief painted on the atlas at alpha 1, mm.
const PAINT_MM: f64 = 3.5;

/// Bare point and normal `w` mm off the exact crest in column `x`, between the rows either side.
fn base_at(a: &Atlas, hide: &Hide, zero: &[f64], x: usize, w: f64) -> Option<([f64; 3], [f64; 3])> {
    let across = |y: usize| hide.across[y * a.width + x] - zero[x];
    let y = (0..a.height - 1).find(|&y| (across(y) - w) * (across(y + 1) - w) <= 0.0 && across(y) != across(y + 1))?;
    let t = (w - across(y)) / (across(y + 1) - across(y));
    let (p, q) = (a.at(x, y), a.at(x, y + 1));
    Some((std::array::from_fn(|k| p.p[k] + (q.p[k] - p.p[k]) * t), std::array::from_fn(|k| p.n[k] + (q.n[k] - p.n[k]) * t)))
}

/// Columns either side the heads' bare surface is faired over, and the fairing's spread in columns.
const FAIR_REACH: usize = 48;
const FAIR_SIGMA: f64 = 16.0;

/// Both heads painted on the atlas, as a share of [`PAINT_MM`]: each head stands on the bare surface faired along the ring, so an arm's tip under it does not crease it.
fn paint_heads(a: &Atlas, hide: &Hide, zero: &[f64], lean: &Lean) -> Alpha {
    let palm = hide.reach();
    a.paint("Raven heads", |s| {
        let p = hide_at(hide, zero, s);
        let x = s.i % a.width;
        let h = BIRDS
            .iter()
            .map(|&(_, sign)| {
                let q = on_bird(p, sign, palm);
                let u = -q.o - HEAD.nape;
                let h = HEAD.head_mm(u, q.w.abs(), room(q.rim));
                let k = SKULL_LEVEL + (LEVEL - SKULL_LEVEL) * smoother(HEAD.skull - 1.2, HEAD.skull - 0.2, u);
                if h > 0.0 { (h - lean.level(x, HEAD.head_mm(u, 0.0, room(q.rim)), q.w, k)).max(0.0) } else { 0.0 }
            })
            .fold(0.0, f64::max);
        if h <= 0.0 {
            return 0.0;
        }
        let (mut b, mut n, mut wsum) = ([0.0; 3], [0.0; 3], 0.0);
        for k in 0..=2 * FAIR_REACH {
            let c = (x + a.width + k - FAIR_REACH) % a.width;
            if let Some((bp, bn)) = base_at(a, hide, zero, c, p.across) {
                let g = (-0.5 * ((k as f64 - FAIR_REACH as f64) / FAIR_SIGMA).powi(2)).exp();
                for j in 0..3 {
                    b[j] += g * bp[j];
                    n[j] += g * bn[j];
                }
                wsum += g;
            }
        }
        if wsum <= 0.0 {
            return h / PAINT_MM;
        }
        let nl = n.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
        let target: [f64; 3] = std::array::from_fn(|j| b[j] / wsum + h * n[j] / nl);
        (0..3).map(|j| (target[j] - s.p[j]) * s.n[j]).sum::<f64>().max(0.0) / PAINT_MM
    })
}

/// A raven's contour-feather row spacing `o` mm along its body: 1.25 at the neck to 2.4 on the back.
fn pitch(o: f64) -> f64 {
    1.25 + 1.15 * smoother(7.0, 19.0, o)
}

/// Rows each lane lags the one inside it, so a row's tips run back from the crest as a chevron.
const CHEVRON: f64 = 0.25;
/// How far a feather's tip sweeps back at its lane's outer edge, as a share of its row.
const TIP_SWEEP: f64 = 0.5;
/// Height each feather rises from its root to its tip, and so steps down at its tip, mm.
const SAW_MM: f64 = 0.22;
/// Height each lane stands below the one inside it, mm.
const LANE_DROP: f64 = 0.28;
/// Length of each tail along the body, mm.
const TAIL_LEN: f64 = 10.5;
/// Outer edges of the central rectrix and each pair outside it, mm off the crest.
const RECTRIX: [f64; 4] = [0.0, 0.6, 1.8, 3.0];
/// Height of the coverts bed between the tails, mm.
const COVERT_MM: f64 = 0.3;
/// Extra height the necks gather to meet the collet's foot, mm.
const COLLAR_MM: f64 = 1.1;

/// One contour feather under a point: lane, share across it, rachis offset and half-width, phase from root to tip, and where its tip ends.
struct Contour {
    lane: f64,
    within: f64,
    centre: f64,
    half: f64,
    t: f64,
    end: f64,
    pitch: f64,
}

/// A raven's body plumage: graded contour feathers in chevron rows and a wedge tail, along `o`.
struct Plumage {
    starts: Vec<f64>,
    last: f64,
    palm: f64,
}

impl Plumage {
    fn new(palm: f64) -> Self {
        let mut starts = vec![-HEAD.nape - 0.8];
        while starts[starts.len() - 1] < palm + 3.0 {
            let o = starts[starts.len() - 1];
            starts.push(o + pitch(o));
        }
        let last = starts.iter().position(|s| *s >= palm - TAIL_LEN + 1.2).unwrap_or(starts.len() - 1) as f64;
        Self { starts, last, palm }
    }

    /// Rows from the first start to `o`, fractional.
    fn rows(&self, o: f64) -> f64 {
        let j = self.starts.partition_point(|s| *s <= o).clamp(1, self.starts.len() - 1) - 1;
        let (a, b) = (self.starts[j], self.starts[j + 1]);
        j as f64 + (o - a) / (b - a)
    }

    /// The contour feather at `(o, x)`, `x` the distance off the crest.
    fn contour(&self, o: f64, x: f64) -> Option<Contour> {
        let p = pitch(o);
        let lw = 0.9 * p;
        let lane = if x < CREST_FLAT + 0.5 * lw { 0.0 } else { ((x - CREST_FLAT) / lw + 0.5).floor() };
        let (inner, outer) = if lane == 0.0 { (CREST_FLAT, CREST_FLAT + 0.5 * lw) } else { (CREST_FLAT + (lane - 0.5) * lw, CREST_FLAT + (lane + 0.5) * lw) };
        let within = ((x - inner) / (outer - inner)).clamp(0.0, 1.0);
        let n = self.rows(o) - CHEVRON * lane;
        let (mut row, mut t) = (n.floor(), n - n.floor());
        let end = 1.0 - TIP_SWEEP * within.powf(1.6);
        if t > end {
            row += 1.0;
            t -= 1.0;
        }
        if row < 0.0 || row >= self.last {
            return None;
        }
        let (centre, half) = if lane == 0.0 { (0.0, outer) } else { (CREST_FLAT + lane * lw, 0.5 * lw) };
        Some(Contour { lane, within, centre, half, t, end, pitch: p })
    }

    /// Contour-feather height at `(o, x)`; `calm` near 1 flattens the lanes and rows so the necks climb the collet's foot.
    fn crown_mm(&self, o: f64, x: f64, calm: f64) -> f64 {
        let Some(f) = self.contour(o, x) else { return 0.0 };
        let top = HEAD_FLOOR + 0.25 * smoother(16.0, 22.0, o);
        (top - LANE_DROP * (1.0 - 0.5 * calm) * f.lane - 0.05 * f.within - SAW_MM * (1.0 - 0.6 * calm) * (1.0 - f.t)).max(0.0)
    }

    /// The rectrix at `(o, x)`: its index, rachis offset, half-width, share across it and phase from root to tip.
    fn rectrix(&self, o: f64, x: f64) -> Option<(usize, f64, f64, f64, f64)> {
        let k = RECTRIX.windows(2).position(|e| x >= e[0] && x < e[1])?;
        let (inner, outer) = (if k == 0 { CREST_FLAT } else { RECTRIX[k] }, RECTRIX[k + 1]);
        let within = ((x - inner) / (outer - inner)).clamp(0.0, 1.0);
        let root = self.palm - TAIL_LEN;
        let tip = self.palm - 0.3 - 1.9 * k as f64;
        let end = if k == 0 { tip - 0.45 * smoother(CREST_FLAT, 0.6, x) } else { tip - 0.9 * within.powf(1.6) };
        if o < root || o > end {
            return None;
        }
        let centre = if k == 0 { 0.0 } else { 0.5 * (inner + outer) };
        let half = if k == 0 { outer } else { 0.5 * (outer - inner) };
        Some((k, centre, half, within, (o - root) / (tip - root)))
    }

    fn tail_mm(&self, o: f64, x: f64) -> f64 {
        let bed = if o > self.palm - 9.0 && x < RECTRIX[3] { COVERT_MM } else { 0.0 };
        match self.rectrix(o, x) {
            Some((k, _, _, within, t)) => (0.95 - SAW_MM * k as f64 + 0.05 * t - 0.04 * within).max(bed),
            None => bed,
        }
    }

    /// One raven's plumage at `q`, mm: its neck on its own arm's half at the crossing, contour feathers and its tail.
    fn bird_mm(&self, q: On, sign: f64, calm: f64) -> f64 {
        if q.o < -HEAD.nape - 0.8 || (q.o < COLLET_CLEAR && q.w * sign < -CREST_FLAT) {
            return 0.0;
        }
        let x = q.w.abs();
        self.crown_mm(q.o, x, calm).max(self.tail_mm(q.o, x))
    }
}

/// Along the crest past the collet, mm from the top: where each neck takes the whole crown.
const COLLET_CLEAR: f64 = 5.4;

/// The collet's footprint in hide millimetres, as semi-axes along and across the crest.
fn collet_semi(d: &RingDesign) -> Result<(f64, f64)> {
    let (stone, f) = ringdesign_core::stones::stone_frames(d).into_iter().next().ok_or_else(|| anyhow::anyhow!("No onyx"))?;
    let wall = setting::collet_wall_mm(stone.gem) + 0.03;
    Ok((f.semi.0 + wall, f.semi.1 + wall))
}

/// Distance outside the collet's footprint, mm, flat across the crest's own flat.
fn past_collet(p: &skin::HidePoint, semi: (f64, f64)) -> f64 {
    let x = (p.across.abs() - CREST_FLAT).max(0.0);
    let rho = (p.along / semi.0).hypot(x / semi.1);
    (rho - 1.0) * semi.0.min(semi.1)
}

/// Height of body plumage at alpha 1, mm.
const PLUME_MM: f64 = 2.0;

/// Both ravens' body plumage on the atlas, as a share of [`PLUME_MM`].
fn paint_plumage(a: &Atlas, hide: &Hide, zero: &[f64], semi: (f64, f64)) -> Alpha {
    let palm = hide.reach();
    let plume = Plumage::new(palm);
    let value = |p: skin::HidePoint| {
        let near = 1.0 - smoother(0.0, 1.6, past_collet(&p, semi));
        let gather = COLLAR_MM * near;
        let edge = 1.0 - smooth(p.rim - 0.1, p.rim + 0.45 * p.wall, p.across.abs());
        BIRDS
            .iter()
            .map(|&(_, sign)| {
                let v = plume.bird_mm(on_bird(p, sign, palm), sign, near);
                if v > 0.0 { v + gather } else { 0.0 }
            })
            .fold(0.0, f64::max)
            * edge
    };
    a.paint("Raven plumage", |s| value(hide_at(hide, zero, s)) / PLUME_MM)
}

/// Height of the collet's bed at alpha 1, mm.
const BED_MM: f64 = 1.6;

/// The collet's bed: filled up to its base, falling 3.5 deg across, ending just past its wall.
fn paint_bed(d: &RingDesign, a: &Atlas, hide: &Hide, semi: (f64, f64)) -> Result<Alpha> {
    let (stone, f) = ringdesign_core::stones::stone_frames(d).into_iter().next().ok_or_else(|| anyhow::anyhow!("No onyx"))?;
    let depth = setting::collet_depth_mm(stone.gem);
    let base: [f64; 3] = std::array::from_fn(|k| f.girdle[k] - f.normal[k] * depth);
    let draft = 3.5_f64.to_radians().tan();
    Ok(a.paint("Collet bed", |s| {
        let p = hide.at(s);
        let rel: [f64; 3] = std::array::from_fn(|k| s.p[k] - base[k]);
        let below = -rel.iter().zip(f.normal).map(|(a, b)| a * b).sum::<f64>();
        let lift = (below + 0.15 - draft * (s.p[2].abs() - CREST_FLAT).max(0.0)).max(0.0);
        let fade = 1.0 - smoother(0.0, 0.25, past_collet(&p, semi));
        lift * fade / BED_MM
    }))
}

/// Ring angle of the atlas column whose crest stands nearest `along`.
fn theta_at(a: &Atlas, hide: &Hide, along: f64) -> f64 {
    let x = (0..a.width).min_by(|p, q| (hide.along[*p] - along).abs().total_cmp(&(hide.along[*q] - along).abs())).unwrap_or(0);
    a.at(x, 0).theta
}

/// The chart point `along` mm from the top on the crest and `across` mm off it, measured from the exact parting line.
fn chart_at(a: &Atlas, hide: &Hide, zero: &[f64], along: f64, across: f64) -> (f64, f64) {
    let x = column(a, theta_at(a, hide, along));
    let y = (0..a.height).min_by(|p, q| (hide.across[*p * a.width + x] - zero[x] - across).abs().total_cmp(&(hide.across[*q * a.width + x] - zero[x] - across).abs())).unwrap_or(0);
    let s = a.at(x, y);
    (s.theta, s.v)
}

/// The atlas column nearest ring angle `theta`.
fn column(a: &Atlas, theta: f64) -> usize {
    ((theta.rem_euclid(360.0) / 360.0 * a.width as f64).round() as usize) % a.width
}

/// Chart `v` where the surface facing `side` (+1 high z, -1 low z) by more than `facing` reaches radius `r` in column `x`.
fn face_v(a: &Atlas, x: usize, side: f64, r: f64, facing: f64) -> Option<f64> {
    let rows: Vec<(f64, f64)> = (0..a.height)
        .map(|y| a.at(x, y))
        .filter(|s| s.n[2] * side > facing)
        .map(|s| (s.p[0].hypot(s.p[1]), s.v))
        .collect();
    rows.windows(2).find_map(|w| {
        let ((r0, v0), (r1, v1)) = (w[0], w[1]);
        ((r0 - r) * (r1 - r) <= 0.0 && (r1 - r0).abs() > 1e-9 && (v1 - v0).abs() < 0.2).then(|| v0 + (v1 - v0) * (r - r0) / (r1 - r0))
    })
}

/// Whether the surface facing `side` by more than `facing` spans radii `lo..hi` in every column over `t0..t1` degrees.
fn face_spans(a: &Atlas, t0: f64, t1: f64, side: f64, lo: f64, hi: f64, facing: f64) -> bool {
    let steps = ((t1 - t0).abs() / 0.25).ceil().max(1.0) as usize;
    (0..=steps).all(|k| {
        let x = column(a, t0 + (t1 - t0) * k as f64 / steps as f64);
        face_v(a, x, side, lo, facing).is_some() && face_v(a, x, side, hi, facing).is_some()
    })
}

/// A feather outline laid along the ring's arc at radius `r0`, tip toward increasing angle when `tip` is +1, in the frame of the face on `side`.
fn bent(shape: &[P2], r0: f64, tip: f64, side: f64) -> Vec<P2> {
    let s = if side > 0.0 { -1.0 } else { 1.0 };
    let mut out: Vec<P2> = shape
        .iter()
        .map(|&[u, w]| {
            let (phi, rho) = (tip * u / r0, r0 + w);
            [rho * phi.sin(), s * (rho * phi.cos() - r0)]
        })
        .collect();
    if tip * s < 0.0 {
        out.reverse();
    }
    out
}

/// A feather struck along the pull on the surface facing `side` by more than `facing`, centred at `theta` and radius `r0`, its top rising from `rise.0` at the root to `rise.1` at the tip.
#[allow(clippy::too_many_arguments)]
fn side_feather(a: &Atlas, name: String, theta: f64, side: f64, r0: f64, tip: f64, shape: &[P2], rise: (f64, f64), facing: f64) -> Option<Stamp> {
    let len = shape.iter().map(|p| p[0].abs()).fold(0.0, f64::max);
    let wid = shape.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
    let span = (len / r0).to_degrees();
    if !face_spans(a, theta - span, theta + span, side, r0 - wid - 0.1, r0 + wid + 0.1, facing) {
        return None;
    }
    let v = face_v(a, column(a, theta), side, r0, facing)?;
    Some(Stamp {
        name,
        theta_deg: theta,
        v_mm: v,
        rot_deg: 0.0,
        outline: bent(shape, r0, tip, side),
        height_mm: rise.0,
        sink_mm: 0.4,
        draft_deg: 3.0,
        cut: false,
        bench: false,
        along_pull: true,
        tier: 0,
        top: StampTop::Taper { axis_deg: if tip > 0.0 { 0.0 } else { 180.0 }, tip_mm: rise.1 },
    })
}

/// One raven's struck beak on the parting line, pointing away from the stone, its culmen a ridge falling to the tip, and its two bench-cut almond eyes.
fn raven_head(a: &Atlas, hide: &Hide, zero: &[f64], name: &str, sign: f64) -> Vec<Stamp> {
    let h = HEAD;
    let mut out = Vec::new();
    let eye = banded(-0.55, 0.55, |x| 0.35 * (1.0 - (x / 0.55).powi(2)).max(0.0).powf(0.75), |x| -0.35 * (1.0 - (x / 0.55).powi(2)).max(0.0).powf(0.75));
    let along = sign * (h.nape + EYE_U);
    let col = column(a, theta_at(a, hide, along));
    for (label, side) in [("high", 1.0), ("low", -1.0)] {
        let ex = h.eye_x(room(hide.rim[col][usize::from(side > 0.0)]));
        let (theta, v) = chart_at(a, hide, zero, along, side * ex);
        out.push(Stamp {
            name: format!("{name}, eye {label}"),
            theta_deg: theta,
            v_mm: v,
            rot_deg: 0.0,
            outline: eye.clone(),
            height_mm: 0.6,
            sink_mm: 0.22,
            draft_deg: 0.0,
            cut: true,
            bench: true,
            along_pull: false,
            tier: 0,
            top: StampTop::Flat,
        });
    }
    out
}

/// One raven's throat hackles: shingled lances under its skull and beak on both side faces, pointing back toward the nape.
fn raven_hackles(a: &Atlas, hide: &Hide, name: &str, sign: f64) -> Vec<Stamp> {
    let shape = outline::lanceolate(2.0, 0.6, 0.1);
    let mut out = Vec::new();
    for (label, side) in [("high", 1.0), ("low", -1.0)] {
        for (i, u) in [0.9, 1.8, 2.7, 3.6, 4.5, 5.4, 6.3].iter().enumerate() {
            let theta = theta_at(a, hide, sign * (HEAD.nape + u));
            let r0 = 9.88 + 0.16 * (i % 2) as f64;
            if let Some(st) = side_feather(a, format!("{name}, hackle {label} {}", i + 1), theta, side, r0, -sign, &shape, (0.22, 0.38), 0.9) {
                out.push(st);
            }
        }
    }
    out
}

/// One raven's folded wings: seven quills shingled along each side face of its shoulder, tips toward the palm.
fn raven_wings(a: &Atlas, name: &str, sign: f64) -> Vec<Stamp> {
    let (len, r0) = (8.5, 9.95);
    let shape = outline::quill(len, 1.0, 0.12);
    let half = (0.5 * len / r0).to_degrees();
    let (root, tipmost) = if sign > 0.0 { (26.0, 271.5 - 360.0) } else { (154.0, 268.5) };
    let (c0, c1) = (root - sign * half, tipmost + sign * half);
    let mut out = Vec::new();
    for (label, side) in [("high", 1.0), ("low", -1.0)] {
        for i in 0..7 {
            let theta = c0 + (c1 - c0) * i as f64 / 6.0;
            if let Some(st) = side_feather(a, format!("{name}, primary {label} {}", i + 1), theta, side, r0, -sign, &shape, (0.25, 0.45), 0.9) {
                out.push(st);
            }
        }
    }
    out
}

/// One raven's neck feathers above the seam channel on its own arm's side face under the collet, pointing toward its body.
fn raven_neck(a: &Atlas, name: &str, sign: f64) -> Vec<Stamp> {
    let shape = outline::lanceolate(1.4, 0.5, 0.08);
    let (from, to) = if sign > 0.0 { (100.0, 60.0) } else { (80.0, 120.0) };
    (0..10)
        .filter_map(|i| {
            let theta = from + (to - from) * i as f64 / 9.0;
            side_feather(a, format!("{name}, neck {}", i + 1), theta, sign, 10.92, -sign, &shape, (0.2, 0.32), 0.45)
        })
        .collect()
}

/// Depth of the graver's work at alpha 1, mm.
const BARB_MM: f64 = 0.1;
/// Share of the graver's depth the barbs, rachises and bristles take.
const FINE: f64 = 0.7;
/// How far a barb trails back toward the root per mm it runs out from the rachis.
const BARB_SLOPE: f64 = 1.0;

/// Barbs trailing back from a rachis, `b` mm across a vane `half` mm wide, chevrons pointing along +`o`, `pitch` apart.
fn vane(o: f64, b: f64, half: f64, pitch: f64) -> f64 {
    let across = b.abs();
    if across < 0.05 || across > half - 0.08 {
        return 0.0;
    }
    let phase = (o + BARB_SLOPE * across) / pitch;
    let d = (phase - phase.round()).abs() * pitch / (1.0 + BARB_SLOPE * BARB_SLOPE).sqrt();
    0.8 * (1.0 - smooth(0.03, 0.06, d))
}

/// A cut `half` mm either side of a line, `d` off it.
fn line(d: f64, half: f64) -> f64 {
    1.0 - smooth(half * 0.6, half, d.abs())
}

/// Bench cuts: a rachis and barbs in every contour feather and rectrix, and each beak's nasal bristles.
fn paint_barbs(a: &Atlas, hide: &Hide, zero: &[f64]) -> Alpha {
    let palm = hide.reach();
    let plume = Plumage::new(palm);
    a.paint("Graver's work", |s| {
        let p = hide_at(hide, zero, s);
        let mut cut: f64 = 0.0;
        let heads = BIRDS.iter().any(|&(_, sign)| {
            let q = on_bird(p, sign, palm);
            let u = -q.o - HEAD.nape;
            (0.0..=HEAD.root + HEAD.beak + HOOK).contains(&u) && q.w.abs() < HEAD.head(u, q.w.abs(), room(q.rim)).1 + 0.15
        });
        for (_, sign) in BIRDS {
            let q = on_bird(p, sign, palm);
            let u = -q.o - HEAD.nape;
            if (0.0..=HEAD.root + HEAD.beak + HOOK).contains(&u) && q.w.abs() < HEAD.head(u, q.w.abs(), room(q.rim)).1 + 0.15 {
                if (HEAD.root - 0.75..HEAD.root + 0.05).contains(&u) {
                    for w0 in [-0.75, -0.45, -0.15, 0.15, 0.45, 0.75] {
                        let fan = w0 * (1.0 + 0.35 * (u - HEAD.root + 0.75));
                        cut = cut.max(FINE * line(q.w - fan, 0.05) * smooth(HEAD.root - 0.75, HEAD.root - 0.6, u));
                    }
                }
                let end = HEAD.root + HEAD.beak - 0.2;
                if (RICTUS_U..end).contains(&u) {
                    let fade = smooth(RICTUS_U, RICTUS_U + 0.25, u) * (1.0 - smooth(end - 0.6, end, u));
                    cut = cut.max(line(q.w.abs() - HEAD.gape_x(u, room(q.rim)), 0.065) * fade);
                }
                continue;
            }
            if heads || plume.bird_mm(q, sign, 0.0) <= 0.0 {
                continue;
            }
            let x = q.w.abs();
            if let Some((_, centre, half, _, t)) = plume.rectrix(q.o, x) {
                let b = x - centre;
                if t > 0.2 && t < 0.97 {
                    cut = cut.max(FINE * line(b, 0.06));
                }
                cut = cut.max(FINE * vane(q.o, b, half, 0.42));
            } else if let Some(f) = plume.contour(q.o, x) {
                let b = x - f.centre;
                if f.t > 0.1 && f.t < f.end - 0.12 {
                    cut = cut.max(FINE * line(b, 0.055));
                }
                if f.pitch > 1.6 {
                    cut = cut.max(FINE * vane(q.o + 0.21 * f.lane, b, f.half, 0.3 + 0.12 * (f.pitch - 1.0)) * smooth(0.0, 0.1, f.t));
                }
            }
        }
        cut
    })
}

/// Atlas columns round the ring.
const ATLAS_W: usize = 2048;

/// Atlas rows across the section for square texels.
fn atlas_rows(d: &RingDesign) -> usize {
    let ctx = d.field_context();
    ((ATLAS_W as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize).max(64)
}

/// Separable box blur of `r` columns and `s` rows, wrapping round the ring.
fn blur(alpha: &mut Alpha, r: usize, s: usize) {
    let (w, h) = (alpha.width, alpha.height);
    let src = alpha.data.clone();
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for k in 0..=2 * r {
                acc += src[y * w + (x + w + k - r) % w];
            }
            tmp[y * w + x] = acc / (2 * r + 1) as f32;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let (lo, hi) = (y.saturating_sub(s), (y + s).min(h - 1));
            let acc: f32 = (lo..=hi).map(|k| tmp[k * w + x]).sum();
            alpha.data[y * w + x] = acc / (hi - lo + 1) as f32;
        }
    }
}

/// Blur, draft-clamp and store one painted layer, and add it over the whole chart.
fn painted(d: &mut RingDesign, lib: &mut AlphaLibrary, a: &Atlas, mut alpha: Alpha, height: f64, rows: usize) -> Result<skin::ClampReport> {
    blur(&mut alpha, 3, rows);
    let clamp = skin::draft_clamp(a, &mut alpha, height)?;
    println!("  {}: clamp {:.4} mm over {} texels", alpha.name, clamp.worst_mm, clamp.texels_cut);
    lib.insert(Alpha::from_png16(alpha.name.clone(), &alpha.to_png16()?)?);
    d.layers.layers.push(skin::hide_layer(d, &alpha.name, height, Window::around(90.0, 360.0)));
    Ok(clamp)
}

/// Every painted layer's draft-clamp report, by name.
type Clamps = Vec<(String, skin::ClampReport)>;

/// The whole ring, with each painted layer written to `art`.
fn author(art: Option<&Path>) -> Result<(RingDesign, AlphaLibrary, Clamps)> {
    let mut d = band();
    seat(&mut d)?;
    let a = Atlas::of(&d, ATLAS_W, atlas_rows(&d))?;
    let mut hide = Hide::of(&a);
    steady(&mut hide);
    let mut lib = AlphaLibrary::default();
    let mut clamps = Vec::new();
    let zero = zero_across(&a, &hide);
    let lean = Lean::of(&a, &hide, &zero);
    let semi = collet_semi(&d)?;
    for (alpha, height, rows) in [
        (paint_bed(&d, &a, &hide, semi)?, BED_MM, 2),
        (paint_plumage(&a, &hide, &zero, semi), PLUME_MM, 2),
        (paint_heads(&a, &hide, &zero, &lean), PAINT_MM, 0),
    ] {
        let name = alpha.name.clone();
        let clamp = painted(&mut d, &mut lib, &a, alpha, height, rows)?;
        if let Some(art) = art {
            let slug = name.to_lowercase().replace(' ', "-");
            std::fs::write(art.join(format!("{slug}.png")), lib.get(&name).map(|x| x.to_png16()).transpose()?.unwrap_or_default())?;
        }
        clamps.push((name, clamp));
    }
    if std::env::var("CORVUS_DISP").is_ok() {
        let heads = paint_heads(&a, &hide, &zero, &lean);
        for w in [0.0, 0.5, 1.0, 1.5, 2.0] {
            for k in 0..=16 {
                let t = 116.0 + k as f64;
                let x = column(&a, t);
                let y = (0..a.height).min_by(|p, q| (hide.across[*p * a.width + x] - zero[x] - w).abs().total_cmp(&(hide.across[*q * a.width + x] - zero[x] - w).abs())).unwrap_or(0);
                let s = a.at(x, y);
                let h = heads.data[y * a.width + x] as f64 * PAINT_MM;
                let dz = s.p[2] + h * s.n[2];
                let dr = s.p[0].hypot(s.p[1]) + h * (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / s.p[0].hypot(s.p[1]);
                println!("    w {w:.1} th {t:.0}: B r {:.2} z {:+.2} nz {:+.2} h {h:.2} -> D r {dr:.2} z {dz:+.2}", s.p[0].hypot(s.p[1]), s.p[2], s.n[2]);
            }
        }
    }
    let barbs = paint_barbs(&a, &hide, &zero);
    lib.insert(Alpha::from_png16(barbs.name.clone(), &barbs.to_png16()?)?);
    if let Some(art) = art {
        std::fs::write(art.join("graver-work.png"), barbs.to_png16()?)?;
    }
    let mut e = skin::hide_layer(&d, &barbs.name, BARB_MM, Window::around(90.0, 360.0));
    e.blend = ringdesign_core::Blend::Subtract;
    e.bench_only = true;
    d.layers.layers.push(e);
    for (name, sign) in BIRDS {
        d.stamps.extend(raven_head(&a, &hide, &zero, name, sign));
        d.stamps.extend(raven_hackles(&a, &hide, name, sign));
        d.stamps.extend(raven_wings(&a, name, sign));
        d.stamps.extend(raven_neck(&a, name, sign));
    }
    if std::env::var("CORVUS_SEC").is_ok() {
        for t in [105.0, 112.0, 118.0, 122.0, 124.0, 126.0, 128.0, 132.0, 136.0, 142.0, 150.0] {
            let x = column(&a, t);
            let pts: Vec<String> = [-2.5, -2.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5]
                .iter()
                .map(|w: &f64| {
                    let y = (0..a.height).min_by(|p, q| (hide.across[*p * a.width + x] - zero[x] - w).abs().total_cmp(&(hide.across[*q * a.width + x] - zero[x] - w).abs())).unwrap_or(0);
                    let s = a.at(x, y);
                    format!("{w:+.1}:({:.2},{:+.2},n{:+.2})", s.p[0].hypot(s.p[1]), s.p[2], s.n[2])
                })
                .collect();
            println!("    th {t}: {}", pts.join(" "));
        }
    }
    if std::env::var("CORVUS_HEAD").is_ok() {
        for (name, sign) in BIRDS {
            for k in 0..26 {
                let u = 0.5 * k as f64;
                let along = sign * (HEAD.nape + u);
                let x = column(&a, theta_at(&a, &hide, along));
                let (rl, rh) = (hide.rim[x][0], hide.rim[x][1]);
                println!("    {name} u {u:.1} theta {:.1} rim lo {rl:.2} hi {rh:.2} wall {:.2}/{:.2} skull_half {:.2} head half lo {:.2} hi {:.2} crest {:.2}", a.at(x, 0).theta, hide.wall[x][0], hide.wall[x][1], HEAD.skull_half(u), HEAD.head(u, 0.0, room(rl)).1, HEAD.head(u, 0.0, room(rh)).1, HEAD.head_mm(u, 0.0, room(rh)));
            }
        }
    }
    let ctx = d.field_context();
    for s in d.stamps.iter().filter(|s| s.along_pull) {
        let f = s.frame(&d, &ctx);
        let r = f.origin[0].hypot(f.origin[1]);
        let out = (f.y[0] * f.origin[0] + f.y[1] * f.origin[1]) / r;
        ensure!((out + f.z[2]).abs() < 0.2, "{}: frame y leans {out:.2} against face {:.2}", s.name, f.z[2]);
    }
    Ok((d, lib, clamps))
}

fn solid(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn checked_part(name: &str, part: &csg::Solid) -> serde_json::Value {
    let c = part.check(true);
    json!({"name": name, "vertices": part.v.len(), "faces": part.f.len(), "open_edges": c.open_edges, "repeated_edges": c.repeated_edges,
        "degenerate_faces": c.zero_area_faces, "self_crossings": c.self_crossings, "volume_mm3": c.volume})
}

/// Every made part as placed: each stamp on the ring built to its tier, and the collet.
fn made_parts(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams) -> Result<Vec<serde_json::Value>> {
    let ctx = d.field_context();
    let mut parts = Vec::new();
    let tiers: std::collections::BTreeSet<_> = d.stamps.iter().map(|s| s.tier).collect();
    for tier in tiers {
        let mut prior = d.clone();
        prior.stamps.retain(|s| s.tier < tier);
        let staged = mesh::try_build(&prior, lib, p)?;
        ensure!(staged.solids.notes.is_empty(), "Tier {tier}'s ground did not resolve: {:?}", staged.solids.notes);
        let on = solid(&staged.mesh);
        for s in d.stamps.iter().filter(|s| s.tier == tier) {
            let part = s.solid(&s.frame(d, &ctx), &on).map_err(anyhow::Error::msg)?;
            parts.push(checked_part(&s.name, &part));
        }
    }
    for (stone, _) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let uv = ringdesign_core::field::Uv { u: ctx.u_of_theta(stone.theta_deg), v: stone.v_mm };
        let relief = d.layers.height(uv, &ctx, lib);
        let fit = setting::Fit { surface_z: relief - stand, through_mm: None, prongs: stone.seat.prongs };
        let made = setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow::anyhow!("{e}"))?;
        for (phase, solids) in [("head", &made.add), ("cutter", &made.cut)] {
            for (i, part) in solids.iter().enumerate() {
                parts.push(checked_part(&format!("{} {phase} {i}", stone.label), part));
            }
        }
    }
    Ok(parts)
}

/// Median height of the built crest line, mm, from the weighted peak of each half-degree slice.
fn crest_height(m: &mesh::Mesh) -> f64 {
    const BINS: usize = 720;
    let mut top: Vec<Vec<(f64, f64)>> = vec![Vec::new(); BINS];
    for p in &m.vertices {
        let z = p.2 as f64;
        if z.abs() > 0.2 {
            continue;
        }
        let th = (p.1 as f64).atan2(p.0 as f64).to_degrees().rem_euclid(360.0);
        top[((th / 360.0 * BINS as f64) as usize).min(BINS - 1)].push(((p.0 as f64).hypot(p.1 as f64), z));
    }
    let mut peaks: Vec<f64> = top
        .iter_mut()
        .filter(|pts| pts.len() >= 3)
        .map(|pts| {
            pts.sort_by(|a, b| b.0.total_cmp(&a.0));
            let best = pts[0];
            let (sw, sz) = pts.iter().filter(|q| (q.1 - best.1).abs() < 0.09).take(6).fold((0.0, 0.0), |(w, s), q| {
                let k = (q.0 - best.0 + 0.002).max(0.0);
                (w + k, s + k * q.1)
            });
            if sw > 0.0 { sz / sw } else { best.1 }
        })
        .collect();
    peaks.sort_by(|a, b| a.total_cmp(b));
    peaks.get(peaks.len() / 2).copied().unwrap_or(0.0)
}

/// Delft clay inspection setup at sample pitch `pitch`, mm.
fn sand_setup(pitch: f64) -> mf::Setup {
    mf::Setup { recipe: mf::Recipe::sand(SandProcess::DelftClay), sample_pitch_mm: pitch, ..Default::default() }
}

/// Every obstruction as ring angle, radius, height and depth.
fn obstructions(r: &mf::release::ReleaseReport) -> Vec<serde_json::Value> {
    r.obstructions
        .iter()
        .map(|o| json!({"theta": o.world[1].atan2(o.world[0]).to_degrees().rem_euclid(360.0), "r": o.world[0].hypot(o.world[1]), "z": o.world[2], "depth_mm": o.depth_mm, "samples": o.samples}))
        .collect()
}

/// Release of the pattern built at `params`, at 0.100 and 0.075 mm, as a report block and whether both are clear.
fn release_block(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(serde_json::Value, bool)> {
    let insp = mf::inspect(d, lib, &sand_setup(0.1), params)?;
    let fine = mf::release::analyze(&insp.prepared.mesh, &sand_setup(0.075))?;
    let clear = insp.release.obstructions.is_empty() && insp.release.unresolved_rays == 0 && fine.obstructions.is_empty() && fine.unresolved_rays == 0;
    Ok((
        json!({"build": [params.theta_steps, params.profile_steps],
            "release_0_1": {"obstructions": insp.release.obstructions.len(), "unresolved": insp.release.unresolved_rays, "parting_mm": insp.release.parting_mm, "at": obstructions(&insp.release)},
            "release_0_075": {"obstructions": fine.obstructions.len(), "unresolved": fine.unresolved_rays, "parting_mm": fine.parting_mm, "at": obstructions(&fine)}}),
        clear,
    ))
}

/// Plain collet wall standing over the relief just outside it, mm, in each of 16 sectors round the stone.
fn collet_walls(d: &RingDesign, m: &mesh::Mesh) -> Vec<f64> {
    let Some((stone, f)) = ringdesign_core::stones::stone_frames(d).into_iter().next() else { return Vec::new() };
    let wall = setting::collet_wall_mm(stone.gem);
    let (sa, sb) = (f.semi.0 + wall, f.semi.1 + wall);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (mut lip, mut near) = ([f64::MIN; 16], [f64::MIN; 16]);
    for p in &m.vertices {
        let rel = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
        let (x, y, h) = (dot(rel, f.long), dot(rel, f.short), dot(rel, f.normal));
        let rho = (x / sa).hypot(y / sb);
        let k = (((y / sb).atan2(x / sa) / (2.0 * PI) + 1.0) * 16.0) as usize % 16;
        if (0.85..1.0).contains(&rho) {
            lip[k] = lip[k].max(h);
        } else if (1.12..1.4).contains(&rho) {
            near[k] = near[k].max(h);
        }
    }
    (0..16).map(|k| lip[k] - near[k]).collect()
}

/// Least vertex margin outside the bore radius, mm, and the count more than 0.01 mm inside it.
fn bore_intrusion(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    let mut least = f64::MAX;
    let mut inside = 0;
    for p in &m.vertices {
        let margin = (p.0 as f64).hypot(p.1 as f64) - bore;
        least = least.min(margin);
        inside += usize::from(margin < -0.01);
    }
    (least, inside)
}

/// The preview stones welded per tint, and how many separate stones they hold.
fn preview_stones(d: &RingDesign, lib: &AlphaLibrary, b: &mesh::BuildResult) -> (Vec<(mesh::Mesh, [f32; 3])>, usize) {
    let mut count = 0;
    let groups = ringdesign_core::gems::built_meshes(d, lib, b)
        .into_iter()
        .map(|(m, tint)| {
            let mut out = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for f in m.faces {
                let face = f.map(|i| {
                    let p = m.vertices[i as usize];
                    let key = [p.0, p.1, p.2].map(|x| (x * 1e5).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        out.vertices.push(p);
                        (out.vertices.len() - 1) as u32
                    })
                });
                out.faces.push(face);
            }
            let mut parent: Vec<usize> = (0..out.vertices.len()).collect();
            fn root(p: &mut [usize], mut i: usize) -> usize {
                while p[i] != i {
                    p[i] = p[p[i]];
                    i = p[i];
                }
                i
            }
            for f in &out.faces {
                for j in [1, 2] {
                    let a = root(&mut parent, f[0] as usize);
                    let z = root(&mut parent, f[j] as usize);
                    parent[z] = a;
                }
            }
            let roots: std::collections::BTreeSet<usize> = (0..parent.len()).map(|i| root(&mut parent, i)).collect();
            count += roots.len();
            out.normals = vec![mesh::Vec3(0.0, 0.0, 0.0); out.vertices.len()];
            for f in &out.faces {
                let [a, b, c] = f.map(|i| out.vertices[i as usize]);
                let (u, v) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                for i in f {
                    let q = &mut out.normals[*i as usize];
                    q.0 += n[0];
                    q.1 += n[1];
                    q.2 += n[2];
                }
            }
            for n in &mut out.normals {
                let len = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-12);
                *n = mesh::Vec3(n.0 / len, n.1 / len, n.2 / len);
            }
            (out, tint)
        })
        .collect();
    (groups, count)
}

/// Views of the finished ring, yaw and pitch.
const VIEWS: [(&str, f64, f64); 6] =
    [("hero", 0.62, 0.7), ("face", 0.0, PI * 0.5), ("palm", PI, PI * 0.5), ("side", 0.0, 0.0), ("shoulder", -0.95, 0.55), ("reverse", PI + 0.5, 0.35)];

/// The metal as the renders show it, with smooth vertex normals.
fn display(b: &mesh::BuildResult) -> mesh::Mesh {
    let mut m = b.mesh.clone();
    m.corner_normals.clear();
    m
}

/// A three-quarter close-up of Muninn's head from its open side: its crop, yaw and pitch.
fn head_view(d: &RingDesign, m: &mesh::Mesh) -> Result<(mesh::Mesh, f64, f64)> {
    let ctx = d.field_context();
    let eye = d.stamps.iter().find(|s| s.name == "Muninn, eye high").ok_or_else(|| anyhow::anyhow!("No eye"))?;
    let f = eye.frame(d, &ctx);
    let t = f.origin[1].atan2(f.origin[0]) - 0.133;
    let centre = [12.8 * t.cos(), 12.8 * t.sin(), 0.0];
    Ok((crop(m, centre, 8.0), PI * 0.5 - t - 0.55, 0.62))
}

/// Studio-gold renders with the onyx set: six views, 300 px hero and face, a head close-up, the stone close-up and bare against finished.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, b: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])], edge: usize) -> Result<()> {
    let display = display(b);
    let stones = || {
        gems.iter().map(|(m, t)| {
            let mut p = render::Part::tinted_stone(m, *t);
            p.smooth = true;
            p
        })
    };
    let mut parts = vec![render::Part::metal(&display, render::GOLD)];
    parts.extend(stones());
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    for (name, yaw, pitch) in VIEWS[..2].iter() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 4);
        image::save_buffer(out.join(format!("{name}-300.png")), &img, 300, 300, image::ColorType::Rgb8)?;
    }
    let (head, yaw, pitch) = head_view(d, &display)?;
    let mut hparts = vec![render::Part::metal(&head, render::GOLD), render::Part::metal(&display, render::GOLD)];
    hparts.extend(stones());
    render::write_png_parts(out.join("head.png"), &hparts, yaw, pitch, edge)?;
    let f = &ringdesign_core::stones::stone_frames(d)[0].1;
    let close = crop(&display, f.girdle, 9.5);
    let mut detail = vec![render::Part::metal(&close, render::GOLD), render::Part::metal(&display, render::GOLD)];
    detail.extend(stones());
    render::write_png_parts(out.join("stones.png"), &detail, 0.45, 1.05, edge)?;
    let mut bare = band();
    seat(&mut bare)?;
    let bb = mesh::try_build(&bare, lib, draft_params())?;
    let (hero_yaw, hero_pitch) = (VIEWS[0].1, VIEWS[0].2);
    let left = render::render_parts_ss(&[render::Part::metal(&bb.mesh, render::GOLD)], hero_yaw, hero_pitch, edge, edge, 3);
    let right = render::render_parts_ss(&parts, hero_yaw, hero_pitch, edge, edge, 3);
    sheet(&out.join("bare-vs-finished.png"), &[left, right], edge, 2)?;
    Ok(())
}

/// Authoring sheets: head close-ups, the ring at 300 px, the views and candidate hero angles.
fn preview_sheets(out: &Path, d: &RingDesign, b: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])]) -> Result<()> {
    let display = display(b);
    let edge = 700;
    let mut parts = vec![render::Part::metal(&display, render::GOLD)];
    parts.extend(gems.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    let (head, yaw, pitch) = head_view(d, &display)?;
    let mut cparts = vec![render::Part::metal(&head, render::GOLD), render::Part::metal(&display, render::GOLD)];
    cparts.extend(gems.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    let tiles: Vec<Vec<u8>> = [(yaw, pitch), (yaw - 0.6, pitch), (0.0, 0.0), (PI, 0.0), (yaw + 0.55, PI * 0.5), (yaw - 1.2, 0.9)]
        .iter()
        .map(|(y, p)| render::render_parts_ss(&cparts, *y, *p, edge, edge, 2))
        .collect();
    sheet(&out.join("preview-head.png"), &tiles, edge, 3)?;
    let small: Vec<Vec<u8>> = VIEWS.iter().map(|(_, y, p)| render::render_parts_ss(&parts, *y, *p, 300, 300, 3)).collect();
    sheet(&out.join("preview-300.png"), &small, 300, 3)?;
    let big: Vec<Vec<u8>> = VIEWS.iter().map(|(_, y, p)| render::render_parts_ss(&parts, *y, *p, 600, 600, 2)).collect();
    sheet(&out.join("preview-views.png"), &big, 600, 3)?;
    let heroes: Vec<Vec<u8>> = [(0.4, 0.8), (0.6, 0.7), (0.75, 0.62), (0.9, 0.55), (0.6, 0.9), (-0.6, 0.7)]
        .iter()
        .map(|(y, p)| render::render_parts_ss(&parts, *y, *p, 500, 500, 2))
        .collect();
    sheet(&out.join("preview-heroes.png"), &heroes, 500, 3)?;
    Ok(())
}

/// Square RGB tiles laid out `cols` across into one PNG.
fn sheet(path: &Path, tiles: &[Vec<u8>], edge: usize, cols: usize) -> Result<()> {
    let rows = tiles.len().div_ceil(cols);
    let (w, h) = (edge * cols, edge * rows);
    let mut out = vec![0u8; w * h * 3];
    for (k, t) in tiles.iter().enumerate() {
        let (cx, cy) = ((k % cols) * edge, (k / cols) * edge);
        for y in 0..edge {
            let dst = ((cy + y) * w + cx) * 3;
            out[dst..dst + edge * 3].copy_from_slice(&t[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The faces of `m` within `radius` of `centre`, as their own mesh.
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/corvus"));
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let preview = args.iter().any(|a| a == "--preview");
    std::fs::create_dir_all(&out)?;
    println!("Corvus");
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/bestiarium/art/corvus");
    std::fs::create_dir_all(&art)?;
    let (d, lib, clamps) = author(Some(&art))?;
    let clamp_worst = clamps.iter().map(|(_, c)| c.worst_mm).fold(0.0, f64::max);
    let mut monotone = Vec::new();
    for s in &d.stamps {
        if let Err(bad) = s.parting_monotone(&d) {
            monotone.push(format!("{}: {} outline points", s.name, bad.len()));
        }
    }
    println!("  stamps {}, parting-monotone failures {:?}", d.stamps.len(), monotone);
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    println!(
        "  {} x {}: {} triangles in {build_s:.1} s; watertight {}; degenerate {}; notes {:?}; stamped {}",
        params.theta_steps,
        params.profile_steps,
        built.mesh.faces.len(),
        v.watertight,
        q.degenerate_faces,
        built.solids.notes,
        built.solids.stamped
    );
    if std::env::var("CORVUS_BARE").is_ok() {
        let mut bare = band();
        seat(&mut bare)?;
        let f = castability::attributed_field_report(&bare, &AlphaLibrary::default(), &bare.draft, 256, 128);
        println!("    bare: {:?} {:.4}% worst {:.2} parting {:.4} undercut {:.4} mm2 {:?}", f.verdict, f.undercut_fraction() * 100.0, f.worst_draft_deg, f.parting_z_mm, f.undercut_area_mm2, f.notes);
        let mut layers = d.clone();
        layers.stamps.clear();
        for i in 0..layers.layers.layers.len() {
            let mut one = layers.clone();
            for (k, e) in one.layers.layers.iter_mut().enumerate() {
                e.enabled = k == i || k == 0;
            }
            let f = castability::attributed_field_report(&one, &lib, &one.draft, 256, 128);
            println!("    only {}: {:.4}% worst {:.2} parting {:.4} undercut {:.4}", one.layers.layers[i].name, f.undercut_fraction() * 100.0, f.worst_draft_deg, f.parting_z_mm, f.undercut_area_mm2);
        }
        return Ok(());
    }
    if let Ok(t) = std::env::var("CORVUS_ROWS") {
        let t: f64 = t.parse().unwrap_or(50.625);
        let cast = castability::casting_pattern(&d, &lib).0.into_owned();
        for x in [t - 360.0 / 256.0, t, t + 360.0 / 256.0] {
            let sec = castability::section_at(&cast, &lib, x, 128);
            let rows: Vec<String> = (72..90).map(|j| format!("{j}:{:.3}/{:.3}", sec.points[j].r, sec.points[j].z)).collect();
            println!("    rows th {x:.2}: {}", rows.join(" "));
        }
        return Ok(());
    }
    if std::env::var("CORVUS_CREST").is_ok() {
        for t in [15.0, 30.0, 45.0, 55.0, 75.0, 90.0, 97.0, 125.0, 135.0, 150.0, 165.0, 200.0, 230.0, 262.0, 268.0, 300.0, 340.0] {
            for steps in [128usize, 512] {
                let sec = castability::section_at(&d, &lib, t, steps);
                let best = sec.points.iter().filter(|p| p.surface).max_by(|a, b| a.r.total_cmp(&b.r));
                if let Some(b) = best {
                    let near: Vec<String> = sec.points.iter().filter(|p| p.surface && p.r > b.r - 0.004).map(|p| format!("{:.3}", p.z)).collect();
                    println!("    crest th {t} steps {steps}: r {:.4} z {:.4} within 4um at z [{}]", b.r, b.z, near.join(" "));
                }
            }
        }
        return Ok(());
    }
    if let Ok(range) = std::env::var("CORVUS_UNDER") {
        let (t0, t1): (f64, f64) = range.split_once(',').map(|(a, b)| (a.parse().unwrap_or(20.0), b.parse().unwrap_or(60.0))).unwrap_or((20.0, 60.0));
        let parting: f64 = std::env::var("CORVUS_PARTING").ok().and_then(|v| v.parse().ok()).unwrap_or(-0.0255);
        let (steps, dt): (usize, f64) = (128, 360.0 / 256.0);
        let cast = castability::casting_pattern(&d, &lib).0.into_owned();
        let pt = |sec: &castability::Section, j: usize| {
            let p = &sec.points[j];
            let (s, c) = sec.theta_deg.to_radians().sin_cos();
            [p.r * c, p.r * s, p.z]
        };
        let mut t = t0;
        while t <= t1 {
            let secs: Vec<castability::Section> = [t - dt, t, t + dt].iter().map(|&x| castability::section_at(&cast, &lib, x, steps)).collect();
            let rows = secs[1].points.len();
            for j in 1..rows - 1 {
                if !secs[1].points[j].surface {
                    continue;
                }
                let (a, b) = (pt(&secs[2], j), pt(&secs[0], j));
                let (c, e) = (pt(&secs[1], j + 1), pt(&secs[1], j - 1));
                let tu = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                let ts = [c[0] - e[0], c[1] - e[1], c[2] - e[2]];
                let n = [tu[1] * ts[2] - tu[2] * ts[1], tu[2] * ts[0] - tu[0] * ts[2], tu[0] * ts[1] - tu[1] * ts[0]];
                let draft = castability::draft_angle(n, secs[1].points[j].z, parting);
                if draft < -0.5 {
                    let p = &secs[1].points[j];
                    let area = (tu[0] * ts[0] + tu[1] * ts[1] + tu[2] * ts[2]).abs().max(0.0) * 0.0 + (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() * 0.25;
                    println!("    under th {t:.2} j {j} z {:.3} r {:.3} draft {draft:.2} area {area:.4}", p.z, p.r);
                }
            }
            t += dt;
        }
        return Ok(());
    }
    if std::env::var("CORVUS_BISECT").is_ok() {
        for family in ["beak", "eye", "hackle", "primary", "neck"] {
            let mut part = d.clone();
            part.stamps.retain(|s| s.name.contains(family));
            let b = mesh::try_build(&part, &lib, params)?;
            println!("    {family}: {} stamps, crossings {}, notes {:?}", part.stamps.len(), csg::self_crossings(&solid(&b.mesh)), b.solids.notes);
        }
        let mut bare = d.clone();
        bare.stamps.clear();
        let b = mesh::try_build(&bare, &lib, params)?;
        println!("    no stamps: crossings {}", csg::self_crossings(&solid(&b.mesh)));
        return Ok(());
    }
    let (gems, previewed) = preview_stones(&d, &lib, &built);
    if preview {
        preview_sheets(&out, &d, &built, &gems)?;
        println!("  wrote preview sheets");
        return Ok(());
    }
    let crossings = csg::self_crossings(&solid(&built.mesh));
    let (bore_margin, bore_inside) = bore_intrusion(&d, &built.mesh);
    let made = made_parts(&d, &lib, params)?;
    let made_ok = made.iter().all(|p| p["open_edges"] == 0 && p["repeated_edges"] == 0 && p["degenerate_faces"] == 0 && p["self_crossings"] == 0);
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let findings = dfm::findings_in(&d, &lib);
    let setup = sand_setup(0.1);
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &sand_setup(0.075))?;
    let crest_z = crest_height(&built.mesh);
    let walls = collet_walls(&d, &built.mesh);
    println!("  collet wall over its surroundings: {:?}", walls.iter().map(|w| (w * 100.0).round() / 100.0).collect::<Vec<_>>());
    let pattern = &inspection.prepared.mesh;
    let pattern_check = solid(pattern).check(true);
    let stamps_small = {
        let small = mesh::try_build(&d, &lib, stamp_params())?;
        let (mut block, clear) = release_block(&d, &lib, stamp_params())?;
        let crossings = csg::self_crossings(&solid(&small.mesh));
        block["notes"] = json!(small.solids.notes);
        block["stamped"] = json!(small.solids.stamped);
        block["watertight"] = json!(small.report.validation.watertight);
        block["degenerate_faces"] = json!(small.report.quality.degenerate_faces);
        block["self_crossings"] = json!(crossings);
        block["passed"] = json!(
            clear
                && small.solids.notes.is_empty()
                && small.solids.stamped == d.stamps.len()
                && small.report.validation.watertight
                && small.report.quality.degenerate_faces == 0
                && crossings == 0
        );
        block
    };
    let draft_block = if draft { None } else { Some(release_block(&d, &lib, draft_params())?) };
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let stones_count = stones_report.as_ref().map_or(0, |s| s.stone_count as usize);
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let again = mesh::try_build(&saved, &cold_lib, params)?;
        let same = again.mesh.vertices == built.mesh.vertices && again.mesh.faces == built.mesh.faces && again.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let triangles_ok = built.mesh.faces.len() <= 2_000_000;
    let gates = json!({
        "watertight": v.watertight && q.degenerate_faces == 0,
        "self_crossings": crossings == 0 && made_ok,
        "solids_notes_empty": built.solids.notes.is_empty(),
        "stamps_resolved": built.solids.stamped == d.stamps.len() && monotone.is_empty(),
        "bore_clear": bore_inside == 0,
        "field_castable": field.verdict == castability::Verdict::Castable,
        "release_0_1": inspection.release.obstructions.is_empty() && inspection.release.unresolved_rays == 0,
        "release_0_075": release_fine.obstructions.is_empty() && release_fine.unresolved_rays == 0,
        "release_draft": draft_block.as_ref().is_none_or(|b| b.1),
        "clamp_bite": clamp_worst <= 0.05,
        "dfm_zero": findings.is_empty(),
        "stones_match": stones_count == previewed,
        "cold_reload": cold != Some(false),
        "pattern_mesh": pattern_check.open_edges == 0 && pattern_check.zero_area_faces == 0 && pattern_check.self_crossings == Some(0),
        "triangle_budget": triangles_ok,
        "stamps_384": stamps_small["passed"] == true,
    });
    let passed = gates.as_object().is_some_and(|g| g.values().all(|v| v == true));
    let report = json!({
        "design": d.name, "process": d.draft.process.label(), "size": d.size.display(), "bore_mm": built.report.inner_diameter_mm,
        "build": [params.theta_steps, params.profile_steps], "triangles": built.mesh.faces.len(), "build_s": build_s,
        "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces, "min_angle_deg": q.min_angle_deg, "worst_aspect": q.worst_aspect,
        "mesh_self_crossings": crossings, "made_parts": made, "solids_notes": built.solids.notes,
        "stamps": d.stamps.len(), "stamped": built.solids.stamped, "seats_resolved": built.solids.resolved, "parting_monotone_failures": monotone,
        "bore_margin_mm": bore_margin, "bore_vertices_inside": bore_inside,
        "field": field, "clamps": clamps.iter().map(|(n, c)| json!({"layer": n, "worst_mm": c.worst_mm, "texels_cut": c.texels_cut})).collect::<Vec<_>>(),
        "clamp_worst_mm": clamp_worst,
        "dfm": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "release_0_1": inspection.release, "release_0_075": release_fine,
        "release_at": {"0_1": obstructions(&inspection.release), "0_075": obstructions(&release_fine)},
        "release_draft": draft_block.as_ref().map(|b| b.0.clone()),
        "crest_line_median_z_mm": crest_z,
        "collet_wall_mm": walls,
        "pattern": {"triangles": pattern.faces.len(), "scale": inspection.prepared.scale, "open_edges": pattern_check.open_edges,
            "repeated_edges": pattern_check.repeated_edges, "degenerate_faces": pattern_check.zero_area_faces, "self_crossings": pattern_check.self_crossings,
            "bench_layers": inspection.prepared.bench_layers, "notes": inspection.prepared.notes},
        "stamps_384": stamps_small,
        "stones": {"reported": stones_count, "previewed": previewed, "carats": stones_report.as_ref().map(|s| s.total_carats),
            "tight_pairs": stones_report.as_ref().map(|s| s.tight_pairs),
            "warnings": stones_report.as_ref().map(|s| s.seats.iter().flat_map(|x| x.warnings.iter().map(|w| format!("{}: {w}", x.label))).collect::<Vec<_>>())},
        "metals": built.report.metals, "volume_mm3": built.report.volume_mm3,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "design_bytes": text.len(), "embedded_alphas": d.embedded.len(),
        "cold_reload_identical": cold, "gates": gates, "gates_passed": passed,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!(
        "  field {:?} ({:.4}% undercut, worst {:.2} deg, drag {:.1}%), wall {:.2} mm; release {}/{} and {}/{}; draft {:?}; 384 {}; clamp {:.4}; dfm {}; stones {} / {}; bore {:.3} mm; crossings {}; pattern {}/{}/{:?}; sand slots {}",
        field.verdict,
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9) * 100.0,
        field.thinnest_wall_mm,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays,
        release_fine.obstructions.len(),
        release_fine.unresolved_rays,
        draft_block.as_ref().map(|b| b.1),
        stamps_small["passed"],
        clamp_worst,
        findings.len(),
        stones_count,
        previewed,
        bore_margin,
        crossings,
        pattern_check.open_edges,
        pattern_check.zero_area_faces,
        pattern_check.self_crossings,
        inspection.release.sand_findings.len()
    );
    for f in &findings {
        println!("    dfm: {} {}", f.label, f.message);
    }
    for n in &field.notes {
        println!("    field: {n}");
    }
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), pattern, &format!("{} Delft pattern", d.name))?;
        for (m, _) in &gems {
            stl::write_stl(out.join("reference-onyx.stl"), m, "Corvus reference onyx")?;
        }
        let tint = onyx().preview_tint.unwrap_or([0.015, 0.015, 0.02]);
        std::fs::write(
            out.join("stones.json"),
            serde_json::to_vec_pretty(&json!({"stones": [{"mesh": "reference-onyx.stl", "name": "Black onyx", "tint": tint, "ior": 1.54, "dispersion": 0.0, "roughness": 0.05, "transmission": 0.0}]}))?,
        )?;
    }
    renders(&out, &d, &lib, &built, &gems, if draft { 1000 } else { 1600 })?;
    println!("  gates {}: {}", if passed { "passed" } else { "FAILED" }, gates);
    ensure!(passed, "Corvus failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
