//! Bestiarium — Corvus, Huginn and Muninn: Odin's two ravens passing on a bypass, an onyx between their heads, cast in Delft sand.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_corvus
//! target/release/examples/bestiarium_corvus [OUT_DIR] [--draft] [--verify] [--preview]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, LayerEntry, ProfileStyle, RingDesign, RingSize, ShankKind, Window,
    castability::{self, SandProcess},
    csg, dfm,
    field::{Layer, SeatPadLayer, SeatStyle},
    gem::{Gem, GemCut},
    library, manufacturing as mf, mesh, render,
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

/// The bypass LowDome: two arms passing over the top, each the band's own width.
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
    d
}

fn onyx() -> Gem {
    Gem { l_mm: 8.0, preview_tint: Some([0.015, 0.015, 0.02]), ..Gem::cabochon(GemCut::Oval, 6.0) }
}

/// The onyx on a gypsy mound at the crossing, in a made collet soldered on after the pour.
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
    s.height_mm = 0.55;
    d.layers.layers.push(LayerEntry::new("Onyx, made collet", Layer::SeatPad(s)));
    Ok(())
}

type P2 = [f64; 2];

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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

/// A closed polyline resampled at equal steps no longer than `step`.
fn resample(pts: &[P2], step: f64) -> Vec<P2> {
    let mut closed = pts.to_vec();
    closed.push(pts[0]);
    let mut out = run(&closed, step);
    out.pop();
    out
}

/// A closed counter-clockwise outline between `top(x)` and `bottom(x)` over `x0..x1`, its four corners kept, edges under 0.1 mm.
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

/// How a raven's head is drawn, in hide millimetres along the crest from the top and across it.
#[derive(Clone, Copy)]
struct HeadSpec {
    /// Beak tip's distance from the top along the crest.
    tip: f64,
    beak: f64,
    beak_w: f64,
    /// Painted beak height at its root.
    beak_h: f64,
    /// The struck culmen's rise over the painted beak.
    culmen: f64,
    skull: f64,
    skull_w: f64,
    /// Skull height at the crown.
    crown: f64,
    /// Skull half-width kept inside the rim on each side.
    rim_share: f64,
}

const HEAD: HeadSpec = HeadSpec {
    tip: 5.3,
    beak: 6.6,
    beak_w: 1.45,
    beak_h: 2.45,
    culmen: 0.36,
    skull: 6.3,
    skull_w: 2.75,
    crown: 2.9,
    rim_share: 0.88,
};

/// Height of relief painted on the atlas, mm, at alpha 1.
const PAINT_MM: f64 = 3.0;

/// A sample on one bird: distance from the top along the crest toward its shoulder, across from the crest, and the rim on its side.
#[derive(Clone, Copy)]
struct On {
    o: f64,
    w: f64,
    rim: f64,
}

fn on_bird(p: skin::HidePoint, sign: f64) -> On {
    On { o: -sign * p.along, w: p.across, rim: p.rim }
}

/// Per atlas column, the `across` the hide reads where the section truly crosses the parting plane, between its rows.
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

/// A sample in hide millimetres with `across` measured from the exact parting line.
fn hide_at(hide: &Hide, zero: &[f64], s: &skin::Sample) -> skin::HidePoint {
    let mut p = hide.at(s);
    p.across -= zero[s.i % zero.len()];
    p
}

/// A light crown on painted relief, so every plate falls a little away from the crest and nothing near it stands level.
fn crowned(q: On) -> f64 {
    1.0 - 0.07 * (q.w / (q.rim + 0.8).max(1.0)).powi(2).min(1.0)
}

impl HeadSpec {
    /// Beak half-width `u` mm back from its tip.
    fn beak_half(&self, u: f64) -> f64 {
        let t = (u / self.beak).clamp(0.0, 1.0);
        (self.beak_w * (1.0 - (1.0 - t).powf(1.3)).max(0.0).powf(0.55)).max(0.1)
    }
    /// Painted beak mass, mm: arched along the culmen, a sharp-backed wedge across.
    fn beak_mm(&self, q: On) -> f64 {
        let u = q.o - self.tip;
        if !(0.0..=self.beak + 0.8).contains(&u) {
            return 0.0;
        }
        let t = (u / self.beak).min(1.0);
        let arch = (1.0 - (1.0 - t).powi(2)).max(0.0).sqrt();
        let across = (q.w.abs() / self.beak_half(u)).min(1.0);
        self.beak_h * arch * (1.0 - across.powi(3)).max(0.0).powf(0.8) * (1.0 - 0.25 * across * across)
    }
    /// Skull relief, mm: a long flat crown behind a steep forehead, rounding off into the nape and flowing into the beak's root.
    fn skull_mm(&self, q: On) -> f64 {
        let base = self.tip + self.beak;
        let u = q.o - base;
        if !(-1.2..=self.skull).contains(&u) {
            return 0.0;
        }
        let s = (u / self.skull).clamp(0.0, 1.0);
        let width = self.width(q, s);
        let t = (q.w.abs() / width.max(1e-6)).min(1.0);
        let along = if s < 0.3 {
            0.8 + 0.2 * (1.0 - ((0.3 - s) / 0.3).powi(2))
        } else if s < 0.62 {
            1.0 - 0.04 * ((s - 0.3) / 0.32).powi(2)
        } else {
            0.96 * (1.0 - ((s - 0.62) / 0.38).powf(1.6)).max(0.0)
        };
        let root = smooth(-1.2, 0.5, u);
        let dome = (1.0 - t.powi(4)).max(0.0).powf(0.5) * (1.0 - 0.18 * t * t);
        self.crown * along * root * dome
    }
    /// Skull half-width at `s` along it, kept inside the rim on the sample's side.
    fn width(&self, q: On, s: f64) -> f64 {
        self.skull_w.min(self.rim_share * q.rim)
            * (0.52 + 0.48 * (s / 0.42).min(1.0).powf(0.6))
            * (1.0 - ((s - 0.7).max(0.0) / 0.3).powi(2)).max(0.0).sqrt()
    }
}

/// The two skulls painted on the atlas, as a share of [`PAINT_MM`].
fn paint_heads(a: &Atlas, hide: &Hide, zero: &[f64], h: HeadSpec) -> ringdesign_core::Alpha {
    a.paint("Raven heads", |s| {
        let p = hide_at(hide, zero, s);
        [1.0, -1.0].iter().map(|&sign| { let q = on_bird(p, sign); h.skull_mm(q).max(h.beak_mm(q)) * crowned(q) }).fold(0.0, f64::max) / PAINT_MM
    })
}

/// The chart point nearest a hide position: `along` from the top, `across` from the crest.
fn chart_of(a: &Atlas, hide: &Hide, along: f64, across: f64) -> (f64, f64) {
    let x = (0..a.width).min_by(|p, q| (hide.along[*p] - along).abs().total_cmp(&(hide.along[*q] - along).abs())).unwrap_or(0);
    let y = (0..a.height).min_by(|p, q| (hide.across[*p * a.width + x] - across).abs().total_cmp(&(hide.across[*q * a.width + x] - across).abs())).unwrap_or(0);
    let s = a.at(x, y);
    (s.theta, s.v)
}

/// A crest point `o` mm from the top toward `sign`'s shoulder: its ring angle and chart `v`.
fn crest(d: &RingDesign, a: &Atlas, hide: &Hide, sign: f64, o: f64) -> Result<(f64, f64)> {
    let (theta, _) = chart_of(a, hide, -sign * o, 0.0);
    let v = setting::crest_v(d, theta).ok_or_else(|| anyhow::anyhow!("No crest at {theta}"))?;
    Ok((theta, v))
}

/// How far behind each eye its cut is centred, mm, so the cut's reach spans the tall skull it drops onto.
const EYE_REACH: f64 = 1.9;

/// One raven's struck beak, a thin gable riding the painted beak so its outline is crisp, and its eyes cut at the bench
/// into the skull as sockets holding a domed eye. `sign` +1 is the bird on the low shoulder, its beak toward rising theta.
fn raven_beak(d: &RingDesign, a: &Atlas, hide: &Hide, name: &str, sign: f64, h: HeadSpec) -> Result<Vec<Stamp>> {
    let rot = if sign > 0.0 { 0.0 } else { 180.0 };
    let hb = 0.5 * (h.beak - 1.2);
    let at = crest(d, a, hide, sign, h.tip + hb)?;
    let half = |x: f64| h.beak_half(hb - x) * 0.9 * (1.0 - smooth(-hb + 1.4, -hb, x)).max(0.0).sqrt().max(0.06);
    let beak = banded(-hb, hb, half, |x| -half(x));
    let mut out = vec![Stamp {
        name: format!("{name}, beak"),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg: rot,
        outline: beak,
        height_mm: 0.07,
        sink_mm: 0.5,
        draft_deg: 4.0,
        cut: false,
        bench: false,
        along_pull: false,
        tier: 0,
        top: StampTop::Gable { rise_mm: h.culmen, axis_deg: 0.0 },
    }];
    let eye_o = h.tip + h.beak + 0.3 * h.skull;
    let eye: Vec<P2> = (0..72)
        .map(|k| {
            let t = k as f64 / 72.0 * 2.0 * PI;
            [0.78 * t.cos(), 0.58 * t.sin() * (1.0 - 0.18 * t.cos())]
        })
        .collect();
    for (side, frac) in [("near", 1.0), ("far", -1.0)] {
        let across = frac * 0.6 * h.skull_w;
        let (theta, v) = chart_of(a, hide, -sign * (eye_o + EYE_REACH), across);
        out.push(Stamp {
            name: format!("{name}, eye {side}"),
            theta_deg: theta,
            v_mm: v,
            rot_deg: rot,
            outline: resample(&eye.iter().map(|p| [p[0] + EYE_REACH, p[1]]).collect::<Vec<_>>(), 0.05),
            height_mm: 3.2,
            sink_mm: 0.5,
            draft_deg: 0.0,
            cut: true,
            bench: true,
            along_pull: false,
            tier: 1,
            top: StampTop::Dome { crown_mm: -0.36 },
        });
    }
    Ok(out)
}

/// Height of the body plumage at alpha 1, mm.
const PLUME_MM: f64 = 1.0;

/// Where each raven's body lies along the crest, mm from the top, and how its feathers are cut.
#[derive(Clone, Copy)]
struct BodySpec {
    /// Neck rows start under the skull, at the crest this far out.
    neck: f64,
    /// Wing front at the crest: the first covert row.
    shoulder: f64,
    /// Secondaries start.
    secondaries: f64,
    /// Primaries start; they fan in to the apex at the crest.
    primaries: f64,
    apex: f64,
    lanes: usize,
    /// How much shorter each primary is than the one inside it.
    lane_step: f64,
    tail_lanes: usize,
    /// How far each bird's tail feathers reach past the palm into the other's.
    overlap: f64,
}

const BODY: BodySpec = BodySpec {
    neck: 13.0,
    shoulder: 20.4,
    secondaries: 24.2,
    primaries: 27.4,
    apex: 34.6,
    lanes: 4,
    lane_step: 1.25,
    tail_lanes: 4,
    overlap: 1.7,
};

/// The primaries' fan narrows no further than this across both sides of the crest, mm, so its last feather ends as a blade.
const FAN_MIN: f64 = 1.7;

/// Feathers in lanes: `lanes` side by side from the crest to the reach, each lane a column of feathers with tips every `pitch`
/// along the crest from `from`, staggered lane to lane, each tip rounded on its outer corner.
struct Lanes {
    from: f64,
    lanes: f64,
    pitch: f64,
    stagger: f64,
    round: f64,
}

impl Lanes {
    /// The feather under `(o, x)`, `x` the share of the reach across: its lane and its phase along its own length.
    fn at(&self, o: f64, x: f64) -> (f64, f64) {
        let x = x.clamp(0.0, 0.999) * self.lanes;
        let lane = x.floor();
        let within = x - lane;
        let corner = self.round * (1.0 - (1.0 - within * within).max(0.0).sqrt());
        let local = o - self.from - lane * self.stagger + corner;
        (lane, (local / self.pitch).rem_euclid(1.0))
    }
}

impl BodySpec {
    /// Body plumage of one raven at `q`, mm: neck, coverts and secondaries as lanes of overlapping feathers, the primaries
    /// fanning in over the tail, and the wedge tail interlocking with the other bird's at the palm. Each lane stands a step
    /// under the one inside it and each feather rises less than that step, so every section falls away from the crest.
    fn plume_mm(&self, q: On, palm: f64, sign: f64) -> f64 {
        let w = q.w.abs();
        let reach = (q.rim + 0.8).max(1.0);
        let x = (w / reach).min(1.0);
        let bulge = |at: f64, by: f64| at - by * (1.0 - x * x);
        let mut h: f64 = 0.0;
        let groups = [
            (Lanes { from: self.neck - 3.6, lanes: 5.0, pitch: 0.95, stagger: 0.45, round: 0.55 }, bulge(self.neck, 3.6), 0.52, 0.09),
            (Lanes { from: self.shoulder - 1.6, lanes: 4.0, pitch: 1.3, stagger: 0.6, round: 0.8 }, bulge(self.shoulder, 1.6), 0.86, 0.1),
            (Lanes { from: self.secondaries - 1.2, lanes: 3.0, pitch: 1.8, stagger: 0.85, round: 1.1 }, bulge(self.secondaries, 1.2), 0.98, 0.1),
        ];
        let primaries = bulge(self.primaries, 1.0);
        for (g, (lanes, front, top, rise)) in groups.iter().enumerate() {
            let next = groups.get(g + 1).map_or(primaries, |n| n.1);
            if q.o >= *front && q.o < next {
                let (lane, t) = lanes.at(q.o, x);
                h = h.max(top - rise - (rise + 0.03) * lane + rise * t.powf(0.85));
            }
        }
        let primaries = bulge(self.primaries, 1.0);
        if q.o >= primaries {
            let run = (self.apex - self.primaries).max(1.0);
            let s = ((q.o - self.primaries) / run).clamp(0.0, 1.0);
            let fan = (reach * (1.0 - s.powf(1.3))).max(FAN_MIN);
            if w < fan {
                let lanes = self.lanes as f64;
                let j = (lanes * w / fan).floor().min(lanes - 1.0);
                let within = lanes * w / fan - j;
                let tip = self.apex - j * self.lane_step - 0.7 * self.lane_step * within;
                if q.o <= tip {
                    h = h.max(1.0 - 0.1 * j - 0.06 * smooth(tip - 1.5, tip, q.o));
                }
            }
            let lanes = self.tail_lanes as f64;
            let lane_w = reach / lanes;
            let k = (w / lane_w).floor().min(lanes - 1.0);
            let within = (w - k * lane_w) / lane_w;
            let own = ((k as usize) % 2 == 0) == (sign > 0.0);
            let past = self.overlap - 0.45 * k - 0.55 * within;
            let end = if own { palm + past } else { palm - past };
            if q.o <= end {
                h = h.max(0.62 - 0.09 * k - 0.07 * smooth(end - 1.3, end, q.o));
            }
        }
        h
    }
    /// Throat hackles beside the beak: pointed lanes lying toward the onyx, inner lanes longest.
    fn hackles_mm(&self, q: On, h: HeadSpec) -> f64 {
        let w = q.w.abs();
        let root = h.tip + h.beak + 2.2;
        if q.o > root + 1.5 || q.o < h.tip - 0.5 {
            return 0.0;
        }
        let reach = (q.rim + 0.8).max(1.0);
        let lane_w = reach / 4.0;
        let j = (w / lane_w).floor().min(3.0);
        let within = (w - j * lane_w) / lane_w;
        let start = h.tip + 0.3 + 1.3 * j + 0.9 * within;
        if q.o < start {
            return 0.0;
        }
        (0.46 - 0.07 * j) * smooth(start, start + 1.2, q.o) * (1.0 - 0.4 * smooth(root, root + 1.5, q.o))
    }
}

/// Height of the shaft ridge along each raven's spine, mm: the crest line itself, so no level facet sits on the parting line.
const SPINE_MM: f64 = 0.05;

/// Both ravens' body plumage on the atlas, as a share of [`PLUME_MM`].
fn paint_plumage(a: &Atlas, hide: &Hide, zero: &[f64], h: HeadSpec, b: BodySpec) -> ringdesign_core::Alpha {
    let palm = hide.reach();
    a.paint("Raven plumage", |s| {
        let p = hide_at(hide, zero, s);
        [1.0, -1.0]
            .iter()
            .map(|&sign| {
                let mut q = on_bird(p, sign);
                if q.o < 0.0 {
                    q.o = 2.0 * palm + q.o;
                }
                let edge = 1.0 - smooth(q.rim + 0.7, q.rim + 1.25, q.w.abs());
                let body = b.plume_mm(q, palm, sign);
                let spine = if body > 0.0 { SPINE_MM * (1.0 - q.w.abs() / 0.4).max(0.0) } else { 0.0 };
                (body + spine).max(b.hackles_mm(q, h)) * edge * crowned(q)
            })
            .fold(0.0, f64::max)
            / PLUME_MM
    })
}

/// A separable box blur of `r` columns and `s` rows, wrapping round the ring; symmetric, so a layer falling away from the
/// crest on both sides still does.
fn blur(alpha: &mut ringdesign_core::Alpha, r: usize, s: usize) {
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

/// Height of the nest at alpha 1, mm.
const NEST_MM: f64 = 1.4;

/// A raised nest under the onyx's collet: filled up to the collet's flat base, falling 3.5 deg across so it keeps its draft,
/// and fading beyond the collet's wall, so the made collet stands on metal all round instead of floating.
fn paint_nest(d: &RingDesign, a: &Atlas, hide: &Hide) -> Result<ringdesign_core::Alpha> {
    let (stone, f) = ringdesign_core::stones::stone_frames(d).into_iter().next().ok_or_else(|| anyhow::anyhow!("No onyx"))?;
    let gem = stone.gem;
    let depth = setting::collet_depth_mm(gem);
    let wall = setting::collet_wall_mm(gem) + 0.03;
    let base: [f64; 3] = std::array::from_fn(|k| f.girdle[k] - f.normal[k] * depth);
    let (sa, sb) = (f.semi.0 + wall, f.semi.1 + wall);
    let draft = 3.5_f64.to_radians().tan();
    Ok(a.paint("Onyx nest", |s| {
        if off_top(s.theta) > 45.0 {
            return 0.0;
        }
        let rel: [f64; 3] = std::array::from_fn(|k| s.p[k] - base[k]);
        let x = rel.iter().zip(f.long).map(|(a, b)| a * b).sum::<f64>();
        let y = rel.iter().zip(f.short).map(|(a, b)| a * b).sum::<f64>();
        let below = -rel.iter().zip(f.normal).map(|(a, b)| a * b).sum::<f64>();
        let rho = ((x / sa).powi(2) + (y / sb).powi(2)).sqrt();
        let past = (rho - 1.0) * sa.min(sb);
        let lift = below + 0.25 - draft * s.p[2].abs();
        let axial = (x / sa).powi(2) / (rho * rho).max(1e-9);
        let fade = (1.0 - past.max(0.0) / (0.3 + 0.9 * axial)).max(0.0);
        let p = hide.at(s);
        let crown = 1.0 - smooth(p.rim - 0.35, p.rim - 0.05, p.across.abs());
        (lift.max(0.0) * fade * crown).max(0.0) / NEST_MM
    }))
}

/// Degrees from the top of the ring.
fn off_top(theta: f64) -> f64 {
    ringdesign_core::field::wrap_delta(theta - 90.0, 360.0).abs()
}

/// Atlas columns round the ring.
const ATLAS_W: usize = 2048;

/// Atlas rows across the section, so a texel is as tall as it is wide and the detail measure reads strokes true.
fn atlas_rows(d: &RingDesign) -> usize {
    let ctx = d.field_context();
    ((ATLAS_W as f64 * ctx.band_v_len_mm / ctx.circumference_mm).round() as usize).max(64)
}

/// Paint one layer: clamp it to the sand's pull, store it portably, and add it over the whole chart.
fn painted(d: &mut RingDesign, lib: &mut AlphaLibrary, a: &Atlas, hide: &Hide, mut alpha: ringdesign_core::Alpha, height: f64, rows: usize) -> Result<skin::ClampReport> {
    blur(&mut alpha, 5, rows);
    let before = alpha.data.clone();
    let clamp = skin::draft_clamp(a, &mut alpha, height)?;
    println!("  {}: clamp {:.4} mm over {} texels", alpha.name, clamp.worst_mm, clamp.texels_cut);
    if std::env::var("CORVUS_BITES").is_ok() {
        let mut bites: Vec<(f64, usize)> = before.iter().zip(&alpha.data).enumerate().map(|(i, (b, c))| ((b - c) as f64 * height, i)).filter(|(m, _)| *m > 0.01).collect();
        bites.sort_by(|x, y| y.0.total_cmp(&x.0));
        let mut shown: Vec<(f64, f64)> = Vec::new();
        for (m, i) in bites {
            let s = &a.samples[i];
            let p = hide.at(s);
            if shown.iter().any(|(l, w)| (l - p.along).abs() < 0.6 && (w - p.across).abs() < 0.4) {
                continue;
            }
            shown.push((p.along, p.across));
            println!("    bite {m:.3} mm at theta {:.1}, along {:.2}, across {:.2}, rim {:.2}, n_z {:.2}", s.theta, p.along, p.across, p.rim, s.n[2]);
            if shown.len() >= 12 {
                break;
            }
        }
    }
    lib.insert(Alpha::from_png16(alpha.name.clone(), &alpha.to_png16()?)?);
    d.layers.layers.push(skin::hide_layer(d, &alpha.name, height, Window::around(90.0, 360.0)));
    Ok(clamp)
}

/// Depth of the graver's work at alpha 1, mm.
const BARB_MM: f64 = 0.07;

/// What the graver cuts after the pour: a rachis down every flight and tail feather, and each beak's gape.
fn paint_barbs(a: &Atlas, hide: &Hide, zero: &[f64], h: HeadSpec, b: BodySpec) -> ringdesign_core::Alpha {
    let palm = hide.reach();
    let line = |d: f64, half: f64| 1.0 - smooth(half * 0.6, half, d.abs());
    a.paint("Graver's work", |s| {
        let p = hide_at(hide, zero, s);
        let mut cut: f64 = 0.0;
        for sign in [1.0, -1.0] {
            let mut q = on_bird(p, sign);
            if q.o < 0.0 {
                q.o = 2.0 * palm + q.o;
            }
            let w = q.w.abs();
            let reach = (q.rim + 0.8).max(1.0);
            if q.o >= b.primaries && q.o < b.apex {
                let run = (b.apex - b.primaries).max(1.0);
                let sv = ((q.o - b.primaries) / run).clamp(0.0, 1.0);
                let fan = (reach * (1.0 - sv.powf(1.3))).max(FAN_MIN);
                if w < fan {
                    let lanes = b.lanes as f64;
                    let x = lanes * w / fan;
                    let j = x.floor().min(lanes - 1.0);
                    let tip = b.apex - j * b.lane_step;
                    let half_mm = (x - j - 0.5) * fan / lanes;
                    if q.o < tip - 1.0 {
                        cut = cut.max(line(half_mm, 0.07));
                    }
                }
            }
            let lanes = b.tail_lanes as f64;
            let lane_w = reach / lanes;
            let k = (w / lane_w).floor().min(lanes - 1.0);
            let mid = (w - (k + 0.5) * lane_w).abs();
            let own = ((k as usize) % 2 == 0) == (sign > 0.0);
            let past = b.overlap - 0.45 * k;
            let end = if own { palm + past } else { palm - past };
            if q.o > b.apex - 1.0 && q.o < end - 1.1 {
                cut = cut.max(line(mid, 0.07));
            }
            let u = q.o - h.tip;
            if (0.35..h.beak + 0.3).contains(&u) {
                let edge = h.beak_half(u) * 0.8;
                cut = cut.max(line(w - edge, 0.06) * smooth(0.35, 0.9, u));
            }
        }
        cut
    })
}

/// Every painted layer's draft-clamp report, by name.
type Clamps = Vec<(String, skin::ClampReport)>;

/// The whole ring: the bypass body and its collet, the painted nest, plumage and heads, the struck beaks, the eyes and the
/// graver's lines cut at the bench. The painted layers are written to `art` as they are stored.
fn author(art: Option<&Path>) -> Result<(RingDesign, AlphaLibrary, Clamps)> {
    let mut d = band();
    seat(&mut d)?;
    let a = Atlas::of(&d, ATLAS_W, atlas_rows(&d))?;
    let hide = Hide::of(&a);
    let mut lib = AlphaLibrary::default();
    let mut clamps = Vec::new();
    let zero = zero_across(&a, &hide);
    let nest = paint_nest(&d, &a, &hide)?;
    for (alpha, height, name, rows) in [
        (nest, NEST_MM, "Onyx nest", 3),
        (paint_plumage(&a, &hide, &zero, HEAD, BODY), PLUME_MM, "Raven plumage", 3),
        (paint_heads(&a, &hide, &zero, HEAD), PAINT_MM, "Raven heads", 0),
    ] {
        let clamp = painted(&mut d, &mut lib, &a, &hide, alpha, height, rows)?;
        if let Some(art) = art {
            let slug = name.to_lowercase().replace(' ', "-");
            std::fs::write(art.join(format!("{slug}.png")), lib.get(name).map(|x| x.to_png16()).transpose()?.unwrap_or_default())?;
        }
        clamps.push((name.to_string(), clamp));
    }
    let barbs = paint_barbs(&a, &hide, &zero, HEAD, BODY);
    lib.insert(Alpha::from_png16(barbs.name.clone(), &barbs.to_png16()?)?);
    if let Some(art) = art {
        std::fs::write(art.join("graver-work.png"), barbs.to_png16()?)?;
    }
    let mut e = skin::hide_layer(&d, &barbs.name, BARB_MM, Window::around(90.0, 360.0));
    e.blend = ringdesign_core::Blend::Subtract;
    e.bench_only = true;
    d.layers.layers.push(e);
    for (name, sign) in [("Huginn", 1.0), ("Muninn", -1.0)] {
        d.stamps.extend(raven_beak(&d, &a, &hide, name, sign, HEAD)?);
    }
    Ok((d, lib, clamps))
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn stamp_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

fn solid(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn checked_part(name: &str, part: &csg::Solid) -> serde_json::Value {
    let c = part.check(true);
    json!({"name": name, "vertices": part.v.len(), "faces": part.f.len(), "open_edges": c.open_edges, "repeated_edges": c.repeated_edges,
        "degenerate_faces": c.zero_area_faces, "self_crossings": c.self_crossings, "volume_mm3": c.volume})
}

/// Every made part as placed: each struck stamp against the ring built to its tier, and the onyx's collet.
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
    for (stone, frame) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let uv = ringdesign_core::field::Uv { u: ctx.u_of_theta(stone.theta_deg), v: stone.v_mm };
        let relief = d.layers.height(uv, &ctx, lib);
        let _ = frame;
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

/// The smallest distance any finished vertex stands from the finger axis, less the bore radius, and how many stand inside it.
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
    [("hero", 0.62, 0.92), ("face", 0.0, PI * 0.5), ("palm", PI, PI * 0.5), ("side", 0.0, 0.0), ("shoulder", -0.95, 0.55), ("reverse", PI + 0.5, 0.35)];

/// Studio-gold renders with the onyx set: the named views, a stone close-up, and the bare body against the finished ring.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, b: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])], edge: usize) -> Result<()> {
    let mut display = b.mesh.clone();
    display.corner_normals.clear();
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

/// Close-up sheets for authoring: each head from four sides, each body from above and the side, and the ring at 300 px.
fn preview_sheets(out: &Path, d: &RingDesign, b: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])]) -> Result<()> {
    let mut display = b.mesh.clone();
    display.corner_normals.clear();
    let edge = 700;
    let mut parts = vec![render::Part::metal(&display, render::GOLD)];
    parts.extend(gems.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    let ctx = d.field_context();
    let head = d.stamps.iter().find(|s| s.name == "Huginn, beak").ok_or_else(|| anyhow::anyhow!("No beak"))?;
    let f = head.frame(d, &ctx);
    let close = crop(&display, f.origin, 8.0);
    let mut cparts = vec![render::Part::metal(&close, render::GOLD), render::Part::metal(&display, render::GOLD)];
    cparts.extend(gems.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    let y0 = f.origin[0].atan2(f.origin[1]);
    let tiles: Vec<Vec<u8>> = [(y0, PI * 0.5), (y0, 0.0), (y0 + 0.5, 0.9), (y0 - 0.6, 0.75)]
        .iter()
        .map(|(y, p)| render::render_parts_ss(&cparts, *y, *p, edge, edge, 2))
        .collect();
    sheet(&out.join("preview-head.png"), &tiles, edge, 2)?;
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    let mut tiles = Vec::new();
    for theta in [-15.0_f64, -70.0] {
        let t = theta.to_radians();
        let body = crop(&display, [r * t.cos(), r * t.sin(), 0.0], 8.5);
        let bparts = vec![render::Part::metal(&body, render::GOLD), render::Part::metal(&display, render::GOLD)];
        let y = (90.0 - theta).to_radians();
        tiles.push(render::render_parts_ss(&bparts, y, PI * 0.5, edge, edge, 2));
        tiles.push(render::render_parts_ss(&bparts, y + 0.35, 0.75, edge, edge, 2));
    }
    sheet(&out.join("preview-body.png"), &tiles, edge, 2)?;
    let small: Vec<Vec<u8>> = VIEWS.iter().map(|(_, y, p)| render::render_parts_ss(&parts, *y, *p, 300, 300, 3)).collect();
    sheet(&out.join("preview-300.png"), &small, 300, 3)?;
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

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
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
    let setup = mf::Setup { recipe: mf::Recipe::sand(SandProcess::DelftClay), sample_pitch_mm: 0.1, ..Default::default() };
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let release_fine = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let pattern = &inspection.prepared.mesh;
    let pattern_check = solid(pattern).check(true);
    let stamps_small = {
        let small = mesh::try_build(&d, &lib, stamp_params())?;
        let insp = mf::inspect(&d, &lib, &setup, stamp_params())?;
        let rel = mf::release::analyze(&insp.prepared.mesh, &fine)?;
        json!({"build": [384, 192], "notes": small.solids.notes, "stamped": small.solids.stamped, "watertight": small.report.validation.watertight,
            "degenerate_faces": small.report.quality.degenerate_faces, "self_crossings": csg::self_crossings(&solid(&small.mesh)),
            "release_0_1": {"obstructions": insp.release.obstructions.len(), "unresolved": insp.release.unresolved_rays},
            "release_0_075": {"obstructions": rel.obstructions.len(), "unresolved": rel.unresolved_rays},
            "at": rel.obstructions.iter().chain(&insp.release.obstructions).map(|o| json!({"theta": o.world[1].atan2(o.world[0]).to_degrees(), "r": o.world[0].hypot(o.world[1]), "z": o.world[2], "depth_mm": o.depth_mm})).collect::<Vec<_>>()})
    };
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
        "clamp_bite": clamp_worst <= 0.05,
        "dfm_zero": findings.is_empty(),
        "stones_match": stones_count == previewed,
        "cold_reload": cold != Some(false),
        "pattern_mesh": pattern_check.open_edges == 0 && pattern_check.zero_area_faces == 0 && pattern_check.self_crossings == Some(0),
        "triangle_budget": triangles_ok,
        "stamps_384": stamps_small["notes"].as_array().is_some_and(|n| n.is_empty())
            && stamps_small["stamped"] == d.stamps.len()
            && stamps_small["watertight"] == true
            && stamps_small["degenerate_faces"] == 0
            && stamps_small["self_crossings"] == 0,
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
        "  field {:?} ({:.4}% undercut, worst {:.2} deg, drag {:.1}%), wall {:.2} mm; release {}/{} and {}/{}; clamp {:.4}; dfm {}; stones {} / {}; bore {:.3} mm; crossings {}; pattern {}/{}/{:?}",
        field.verdict,
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        (field.marginal_area_mm2 + field.vertical_area_mm2) / field.total_area_mm2.max(1e-9) * 100.0,
        field.thinnest_wall_mm,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays,
        release_fine.obstructions.len(),
        release_fine.unresolved_rays,
        clamp_worst,
        findings.len(),
        stones_count,
        previewed,
        bore_margin,
        crossings,
        pattern_check.open_edges,
        pattern_check.zero_area_faces,
        pattern_check.self_crossings
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
