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
const TIP_R: f64 = 0.425;
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

/// A face's section across its centre line, `w` either side and `depth` thick: a flat back, round sides, cheeks standing
/// `cheek` forward, and the centre line `ridge` proud of them over `q` either side, in six cubic pieces.
fn face_section(w: f64, depth: f64, cheek: f64, ridge: f64, q: f64) -> Sketch {
    let e = 0.5 * depth;
    let xn = (w - e).max(0.05);
    let q = q.min(0.8 * xn);
    let l = 4.0 / 3.0 * e;
    let top = e + cheek;
    let h = 4.0 / 3.0 * ridge;
    chain(&[
        [[-xn, -e], [-xn / 3.0, -e], [xn / 3.0, -e], [xn, -e]],
        [[xn, -e], [xn + l, -e], [xn + l, e], [xn, e]],
        [[xn, e], [0.55 * xn, e + 1.1 * cheek], [1.8 * q, top], [q, top]],
        [[q, top], [0.45 * q, top + h], [-0.45 * q, top + h], [-q, top]],
        [[-q, top], [-1.8 * q, top], [-0.55 * xn, e + 1.1 * cheek], [-xn, e]],
        [[-xn, e], [-xn - l, e], [-xn - l, -e], [-xn, -e]],
    ])
}

/// Her face from the brow down to the chin: share down it, half-width, depth, cheeks, the centre line's rise and its half-width.
const FACE: [[f64; 6]; 12] = [
    [0.0, 0.55, 0.85, 0.05, 0.0, 0.2],
    [0.1, 0.72, 0.9, 0.12, 0.02, 0.2],
    [0.22, 0.8, 0.95, 0.2, 0.06, 0.2],
    [0.34, 0.8, 0.85, 0.02, 0.2, 0.18],
    [0.48, 0.74, 0.85, 0.12, 0.34, 0.2],
    [0.58, 0.7, 0.85, 0.12, 0.42, 0.24],
    [0.66, 0.66, 0.85, 0.1, 0.1, 0.22],
    [0.74, 0.6, 0.85, 0.08, 0.2, 0.26],
    [0.8, 0.58, 0.85, 0.06, 0.05, 0.24],
    [0.86, 0.55, 0.85, 0.08, 0.16, 0.24],
    [0.93, 0.5, 0.9, 0.12, 0.12, 0.22],
    [1.0, 0.4, 0.85, 0.05, 0.0, 0.2],
];
/// How tall her face is, brow to chin, mm.
const FACE_MM: f64 = 1.9;

/// Half a feather's vane at share `s` of its length: a narrow quill, the vane opening and holding, narrowing past an
/// emargination `notch` (share, kept width) when it has one, then rounding off at the tip.
fn feather_half(s: f64, width: f64, thick: f64, quill: f64, round: f64, notch: Option<(f64, f64)>) -> f64 {
    let full = 0.5 * width;
    let q = (0.5 * thick + 0.06).max(0.46);
    let open = smooth(quill, quill + 0.18, s);
    let mut hold = full * (1.0 - 0.1 * smooth(0.3, 1.0 - round, s));
    if let Some((at, keep)) = notch {
        hold *= 1.0 - (1.0 - keep) * smooth(at, at + 0.1, s);
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
fn path_plume(path: &[P3], ups: &[P3], width: f64, thick: f64, channels: bool) -> Result<Operation> {
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
            let half = feather_half(s, width, thick, 0.1, 0.42, None);
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
}

/// Stations along a primary, dense round its emargination.
const PRIMARY: &[f64] = &[0.0, 0.1, 0.3, 0.52, 0.64, 0.8, 0.92, 1.0];
/// Stations along a secondary.
const LONG: &[f64] = &[0.0, 0.08, 0.26, 0.55, 0.82, 0.95, 1.0];
/// Stations along a covert.
const SHORT: &[f64] = &[0.0, 0.18, 0.5, 0.82, 1.0];
/// Stations along a marginal covert.
const TINY: &[f64] = &[0.0, 0.3, 0.72, 1.0];

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
        feather_half(s, self.width, self.thick, self.quill, self.round, self.notch)
    }
    fn loft(&self) -> Result<Operation> {
        ensure!(self.thick >= MIN_SECTION_MM, "{}: section {} under the wax floor", self.name, self.thick);
        let inside = self
            .stations
            .iter()
            .flat_map(|&s| {
                let (p, t) = self.rachis(s);
                let h = self.half(s) + 0.4;
                [[p[0] - t[1] * h, p[1] + t[0] * h], [p[0] + t[1] * h, p[1] - t[0] * h]]
            })
            .map(|q| q[0].hypot(q[1]))
            .fold(f64::MAX, f64::min);
        ensure!(inside >= BORE_CLEAR_R, "{} reaches {inside:.2} mm from the finger's axis, inside {BORE_CLEAR_R} mm", self.name);
        let e = 0.5 * self.thick;
        let section = |p: [f64; 2], t: [f64; 2], s: f64, k: f64| {
            let z = CHEEK + self.under + e + self.lift * smooth(0.25, 1.0, s);
            let half = self.half(s) * k;
            let mut v = vane(half, self.thick * k, 0.16 * half, 0.15 * k, 0.08 * k, self.channels);
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
            for x in [0.55 * e, 0.88 * e] {
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

/// The wing's leading edge from her shoulder to the wrist, (theta degrees, radius mm) at `u` in 0..1.
fn arm(u: f64) -> [f64; 2] {
    const KEYS: [[f64; 2]; 5] = [[109.0, 15.3], [115.0, 16.1], [121.0, 16.35], [127.0, 16.1], [132.5, 15.6]];
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
            ..base(format!("Secondary {}", k + 1), polar(th, r - 0.6), polar(lerp(210.0, 162.0, f), lerp(12.3, 13.6, f)))
        });
    }
    // Primaries: graded fingers fanned from the wrist, emarginate past their middles, the middle ones longest.
    const HEADINGS: [f64; 7] = [185.0, 200.0, 215.0, 230.0, 246.0, 262.0, 278.0];
    const LENGTHS: [f64; 7] = [4.6, 6.0, 7.2, 7.6, 7.2, 6.4, 5.4];
    for (k, (&heading, &length)) in HEADINGS.iter().zip(&LENGTHS).enumerate() {
        let f = k as f64 / (HEADINGS.len() - 1) as f64;
        let [th, r] = arm(lerp(1.0, 0.72, f));
        let root = polar(th, r - lerp(0.4, 1.0, f));
        out.push(Feather {
            bow: 0.35,
            width: 3.0,
            thick: 0.9,
            under: under + 0.12 - 0.015 * k as f64,
            lift: 0.12,
            round: 0.3,
            notch: (k < 5).then_some((0.6, 0.6)),
            stations: PRIMARY,
            channels: true,
            dome: true,
            ..base(format!("Primary {}", k + 1), root, aimed(root, heading, length))
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

/// Torso keys: share along its spine, half-width across the band, half-depth, and how much fuller its breast is.
const TORSO: [[f64; 4]; 7] = [
    [0.0, 2.0, 1.1, 0.0],
    [0.18, 2.8, 1.55, 0.12],
    [0.4, 3.2, 1.85, 0.22],
    [0.62, 3.45, 1.85, 0.25],
    [0.82, 3.55, 1.6, 0.18],
    [0.93, 3.1, 1.3, 0.1],
    [1.0, 2.4, 1.0, 0.05],
];
/// Head radii from the occiput to the face.
const HEAD: [[f64; 3]; 10] = [
    [-0.1, 0.3, 0.0],
    [-0.05, 0.6, 0.0],
    [0.0, 0.78, 0.0],
    [0.1, 1.02, 0.0],
    [0.25, 1.16, 0.02],
    [0.42, 1.2, 0.06],
    [0.6, 1.12, 0.14],
    [0.75, 0.98, 0.2],
    [0.9, 0.78, 0.15],
    [1.0, 0.55, 0.08],
];
/// The head's across-to-up ratio.
const HEAD_WIDTH: f64 = 0.9;

/// The harpy's body: torso, neck and head, each a lofted spine in the band's mid-plane west of the stone.
struct Figure {
    torso: Spine,
    neck: Spine,
    head: Spine,
    /// The head's length from occiput to face.
    head_len: f64,
}

const Z: P3 = [0.0, 0.0, 1.0];

impl Figure {
    fn new(crest: f64, g: f64) -> Self {
        let rump = polar(127.0, crest + 0.45);
        let torso = Spine::through(&[[rump[0], rump[1], 0.0], [-7.7, g - 1.9, 0.0], [-7.05, g - 0.2, 0.0], [-6.45, g + 1.3, 0.0]], Z);
        let neck = Spine::through(&[[-6.6, g + 0.85, 0.0], [-5.95, g + 1.5, 0.0], [-5.3, g + 2.0, 0.0]], Z);
        let (o, f) = ([-5.6, g + 2.26, 0.0], [-3.4, g + 1.95, 0.0]);
        let back = sub(o, mul(sub(f, o), 0.1));
        let head = Spine::through(&[back, f], Z);
        Self { torso, neck, head, head_len: norm(sub(f, o)) }
    }
    fn torso_keys(s: f64) -> [f64; 3] {
        let k = keyed(&TORSO, s);
        [k[1], k[2], k[3]]
    }
    /// The torso's skin at share `s` and `phi` degrees round from the breast, with its outward normal.
    fn torso_skin(&self, s: f64, phi: f64) -> (P3, P3) {
        let (p, t, _) = self.torso.at(s * self.torso.length());
        let (x, y) = (Z, cross(t, Z));
        let [a, b, bias] = Self::torso_keys(s);
        let (sp, cp) = phi.to_radians().sin_cos();
        let bb = if cp >= 0.0 { b * (1.0 + bias) } else { b * (1.0 - bias) };
        (add(p, add(mul(x, a * sp), mul(y, bb * cp))), unit(add(mul(x, sp / a), mul(y, cp / bb))))
    }
    fn head_r(u: f64) -> f64 {
        keyed(&HEAD, u)[1]
    }
    fn head_bias(u: f64) -> f64 {
        keyed(&HEAD, u)[2]
    }
    /// The head's axis at `u` (0 occiput, 1 face), with its top's direction.
    fn head_axis(&self, u: f64) -> (P3, P3) {
        let d = (u + 0.1) * self.head_len;
        let (p, t, _) = self.head.at(d);
        (p, mul(cross(t, Z), -1.0))
    }
    /// The head's skin at `u` and `phi` degrees round from its top, with its outward normal.
    fn head_skin(&self, u: f64, phi: f64) -> (P3, P3) {
        let (p, up) = self.head_axis(u);
        let u = u.clamp(-0.1, 1.0);
        let r = Self::head_r(u);
        let (sp, cp) = phi.to_radians().sin_cos();
        let a = HEAD_WIDTH * r;
        let b = if cp >= 0.0 { r * (1.0 - Self::head_bias(u)) } else { r * (1.0 + Self::head_bias(u)) };
        (add(p, add(mul(Z, a * sp), mul(up, b * cp))), unit(add(mul(Z, sp / a), mul(up, cp / b))))
    }
    /// The nearest the torso, neck, head and face come to the stone, sampled round their sections.
    fn stone_gap(&self, st: &Stone) -> f64 {
        let mut nearest = f64::MAX;
        let (o, up) = self.head_axis(0.0);
        let fwd = unit(sub(self.head_axis(1.0).0, o));
        let (c, _) = self.head_axis(0.8);
        let c = add(add(c, mul(fwd, 0.42)), mul(up, 0.18));
        for &[s, w, depth, cheek, ridge, _] in &FACE {
            let set_back = 0.15 * (2.0 * s - 1.0).powi(2);
            let p = add(add(c, mul(up, FACE_MM * (0.5 - s))), mul(fwd, -set_back));
            let e = 0.5 * depth;
            for (x, y) in [(0.0, e + cheek + ridge), (0.5 * w, e + cheek), (w, 0.0), (-0.5 * w, e + cheek), (-w, 0.0)] {
                nearest = nearest.min(st.gap(add(add(p, mul(Z, x)), mul(fwd, y))));
            }
        }
        for i in 0..=40 {
            let s = i as f64 / 40.0;
            for k in 0..24 {
                let phi = k as f64 * 15.0;
                nearest = nearest.min(st.gap(self.torso_skin(s, phi).0));
                nearest = nearest.min(st.gap(self.head_skin(lerp(-0.1, 1.0, s), phi).0));
                let (p, t, nn) = self.neck.at(s * self.neck.length());
                let r = lerp(1.4, 1.12, s);
                let b = cross(t, nn);
                let (sp, cp) = phi.to_radians().sin_cos();
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
        let at: Vec<f64> = (0..=6).map(|i| len * i as f64 / 6.0).collect();
        round_loft(&self.neck, &at, |d| lerp(1.4, 1.12, d / len))
    }
    /// Her face on the front of the head, looking east over the stone: a loft down it from brow to chin whose centre
    /// line carries the brow, the nose, the lips and the chin, with the eyes' hollows either side of the bridge.
    fn face_loft(&self) -> (Operation, P3) {
        let (o, up) = self.head_axis(0.0);
        let (f, _) = self.head_axis(1.0);
        let fwd = unit(sub(f, o));
        let (c, _) = self.head_axis(0.8);
        let c = add(add(c, mul(fwd, 0.42)), mul(up, 0.18));
        let sections = FACE
            .iter()
            .map(|&[s, w, depth, cheek, ridge, q]| {
                let set_back = 0.15 * (2.0 * s - 1.0).powi(2);
                let p = add(add(c, mul(up, FACE_MM * (0.5 - s))), mul(fwd, -set_back));
                let mut sk = face_section(w, depth, cheek, ridge, q);
                sk.plane = plane(p, Z, fwd);
                sk.into()
            })
            .collect();
        (Operation::Loft { sections, meshed: true }, fwd)
    }
    fn head_loft(&self) -> Operation {
        let at: Vec<[f64; 4]> = HEAD.iter().map(|&[u, r, bias]| [(u + 0.1) * self.head_len, HEAD_WIDTH * r, r, bias]).collect();
        oval_loft(&self.head, &at, Z)
    }
    /// Her head's feathering: hair plumes swept back from the brow over the crown and down the nape.
    fn hood(&self) -> Result<Vec<(String, Operation, P3)>> {
        let mut out = Vec::new();
        let (o, top) = self.head_axis(0.0);
        let (f, _) = self.head_axis(1.0);
        let fwd = unit(sub(f, o));
        let down_back = unit(add(mul(fwd, -0.6), mul(top, -0.8)));
        let thick = 0.84;
        for (k, phi) in [0.0, 42.0, -42.0].iter().enumerate() {
            let width = if k == 0 { 2.0 } else { 1.6 };
            let e = 0.5 * thick - 0.46;
            let skin = |u: f64| {
                let (p, n) = self.head_skin(u, *phi);
                (add(p, mul(n, e)), n)
            };
            let mut path = Vec::new();
            let mut ups = Vec::new();
            for u in [0.78, 0.56, 0.32, 0.1] {
                let (p, n) = skin(u);
                path.push(if u > 0.7 { sub(p, mul(n, 0.45)) } else { p });
                ups.push(n);
            }
            let (last, ln) = skin(0.1);
            path.push(add(last, mul(down_back, 0.75)));
            ups.push(unit(add(ln, mul(down_back, -0.25))));
            path.push(add(last, mul(down_back, 1.45)));
            ups.push(unit(add(ln, mul(down_back, -0.4))));
            out.push((format!("Hair plume {}", k + 1), path_plume(&path, &ups, width, thick, false)?, self.head_skin(0.4, *phi).1));
        }
        Ok(out)
    }
    /// Contour feathers dressing her breast and back, each lying along the torso's skin as it flows down it.
    fn plumes(&self) -> Result<Vec<(String, Operation, P3)>> {
        let mut out = Vec::new();
        let rows: [(&str, f64, &[f64], f64); 4] = [
            ("Breast", 0.95, &[0.0, 42.0, -42.0], 1.9),
            ("Breast", 0.76, &[24.0, -24.0, 66.0, -66.0], 2.1),
            ("Breast", 0.57, &[0.0], 2.2),
            ("Mantle", 0.9, &[180.0, 148.0, -148.0], 2.1),
        ];
        let mut n = HashMap::new();
        for (what, s, phis, width) in rows {
            for &phi in phis {
                let thick = 0.85;
                let e = 0.5 * thick - 0.3;
                let stations = [s, s - 0.12, s - 0.25, s - 0.36];
                let mut path = Vec::new();
                let mut ups = Vec::new();
                for (i, &si) in stations.iter().enumerate() {
                    let (p, nrm) = self.torso_skin(si, phi);
                    path.push(add(p, mul(nrm, e + 0.06 * i as f64)));
                    ups.push(nrm);
                }
                let up = ups[1];
                let count = n.entry(what).or_insert(0);
                *count += 1;
                out.push((format!("{what} feather {count}"), path_plume(&path, &ups, width, thick, false)?, up));
            }
        }
        Ok(out)
    }
}

// --- The feet --------------------------------------------------------------------------------------------------------

/// The ankle in the stone's cylinder: radius, degrees round from east toward the high cheek, height over the girdle.
const ANKLE: [f64; 3] = [4.9, 126.0, -0.6];
/// Radius from the stone's axis at which the toes run round the girdle, mm.
const RUN_RHO: f64 = 4.78;
/// Where on the crown each claw's round tip rests, mm from the stone's axis.
const CONTACT_RHO: f64 = 3.7;

/// A toe of the high foot: which way round it runs from the ankle, where it hooks over the girdle, and the height it runs at.
struct Toe {
    name: &'static str,
    sign: f64,
    hook_psi: f64,
    run_h: f64,
}

const TOES: [Toe; 4] = [
    Toe { name: "hallux", sign: 1.0, hook_psi: 152.0, run_h: -0.9 },
    Toe { name: "inner toe", sign: -1.0, hook_psi: 104.0, run_h: -0.85 },
    Toe { name: "middle toe", sign: -1.0, hook_psi: 83.0, run_h: -1.05 },
    Toe { name: "outer toe", sign: -1.0, hook_psi: 58.0, run_h: -1.55 },
];

/// A toe's path: round the girdle from the ankle at its run height, up the girdle's side in a quarter turn, then curled
/// over the rim to its claw's round tip resting on the crown, ending at the tip's apex.
fn toe_path(st: &Stone, toe: &Toe) -> Vec<P3> {
    let slope = (0.55 * st.crown - st.half) / (0.22 * st.r);
    let n = [slope / slope.hypot(1.0), 1.0 / slope.hypot(1.0)];
    let contact = [CONTACT_RHO, st.half + (st.r - CONTACT_RHO) * slope];
    let tip = [contact[0] + (TIP_R + STONE_GAP + 0.01) * n[0], contact[1] + (TIP_R + STONE_GAP + 0.01) * n[1]];
    let curl = RUN_RHO - tip[0];
    let base_h = tip[1] - curl;
    let up = base_h - toe.run_h;
    let s_hook = RUN_RHO * toe.hook_psi.to_radians();
    let s_start = s_hook - toe.sign * up;
    let s_ankle = RUN_RHO * ANKLE[1].to_radians();
    let fall = (6.0 * (ANKLE[2] - toe.run_h).abs() * 0.9).sqrt().clamp(1.0, 2.6);
    let mut path = Vec::new();
    let run = (s_start - s_ankle).abs();
    let steps = (run / 0.2).ceil().max(1.0) as usize;
    for i in 0..steps {
        let x = run * i as f64 / steps as f64;
        let h = lerp(ANKLE[2], toe.run_h, smooth(0.0, fall, x));
        let rho = lerp(ANKLE[0], RUN_RHO, smooth(0.0, 1.0, x));
        path.push(st.world(rho, (s_ankle + toe.sign * x) / RUN_RHO * 180.0 / PI, h));
    }
    for i in 0..9 {
        let b = (i as f64 * 10.0).to_radians();
        let s = s_start + toe.sign * up * b.sin();
        path.push(st.world(RUN_RHO, s / RUN_RHO * 180.0 / PI, toe.run_h + up * (1.0 - b.cos())));
    }
    for i in 0..=9 {
        let a = (i as f64 * 10.0).to_radians();
        path.push(st.world(tip[0] + curl * a.cos(), toe.hook_psi, base_h + curl * a.sin()));
    }
    path.push(st.world(tip[0] - TIP_R, toe.hook_psi, tip[1]));
    path
}

/// A toe's radius `d` mm along its `len`: knuckles swelling at a third and three fifths, the claw tapering to a round tip.
fn toe_radius(d: f64, len: f64) -> f64 {
    let dome = len - TIP_R;
    if d >= dome {
        let x = d - dome;
        return (TIP_R * TIP_R - x * x).max(0.0).sqrt().max(0.1);
    }
    let claw = len - 2.1;
    let body = |d: f64| 0.45 + 0.07 * (-((d - 0.3 * claw) / 0.28).powi(2)).exp() + 0.07 * (-((d - 0.62 * claw) / 0.28).powi(2)).exp();
    if d <= claw {
        body(d)
    } else {
        let f = (d - claw) / (dome - claw);
        lerp(body(claw), TIP_R, f * f * (3.0 - 2.0 * f))
    }
}

/// How far a spine's tangent turns over its last `span` mm, degrees.
fn end_turn(sp: &Spine, span: f64) -> f64 {
    let (_, a, _) = sp.at(sp.length() - span);
    let (_, b, _) = sp.at(sp.length());
    dot(a, b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The high foot: its ringed tarsus from her thigh to the ankle, and four knuckled toes hooking their claws over the girdle.
struct Foot {
    parts: Vec<(String, Operation)>,
    /// Each toe's last-2-mm curl, degrees.
    curls: Vec<(String, f64)>,
    /// The nearest any toe comes to the stone, mm.
    stone_gap: f64,
}

fn foot(st: &Stone, fig: &Figure) -> Result<Foot> {
    let mut parts = Vec::new();
    let mut curls = Vec::new();
    let mut nearest = f64::MAX;
    let ankle = st.world(ANKLE[0], ANKLE[1], ANKLE[2]);
    let (thigh, tn) = fig.torso_skin(0.46, 64.0);
    let thigh = sub(thigh, mul(tn, 0.55));
    let mid = add(mul(add(thigh, ankle), 0.5), mul(unit([0.2, -0.3, 1.0]), 0.25));
    let tarsus = Spine::through(&[thigh, mid, ankle], [0.0, 1.0, 0.0]);
    let len = tarsus.length();
    let at: Vec<f64> = (0..=((len / 0.21).ceil() as usize)).map(|i| (i as f64 * 0.21).min(len)).collect();
    let ring = |d: f64| lerp(0.66 + 0.045 * (2.0 * PI * d / 0.42).cos(), 0.76, smooth(len - 0.5, len, d));
    for &d in &at {
        let (p, _, _) = tarsus.at(d);
        nearest = nearest.min(st.gap(p) - ring(d));
    }
    parts.push(("Tarsus".to_string(), round_loft(&tarsus, &at, ring)));
    for toe in &TOES {
        let control = toe_path(st, toe);
        let sp = Spine::through(&control, [0.0, 1.0, 0.0]);
        let len = sp.length();
        let dome = len - TIP_R;
        let claw = len - 2.1;
        let step = (claw / 16.0).max(0.3);
        let mut at: Vec<f64> = (0..).map(|i| i as f64 * step).take_while(|d| *d < claw).collect();
        at.extend([0.3 * claw, 0.62 * claw]);
        at.extend((0..).map(|i| claw + i as f64 * 0.2).take_while(|d| *d < dome - 0.05));
        at.extend([0.0, 0.12, 0.22, 0.3, 0.36, 0.4].iter().map(|x| dome + x));
        at.sort_by(f64::total_cmp);
        at.dedup_by(|a, b| (*a - *b).abs() < 0.08);
        // Along the toe its sections' clearance, over the dome the ball its round tip lies on.
        for &d in at.iter().filter(|d| **d < dome) {
            let (p, _, _) = sp.at(d);
            let gap = st.gap(p) - toe_radius(d, len);
            if std::env::var("HARPYIA_DEBUG").is_ok() && gap < 0.06 {
                eprintln!("{}: gap {gap:.4} at {d:.2} of {len:.2} (rho {:.3}, h {:.3})", toe.name, p[0].hypot(p[2]), p[1] - st.g[1]);
            }
            nearest = nearest.min(gap);
        }
        let (tip, _, _) = sp.at(dome);
        nearest = nearest.min(st.gap(tip) - TIP_R);
        let name = toe.name;
        curls.push((format!("Talon, {name}"), end_turn(&sp, 2.0)));
        parts.push((format!("Talon, {name}"), round_loft(&sp, &at, |d| toe_radius(d, len))));
    }
    Ok(Foot { parts, curls, stone_gap: nearest })
}

// --- Painting --------------------------------------------------------------------------------------------------------

/// Whether `theta` lies in `from..to` (degrees, wrapping), eased over `fade` at both ends: 0..1.
fn arc(theta: f64, from: f64, to: f64, fade: f64) -> f64 {
    let span = (to - from).rem_euclid(360.0);
    let d = (theta - from).rem_euclid(360.0);
    if d > span + fade && d < 360.0 - fade {
        return 0.0;
    }
    let inside = if d <= span { d } else { d - 360.0 };
    smooth(-fade, 0.0, inside.min(span - inside).min(inside)) * smooth(-fade, 0.0, span - inside)
}

/// Rounded shingles pointing toward +`along`: 0 at each one's root rising to 1 at its free edge, where across it the point
/// sits (-0.5..0.5), and how far ahead its free edge lies, mm.
fn shingle(along: f64, across: f64, pa: f64, pc: f64, dip: f64) -> (f64, f64, f64) {
    let edge = |k: i64| {
        let shift = if k.rem_euclid(2) == 0 { 0.0 } else { 0.5 };
        let w = (across / pc + shift).rem_euclid(1.0) - 0.5;
        ((k + 1) as f64 * pa - dip * pa * (2.0 * w).powi(2), w)
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

/// Scutes in staggered columns `width` across and `len` along: each a domed tile bevelled on all four sides inside its
/// own groove, its free edge toward +`along` standing a little prouder than its root.
fn scutes(along: f64, across: f64, len: f64, width: f64) -> f64 {
    let col = (across / width).floor();
    let dc = across - (col + 0.5) * width;
    let shift = if (col as i64).rem_euclid(2) == 0 { 0.0 } else { 0.5 * len };
    let du = (along + shift).rem_euclid(len);
    let bevel = smooth(0.0, 0.2, du).min(smooth(0.0, 0.14, len - du)).min(smooth(0.0, 0.2, 0.5 * width - dc.abs()));
    let dome = (1.0 - 0.25 * (2.0 * dc / width).powi(2)) * (0.78 + 0.22 * du / len);
    0.18 + 0.82 * bevel * dome
}

/// Round scales on a hexagonal lattice `size` apart, each domed inside its own groove.
fn hex_scales(x: f64, y: f64, size: f64) -> f64 {
    let row = size * 3f64.sqrt() * 0.5;
    let j0 = (y / row).floor() as i64;
    let mut d = [f64::MAX, f64::MAX];
    for j in j0 - 1..=j0 + 2 {
        let shift = if j.rem_euclid(2) == 0 { 0.0 } else { 0.5 * size };
        let i0 = ((x - shift) / size).floor() as i64;
        for i in i0 - 1..=i0 + 2 {
            let c = [i as f64 * size + shift, j as f64 * row];
            let dist = (x - c[0]).hypot(y - c[1]);
            if dist < d[0] {
                d = [dist, d[0]];
            } else if dist < d[1] {
                d[1] = dist;
            }
        }
    }
    let border = 0.5 * (d[1] - d[0]);
    let dome = (1.0 - (d[0] / (0.55 * size)).powi(2)).max(0.0).sqrt();
    0.2 + 0.8 * dome * smooth(0.0, 0.1, border)
}

/// The tail at the palm, `along` mm from its rump and `across` the section: nine rectrices fanned over crown and cheeks,
/// the centre one on top, each with its own outline, a raised rachis and a rounded tip.
fn tail_fan(along: f64, across: f64) -> f64 {
    if !(0.0..16.5).contains(&along) {
        return 0.0;
    }
    let mut h = 0.0f64;
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
        h = h.max(base * (0.2 + (vane + rachis - 0.2) * edge));
    }
    h * smooth(0.0, 1.0, along)
}

/// The harpy's plumage and scaled leg over the bare band.
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
    // Tail fanned from the rump across crown and cheeks toward the palm.
    let rump = 228.0;
    let tail = tail_fan((theta - rump).rem_euclid(360.0).to_radians() * crest, across) * clear;
    // Thighs: contour feathers flowing on toward the leg.
    let (tt, wt, at) = shingle(u, across, 2.6, 2.0, 0.7);
    let thigh = contour(tt, wt, at, 2.0) * arc(theta, 284.0, 352.0, 5.0) * clear;
    // The scaled leg: transverse scutes on the crown in two columns, free edges toward the foot.
    let leg = scutes(u, across + 1.1, 1.55, 2.2) * crown * arc(theta, 348.0, 80.0, 4.0);
    // Its sides: round scales on both cheeks.
    let side = hex_scales(theta.to_radians() * r, r, 0.95) * cheek * clear * arc(theta, 348.0, 84.0, 4.0);
    // Cheek feathers under the wings' trailing edge and beside the stone.
    let (tc, wc, ac) = shingle(theta.to_radians() * r, r, 2.4, 1.6, 0.7);
    let cheeks = contour(tc, wc, ac, 1.6) * cheek * clear * arc(theta, 84.0, 232.0, 4.0);
    mantle.max(tail).max(thigh).max(leg).max(side).max(cheeks)
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
    stone: Stone,
    feathers: Vec<Feather>,
    figure_stone_gap: f64,
    foot_curls: Vec<(String, f64)>,
    foot_stone_gap: f64,
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
    let fig = Figure::new(crest, g);
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Band".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    doc.append(builders::stone_feature(2, gem, at))?;
    doc.append(builders::feature_on(3, "Seat bur", BUR, 2, json!({"through": true})))?;
    let mut p = Parts { doc, id: 4, families: HashMap::new(), copies: HashMap::new() };
    p.add("Torso".into(), fig.torso_loft(), Family::Round)?;
    p.add("Neck".into(), fig.neck_loft(), Family::Round)?;
    p.add("Head".into(), fig.head_loft(), Family::Round)?;
    let (face, fwd) = fig.face_loft();
    p.add("Face".into(), face, Family::Plate(fwd))?;
    for (name, op, up) in fig.hood()? {
        p.add(name, op, Family::Plate(up))?;
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
    Ok(Authored { stone, feathers, figure_stone_gap, foot_curls: f.curls, foot_stone_gap: f.stone_gap, families: p.families, copies: p.copies })
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

/// Studio-gold renders with the sapphire set: the named views, a close-up on the grip, the bare band against the finished
/// ring, and a 300 px contact strip.
fn renders(out: &Path, finished: &render::Finished, st: &Stone, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    for (name, yaw, pitch) in [("hero", 0.55, 0.95), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", -0.9, 0.62), ("reverse", 1.6, 0.8)] {
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

/// A part's land width: from each face the family measures, into the metal along its normal to the first face turned
/// back against it within 12 degrees. Returns the thinnest and the area of faces reading under the floor.
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
        while t < 3.0 && first.0 == f64::MAX {
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
    (thinnest, thin_area)
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
fn land_widths(built: &mesh::BuildResult, a: &Authored) -> Vec<Land> {
    let mut out = Vec::new();
    for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
        let Some(family) = a.families.get(&c.name).copied() else { continue };
        let Some(m) = &c.made else { continue };
        let (thin, area) = land_width(m.solid(), family);
        let copy = a.copies.get(&c.name);
        let note = match (copy, c.name.starts_with("High talon") || c.name == "Low foot") {
            (Some(of), _) if of.iter().any(|n| n.starts_with("High talon")) => {
                "mirror of the high foot; every claw cast blunt with a 0.85 mm round tip, points filed after setting if wanted".into()
            }
            (Some(of), _) => format!("mirror of {} parts", of.len()),
            (None, true) => "claw cast blunt with a 0.85 mm round tip; points filed after setting if wanted".into(),
            (None, false) => String::new(),
        };
        let measured = match family {
            Family::Plate(_) => "plate, belly to top".into(),
            Family::Round => "round, every direction".into(),
        };
        out.push(Land { part: c.name.clone(), measured, min_section_mm: thin, under_floor_mm2: area, note });
    }
    out
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
    let lands = land_widths(&built, &authored);
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
        setup.bench_notes = "Invest the band, figure, feet and both wings as one tree; clear investment from between the primaries' fingers and under the hood's brim. Seat the 8 mm sapphire in the cut bearing, then close the eight claws over its girdle; file the claws' blunt tips to points if wanted.".into();
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
