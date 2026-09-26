//! Bestiarium — Harpyia, the snatcher: a hooded harpy crouched behind a storm sapphire and bowed over it, clutching it
//! in two lofted raptor feet, her wings swept back from her shoulders down both cheeks, her mantle, tail, thighs and
//! scaled leg painted round the band.
//! Build unguarded, then run under an 8 GiB scope:
//! cargo build --offline --release -p ringdesign-core --example bestiarium_harpyia
//! systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_harpyia [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        self, Attach, Component, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, Stage,
        builders::{self, BUR},
        pattern::Sources,
    },
    castability::{self, CastProcess},
    csg, dfm,
    field::Window,
    gem::{Gem, GemCut},
    library,
    manufacturing as mf,
    mesh, render, setting,
    sketch::{Geometry, Sketch, Workplane},
    skin::{self, Atlas, Sample},
    stl,
};
use serde_json::json;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

/// Half the band's width: the cheeks are the planes z = ±CHEEK.
const CHEEK: f64 = 3.6;
/// The band's thickness, mm.
const THICK: f64 = 3.2;
/// The investment section every part keeps.
const MIN_SECTION_MM: f64 = 0.8;
/// Where every wing feather's belly stands, sunk into the cheek.
const BED_Z: f64 = 3.3;
/// Radius past which the wing leans out from the cheek's plane.
const HINGE_R: f64 = 13.0;
/// Most the wing leans out above the crown, degrees.
const SPLAY_DEG: f64 = 23.0;
/// Rise over which the lean comes in, mm.
const SPLAY_RAMP: f64 = 2.5;
/// Nearest a feather's edge comes to the finger's axis, mm.
const BORE_CLEAR_R: f64 = 9.85;
/// The sapphire's girdle over the crest, mm.
const STAND_MM: f64 = 1.3;
/// Clearance every part keeps from the stone, mm.
const STONE_GAP: f64 = 0.03;
/// Radius of a claw's blunt tip, mm.
const TIP_R: f64 = 0.46;
/// Most triangles an export build may carry.
const TRIANGLE_BUDGET: usize = 2_000_000;
/// Most the ring may reach from the finger's axis, mm.
const REACH_MAX_MM: f64 = 18.6;
/// Most the ring may run along the finger, mm.
const Z_MAX_MM: f64 = 12.6;
/// Most the ring may weigh in 18k, grams.
const GRAMS_MAX: f64 = 35.0;
/// Height of the painted plumage, mm.
const PLUMAGE_MM: f64 = 0.36;
/// Atlas columns round the ring.
const ATLAS_W: usize = 1024;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..Default::default() }
}

fn preview_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 144, ..Default::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..Default::default() }
}

fn sapphire() -> Gem {
    Gem { preview_tint: Some([0.03, 0.09, 0.42]), ..Gem::calibrated(GemCut::Round, 8.0) }
}

fn band() -> RingDesign {
    let mut d = RingDesign { name: "Harpyia — the snatcher".into(), ..Default::default() };
    d.size = ringdesign_core::RingSize::from_diameter_mm(19.0);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 2.0 * CHEEK;
    d.profile.thickness_mm = THICK;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Uniform;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

fn part() -> Component {
    Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: 0.0, placement: Placement::Free, material: "Gold 18k".into(), ..Default::default() }
}

type P3 = [f64; 3];
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P3) -> P3 {
    mul(a, 1.0 / norm(a).max(1e-30))
}
fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn polar(theta_deg: f64, r: f64) -> [f64; 2] {
    let t = theta_deg.to_radians();
    [r * t.cos(), r * t.sin()]
}
/// Columns 1.. of `keys` at `s`, linear between the keys' shares in column 0.
fn keyed<const N: usize>(keys: &[[f64; N]], s: f64) -> [f64; N] {
    let i = keys.partition_point(|k| k[0] < s).clamp(1, keys.len() - 1);
    let (a, b) = (keys[i - 1], keys[i]);
    let f = ((s - a[0]) / (b[0] - a[0]).max(1e-12)).clamp(0.0, 1.0);
    std::array::from_fn(|k| lerp(a[k], b[k], f))
}

// --- Sections --------------------------------------------------------------------------------------------------------

/// Cubic pieces joined end to start into one closed loop, each shared end one sketch point, to a ten-thousandth of a millimetre.
fn chain(pieces: &[[[f64; 2]; 4]]) -> Sketch {
    let q = |p: [f64; 2]| p.map(|v| (v * 1e4).round() / 1e4);
    let mut s = Sketch::default();
    let first = s.point(q(pieces[0][0]));
    let mut start = first;
    for (i, p) in pieces.iter().enumerate() {
        let (c1, c2) = (s.point(q(p[1])), s.point(q(p[2])));
        let end = if i + 1 == pieces.len() { first } else { s.point(q(p[3])) };
        s.entity(Geometry::Bezier { points: [start, c1, c2, end] });
        start = end;
    }
    s
}

/// A workplane with its origin to a hundred-thousandth of a millimetre.
fn plane(origin: P3, x: P3, y: P3) -> Workplane {
    Workplane { origin: origin.map(|c| (c * 1e5).round() / 1e5), x, y, on_face: None }
}

/// A feather's section `half` either side of its rachis and `t` thick: a bellied underside, round edges, vanes cambered
/// up to a raised rachis, with a channel either side of it when `channels`.
fn vane(half: f64, t: f64, camber: f64, ridge: f64, belly: f64, channels: bool) -> Sketch {
    let e = 0.5 * t;
    let xn = (half - e).max(0.06);
    let fit = (xn / 0.6).min(1.0);
    let (camber, belly, depth, ridge) = (camber * fit, belly * fit, 0.1 * fit, ridge * fit);
    let q1 = (0.2 + 0.05 * half).min(0.32 * xn);
    let q2 = if channels { (q1 + 0.3 * (xn - q1)).min(0.7 * xn) } else { q1 };
    let rim = e + camber;
    let l = 4.0 / 3.0 * e;
    // Slope the nose's upper end turns to, rising into the outer vane.
    let slope_edge = 2.0 * camber / (xn - q2).max(1e-6);
    let lean = (1.0 + slope_edge * slope_edge).sqrt();
    let k = 4.0 / 3.0 * belly;
    let under = 1f64.hypot(k / (2.0 * xn / 3.0));
    let h1 = 4.0 / 3.0 * ridge;
    let outer = |sign: f64| -> [[f64; 2]; 4] {
        let (a, b) = ([sign * xn, e], [sign * q2, rim]);
        [a, [a[0] + 2.0 / 3.0 * (b[0] - a[0]), a[1] + 2.0 / 3.0 * (b[1] - a[1])], b, b]
    };
    let channel = |sign: f64| -> [[f64; 2]; 4] {
        let w = q2 - q1;
        [[sign * q2, rim], [sign * (q2 - 0.3 * w), rim - depth], [sign * (q1 + 0.3 * w), rim - depth], [sign * q1, rim]]
    };
    let rev = |p: [[f64; 2]; 4]| [p[3], p[2], p[1], p[0]];
    let mut pieces = vec![
        [[-xn, -e], [-xn / 3.0, -e - k], [xn / 3.0, -e - k], [xn, -e]],
        [[xn, -e], [xn + l / under, -e + l * (k / (2.0 * xn / 3.0)) / under], [xn + l / lean, e - l * slope_edge / lean], [xn, e]],
        outer(1.0),
    ];
    if channels {
        pieces.push(channel(1.0));
    }
    pieces.push([[q1, rim], [0.55 * q1, rim + h1], [-0.55 * q1, rim + h1], [-q1, rim]]);
    if channels {
        pieces.push(rev(channel(-1.0)));
    }
    pieces.push(rev(outer(-1.0)));
    pieces.push([[-xn, e], [-xn - l / lean, e - l * slope_edge / lean], [-xn - l / under, -e + l * (k / (2.0 * xn / 3.0)) / under], [-xn, -e]]);
    chain(&pieces)
}

/// An oval `a` across and `b` up its plane, fuller by `bias` on its upper side, in four cubic pieces from its +x point.
fn oval(a: f64, b: f64, bias: f64) -> Sketch {
    let k = 0.5523;
    let (top, bot) = (b * (1.0 + bias), b * (1.0 - bias));
    chain(&[
        [[a, 0.0], [a, k * top], [k * a, top], [0.0, top]],
        [[0.0, top], [-k * a, top], [-a, k * top], [-a, 0.0]],
        [[-a, 0.0], [-a, -k * bot], [-k * a, -bot], [0.0, -bot]],
        [[0.0, -bot], [k * a, -bot], [a, -k * bot], [a, 0.0]],
    ])
}

/// Half a feather's vane at share `s` of its length: a narrow quill, the vane opening and holding, narrowing past an
/// emargination `notch` (share, kept width) when it has one, closing toward the tip to `taper` of its width, then
/// rounding off.
fn feather_half(s: f64, width: f64, thick: f64, quill: f64, round: f64, notch: Option<(f64, f64)>, taper: f64) -> f64 {
    let full = 0.5 * width;
    let q = (0.5 * thick + 0.06).max(0.46);
    let open = smooth(quill, quill + 0.18, s);
    let mut hold = full * (1.0 - 0.1 * smooth(0.3, 1.0 - round, s));
    if let Some((at, keep)) = notch {
        hold *= 1.0 - (1.0 - keep) * smooth(at, at + 0.1, s);
    }
    if taper < 1.0 {
        let f = ((s - 0.5) / 0.5).clamp(0.0, 1.0);
        hold *= 1.0 - (1.0 - taper) * f.powf(1.3);
    }
    let from = 1.0 - round;
    let rnd = if s > from { (1.0 - ((s - from) / round).powi(2)).max(0.0).sqrt() } else { 1.0 };
    (q + (hold - q) * open * rnd).max(q)
}

// --- Spines ----------------------------------------------------------------------------------------------------------

/// A smooth path through its control points, measured by length, with a frame carried along it.
struct Spine {
    pts: Vec<P3>,
    run: Vec<f64>,
    tan: Vec<P3>,
    nor: Vec<P3>,
}

impl Spine {
    /// A centripetal Catmull-Rom path through `control`, its frame's normal starting as `up` squared to the path.
    fn through(control: &[P3], up: P3) -> Self {
        let n = control.len();
        let at = |i: isize| -> P3 {
            if i < 0 {
                sub(mul(control[0], 2.0), control[1])
            } else if i as usize >= n {
                sub(mul(control[n - 1], 2.0), control[n - 2])
            } else {
                control[i as usize]
            }
        };
        let mut pts = Vec::new();
        for i in 0..n - 1 {
            let (p0, p1, p2, p3) = (at(i as isize - 1), at(i as isize), at(i as isize + 1), at(i as isize + 2));
            let knot = |a: P3, b: P3| norm(sub(b, a)).sqrt().max(1e-6);
            let (t0, t1) = (0.0, knot(p0, p1));
            let t2 = t1 + knot(p1, p2);
            let t3 = t2 + knot(p2, p3);
            let mix = |a: P3, b: P3, ta: f64, tb: f64, t: f64| add(mul(a, (tb - t) / (tb - ta)), mul(b, (t - ta) / (tb - ta)));
            for k in 0..48 {
                let t = t1 + (t2 - t1) * k as f64 / 48.0;
                let (a1, a2, a3) = (mix(p0, p1, t0, t1, t), mix(p1, p2, t1, t2, t), mix(p2, p3, t2, t3, t));
                let (b1, b2) = (mix(a1, a2, t0, t2, t), mix(a2, a3, t1, t3, t));
                pts.push(mix(b1, b2, t1, t2, t));
            }
        }
        pts.push(control[n - 1]);
        let mut run = vec![0.0];
        for w in pts.windows(2) {
            run.push(run[run.len() - 1] + norm(sub(w[1], w[0])));
        }
        let m = pts.len();
        let tan: Vec<P3> = (0..m).map(|i| unit(sub(pts[(i + 1).min(m - 1)], pts[i.saturating_sub(1)]))).collect();
        let mut nor = Vec::with_capacity(m);
        let mut prev = up;
        for t in &tan {
            let v = unit(sub(prev, mul(*t, dot(prev, *t))));
            nor.push(v);
            prev = v;
        }
        Self { pts, run, tan, nor }
    }
    fn length(&self) -> f64 {
        self.run[self.run.len() - 1]
    }
    /// The point, tangent and normal `d` mm along the path.
    fn at(&self, d: f64) -> (P3, P3, P3) {
        let d = d.clamp(0.0, self.length());
        let i = self.run.partition_point(|r| *r < d).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.run[i - 1], self.run[i]);
        let f = if b > a { (d - a) / (b - a) } else { 0.0 };
        let p = add(self.pts[i - 1], mul(sub(self.pts[i], self.pts[i - 1]), f));
        let t = unit(add(mul(self.tan[i - 1], 1.0 - f), mul(self.tan[i], f)));
        let n = add(mul(self.nor[i - 1], 1.0 - f), mul(self.nor[i], f));
        (p, t, unit(sub(n, mul(t, dot(n, t)))))
    }
}

/// A round loft along `sp`: a circle of radius `r(d)` at each of `at` mm along it.
fn round_loft(sp: &Spine, at: &[f64], r: impl Fn(f64) -> f64) -> Operation {
    let sections = at
        .iter()
        .map(|&d| {
            let (p, t, n) = sp.at(d);
            let mut c = Sketch::circle((r(d) * 1e4).round() / 1e4);
            c.plane = plane(p, n, cross(t, n));
            c.into()
        })
        .collect();
    Operation::Loft { sections, meshed: true }
}

/// An oval loft along `sp`: at each (mm along, half across, half up, bias) an oval whose across axis is `across` squared to the path.
fn oval_loft(sp: &Spine, at: &[[f64; 4]], across: P3) -> Operation {
    let sections = at
        .iter()
        .map(|&[d, a, b, bias]| {
            let (p, t, _) = sp.at(d);
            let x = unit(sub(across, mul(t, dot(across, t))));
            let mut o = oval(a, b, bias);
            o.plane = plane(p, x, cross(t, x));
            o.into()
        })
        .collect();
    Operation::Loft { sections, meshed: true }
}

/// A feather lofted through `path`, its vane facing `ups`, from its quill at the first point to its rounded tip at the last.
fn path_plume(path: &[P3], ups: &[P3], width: f64, thick: f64, channels: bool, taper: f64) -> Result<Operation> {
    ensure!(thick >= MIN_SECTION_MM && path.len() == ups.len() && path.len() >= 3, "a plume needs three stations and the wax section");
    let mut run = vec![0.0];
    for w in path.windows(2) {
        run.push(run[run.len() - 1] + norm(sub(w[1], w[0])));
    }
    let len = run[run.len() - 1];
    let sections = (0..path.len())
        .map(|i| {
            let s = run[i] / len;
            let t = unit(sub(path[(i + 1).min(path.len() - 1)], path[i.saturating_sub(1)]));
            let y = unit(sub(ups[i], mul(t, dot(ups[i], t))));
            let half = feather_half(s, width, thick, 0.1, 0.42, None, taper);
            let mut v = vane(half, thick, 0.16 * half, 0.12, 0.06, channels);
            v.plane = plane(path[i], cross(y, t), y);
            v.into()
        })
        .collect();
    Ok(Operation::Loft { sections, meshed: true })
}

// --- The stone -------------------------------------------------------------------------------------------------------

/// The sapphire in its frame: the girdle's centre, its axis the ring's radial at the top, and the cut's own proportions.
struct Stone {
    g: P3,
    r: f64,
    crown: f64,
    pav: f64,
    half: f64,
}

impl Stone {
    fn new(gem: Gem, girdle_y: f64) -> Self {
        Self { g: [0.0, girdle_y, 0.0], r: 0.5 * gem.w_mm, crown: gem.crown_mm(), pav: gem.pavilion_mm(), half: setting::girdle_half_mm(gem) }
    }
    /// A point in the stone's cylinder: radius from its axis, degrees round from east toward the high cheek, height over the girdle.
    fn world(&self, rho: f64, psi: f64, h: f64) -> P3 {
        let a = psi.to_radians();
        [rho * a.cos(), self.g[1] + h, rho * a.sin()]
    }
    /// The stone's meridian as the preview draws it, culet to table.
    fn profile(&self) -> [[f64; 2]; 6] {
        let (r, c) = (self.r, self.crown);
        [[0.0, -self.pav], [r, -self.half], [r, self.half], [0.78 * r, 0.55 * c], [0.52 * r, c], [0.0, c]]
    }
    /// Distance from `p` to the stone's surface, negative inside it.
    fn gap(&self, p: P3) -> f64 {
        let q = [p[0].hypot(p[2]), p[1] - self.g[1]];
        let prof = self.profile();
        let seg = |a: [f64; 2], b: [f64; 2]| {
            let (d, e) = ([b[0] - a[0], b[1] - a[1]], [q[0] - a[0], q[1] - a[1]]);
            let t = ((e[0] * d[0] + e[1] * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-18)).clamp(0.0, 1.0);
            (e[0] - t * d[0]).hypot(e[1] - t * d[1])
        };
        let dist = prof.windows(2).map(|w| seg(w[0], w[1])).fold(f64::MAX, f64::min);
        let mut inside = false;
        for i in 0..prof.len() {
            let (a, b) = (prof[i], prof[(i + 1) % prof.len()]);
            if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
        }
        if inside { -dist } else { dist }
    }
}

// --- The wing --------------------------------------------------------------------------------------------------------

/// One lofted feather on the high cheek, its rachis from `root` to `tip` in the cheek's plane.
#[derive(Clone, Debug)]
struct Feather {
    name: String,
    root: [f64; 2],
    tip: [f64; 2],
    /// Sideways bow of the rachis at its middle, mm; positive toward the rachis' left.
    bow: f64,
    /// Vane width, mm.
    width: f64,
    /// Section thickness, mm.
    thick: f64,
    /// Belly height over the cheek at the root, mm of z.
    under: f64,
    /// Rise of the tip over the root, mm of z.
    lift: f64,
    /// Share of the length the quill takes before the vane opens.
    quill: f64,
    /// Share of the length the tip takes to round off.
    round: f64,
    /// How far the rachis follows the ring's own circle instead of its chord, 0..1.
    follow: f64,
    /// Emargination: the share where the vane narrows, and the width it keeps.
    notch: Option<(f64, f64)>,
    /// Where along the rachis its sections stand.
    stations: &'static [f64],
    /// Whether its vanes carry channels either side of the rachis.
    channels: bool,
    /// Whether its tip closes in a round dome rather than a flat end.
    dome: bool,
    /// Share of the width its leading vane (the one to the rachis' right) takes: a half for a symmetric feather.
    lead: f64,
    /// Width its tip closes to, as a share of its width: 1 for none.
    taper: f64,
}

/// Stations along a primary, dense round its emargination.
const PRIMARY: &[f64] = &[0.0, 0.12, 0.36, 0.56, 0.68, 0.86, 1.0];
/// Stations along a secondary.
const LONG: &[f64] = &[0.0, 0.08, 0.26, 0.55, 0.82, 0.95, 1.0];
/// Stations along a covert.
const SHORT: &[f64] = &[0.0, 0.25, 0.68, 1.0];
/// Stations along a marginal covert.
const TINY: &[f64] = &[0.0, 0.5, 1.0];

impl Feather {
    /// The rachis at `s`: its point in the cheek plane and its unit heading.
    fn rachis(&self, s: f64) -> ([f64; 2], [f64; 2]) {
        let chord = [self.tip[0] - self.root[0], self.tip[1] - self.root[1]];
        let len = chord[0].hypot(chord[1]);
        let left = [-chord[1] / len, chord[0] / len];
        let (t0, r0) = (self.root[1].atan2(self.root[0]), self.root[0].hypot(self.root[1]));
        let (t1, r1) = (self.tip[1].atan2(self.tip[0]), self.tip[0].hypot(self.tip[1]));
        let turn = (t1 - t0 + PI).rem_euclid(2.0 * PI) - PI;
        let at = |s: f64| {
            let b = self.bow * (PI * s).sin();
            let (th, r) = (t0 + turn * s, r0 + (r1 - r0) * s);
            let round = [r * th.cos(), r * th.sin()];
            let straight = [self.root[0] + chord[0] * s, self.root[1] + chord[1] * s];
            [lerp(straight[0], round[0], self.follow) + left[0] * b, lerp(straight[1], round[1], self.follow) + left[1] * b]
        };
        let (a, b) = (at((s - 1e-4).max(0.0)), at((s + 1e-4).min(1.0)));
        let d = [b[0] - a[0], b[1] - a[1]];
        let l = d[0].hypot(d[1]);
        (at(s), [d[0] / l, d[1] / l])
    }
    fn half(&self, s: f64) -> f64 {
        feather_half(s, self.width, self.thick, self.quill, self.round, self.notch, self.taper)
    }
    /// How far each vane reaches from the rachis against a symmetric one: (leading, trailing).
    fn sides(&self) -> (f64, f64) {
        (2.0 * self.lead, 2.0 * (1.0 - self.lead))
    }
    fn loft(&self) -> Result<Operation> {
        ensure!(self.thick >= MIN_SECTION_MM, "{}: section {} under the wax floor", self.name, self.thick);
        let inside = self
            .stations
            .iter()
            .flat_map(|&s| {
                let (p, t) = self.rachis(s);
                let (lead, trail) = self.sides();
                let (hl, ht) = (self.half(s) * trail + 0.4, self.half(s) * lead + 0.4);
                [[p[0] - t[1] * hl, p[1] + t[0] * hl], [p[0] + t[1] * ht, p[1] - t[0] * ht]]
            })
            .map(|q| q[0].hypot(q[1]))
            .fold(f64::MAX, f64::min);
        ensure!(inside >= BORE_CLEAR_R, "{} reaches {inside:.2} mm from the finger's axis, inside {BORE_CLEAR_R} mm", self.name);
        let e = 0.5 * self.thick;
        let section = |p: [f64; 2], t: [f64; 2], s: f64, k: f64| {
            let z = CHEEK + self.under + e + self.lift * smooth(0.25, 1.0, s);
            let half = self.half(s) * k;
            let mut v = vane(half, self.thick * k, 0.16 * half, 0.15 * k, 0.08 * k, self.channels);
            let (lead, trail) = self.sides();
            if lead != 1.0 {
                // The leading vane narrows and the trailing one widens about the same rachis.
                for pt in &mut v.points {
                    let x = pt.xy[0] * if pt.xy[0] < 0.0 { lead } else { trail };
                    pt.xy[0] = (x * 1e4).round() / 1e4;
                }
            }
            v.plane = splayed([p[0], p[1], z], [-t[1], t[0], 0.0], [0.0, 0.0, 1.0]);
            v.into()
        };
        let mut sections: Vec<cad::Profile> = self.stations.iter().map(|&s| {
            let (p, t) = self.rachis(s);
            section(p, t, s, 1.0)
        }).collect();
        if self.dome {
            // A round dome past the tip, the section shrinking as a ball of half its thickness.
            let (p, t) = self.rachis(1.0);
            for x in [0.8 * e] {
                let k = (1.0 - (x / e).powi(2)).sqrt();
                sections.push(section([p[0] + t[0] * x, p[1] + t[1] * x], t, 1.0, k));
            }
        }
        Ok(Operation::Loft { sections, meshed: true })
    }
}

/// The high cheek's plane bent outward above the crown: a point `h` past the hinge lies on a curve leaning out by up
/// to SPLAY_DEG, keeping its offset from the plane along the curve's normal.
fn splay(p: P3) -> P3 {
    let r = p[0].hypot(p[1]);
    let h = r - HINGE_R;
    if h <= 0.0 {
        return p;
    }
    let steps = ((h / 0.02).ceil() as usize).max(1);
    let dh = h / steps as f64;
    let (mut cr, mut cz) = (0.0, 0.0);
    for k in 0..steps {
        let a = SPLAY_DEG.to_radians() * smooth(0.0, SPLAY_RAMP, (k as f64 + 0.5) * dh);
        cr += a.cos() * dh;
        cz += a.sin() * dh;
    }
    let a = SPLAY_DEG.to_radians() * smooth(0.0, SPLAY_RAMP, h);
    let w = p[2] - CHEEK;
    let (r2, z2) = (HINGE_R + cr - w * a.sin(), CHEEK + cz + w * a.cos());
    [p[0] / r * r2, p[1] / r * r2, z2]
}

/// A section's frame carried through the splay: its origin mapped, its axes the map's own directions there, square again.
fn splayed(origin: P3, x: P3, y: P3) -> Workplane {
    let e = 1e-4;
    let o = splay(origin);
    let dir = |v: P3| unit(sub(splay(add(origin, mul(v, e))), o));
    let x2 = dir(x);
    let y1 = dir(y);
    plane(o, x2, unit(sub(y1, mul(x2, dot(x2, y1)))))
}

/// How far behind her head the wing roots, degrees: clear of her face seen from the high cheek.
const WING_BACK: f64 = 12.0;

/// The wing's leading edge from her shoulder to the wrist, (theta degrees, radius mm) at `u` in 0..1.
fn arm(u: f64) -> [f64; 2] {
    const KEYS: [[f64; 2]; 5] = [[109.0 + WING_BACK, 15.3], [115.0 + WING_BACK, 16.1], [121.0 + WING_BACK, 16.35], [127.0 + WING_BACK, 16.1], [132.5 + WING_BACK, 15.6]];
    let n = KEYS.len() - 1;
    let x = u.clamp(0.0, 1.0) * n as f64;
    let i = (x.floor() as usize).min(n - 1);
    let t = x - i as f64;
    let k = |j: isize| KEYS[j.clamp(0, n as isize) as usize];
    let (p0, p1, p2, p3) = (k(i as isize - 1), k(i as isize), k(i as isize + 1), k(i as isize + 2));
    let cr = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (-a + 3.0 * b - 3.0 * c + d) * t * t * t);
    [cr(p0[0], p1[0], p2[0], p3[0]), cr(p0[1], p1[1], p2[1], p3[1])]
}

/// The arm's own heading at `u`, degrees in the cheek plane.
fn arm_heading(u: f64) -> f64 {
    let (a, b) = (arm((u - 0.01).max(0.0)), arm((u + 0.01).min(1.0)));
    let (p, q) = (polar(a[0], a[1]), polar(b[0], b[1]));
    (q[1] - p[1]).atan2(q[0] - p[0]).to_degrees()
}

/// A feather from `root` heading `heading` degrees in the cheek plane for `reach` mm.
fn aimed(root: [f64; 2], heading: f64, reach: f64) -> [f64; 2] {
    let h = heading.to_radians();
    [root[0] + reach * h.cos(), root[1] + reach * h.sin()]
}

/// The wing on the high cheek, bottom layer first: secondaries sweeping back down the cheek, the primaries' graded,
/// emarginate fingers fanned from the wrist, then greater, lesser and two staggered rows of marginal coverts.
fn wing() -> Vec<Feather> {
    let mut out = Vec::new();
    let under = BED_Z - CHEEK;
    let base = |name: String, root: [f64; 2], tip: [f64; 2]| Feather {
        name,
        root,
        tip,
        bow: 0.0,
        width: 3.0,
        thick: 0.9,
        under,
        lift: 0.1,
        quill: 0.05,
        round: 0.3,
        follow: 0.0,
        notch: None,
        stations: SHORT,
        channels: false,
        dome: false,
        lead: 0.5,
        taper: 1.0,
    };
    // Secondaries hang from the forearm and sweep back along the crown's edge toward the palm.
    let secondaries = 6;
    for k in 0..secondaries {
        let f = k as f64 / (secondaries - 1) as f64;
        let [th, r] = arm(lerp(0.1, 0.62, f));
        out.push(Feather {
            width: 2.8,
            bow: -0.3,
            under: under + 0.02 * k as f64,
            lift: 0.25,
            quill: 0.04,
            round: 0.26,
            follow: 0.75,
            stations: LONG,
            dome: true,
            ..base(format!("Secondary {}", k + 1), polar(th, r - 0.6), polar(lerp(210.0, 162.0, f) + WING_BACK, lerp(12.3, 13.6, f)))
        });
    }
    // Primaries: graded fingers fanned from the wrist, emarginate past their middles, the middle ones longest.
    const HEADINGS: [f64; 7] = [185.0, 200.0, 215.0, 230.0, 246.0, 262.0, 278.0];
    const LENGTHS: [f64; 7] = [4.6, 6.0, 7.2, 7.6, 7.2, 6.4, 5.4];
    // Each tip stands 0.3 to 0.6 mm off its neighbours' radius, so the fan's edge steps instead of ending in a row.
    const STAGGER: [f64; 7] = [0.0, 0.45, -0.2, 0.35, -0.25, 0.4, -0.1];
    for (k, ((&heading, &length), &stagger)) in HEADINGS.iter().zip(&LENGTHS).zip(&STAGGER).enumerate() {
        let heading = heading + WING_BACK;
        let f = k as f64 / (HEADINGS.len() - 1) as f64;
        let [th, r] = arm(lerp(1.0, 0.72, f));
        let root = polar(th, r - lerp(0.4, 1.0, f));
        out.push(Feather {
            bow: 0.35,
            width: 3.2,
            thick: 0.9,
            under: under + 0.12 - 0.015 * k as f64,
            lift: 0.12,
            round: 0.16,
            notch: (k < 5).then_some((0.6, 0.8)),
            stations: PRIMARY,
            channels: k < 2,
            dome: true,
            lead: 1.0 / 3.0,
            taper: 0.3,
            ..base(format!("Primary {}", k + 1), root, aimed(root, heading, length + stagger))
        });
    }
    // Greater coverts over the flight feathers' roots, from the shoulder to the wrist.
    let greater = 5;
    for k in 0..greater {
        let f = k as f64 / (greater - 1) as f64;
        let [th, r] = arm(lerp(0.1, 0.96, f));
        let root = polar(th, r - 0.6);
        out.push(Feather {
            bow: 0.2,
            width: 3.0,
            thick: 1.2,
            under: under + 0.05 - 0.012 * k as f64,
            lift: 0.06,
            quill: 0.08,
            round: 0.36,
            ..base(format!("Greater covert {}", k + 1), root, aimed(root, arm_heading(lerp(0.1, 0.96, f)) + lerp(52.0, 62.0, f), lerp(4.2, 4.9, f)))
        });
    }
    // Lesser coverts between the greater row and the leading edge.
    let lesser = 5;
    for k in 0..lesser {
        let f = k as f64 / (lesser - 1) as f64;
        let u = lerp(0.06, 0.9, f);
        let [th, r] = arm(u);
        let root = polar(th, r - 0.2);
        out.push(Feather {
            bow: 0.1,
            width: 2.6,
            thick: 1.4,
            under: under - 0.2 - 0.012 * k as f64,
            lift: 0.04,
            quill: 0.1,
            round: 0.42,
            ..base(format!("Lesser covert {}", k + 1), root, aimed(root, arm_heading(u) + lerp(48.0, 58.0, f), lerp(2.7, 3.0, f)))
        });
    }
    // Marginal coverts: two staggered rows along the leading edge, 2.5 mm at the shoulder graded to 1.2 at the wrist, pointing back.
    for row in 0..2 {
        let count = 7 - row;
        for k in 0..count {
            let u = (k as f64 + 0.5 * row as f64) / 6.3;
            let [th, r] = arm(u);
            let len = lerp(2.5, 1.2, u);
            let root = polar(th, r + 0.25 - 0.85 * row as f64);
            out.push(Feather {
                width: (0.75 * len + 0.35).max(1.1),
                thick: 1.0,
                under: under - 0.45 - 0.1 * row as f64 - 0.01 * k as f64,
                lift: 0.05,
                quill: 0.1,
                round: 0.5,
                stations: TINY,
                ..base(format!("Marginal covert {}{}", ["a", "b"][row], k + 1), root, aimed(root, arm_heading(u) + 38.0, len))
            });
        }
    }
    out
}

// --- The figure ------------------------------------------------------------------------------------------------------

/// Torso keys: share along its spine from the tail end, half-width across the band, half-depth, and how much fuller its
/// belly is than its back.
const TORSO: [[f64; 4]; 6] = [
    [0.0, 0.9, 0.4, 0.0],
    [0.22, 1.8, 0.85, 0.1],
    [0.46, 2.65, 1.25, 0.16],
    [0.7, 3.0, 1.5, 0.2],
    [0.88, 2.75, 1.42, 0.18],
    [1.0, 1.9, 1.0, 0.1],
];
/// Her spine as (theta degrees, height over the crest mm): from its tail end sunk in the crown behind her, along her back,
/// to her chest under the head.
const TORSO_SPINE: [[f64; 2]; 5] = [[160.0, -0.25], [146.0, 0.6], [133.0, 1.45], [124.0, 2.0], [117.5, 2.25]];

/// Her head's centre, (theta degrees, radius mm).
const HEAD_AT: [f64; 2] = [110.0, 16.05];
/// How far her head bows east toward the stone, degrees.
const HEAD_BOW: f64 = 30.0;
/// How far her face turns toward the high cheek, degrees.
const HEAD_TURN: f64 = 55.0;
/// Her head's size against the unit head below, whose face runs 2.9 from brow to chin.
const HEAD_SCALE: f64 = 1.0;
/// How much longer her face runs below the eyes than the unit head's, so brow to chin comes to 2.8 mm.
const FACE_STRETCH: f64 = 1.1;
/// The unit head's outline up its axis: height, half-width, depth in front of the axis and behind it.
const HEAD_PROFILE: [[f64; 4]; 14] = [
    [-2.3, 0.78, 0.55, 0.8],
    [-2.0, 0.72, 0.95, 0.88],
    [-1.72, 0.8, 1.28, 1.0],
    [-1.4, 1.0, 1.42, 1.12],
    [-1.0, 1.2, 1.52, 1.3],
    [-0.6, 1.33, 1.58, 1.45],
    [-0.1, 1.42, 1.58, 1.62],
    [0.3, 1.45, 1.56, 1.72],
    [0.75, 1.44, 1.5, 1.76],
    [1.15, 1.38, 1.38, 1.72],
    [1.55, 1.24, 1.18, 1.56],
    [1.8, 1.08, 1.0, 1.36],
    [2.0, 0.95, 0.87, 1.18],
    [2.2, 0.82, 0.74, 1.0],
];
/// Heights of the head's slices: close through the mouth, nose, eyes and brow.
const HEAD_LEVELS: [f64; 21] = [-2.3, -1.98, -1.72, -1.52, -1.38, -1.22, -1.06, -0.86, -0.7, -0.55, -0.32, -0.1, 0.1, 0.3, 0.5, 0.72, 1.05, 1.4, 1.72, 1.98, 2.2];
/// Where across the face each slice's front passes, as shares of its half-width, from one side round to the other.
const HEAD_FRONT: [f64; 14] = [1.0, 0.86, 0.7, 0.55, 0.42, 0.3, 0.16, 0.0, -0.16, -0.3, -0.42, -0.55, -0.7, -0.86];
/// Her nose down its length: height, rise off the face, half-width.
const NOSE: [[f64; 3]; 7] = [[-0.85, 0.0, 0.22], [-0.72, 0.26, 0.3], [-0.55, 0.5, 0.32], [-0.3, 0.4, 0.27], [0.0, 0.24, 0.23], [0.25, 0.1, 0.2], [0.45, 0.0, 0.2]];

fn bump(x: f64, u: f64, cx: f64, cu: f64, sx: f64, su: f64) -> f64 {
    (-((x - cx) / sx).powi(2) - ((u - cu) / su).powi(2)).exp()
}

/// Her features over the unit head's front, at `x` across and `u` up: a brow ridge overhanging two deep sockets with the
/// eyes' balls in them, the nose from its bridge to its tip, the lips parted by the mouth's cleft, the chin and cheekbones.
fn face_relief(x: f64, u: f64) -> f64 {
    let ax = x.abs();
    let brow = 0.3 * (-((u - 0.5) / 0.11).powi(2)).exp() * (1.0 - smooth(0.8, 1.1, ax)) * (1.0 - 0.35 * (-(x / 0.2).powi(2)).exp());
    let socket = -0.5 * bump(ax, u, 0.55, 0.12, 0.28, 0.19);
    let eye = 0.2 * bump(ax, u, 0.56, 0.1, 0.13, 0.09);
    let nose = if (-0.85..=0.45).contains(&u) {
        let [_, h, w] = keyed(&NOSE, u);
        h * (-(x / w).powi(2)).exp()
    } else {
        0.0
    };
    let lips = 0.1 * bump(ax, u, 0.0, -1.06, 0.4, 0.08) + 0.12 * bump(ax, u, 0.0, -1.38, 0.34, 0.09);
    let cleft = -0.16 * bump(ax, u, 0.0, -1.22, 0.42, 0.06);
    let chin = 0.16 * bump(ax, u, 0.0, -1.72, 0.36, 0.2);
    let cheekbone = 0.1 * bump(ax, u, 0.95, -0.15, 0.25, 0.25);
    brow + socket + eye + nose + lips + cleft + chin + cheekbone
}

/// A closed Catmull-Rom loop through `pts` as cubic pieces.
fn loop_through(pts: &[[f64; 2]]) -> Sketch {
    let n = pts.len();
    let p = |i: usize| pts[i % n];
    let pieces: Vec<[[f64; 2]; 4]> = (0..n)
        .map(|i| {
            let (a, b, c, d) = (p(i + n - 1), p(i), p(i + 1), p(i + 2));
            [b, [b[0] + (c[0] - a[0]) / 6.0, b[1] + (c[1] - a[1]) / 6.0], [c[0] - (d[0] - b[0]) / 6.0, c[1] - (d[1] - b[1]) / 6.0], c]
        })
        .collect();
    chain(&pieces)
}

/// Her head: slices across its up axis, each a loop through her face in front and her skull behind.
struct Head {
    c: P3,
    up: P3,
    fwd: P3,
    side: P3,
}

impl Head {
    fn new() -> Self {
        let c = polar(HEAD_AT[0], HEAD_AT[1]);
        let b = HEAD_BOW.to_radians();
        let up = [b.sin(), b.cos(), 0.0];
        let t = HEAD_TURN.to_radians();
        let fwd = add(mul([b.cos(), -b.sin(), 0.0], t.cos()), mul(Z, t.sin()));
        Self { c: [c[0], c[1], 0.0], up, fwd, side: unit(cross(up, fwd)) }
    }
    /// The unit head's half-width, depth in front and depth behind at height `u`.
    fn profile(u: f64) -> [f64; 3] {
        let [_, w, front, back] = keyed(&HEAD_PROFILE, u);
        [w, front, back]
    }
    /// How far forward the unit head's front stands at `x` across and `u` up.
    fn front(x: f64, u: f64) -> f64 {
        let [w, dd, _] = Self::profile(u);
        let ax = (x / w).abs().min(1.0);
        dd * (1.0 - ax.powf(2.3)).max(0.0).powf(1.0 / 2.3) + face_relief(x, u) * (1.0 - smooth(0.72, 0.95, ax))
    }
    /// The unit head's slice at `u`: across and forward, round from one side through the face and back behind.
    fn ring(u: f64) -> Vec<[f64; 2]> {
        let [w, _, bb] = Self::profile(u);
        let mut pts: Vec<[f64; 2]> = HEAD_FRONT.iter().map(|&f| [f * w, Self::front(f * w, u)]).collect();
        for g in [0.0f64, 36.0, 72.0, 108.0, 144.0] {
            let g = g.to_radians();
            pts.push([-w * g.cos(), -bb * g.sin()]);
        }
        pts
    }
    /// A unit-head point in the world, the face below the eyes drawn out by FACE_STRETCH.
    fn world(&self, u: f64, x: f64, y: f64) -> P3 {
        let k = HEAD_SCALE;
        let u = if u < 0.3 { 0.3 + (u - 0.3) * FACE_STRETCH } else { u };
        add(self.c, add(mul(self.up, u * k), add(mul(self.side, x * k), mul(self.fwd, y * k))))
    }
    fn loft(&self) -> Operation {
        let sections = HEAD_LEVELS
            .iter()
            .map(|&u| {
                let pts: Vec<[f64; 2]> = Self::ring(u).iter().map(|p| [p[0] * HEAD_SCALE, p[1] * HEAD_SCALE]).collect();
                let mut sk = loop_through(&pts);
                sk.plane = plane(self.world(u, 0.0, 0.0), self.side, self.fwd);
                sk.into()
            })
            .collect();
        Operation::Loft { sections, meshed: true }
    }
    /// Every point the slices pass through, for measuring her against the stone.
    fn samples(&self) -> Vec<P3> {
        let mut out = Vec::new();
        for i in 0..=46 {
            let u = lerp(-2.3, 2.3, i as f64 / 46.0);
            for p in Self::ring(u) {
                out.push(self.world(u, p[0], p[1]));
            }
        }
        out
    }
    /// Whether a unit-head point (up, across, forward) lies inside her head.
    fn inside(u: f64, x: f64, y: f64) -> bool {
        if !(HEAD_LEVELS[0]..=HEAD_LEVELS[HEAD_LEVELS.len() - 1]).contains(&u) {
            return false;
        }
        let [w, _, bb] = Self::profile(u);
        if x.abs() >= w {
            return false;
        }
        if y >= 0.0 { y <= Self::front(x, u) } else { (x / w).powi(2) + (y / bb).powi(2) <= 1.0 }
    }
    /// Where a ray from the head's centre, `eta` degrees from her face up over the crown and `lambda` toward `side`,
    /// leaves the head, with the ray's direction there.
    fn skull(&self, eta: f64, lambda: f64) -> (P3, P3) {
        let (se, ce) = eta.to_radians().sin_cos();
        let (sl, cl) = lambda.to_radians().sin_cos();
        let d = [cl * se, sl, cl * ce];
        let (mut lo, mut hi) = (0.0, 3.0);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if Self::inside(d[0] * m, d[1] * m, d[2] * m) { lo = m } else { hi = m }
        }
        (self.world(d[0] * lo, d[1] * lo, d[2] * lo), unit(add(mul(self.up, d[0]), add(mul(self.side, d[1]), mul(self.fwd, d[2])))))
    }
    /// Her hair, parted over the brow: six round locks laid side by side, swept back over the skull and falling from
    /// her nape down her back with a wave.
    fn hair(&self) -> Result<Vec<(String, Operation, P3)>> {
        let mut out = Vec::new();
        let r = 0.5;
        for (k, lambda) in [12.0f64, -12.0, 39.0, -39.0, 66.0, -66.0].into_iter().enumerate() {
            let mut path = Vec::new();
            let start = if lambda.abs() < 60.0 { 60.0 + 0.25 * lambda.abs() } else { 66.0 };
            for (i, eta) in [start, 100.0, 140.0, 180.0].into_iter().enumerate() {
                let (p, n) = self.skull(eta, lambda * lerp(1.0, 0.72, i as f64 / 3.0));
                // The lock rises out of the scalp at the hairline.
                path.push(add(p, mul(n, r - if i == 0 && lambda.abs() < 60.0 { 0.85 } else { 0.56 })));
            }
            let fall = unit(add(mul(self.up, -1.0), mul(self.fwd, -0.3)));
            let last = path[3];
            let wave = mul(self.side, 0.25 * lambda.signum());
            path.push(add(add(last, mul(fall, 1.2)), wave));
            path.push(add(last, mul(fall, 2.3)));
            let sp = Spine::through(&path, self.side);
            let len = sp.length();
            let mut at: Vec<f64> = (0..=5).map(|i| (len - r) * i as f64 / 5.0).collect();
            at.extend([len - 0.5 * r, len - 0.12 * r]);
            let radius = |d: f64| if d > len - r { let x = (d - (len - r)) / r; r * (1.0 - x * x).max(0.0).sqrt() } else { r };
            out.push((format!("Hair lock {}", k + 1), round_loft(&sp, &at, radius), self.up));
        }
        Ok(out)
    }
}

/// The harpy's body: torso, neck and head, west of the stone in the band's mid-plane.
struct Figure {
    torso: Spine,
    neck: Spine,
    head: Head,
}

const Z: P3 = [0.0, 0.0, 1.0];

impl Figure {
    fn new(crest: f64) -> Self {
        let spine: Vec<P3> = TORSO_SPINE
            .iter()
            .map(|&[th, h]| {
                let p = polar(th, crest + h);
                [p[0], p[1], 0.0]
            })
            .collect();
        let torso = Spine::through(&spine, Z);
        let head = Head::new();
        let (chest, _, _) = torso.at(0.9 * torso.length());
        let root = add(chest, [0.0, 0.25, 0.0]);
        let top = head.world(-1.45, 0.0, -0.3);
        let mid = add(mul(add(root, top), 0.5), mul(head.fwd, -0.2));
        let neck = Spine::through(&[root, mid, top], Z);
        Self { torso, neck, head }
    }
    fn torso_keys(s: f64) -> [f64; 3] {
        let k = keyed(&TORSO, s);
        [k[1], k[2], k[3]]
    }
    /// The torso's skin at share `s` and `phi` degrees round from the belly toward the high cheek, with its outward normal.
    fn torso_skin(&self, s: f64, phi: f64) -> (P3, P3) {
        let (p, t, _) = self.torso.at(s * self.torso.length());
        let (x, y) = (Z, cross(t, Z));
        let [a, b, bias] = Self::torso_keys(s);
        let (sp, cp) = phi.to_radians().sin_cos();
        let bb = if cp >= 0.0 { b * (1.0 + bias) } else { b * (1.0 - bias) };
        (add(p, add(mul(x, a * sp), mul(y, bb * cp))), unit(add(mul(x, sp / a), mul(y, cp / bb))))
    }
    fn neck_r(s: f64) -> f64 {
        lerp(1.05, 0.88, s)
    }
    /// The nearest her torso, neck and head come to the stone, sampled round their sections.
    fn stone_gap(&self, st: &Stone) -> f64 {
        let mut nearest = self.head.samples().iter().map(|p| st.gap(*p)).fold(f64::MAX, f64::min);
        for i in 0..=40 {
            let s = i as f64 / 40.0;
            for k in 0..24 {
                let phi = k as f64 * 15.0;
                nearest = nearest.min(st.gap(self.torso_skin(s, phi).0));
                let (p, t, nn) = self.neck.at(s * self.neck.length());
                let b = cross(t, nn);
                let (sp, cp) = phi.to_radians().sin_cos();
                let r = Self::neck_r(s);
                nearest = nearest.min(st.gap(add(p, add(mul(nn, r * cp), mul(b, r * sp)))));
            }
        }
        nearest
    }
    fn torso_loft(&self) -> Operation {
        let len = self.torso.length();
        let at: Vec<[f64; 4]> = (0..=10)
            .map(|i| {
                let s = i as f64 / 10.0;
                let [a, b, bias] = Self::torso_keys(s);
                [s * len, a, b, bias]
            })
            .collect();
        oval_loft(&self.torso, &at, Z)
    }
    fn neck_loft(&self) -> Operation {
        let len = self.neck.length();
        let at: Vec<f64> = (0..=4).map(|i| len * i as f64 / 4.0).collect();
        round_loft(&self.neck, &at, |d| Self::neck_r(d / len))
    }
    /// Lanceolate contour feathers laid flat on her breast and mantle, each flowing down the torso toward the tail.
    fn plumes(&self) -> Result<Vec<(String, Operation, P3)>> {
        let mut out = Vec::new();
        let rows: [(&str, f64, &[f64]); 5] = [
            ("Breast", 0.99, &[0.0, 50.0, -50.0]),
            ("Breast", 0.8, &[85.0, -85.0]),
            ("Mantle", 0.92, &[180.0, 145.0, -145.0]),
            ("Mantle", 0.7, &[162.0, -162.0]),
            ("Mantle", 0.48, &[180.0]),
        ];
        let mut n = HashMap::new();
        for (what, s, phis) in rows {
            for &phi in phis {
                let thick = 0.85;
                let e = 0.5 * thick - 0.3;
                let mut path = Vec::new();
                let mut ups = Vec::new();
                for (i, si) in [s, s - 0.13, s - 0.26].into_iter().enumerate() {
                    let (p, nrm) = self.torso_skin(si, phi);
                    path.push(add(p, mul(nrm, e - 0.08 * i as f64)));
                    ups.push(nrm);
                }
                let up = ups[1];
                let count = n.entry(what).or_insert(0);
                *count += 1;
                out.push((format!("{what} feather {count}"), path_plume(&path, &ups, 1.5, thick, false, 0.35)?, up));
            }
        }
        Ok(out)
    }
}

// --- The feet --------------------------------------------------------------------------------------------------------

/// The high foot's ankle in the stone's cylinder: radius, degrees round from east toward the high cheek, height over the
/// girdle. It sits low beside the band's edge, so the toes fan up from it to the girdle apart from one another.
const ANKLE: [f64; 3] = [5.35, 118.0, -2.9];
/// How near the stone a toe runs where its gap to its neighbours counts: along the girdle, where round 2's toes closed
/// into a ring. Nearer the ankle they spring from one pad.
const GIRDLE_BAND: f64 = 1.3;
/// The knee from the ankle, mm: tucked under her flank, the long bare tarsus thrust forward and down from it to the foot,
/// a raptor's strike.
const KNEE: P3 = [-3.2, 1.3, -0.85];
/// Radius and height where each claw leaves its toe, below the girdle.
const CLAW_BASE: [f64; 2] = [4.8, -1.6];
/// A toe's radius where it leaves the ankle, mm: 1.2 mm across.
const TOE_R: f64 = 0.62;
/// A claw's half-thickness across its curl and in it where it leaves its sheath, mm: 0.96 x 1.2, stepping 0.14 in its
/// curl and 0.26 across under the collar at the toe's end.
const CLAW_AB: [f64; 2] = [0.48, 0.6];

/// A toe of the high foot, in order round the girdle from the hallux behind the ankle to the middle toe reaching
/// farthest forward: where its claw hooks over the girdle, how far its knuckles bow out, where its tip rests on the
/// crown, and how far its claw curls, degrees.
struct Toe {
    name: &'static str,
    hook_psi: f64,
    bow: f64,
    contact_rho: f64,
    curl: f64,
}

const TOES: [Toe; 4] = [
    Toe { name: "hallux", hook_psi: 143.0, bow: 0.45, contact_rho: 3.45, curl: 108.0 },
    Toe { name: "inner toe", hook_psi: 117.0, bow: 0.2, contact_rho: 3.55, curl: 94.0 },
    Toe { name: "outer toe", hook_psi: 94.0, bow: 0.25, contact_rho: 3.62, curl: 70.0 },
    Toe { name: "middle toe", hook_psi: 71.0, bow: 0.35, contact_rho: 3.25, curl: 106.0 },
];

/// The high foot: its scaled tarsus, four knuckled toes fanned up from the ankle, and a sheathed claw hooking each over the girdle.
struct Foot {
    parts: Vec<(String, Operation)>,
    /// Each claw's curl over its last 2 mm, degrees.
    curls: Vec<(String, f64)>,
    /// Each toe's length from the ankle to its claw's tip, mm.
    lengths: Vec<(String, f64)>,
    /// The nearest any toe, claw or the tarsus comes to the stone, mm.
    stone_gap: f64,
    /// The nearest two neighbouring toes come to each other along the girdle, mm.
    toe_gap: f64,
}

/// How far a spine's tangent turns over its last `span` mm, degrees.
fn end_turn(sp: &Spine, span: f64) -> f64 {
    let (_, a, _) = sp.at(sp.length() - span);
    let (_, b, _) = sp.at(sp.length());
    dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// A claw's half-thickness in its curl at `d` of its `len`: from the sheath to the start of its round tip.
fn claw_b(d: f64, len: f64) -> f64 {
    let dome = len - TIP_R;
    lerp(CLAW_AB[1], TIP_R, smooth(0.0, dome, d))
}

fn foot(st: &Stone, fig: &Figure) -> Result<Foot> {
    let mut parts = Vec::new();
    let mut curls = Vec::new();
    let mut lengths = Vec::new();
    let mut nearest = f64::MAX;
    // Each toe's centre line with its radius, sampled, for the gaps between them.
    let mut lines: Vec<Vec<(P3, f64)>> = Vec::new();
    let ankle = st.world(ANKLE[0], ANKLE[1], ANKLE[2]);
    // The tarsus from her thigh down to the ankle, shingled scutes down its front and a round ankle joint.
    // The feathered thigh from her flank out to the knee, then the bare tarsus down from the knee to the ankle.
    let (hip, hn) = fig.torso_skin(0.74, 70.0);
    let hip = sub(hip, mul(hn, 0.6));
    let knee = add(ankle, KNEE);
    let past = add(knee, mul(unit(sub(knee, hip)), 0.8));
    let thigh = Spine::through(&[hip, add(mul(add(hip, knee), 0.5), [0.0, 0.35, 0.3]), knee, past], [0.0, 1.0, 0.0]);
    // A drumstick of a thigh, dressed in three lanceolate feathers laid down it toward the knee.
    // It rounds off over the knee.
    let tl = thigh.length() - 0.8;
    let thigh_r = |d: f64| {
        let r = lerp(1.1, 0.85, smooth(0.0, tl, d));
        if d > tl { r * (1.0 - ((d - tl) / 0.8).powi(2)).max(0.0).sqrt() } else { r }
    };
    for i in 0..=20 {
        let d = tl * i as f64 / 20.0;
        nearest = nearest.min(st.gap(thigh.at(d).0) - thigh_r(d));
    }
    let mut at: Vec<f64> = (0..=4).map(|i| tl * i as f64 / 4.0).collect();
    at.extend([tl + 0.45, tl + 0.74]);
    parts.push(("Thigh".to_string(), round_loft(&thigh, &at, thigh_r)));
    for (k, around) in [0.0f64, 55.0, -55.0].into_iter().enumerate() {
        let thick = 0.84;
        let (mut path, mut ups) = (Vec::new(), Vec::new());
        for (i, share) in [0.08, 0.5, 0.97].into_iter().enumerate() {
            let d = share * tl;
            let (p, t, _) = thigh.at(d);
            let out = unit(sub([0.2, 0.55, 1.0], mul(t, dot([0.2, 0.55, 1.0], t))));
            let side = cross(t, out);
            let (sa, ca) = around.to_radians().sin_cos();
            let n = unit(add(mul(out, ca), mul(side, sa)));
            path.push(add(p, mul(n, thigh_r(d) + 0.5 * thick - 0.3 - 0.1 * i as f64)));
            ups.push(n);
        }
        let width = if k == 0 { 1.5 } else { 1.25 };
        parts.push((format!("Thigh feather {}", k + 1), path_plume(&path, &ups, width, thick, false, 0.35)?));
    }
    let heading = unit(sub(ankle, knee));
    let tarsus = Spine::through(&[sub(knee, mul(heading, 0.3)), add(mul(add(knee, ankle), 0.5), [0.0, 0.1, 0.12]), ankle, add(ankle, mul(heading, 0.4))], [0.0, 1.0, 0.0]);
    let len = tarsus.length();
    let joint = len - 0.4;
    const PITCH: f64 = 0.62;
    const STEP: f64 = 0.12;
    let scutes = (((joint - 0.5) / PITCH).floor() as usize).min(3);
    let scuted = joint - scutes as f64 * PITCH;
    let mut at: Vec<(f64, f64)> = vec![(0.0, 0.0), (scuted - 0.02, 0.0)];
    for k in 0..scutes {
        let root = scuted + k as f64 * PITCH;
        at.extend([(root + 0.03, 0.0), (root + 0.55 * PITCH, 0.55), (root + PITCH - 0.03, 1.0)]);
    }
    at.extend([(joint + 0.02, 0.0), (joint + 0.22, 0.0), (joint + 0.33, 0.0), (len - 0.01, 0.0)]);
    let radius = |d: f64| -> f64 {
        let r = lerp(0.62, 0.7, smooth(0.0, joint, d));
        if d > joint {
            let x = (d - joint) / (len - joint);
            r * (1.0 - (0.92 * x).powi(2)).max(0.05).sqrt()
        } else {
            r
        }
    };
    let front = unit([0.0, 0.45, 1.0]);
    let sections = at
        .iter()
        .map(|&(d, s)| {
            let (p, t, n) = tarsus.at(d);
            let f = unit(sub(front, mul(t, dot(front, t))));
            let r = radius(d) + 0.5 * STEP * s;
            let centre = add(p, mul(f, 0.5 * STEP * s));
            nearest = nearest.min(st.gap(centre) - r);
            let mut c = Sketch::circle((r * 1e4).round() / 1e4);
            c.plane = plane(centre, n, cross(t, n));
            c.into()
        })
        .collect();
    parts.push(("Tarsus".to_string(), Operation::Loft { sections, meshed: true }));
    let slope = (0.55 * st.crown - st.half) / (0.22 * st.r);
    let facet_n = [slope / slope.hypot(1.0), 1.0 / slope.hypot(1.0)];
    for toe in &TOES {
        let psi = toe.hook_psi;
        let meridian = |rho: f64, h: f64| st.world(rho, psi, h);
        // The toe: from the ankle, bowed out at its knuckles, up to the claw's base.
        let knuckle = st.world(lerp(ANKLE[0], CLAW_BASE[0], 0.5) + toe.bow, lerp(ANKLE[1], psi, 0.55), lerp(ANKLE[2], CLAW_BASE[1], 0.4));
        let control = [ankle, knuckle, meridian(CLAW_BASE[0], CLAW_BASE[1])];
        let sp = Spine::through(&control, [0.0, 1.0, 0.0]);
        let tl = sp.length();
        // Three phalanges, each swelling to a knuckle pad at its joint, the last to the claw's sheath; a shingled
        // scute over the top of each.
        let joints = [0.36 * tl, 0.7 * tl];
        let r = |d: f64| {
            let knob: f64 = joints.iter().map(|j| (-((d - j) / 0.22).powi(2)).exp()).sum();
            TOE_R - 0.04 * smooth(0.0, 0.5 * tl, d) + 0.08 * knob + 0.16 * smooth(tl - 0.3, tl - 0.05, d)
        };
        let mut shingles: Vec<(f64, f64)> = vec![(0.0, 0.0)];
        let edges = [0.0, joints[0], joints[1], tl];
        for w in edges.windows(2) {
            let (a, b) = (w[0] + 0.1, w[1] - 0.06);
            shingles.extend([(a, 0.0), (lerp(a, b, 0.55), 0.6), (b, 1.0)]);
        }
        shingles.push((tl, 0.0));
        shingles.dedup_by(|a, b| (a.0 - b.0).abs() < 0.05);
        let mut line = Vec::new();
        for i in 0..=40 {
            let d = tl * i as f64 / 40.0;
            let (p, _, _) = sp.at(d);
            nearest = nearest.min(st.gap(p) - r(d));
            if st.gap(p) < GIRDLE_BAND {
                line.push((p, r(d)));
            }
        }
        let top = unit([st.world(1.0, psi, 0.0)[0], 1.2, st.world(1.0, psi, 0.0)[2]]);
        let sections = shingles
            .iter()
            .map(|&(d, s)| {
                let (p, t, n) = sp.at(d);
                let f = unit(sub(top, mul(t, dot(top, t))));
                let rr = r(d) + 0.5 * 0.1 * s;
                let mut c = Sketch::circle((rr * 1e4).round() / 1e4);
                c.plane = plane(add(p, mul(f, 0.05 * s)), n, cross(t, n));
                c.into()
            })
            .collect();
        parts.push((format!("Toe, {}", toe.name), Operation::Loft { sections, meshed: true }));
        // The claw: out of the toe's end in a sheath step, up the girdle's side and hooked over its rim onto the crown.
        let contact = [toe.contact_rho, st.half + (st.r - toe.contact_rho) * slope];
        let lift = TIP_R + STONE_GAP + 0.03;
        let tip = [contact[0] + facet_n[0] * lift, contact[1] + facet_n[1] * lift];
        let a = (90.0 + toe.curl).to_radians();
        let dir = [a.cos(), a.sin()];
        let (q0, q3) = (CLAW_BASE, tip);
        let q1 = [q0[0] + 0.05, q0[1] + 1.6];
        let q2 = [q3[0] - 0.9 * dir[0], q3[1] - 0.9 * dir[1]];
        let bez = |t: f64| {
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            [w[0] * q0[0] + w[1] * q1[0] + w[2] * q2[0] + w[3] * q3[0], w[0] * q0[1] + w[1] * q1[1] + w[2] * q2[1] + w[3] * q3[1]]
        };
        let mut pts = vec![meridian(CLAW_BASE[0], CLAW_BASE[1] - 0.3)];
        pts.extend((0..=10).map(|i| {
            let q = bez(i as f64 / 10.0);
            meridian(q[0], q[1])
        }));
        let hooked = Spine::through(&pts, [0.0, 1.0, 0.0]);
        curls.push((format!("Claw, {}", toe.name), end_turn(&hooked, 2.0)));
        let dome_at = hooked.length();
        pts.push(meridian(tip[0] + 0.45 * dir[0], tip[1] + 0.45 * dir[1]));
        let claw = Spine::through(&pts, [0.0, 1.0, 0.0]);
        let len = dome_at + TIP_R;
        let mut at: Vec<[f64; 4]> = [0.0, 0.3, 0.3 + 0.25 * (dome_at - 0.3), 0.3 + 0.5 * (dome_at - 0.3), 0.3 + 0.75 * (dome_at - 0.3)]
            .iter()
            .map(|&d| [d, CLAW_AB[0], claw_b(d, len), 0.0])
            .collect();
        for x in [0.0, 0.2, 0.33, 0.4] {
            let k = (1.0 - (x / TIP_R).powi(2)).max(0.0).sqrt();
            at.push([dome_at + x, TIP_R * k, TIP_R * k, 0.0]);
        }
        let across = [-psi.to_radians().sin(), 0.0, psi.to_radians().cos()];
        for &[d, _, b, _] in at.iter().filter(|s| s[0] < dome_at) {
            let (p, _, _) = claw.at(d);
            nearest = nearest.min(st.gap(p) - b.max(CLAW_AB[0]));
            if st.gap(p) < GIRDLE_BAND {
                line.push((p, b.max(CLAW_AB[0])));
            }
        }
        let (tip_p, _, _) = claw.at(dome_at);
        nearest = nearest.min(st.gap(tip_p) - TIP_R);
        line.push((tip_p, TIP_R));
        lengths.push((toe.name.to_string(), tl + dome_at - 0.3 + TIP_R));
        lines.push(line);
        parts.push((format!("Claw, {}", toe.name), oval_loft(&claw, &at, across)));
    }
    let mut toe_gap = f64::MAX;
    for w in lines.windows(2) {
        for (p, r) in &w[0] {
            for (q, s) in &w[1] {
                toe_gap = toe_gap.min(norm(sub(*p, *q)) - r - s);
            }
        }
    }
    Ok(Foot { parts, curls, lengths, stone_gap: nearest, toe_gap })
}

// --- Painting --------------------------------------------------------------------------------------------------------

/// Whether `theta` lies in `from..to` (degrees, wrapping): 1 inside, easing to 0 over `fade` degrees outside either end.
fn arc(theta: f64, from: f64, to: f64, fade: f64) -> f64 {
    let span = (to - from).rem_euclid(360.0);
    let d = (theta - from).rem_euclid(360.0);
    if d <= span {
        return 1.0;
    }
    let outside = (d - span).min(360.0 - d);
    1.0 - smooth(0.0, fade, outside)
}

/// Rounded shingles pointing toward +`along`: 0 at each one's root rising to 1 at its free edge, where across it the point
/// sits (-0.5..0.5), and how far ahead its free edge lies, mm.
fn shingle(along: f64, across: f64, pa: f64, pc: f64, dip: f64) -> (f64, f64, f64) {
    shingle_shaped(along, across, pa, pc, dip, 2.0, 0.0)
}

/// Shingles whose free edge falls back `dip` of a row at their sides as the power `pow` of the distance from the middle
/// (2 round, nearer 1 pointed), and is cut into four barb teeth a side `serr` of a row deep.
fn shingle_shaped(along: f64, across: f64, pa: f64, pc: f64, dip: f64, pow: f64, serr: f64) -> (f64, f64, f64) {
    let edge = |k: i64| {
        let shift = if k.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let w = (across / pc + shift).rem_euclid(1.0) - 0.5;
        let tooth = (2.0 * w).abs() * 4.0;
        let saw = 1.0 - 2.0 * (tooth - tooth.floor() - 0.5).abs();
        ((k + 1) as f64 * pa - dip * pa * (2.0 * w).abs().powf(pow) - serr * pa * saw, w)
    };
    let k = (along / pa).floor() as i64;
    let mut prev = edge(k - 2).0;
    for j in k - 1..=k + 2 {
        let (e, w) = edge(j);
        if e > along {
            let t = ((along - prev) / (e - prev).max(1e-9)).clamp(0.0, 1.0);
            return (t, w, e - along);
        }
        prev = e;
    }
    (0.0, 0.0, 0.0)
}

/// A contour feather's relief from its shingle coordinates: a convex vane rising to a rolled free edge, a raised rachis
/// down its middle `pc` mm wide feather.
fn contour(t: f64, w: f64, ahead: f64, pc: f64) -> f64 {
    let floor = 0.22;
    let vane = floor + (1.0 - floor) * t.powf(0.7) * (1.0 - 0.18 * (2.0 * w).powi(2));
    let rachis = 0.12 * (1.0 - smooth(0.1, 0.16, (w * pc).abs())) * smooth(0.1, 0.35, t);
    let roll = smooth(0.0, 0.14, ahead);
    floor + (vane + rachis - floor).max(0.0) * roll
}

/// Contour feathers pointing toward +`along`, graded from `p0` mm long where `along` is 0 by `k` mm more per mm on, each
/// `ratio` of its length wide: rows counted by the local length so every feather keeps its own outline as they grow.
fn graded(along: f64, across: f64, p0: f64, k: f64, ratio: f64) -> f64 {
    let along = along.max(0.0);
    let p = p0 + k * along;
    let rows = (1.0 + k * along / p0).ln() / k;
    let (t, w, ahead) = shingle_shaped(rows, across / p, 1.0, ratio, 0.75, 1.6, 0.0);
    contour(t, w, ahead * p, ratio * p)
}

/// The tail at the palm, `along` mm from its rump and `across` the section: nine rectrices fanned over crown and cheeks,
/// the centre one on top, each with its own outline, a raised rachis and a rounded tip. None where no rectrix lies.
fn tail_fan(along: f64, across: f64) -> Option<f64> {
    if !(0.0..16.5).contains(&along) {
        return None;
    }
    let mut h: Option<f64> = None;
    for k in -4i32..=4 {
        let (sa, ca) = (k as f64 * 8.4).to_radians().sin_cos();
        let (l, c) = (along * ca + across * sa, -along * sa + across * ca);
        let len = 14.2 - 0.16 * (k * k) as f64;
        if l < 0.3 || l > len {
            continue;
        }
        let half = 0.5 + 0.5 * smooth(0.3, 4.5, l);
        let tip = if l > len - 1.2 { (1.0 - ((l - (len - 1.2)) / 1.2).powi(2)).max(0.0).sqrt() } else { 1.0 };
        let hw = half * tip;
        if c.abs() >= hw {
            continue;
        }
        let base = 1.0 - 0.12 * k.abs() as f64;
        let vane = 0.6 + 0.4 * (1.0 - (c / hw).powi(2)).max(0.0).sqrt();
        let edge = smooth(0.0, 0.14, hw - c.abs()) * smooth(0.0, 0.14, len - l);
        let rachis = 0.2 * (1.0 - smooth(0.07, 0.14, c.abs())) * smooth(0.6, 2.0, l);
        let v = base * (0.2 + (vane + rachis - 0.2) * edge) * smooth(0.0, 1.0, along);
        h = Some(h.map_or(v, |o| o.max(v)));
    }
    h
}

/// Where the breast plumage starts under the stone's east girdle, degrees.
const BREAST_FROM: f64 = 78.0;
/// Where it ends under the tail, degrees.
const BREAST_TO: f64 = 256.0;

/// The harpy's plumage over the bare band: her mantle and the cheek feathers under her wings west of the stone, her
/// breast and belly feathers flowing east from under the stone round to the tail, and the tail fanned at the palm.
fn plumage_at(a: &Atlas, s: &Sample, crest: f64) -> f64 {
    let r = s.p[0].hypot(s.p[1]);
    let theta = s.theta;
    let u = theta.to_radians() * crest;
    let across = s.v - 0.5 * a.span;
    let cheek = smooth(0.55, 0.8, s.n[2].abs());
    let crown = 1.0 - cheek;
    let clear = smooth(a.bore + 0.75, a.bore + 1.35, r);
    // Mantle behind her: contour feathers flowing from her body toward the tail.
    let (t, w, ahead) = shingle(u, across, 2.9, 2.2, 0.7);
    let mantle = contour(t, w, ahead, 2.2) * crown * arc(theta, 118.0, 244.0, 5.0);
    // Cheek feathers under the wings' trailing edge, on past it under the tail to the palm.
    let (tc, wc, ac) = shingle(theta.to_radians() * r, r, 2.4, 1.6, 0.7);
    let cheeks = contour(tc, wc, ac, 1.6) * cheek * clear * arc(theta, 84.0, BREAST_TO, 4.0);
    // Breast and belly: contour feathers flowing east from under the stone, 1.6 mm long there and 3.0 by θ 340, pointing
    // away from the stone on the crown and down both cheeks, on under the tail.
    let east = (BREAST_FROM - theta).rem_euclid(360.0).to_radians();
    let span = arc(theta, BREAST_TO, BREAST_FROM + 2.0, 4.0);
    let breast = graded(east * crest, across, 1.6, 0.064, 0.72) * crown * span;
    let belly = graded(east * r, r - a.bore, 1.6, 0.064, 0.66) * cheek * clear * span;
    let rest = mantle.max(cheeks).max(breast).max(belly);
    // The tail lies over everything it covers; round its rectrices the plumage it overlaps sits lower.
    let rump = 228.0;
    match tail_fan((theta - rump).rem_euclid(360.0).to_radians() * crest, across) {
        Some(tail) => tail * clear,
        None => rest * lerp(1.0, 0.55, arc(theta, rump, 305.0, 6.0)),
    }
}

/// Paints the plumage on the bare band's atlas with square texels and adds it as one layer.
fn plumage(d: &mut RingDesign, lib: &mut AlphaLibrary, crest: f64) -> Result<()> {
    let span = d.reference_loop().surface_len_mm;
    let height = ((ATLAS_W as f64) * span / (2.0 * PI * crest)).round() as usize;
    let a = Atlas::of(d, ATLAS_W, height)?;
    let mut alpha = a.paint("Harpyia plumage", |s| plumage_at(&a, s, crest));
    for v in &mut alpha.data {
        *v = (*v * 255.0).round() / 255.0;
    }
    lib.insert(Alpha::from_png16(alpha.name.clone(), &alpha.to_png16()?)?);
    let mut e = skin::hide_layer(d, "Harpyia plumage", PLUMAGE_MM, Window::around(90.0, 360.0));
    e.name = "Plumage".into();
    d.layers.layers.push(e);
    Ok(())
}

// --- Assembly --------------------------------------------------------------------------------------------------------

/// How a part's land width is measured: a plate from its belly toward `up`, or a round body in every direction.
#[derive(Clone, Copy, Debug)]
enum Family {
    Plate(P3),
    Round,
}

/// What the authoring made beside the design: the stone's frame, the feathers, the foot's measurements and each part's family.
struct Authored {
    /// Where her face is, to frame a close-up on.
    head: P3,
    stone: Stone,
    feathers: Vec<Feather>,
    figure_stone_gap: f64,
    foot_curls: Vec<(String, f64)>,
    toe_lengths: Vec<(String, f64)>,
    foot_stone_gap: f64,
    toe_gap: f64,
    families: HashMap<String, Family>,
    copies: HashMap<String, Vec<String>>,
}

/// Appends parts to the document, numbering them on and noting how each is measured.
struct Parts {
    doc: Document,
    id: u64,
    families: HashMap<String, Family>,
    copies: HashMap<String, Vec<String>>,
}

impl Parts {
    fn add(&mut self, name: String, operation: Operation, family: Family) -> Result<u64> {
        self.doc.append(Feature { id: self.id, name: name.clone(), enabled: true, operation, component: part() })?;
        self.families.insert(name, family);
        self.id += 1;
        Ok(self.id - 1)
    }
    /// The parts `from` reflected across the band's mid-plane, as one part.
    fn mirror(&mut self, name: String, from: &[u64], family: Family) -> Result<u64> {
        let names: Vec<String> = from.iter().filter_map(|i| self.doc.feature(*i).map(|f| f.name.clone())).collect();
        self.copies.insert(name.clone(), names);
        self.add(name, Operation::Pattern { sources: Sources(from.to_vec()), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, family)
    }
}

/// The stone, its seat, the figure, both feet and both wings as the design's parts.
fn parts(d: &mut RingDesign) -> Result<Authored> {
    let gem = sapphire();
    let crest = d.reference_loop().crest_radius_mm;
    let at = Placement::ring(90.0, STAND_MM);
    let g = at.frame(d)?.origin[1];
    let stone = Stone::new(gem, g);
    let fig = Figure::new(crest);
    if std::env::var("HARPYIA_DEBUG").is_ok() {
        eprintln!("crest {crest:.3} girdle y {g:.3} stone r {:.3} crown {:.3} pavilion {:.3} girdle half {:.3}", stone.r, stone.crown, stone.pav, stone.half);
    }
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Band".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    doc.append(builders::stone_feature(2, gem, at))?;
    doc.append(builders::feature_on(3, "Seat bur", BUR, 2, json!({"through": true})))?;
    let mut p = Parts { doc, id: 4, families: HashMap::new(), copies: HashMap::new() };
    p.add("Torso".into(), fig.torso_loft(), Family::Round)?;
    p.add("Neck".into(), fig.neck_loft(), Family::Round)?;
    p.add("Head".into(), fig.head.loft(), Family::Round)?;
    for (name, op, _) in fig.head.hair()? {
        p.add(name, op, Family::Round)?;
    }
    for (name, op, up) in fig.plumes()? {
        p.add(name, op, Family::Plate(up))?;
    }
    let f = foot(&stone, &fig)?;
    let mut feet = Vec::new();
    for (name, op) in f.parts {
        feet.push(p.add(format!("High {}", name.to_lowercase()), op, Family::Round)?);
    }
    let feathers = wing();
    let mut sources = Vec::new();
    for w in &feathers {
        sources.push(p.add(w.name.clone(), w.loft()?, Family::Plate(Z))?);
    }
    p.mirror("Low foot".into(), &feet, Family::Round)?;
    for (k, chunk) in sources.chunks(cad::pattern::MAX_PATTERN_SOURCES).enumerate() {
        p.mirror(format!("Low wing, {}", k + 1), chunk, Family::Plate([0.0, 0.0, -1.0]))?;
    }
    d.cad = Some(p.doc);
    let figure_stone_gap = fig.stone_gap(&stone);
    let head = fig.head.world(-0.6, 0.0, 0.0);
    Ok(Authored {
        head,
        stone,
        feathers,
        figure_stone_gap,
        foot_curls: f.curls,
        toe_lengths: f.lengths,
        foot_stone_gap: f.stone_gap,
        toe_gap: f.toe_gap,
        families: p.families,
        copies: p.copies,
    })
}

fn author() -> Result<(RingDesign, AlphaLibrary, Authored)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    let crest = d.reference_loop().crest_radius_mm;
    plumage(&mut d, &mut lib, crest)?;
    let authored = parts(&mut d)?;
    d.embed_alphas(&lib);
    Ok((d, lib, authored))
}

// --- Gates -----------------------------------------------------------------------------------------------------------

/// The metal round the stone, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = HashMap::new();
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

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The named views at 300 px in one strip, as a reviewer first meets them.
fn contact_sheet(out: &Path, names: &[&str]) -> Result<()> {
    const EDGE: u32 = 300;
    let mut sheet = image::RgbImage::new(EDGE * names.len() as u32, EDGE);
    for (i, n) in names.iter().enumerate() {
        let im = image::open(out.join(format!("{n}.png")))?.to_rgb8();
        let small = image::imageops::resize(&im, EDGE, EDGE, image::imageops::FilterType::Lanczos3);
        image::imageops::replace(&mut sheet, &small, (i as u32 * EDGE) as i64, 0);
    }
    sheet.save(out.join("contact-300.png"))?;
    Ok(())
}

/// The named views: name, yaw and pitch.
const VIEWS: [(&str, f64, f64); 6] = [("hero", 0.55, 0.95), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", -0.9, 0.62), ("reverse", 1.6, 0.8)];

/// Studio-gold renders with the sapphire set: the named views, a close-up on the grip, the bare band against the finished
/// ring, and a 300 px contact strip.
fn renders(out: &Path, finished: &render::Finished, st: &Stone, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let head = crop(&finished.metal, st.g, 8.5);
    let mut close = vec![render::Part::metal(&head, render::GOLD)];
    close.extend(finished.parts(render::GOLD));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    let bare = mesh::try_build(&band(), &AlphaLibrary::builtin(), draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    contact_sheet(out, &["hero", "face", "side", "shoulder", "palm", "stones"])
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Every made part's self-crossings, as placed.
fn crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

/// Farthest a land-width ray runs into a part, mm: past the widest section any part has (the torso's 7.1 mm).
const LAND_REACH_MM: f64 = 12.0;

/// A part's land width: from each face the family measures, into the metal along its normal to the first face turned
/// back against it within 12 degrees. Returns the thinnest and the area of faces reading under the floor; the thinnest
/// is infinite when no ray found the far side.
fn land_width(solid: &csg::Solid, family: Family) -> (f64, f64) {
    let tri = |i: usize| solid.f[i].map(|k| solid.v[k as usize]);
    let normals: Vec<(P3, f64)> = (0..solid.f.len())
        .map(|i| {
            let [a, b, c] = tri(i);
            let n = cross(sub(b, a), sub(c, a));
            (unit(n), 0.5 * norm(n))
        })
        .collect();
    const CELL: f64 = 0.3;
    let key = |p: P3| p.map(|v| (v / CELL).floor() as i64);
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for i in 0..solid.f.len() {
        let [a, b, c] = tri(i);
        let lo = key([a[0].min(b[0]).min(c[0]), a[1].min(b[1]).min(c[1]), a[2].min(b[2]).min(c[2])]);
        let hi = key([a[0].max(b[0]).max(c[0]), a[1].max(b[1]).max(c[1]), a[2].max(b[2]).max(c[2])]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    grid.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let facing = -(12f64.to_radians().cos());
    let (mut thinnest, mut thin_area) = (f64::MAX, 0.0);
    for (i, &(n, area)) in normals.iter().enumerate() {
        if let Family::Plate(up) = family {
            if dot(n, up) > -0.6 {
                continue;
            }
        }
        let [a, b, c] = tri(i);
        let o = mul(add(add(a, b), c), 1.0 / 3.0);
        let dir = mul(n, -1.0);
        let mut seen = std::collections::HashSet::new();
        let mut first = (f64::MAX, 0.0);
        let mut t = 0.0;
        while t < LAND_REACH_MM && first.0 == f64::MAX {
            if let Some(cell) = grid.get(&key(add(o, mul(dir, t)))) {
                for &j in cell {
                    if j == i || !seen.insert(j) {
                        continue;
                    }
                    let [p0, p1, p2] = tri(j);
                    let (e1, e2) = (sub(p1, p0), sub(p2, p0));
                    let h = cross(dir, e2);
                    let det = dot(e1, h);
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let s0 = sub(o, p0);
                    let u = dot(s0, h) / det;
                    let q = cross(s0, e1);
                    let v = dot(dir, q) / det;
                    if u < 0.0 || v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let hit = dot(e2, q) / det;
                    if hit > 1e-6 && hit < first.0 {
                        first = (hit, dot(normals[j].0, n));
                    }
                }
            }
            t += 0.1;
        }
        if first.0 < f64::MAX && first.1 < facing {
            thinnest = thinnest.min(first.0);
            if first.0 < MIN_SECTION_MM {
                thin_area += area;
            }
        }
    }
    (if thinnest == f64::MAX { f64::INFINITY } else { thinnest }, thin_area)
}

/// One joined part's land width, as the report lists it.
#[derive(serde::Serialize)]
struct Land {
    part: String,
    measured: String,
    min_section_mm: f64,
    under_floor_mm2: f64,
    note: String,
}

/// Land widths for every joined part: each loft on its own mesh, each mirror copy on its own mesh as a plate or round body.
/// A part no ray measured, or one reading wider than any part is, fails: an unmeasured part never passes silently.
fn land_widths(built: &mesh::BuildResult, a: &Authored) -> Result<Vec<Land>> {
    let mut out = Vec::new();
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        let Some(family) = a.families.get(&c.name).copied() else { continue };
        let Some(m) = &c.made else { continue };
        let (thin, area) = land_width(m.solid(), family);
        ensure!(thin.is_finite() && thin > 0.0 && thin <= 10.0, "{}: its land width was not measured ({thin})", c.name);
        let copy = a.copies.get(&c.name);
        let note = match (copy, c.name.starts_with("High claw")) {
            (Some(of), _) if of.iter().any(|n| n.starts_with("High claw")) => {
                "mirror of the high foot; every claw cast blunt with a 0.92 mm round tip, points filed after setting if wanted".into()
            }
            (Some(of), _) => format!("mirror of {} parts", of.len()),
            (None, true) => "claw cast blunt with a 0.92 mm round tip; points filed after setting if wanted".into(),
            (None, false) => String::new(),
        };
        let measured = match family {
            Family::Plate(_) => "plate, belly to top".into(),
            Family::Round => "round, every direction".into(),
        };
        out.push(Land { part: c.name.clone(), measured, min_section_mm: thin, under_floor_mm2: area, note });
    }
    let named: std::collections::HashSet<&str> = out.iter().map(|l| l.part.as_str()).collect();
    let missing: Vec<&String> = a.families.keys().filter(|n| !named.contains(n.as_str())).collect();
    ensure!(missing.is_empty(), "no land width for {missing:?}");
    Ok(out)
}

/// The finished mesh's nearest approach to the finger's axis, and how many vertices stand inside the bore by more than 0.01 mm.
fn bore_clearance(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    let mut nearest = f64::MAX;
    let mut inside = 0;
    for p in &m.vertices {
        let r = (p.0 as f64).hypot(p.1 as f64);
        nearest = nearest.min(r);
        inside += usize::from(r < bore - 0.01);
    }
    (nearest, inside)
}

/// Metal vertices standing inside the stone, and the nearest any metal off the seat comes to it.
fn metal_in_stone(st: &Stone, m: &mesh::Mesh) -> (usize, f64) {
    let (mut inside, mut nearest) = (0, f64::MAX);
    for p in &m.vertices {
        let q = [p.0 as f64, p.1 as f64, p.2 as f64];
        let g = st.gap(q);
        // The seat below the girdle is cut to the pavilion; only metal above it can press on the stone.
        if q[1] > st.g[1] - st.half {
            nearest = nearest.min(g);
            inside += usize::from(g < -1e-3);
            if g < 0.02 && std::env::var("HARPYIA_DEBUG").is_ok() {
                eprintln!("metal {g:.3} at {q:?} rho {:.3} h {:.3}", q[0].hypot(q[2]), q[1] - st.g[1]);
            }
        }
    }
    (inside, nearest)
}

/// Loose stone triangles counted as separate stones by the corners they share.
fn stone_count(meshes: &[(mesh::Mesh, [f32; 3])]) -> usize {
    let mut total = 0;
    for (m, _) in meshes {
        let mut parent: Vec<usize> = (0..m.faces.len()).collect();
        fn find(p: &mut [usize], i: usize) -> usize {
            let mut r = i;
            while p[r] != r {
                r = p[r];
            }
            let mut j = i;
            while p[j] != r {
                let next = p[j];
                p[j] = r;
                j = next;
            }
            r
        }
        let mut owner: HashMap<[i64; 3], usize> = HashMap::new();
        for (fi, f) in m.faces.iter().enumerate() {
            for &vi in f {
                let v = m.vertices[vi as usize];
                let k = [v.0, v.1, v.2].map(|c| (c as f64 * 1e4).round() as i64);
                match owner.get(&k) {
                    Some(&o) => {
                        let (a, b) = (find(&mut parent, o), find(&mut parent, fi));
                        parent[a] = b;
                    }
                    None => {
                        owner.insert(k, fi);
                    }
                }
            }
        }
        total += (0..m.faces.len()).filter(|&i| find(&mut parent, i) == i).count();
    }
    total
}

/// The geometry gates of one build.
#[derive(serde::Serialize)]
struct Build {
    params: [usize; 2],
    triangles: usize,
    build_s: f64,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    min_angle_deg: f64,
    mesh_self_crossings: usize,
    parts_crossed: usize,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    parts_joined: usize,
    parts_cut: usize,
}

impl Build {
    fn of(built: &mesh::BuildResult, params: BuildParams, build_s: f64) -> Self {
        let v = &built.report.validation;
        let q = built.report.quality;
        Self {
            params: [params.theta_steps, params.profile_steps],
            triangles: built.mesh.faces.len(),
            build_s,
            watertight: v.watertight,
            boundary_edges: v.boundary_edges,
            non_manifold_edges: v.non_manifold_edges,
            degenerate_faces: q.degenerate_faces,
            min_angle_deg: q.min_angle_deg,
            mesh_self_crossings: csg::self_crossings(&solid_of(&built.mesh)),
            parts_crossed: crossings(built).iter().filter(|(_, n)| *n > 0).count(),
            solids_notes: built.solids.notes.clone(),
            parts_notes: built.parts.notes.clone(),
            parts_joined: built.parts.joined,
            parts_cut: built.parts.cut,
        }
    }
    fn clean(&self) -> bool {
        self.watertight && self.degenerate_faces == 0 && self.mesh_self_crossings == 0 && self.parts_crossed == 0 && self.solids_notes.is_empty() && self.parts_notes.is_empty()
    }
}

#[derive(serde::Serialize)]
struct Report {
    name: String,
    process: String,
    size: String,
    bore_mm: f64,
    band_mm: [f64; 2],
    build: Build,
    draft: Option<Build>,
    preview_build_s: Option<f64>,
    triangles_at_1536x448: Option<usize>,
    made_parts: Vec<(String, usize)>,
    feathers_per_wing: usize,
    land_widths: Vec<Land>,
    land_min_mm: f64,
    land_under_floor_mm2: f64,
    talon_curls_deg: Vec<(String, f64)>,
    talon_curl_span_deg: f64,
    toe_lengths_mm: Vec<(String, f64)>,
    middle_to_outer_toe: f64,
    toe_gap_mm: f64,
    talon_stone_gap_mm: f64,
    figure_stone_gap_mm: f64,
    metal_inside_stone: usize,
    metal_nearest_stone_mm: f64,
    bore_radius_mm: f64,
    nearest_to_axis_mm: f64,
    vertices_in_bore: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    undercut_percent: f64,
    thinnest_wall_mm: f64,
    thinnest_wall_theta_deg: f64,
    investment_min_section_mm: f64,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    stone_carats: f64,
    stone_warnings: Vec<String>,
    stone_warnings_explained: String,
    z_extent_mm: f64,
    radial_reach_mm: f64,
    grams_18k: f64,
    layers: Vec<String>,
    cad_features: usize,
    design_bytes: u64,
    design_format: u64,
    embedded_alphas: usize,
    cold_reload_identical: Option<bool>,
    pattern: Option<serde_json::Value>,
    geometry_gates_passed: bool,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/harpyia"));
    std::fs::create_dir_all(&out)?;
    println!("Harpyia");
    let (d, lib, authored) = author()?;
    let art = out.join("artwork");
    std::fs::create_dir_all(&art)?;
    if let Some(a) = lib.get("Harpyia plumage") {
        std::fs::write(art.join("plumage.png"), a.to_png16()?)?;
    }
    let authored_min = authored
        .feathers
        .iter()
        .flat_map(|f| f.stations.iter().map(move |&s| f.thick.min(2.0 * f.half(s))))
        .fold(f64::MAX, f64::min);
    ensure!(authored_min >= MIN_SECTION_MM, "a feather is authored under the {MIN_SECTION_MM} mm section: {authored_min:.3}");
    ensure!(authored.foot_stone_gap >= STONE_GAP - 1e-6, "a toe comes {:.4} mm from the stone", authored.foot_stone_gap);
    ensure!(authored.figure_stone_gap >= STONE_GAP, "her body comes {:.4} mm from the stone", authored.figure_stone_gap);
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build = Build::of(&built, params, started.elapsed().as_secs_f64());
    println!(
        "  {} feathers a wing; {} triangles in {:.1} s; watertight {}; degenerate {}; crossings {} on the mesh, {} parts crossed; notes {:?} {:?}",
        authored.feathers.len(),
        build.triangles,
        build.build_s,
        build.watertight,
        build.degenerate_faces,
        build.mesh_self_crossings,
        build.parts_crossed,
        build.solids_notes,
        build.parts_notes
    );
    if std::env::var("HARPYIA_LOOK").is_ok() {
        // A quick look while authoring: the views only, no gates and no outputs.
        library::save_design(out.join("look.ring.json"), &d)?;
        let mut tris: Vec<(usize, String)> = built.parts.evaluated.iter().flat_map(|e| e.components.iter()).map(|c| (c.mesh.faces.len(), c.name.clone())).collect();
        tris.sort_by(|a, b| b.0.cmp(&a.0));
        println!("  part triangles {}: {:?}", tris.iter().map(|t| t.0).sum::<usize>(), &tris);
        println!("  design {} bytes; reach {:.3}; z {:.3}", std::fs::metadata(out.join("look.ring.json"))?.len(), built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max), built.mesh.vertices.iter().map(|p| p.2 as f64).fold(0.0, f64::max) * 2.0);
        let finished = render::finished_from(&d, &lib, built);
        let parts = finished.parts(render::GOLD);
        for (name, yaw, pitch) in VIEWS {
            render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, 900)?;
        }
        let head = crop(&finished.metal, authored.stone.g, 8.5);
        let mut close = vec![render::Part::metal(&head, render::GOLD)];
        close.extend(finished.parts(render::GOLD));
        render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, 900)?;
        let near = crop(&finished.metal, authored.head, 3.4);
        for (name, yaw, pitch) in [("close-side", 0.0, 0.0), ("close-hero", 0.55, 0.95), ("close-shoulder", -0.9, 0.62), ("close-top", 0.0, PI * 0.5)] {
            render::write_png_parts(out.join(format!("{name}.png")), &[render::Part::metal(&near, render::GOLD)], yaw, pitch, 700)?;
        }
        return Ok(());
    }
    let made_parts = crossings(&built);
    if std::env::var("HARPYIA_DEBUG").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            if let Some(m) = &c.made {
                let r = m.solid().v.iter().map(|p| p[0].hypot(p[1])).fold(0.0, f64::max);
                let z = m.solid().v.iter().map(|p| p[2].abs()).fold(0.0, f64::max);
                let top = m.solid().v.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
                if r > 18.0 || z > 6.0 || top > 17.3 {
                    eprintln!("reach {:.2} z {:.2} top {:.2}: {}", r, z, top, c.name);
                }
            }
        }
    }
    let lands = land_widths(&built, &authored)?;
    let land_min = lands.iter().map(|l| l.min_section_mm).fold(f64::MAX, f64::min);
    let land_area: f64 = lands.iter().map(|l| l.under_floor_mm2).sum();
    let (nearest, in_bore) = bore_clearance(&d, &built.mesh);
    let (metal_inside, metal_nearest) = metal_in_stone(&authored.stone, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let zs = built.mesh.vertices.iter().map(|p| p.2 as f64);
    let z_extent = zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min);
    let reach = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max);
    if std::env::var("HARPYIA_DEBUG").is_ok() {
        let far = built.mesh.vertices.iter().max_by(|a, b| (a.0 as f64).hypot(a.1 as f64).total_cmp(&(b.0 as f64).hypot(b.1 as f64))).unwrap();
        eprintln!("farthest vertex {far:?}");
    }
    library::save_design(out.join("design.ring.json"), &d)?;
    let _ = std::fs::remove_file(out.join("design.ring.json.bak"));
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
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
    let (draft_block, preview_s) = if draft {
        (None, None)
    } else {
        let started = std::time::Instant::now();
        let b = mesh::try_build(&d, &lib, draft_params())?;
        let draft_block = Build::of(&b, draft_params(), started.elapsed().as_secs_f64());
        let started = std::time::Instant::now();
        mesh::try_build(&d, &lib, preview_params())?;
        (Some(draft_block), Some(started.elapsed().as_secs_f64()))
    };
    let mut warnings: Vec<String> = stones_report.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let triangles_1536 = (!draft).then_some(build.triangles);
    let finished = render::finished_from(&d, &lib, built);
    let previewed = stone_count(&finished.stones);
    let pattern = if draft {
        None
    } else {
        stl::write_stl(out.join("finished-metal.stl"), &finished.metal, &d.name)?;
        let mut setup = mf::Setup::from_design(&d);
        setup.recipe.name = "Harpyia / investment / Gold 18k".into();
        setup.recipe.alloy = "Gold 18k".into();
        setup.recipe.sand = None;
        setup.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(1.3, |m| m.shrink_pct);
        setup.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
        setup.bench_notes = "Invest the band, figure, feet and both wings as one tree; clear investment from between the primaries, between the toes and from under her hair locks and chin. Seat the 8 mm sapphire in the cut bearing, then close the eight claws over its girdle; file the claws' blunt tips to points if wanted.".into();
        let prepared = mf::prepare(&d, &lib, &setup, params)?;
        let pattern = &prepared.mesh;
        let check = solid_of(pattern).check(true);
        let pv = pattern.validate();
        ensure!(pv.watertight && check.zero_area_faces == 0 && check.self_crossings == Some(0), "The investment pattern failed its mesh gates: {pv:?} {check:?}");
        stl::write_stl(out.join("casting-pattern.stl"), pattern, &d.name)?;
        for (i, (stone, _)) in finished.stones.iter().enumerate() {
            let name = if i == 0 { "reference-sapphire.stl".to_string() } else { format!("reference-sapphire-{}.stl", i + 1) };
            stl::write_stl(out.join(name), stone, "Harpyia reference sapphire; do not cast")?;
        }
        Some(json!({"scale": prepared.scale, "triangles": pattern.faces.len(), "watertight": pv.watertight, "degenerate_faces": check.zero_area_faces, "self_crossings": check.self_crossings, "notes": prepared.notes}))
    };
    let tint = sapphire().preview_tint.unwrap_or([0.03, 0.09, 0.42]);
    std::fs::write(
        out.join("stones.json"),
        serde_json::to_vec_pretty(&json!({"stones": [{"mesh": "reference-sapphire.stl", "name": "Sapphire", "tint": tint, "ior": 1.77, "dispersion": 0.018, "roughness": 0.05, "transmission": 0.7}]}))?,
    )?;
    let explained = if warnings.iter().any(|w| w.starts_with("no setting holds")) {
        "The report knows claws, baskets and bezels built by the setting builders; this stone is held by the eight lofted claws of her two feet, which the report cannot see as a head."
    } else {
        ""
    };
    let mut report = Report {
        name: d.name.clone(),
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: 2.0 * d.inner_radius_mm(),
        band_mm: [2.0 * CHEEK, THICK],
        build,
        draft: draft_block,
        preview_build_s: preview_s,
        triangles_at_1536x448: triangles_1536,
        made_parts,
        feathers_per_wing: authored.feathers.len(),
        land_widths: lands,
        land_min_mm: land_min,
        land_under_floor_mm2: land_area,
        talon_curls_deg: authored.foot_curls.clone(),
        talon_curl_span_deg: {
            let c = authored.foot_curls.iter().map(|(_, c)| *c);
            c.clone().fold(f64::MIN, f64::max) - c.fold(f64::MAX, f64::min)
        },
        toe_lengths_mm: authored.toe_lengths.clone(),
        middle_to_outer_toe: {
            let of = |n: &str| authored.toe_lengths.iter().find(|(t, _)| t == n).map_or(f64::NAN, |(_, l)| *l);
            of("middle toe") / of("outer toe")
        },
        toe_gap_mm: authored.toe_gap,
        talon_stone_gap_mm: authored.foot_stone_gap,
        figure_stone_gap_mm: authored.figure_stone_gap,
        metal_inside_stone: metal_inside,
        metal_nearest_stone_mm: metal_nearest,
        bore_radius_mm: d.inner_radius_mm(),
        nearest_to_axis_mm: nearest,
        vertices_in_bore: in_bore,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        undercut_percent: field.undercut_fraction() * 100.0,
        thinnest_wall_mm: field.thinnest_wall_mm,
        thinnest_wall_theta_deg: field.thinnest_wall_theta_deg,
        investment_min_section_mm: d.draft.min_section_mm,
        dfm_findings: findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        stones_reported: stones_report.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        stone_carats: stones_report.as_ref().map_or(0.0, |s| s.total_carats),
        stone_warnings: warnings.clone(),
        stone_warnings_explained: explained.into(),
        z_extent_mm: z_extent,
        radial_reach_mm: reach,
        grams_18k: grams,
        layers: d.layers.layers.iter().map(|e| e.name.clone()).collect(),
        cad_features: d.cad.as_ref().map_or(0, |c| c.features.len()),
        design_bytes: text.len() as u64,
        design_format,
        embedded_alphas: d.embedded.len(),
        cold_reload_identical: cold,
        pattern,
        geometry_gates_passed: false,
    };
    report.geometry_gates_passed = report.build.clean()
        && report.draft.as_ref().is_none_or(Build::clean)
        && report.build.triangles <= TRIANGLE_BUDGET
        && report.vertices_in_bore == 0
        && report.land_min_mm >= MIN_SECTION_MM - 1e-6
        && report.land_under_floor_mm2 == 0.0
        && report.talon_curls_deg.iter().all(|(_, c)| *c >= 60.0)
        && report.talon_curl_span_deg >= 25.0
        && report.toe_gap_mm >= 0.6
        && report.talon_stone_gap_mm <= 0.15
        && report.metal_inside_stone == 0
        && report.field_verdict == castability::Verdict::Castable.label()
        && report.thinnest_wall_mm >= MIN_SECTION_MM
        && report.dfm_findings.is_empty()
        && report.stones_reported as usize == report.stones_previewed
        && report.radial_reach_mm <= REACH_MAX_MM
        && report.z_extent_mm <= Z_MAX_MM
        && report.grams_18k <= GRAMS_MAX
        && report.cold_reload_identical != Some(false);
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &finished, &authored.stone, if draft { 1000 } else { 1600 })?;
    println!(
        "  toes {:.3} mm apart; middle toe {:.2} x the outer; lengths {:?}",
        report.toe_gap_mm,
        report.middle_to_outer_toe,
        report.toe_lengths_mm.iter().map(|(_, l)| (l * 100.0).round() / 100.0).collect::<Vec<_>>()
    );
    println!(
        "  field {} ({:.3}% two-part undercut, reported only); thinnest wall {:.2} mm; dfm {}; stones {} reported, {} previewed; land widths >= {:.3} mm ({:.3} mm² under the floor) over {} parts; talon curls {:?}; toes {:.3} mm from the stone; metal in the stone {}, nearest {:.3} mm",
        report.field_verdict,
        report.undercut_percent,
        report.thinnest_wall_mm,
        report.dfm_findings.len(),
        report.stones_reported,
        report.stones_previewed,
        report.land_min_mm,
        report.land_under_floor_mm2,
        report.land_widths.len(),
        report.talon_curls_deg.iter().map(|(_, c)| c.round()).collect::<Vec<_>>(),
        report.talon_stone_gap_mm,
        report.metal_inside_stone,
        report.metal_nearest_stone_mm
    );
    println!(
        "  nearest the axis {:.4} mm of {:.4}; z extent {:.2} mm; reach {:.2} mm; {:.2} g of 18k; design {} KB at format {}",
        report.nearest_to_axis_mm,
        report.bore_radius_mm,
        report.z_extent_mm,
        report.radial_reach_mm,
        report.grams_18k,
        report.design_bytes / 1024,
        report.design_format
    );
    if let Some(b) = &report.draft {
        println!("  draft {:?}: {} triangles in {:.1} s, clean {}; preview {:.1} s", b.params, b.triangles, b.build_s, b.clean(), report.preview_build_s.unwrap_or(0.0));
    }
    for l in report.land_widths.iter().filter(|l| l.min_section_mm < MIN_SECTION_MM + 0.02) {
        println!("    land: {} {:.3} mm ({})", l.part, l.min_section_mm, l.measured);
    }
    for f in &report.dfm_findings {
        println!("    dfm: {f}");
    }
    for n in report.build.parts_notes.iter().chain(&report.build.solids_notes).chain(&report.stone_warnings) {
        println!("    note: {n}");
    }
    println!("  geometry gates {}", if report.geometry_gates_passed { "passed" } else { "FAILED" });
    ensure!(report.geometry_gates_passed, "Harpyia failed its geometry gates; see {}", out.join("report.json").display());
    Ok(())
}
