//! Bestiarium — Harpyia, the snatcher: six talons locked on a storm sapphire, her wings displayed either side of it on
//! both cheeks as meshed lofted feathers, her plumage, legs and tail painted over the band.
//! Build unguarded, then run under an 8 GiB scope:
//! cargo build --offline --release -p ringdesign-core --example bestiarium_harpyia
//! systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_harpyia [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use std::collections::HashMap;
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        self, Attach, Component, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, Stage,
        builders::{self, BUR, CATHEDRAL, CLAW},
        pattern::Sources,
    },
    castability::{self, CastProcess},
    csg, dfm, library,
    manufacturing as mf,
    stl,
    field::Window,
    skin::{self, Atlas, Sample},
    gem::{Gem, GemCut},
    mesh, render,
    sketch::{Geometry, Sketch, Workplane},
};
use serde_json::json;
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
};

/// Half the band's width: the cheeks are the planes z = ±CHEEK.
const CHEEK: f64 = 3.6;
/// The investment section every free part keeps.
const MIN_SECTION_MM: f64 = 0.8;
/// Where every feather's belly stands, sunk into the cheek.
const BED_Z: f64 = 3.3;
/// Radius past which the wing leans out from the cheek's plane.
const HINGE_R: f64 = 13.0;
/// Most the wing leans out above the crown, degrees.
const SPLAY_DEG: f64 = 24.0;
/// Rise over which the lean comes in, mm.
const SPLAY_RAMP: f64 = 2.5;
/// Nearest a feather's edge comes to the finger's axis, mm: the bore's radius and a clear margin.
const BORE_CLEAR_R: f64 = 9.5 + 0.35;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..Default::default() }
}

/// The export build: under the triangle budget with all four wings joined.
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1280, profile_steps: 416, ..Default::default() }
}

/// Most triangles an export build may carry.
const TRIANGLE_BUDGET: usize = 2_000_000;

fn sapphire() -> Gem {
    Gem { preview_tint: Some([0.03, 0.09, 0.42]), ..Gem::calibrated(GemCut::Round, 8.0) }
}

fn band() -> RingDesign {
    let mut d = RingDesign { name: "Harpyia — the snatcher".into(), ..Default::default() };
    d.size = ringdesign_core::RingSize::from_diameter_mm(19.0);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 2.0 * CHEEK;
    d.profile.thickness_mm = 3.8;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Uniform;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

fn part(attach: Attach, blend_mm: f64) -> Component {
    Component { attach, stage: Stage::Cast, blend_mm, placement: Placement::Free, material: "Gold 18k".into(), ..Default::default() }
}

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
fn plane(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Workplane {
    Workplane { origin: origin.map(|c| (c * 1e5).round() / 1e5), x, y, on_face: None }
}

/// A feather's section `half` either side of its rachis and `t` thick: a bellied underside, round edges, vanes
/// cambered up to a channel either side of a raised rachis, in eight cubic pieces from the leading edge.
fn vane(half: f64, t: f64, camber: f64, ridge: f64, belly: f64) -> Sketch {
    let e = 0.5 * t;
    let xn = (half - e).max(0.06);
    let fit = (xn / 0.6).min(1.0);
    let (camber, belly, depth, ridge) = (camber * fit, belly * fit, 0.1 * fit, ridge * fit);
    let q1 = (0.2 + 0.05 * half).min(0.32 * xn);
    let q2 = (q1 + 0.3 * (xn - q1)).min(0.7 * xn);
    let rim = e + camber;
    let l = 4.0 / 3.0 * e;
    // Slope the nose's upper end turns to, rising into the outer vane.
    let slope_edge = 2.0 * camber / (xn - q2).max(1e-6);
    let lean = (1.0 + slope_edge * slope_edge).sqrt();
    let k = 4.0 / 3.0 * belly;
    let under = (1.0f64).hypot(k / (2.0 * xn / 3.0));
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
    chain(&[
        [[-xn, -e], [-xn / 3.0, -e - k], [xn / 3.0, -e - k], [xn, -e]],
        [[xn, -e], [xn + l / under, -e + l * (k / (2.0 * xn / 3.0)) / under], [xn + l / lean, e - l * slope_edge / lean], [xn, e]],
        outer(1.0),
        channel(1.0),
        [[q1, rim], [0.55 * q1, rim + h1], [-0.55 * q1, rim + h1], [-q1, rim]],
        rev(channel(-1.0)),
        rev(outer(-1.0)),
        [[-xn, e], [-xn - l / lean, e - l * slope_edge / lean], [-xn - l / under, -e + l * (k / (2.0 * xn / 3.0)) / under], [-xn, -e]],
    ])
}

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
    /// Where along the rachis its sections stand.
    stations: &'static [f64],
}

/// Stations along a flight feather.
const LONG: &[f64] = &[0.0, 0.06, 0.15, 0.28, 0.45, 0.62, 0.76, 0.87, 0.95, 1.0];
/// Stations along a covert.
const SHORT: &[f64] = &[0.0, 0.12, 0.3, 0.55, 0.78, 0.92, 1.0];

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
    /// Half the vane's width at `s`: a narrow quill, the vane opening, holding, then rounding off at the tip.
    fn half(&self, s: f64) -> f64 {
        let full = 0.5 * self.width;
        let quill = (0.5 * self.thick + 0.06).max(0.46);
        let open = smooth(self.quill, self.quill + 0.18, s);
        let hold = full * (1.0 - 0.1 * smooth(0.3, 1.0 - self.round, s));
        let from = 1.0 - self.round;
        let round = if s > from { (1.0 - ((s - from) / self.round).powi(2)).max(0.0).sqrt() } else { 1.0 };
        (quill + (hold - quill) * open * round).max(quill)
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
        let sections = self
            .stations
            .iter()
            .map(|&s| {
                let (p, t) = self.rachis(s);
                let z = CHEEK + self.under + e + self.lift * smooth(0.25, 1.0, s);
                let half = self.half(s);
                let mut v = vane(half, self.thick, 0.16 * half, 0.15, 0.08);
                v.plane = splayed([p[0], p[1], z], [-t[1], t[0], 0.0], [0.0, 0.0, 1.0]);
                v.into()
            })
            .collect();
        Ok(Operation::Loft { sections, meshed: true })
    }
}

/// The high cheek's plane bent outward above the crown: a point `h` past the hinge lies on a curve leaning out by up
/// to SPLAY_DEG, keeping its offset from the plane along the curve's normal.
fn splay(p: [f64; 3]) -> [f64; 3] {
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
fn splayed(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> Workplane {
    let e = 1e-4;
    let o = splay(origin);
    let dir = |v: [f64; 3]| {
        let q = splay([origin[0] + e * v[0], origin[1] + e * v[1], origin[2] + e * v[2]]);
        let d = [q[0] - o[0], q[1] - o[1], q[2] - o[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        d.map(|c| c / l)
    };
    let x2 = dir(x);
    let y1 = dir(y);
    let k = x2[0] * y1[0] + x2[1] * y1[1] + x2[2] * y1[2];
    let y2 = [y1[0] - k * x2[0], y1[1] - k * x2[1], y1[2] - k * x2[2]];
    let l = (y2[0] * y2[0] + y2[1] * y2[1] + y2[2] * y2[2]).sqrt();
    plane(o, x2, y2.map(|c| c / l))
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

/// The wing's leading edge from shoulder to wrist, (theta degrees, radius mm) at `u` in 0..1.
fn arm(u: f64) -> [f64; 2] {
    const KEYS: [[f64; 2]; 5] = [[104.0, 12.9], [108.5, 14.2], [113.5, 15.4], [118.5, 16.2], [124.0, 16.5]];
    let n = KEYS.len() - 1;
    let x = u.clamp(0.0, 1.0) * n as f64;
    let i = (x.floor() as usize).min(n - 1);
    let t = x - i as f64;
    let k = |j: isize| KEYS[j.clamp(0, n as isize) as usize];
    let (p0, p1, p2, p3) = (k(i as isize - 1), k(i as isize), k(i as isize + 1), k(i as isize + 2));
    let cr = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t + (-a + 3.0 * b - 3.0 * c + d) * t * t * t);
    [cr(p0[0], p1[0], p2[0], p3[0]), cr(p0[1], p1[1], p2[1], p3[1])]
}

/// A feather from `root` heading `heading` degrees in the cheek plane for `reach` mm.
fn aimed(root: [f64; 2], heading: f64, reach: f64) -> [f64; 2] {
    let h = heading.to_radians();
    [root[0] + reach * h.cos(), root[1] + reach * h.sin()]
}

/// The spread wing on the high cheek, bottom layer first: secondaries sweeping down the cheek, the primaries' fingers
/// standing clear above the crown, then the coverts over their roots.
fn wing() -> Vec<Feather> {
    let mut out = Vec::new();
    let under = BED_Z - CHEEK;
    // Secondaries sweep back from the forearm, their tips stepping down the cheek.
    let secondaries = 6;
    for k in 0..secondaries {
        let f = k as f64 / (secondaries - 1) as f64;
        out.push(Feather {
            name: format!("Secondary {}", k + 1),
            root: {
                let [th, r] = arm(lerp(0.12, 0.62, f));
                polar(th, r - 0.55)
            },
            tip: polar(lerp(224.0, 170.0, f), lerp(12.0, 13.1, f)),
            bow: 0.0,
            width: 3.2,
            thick: 0.9,
            under: under + 0.012 * k as f64,
            lift: 0.1,
            quill: 0.04,
            round: 0.2,
            follow: 1.0,
            stations: LONG,
        });
    }
    // Primaries: fingers fanned from the hand, emarginate toward their tips and parted above the crown.
    let primaries = 6;
    for k in 0..primaries {
        let f = k as f64 / (primaries - 1) as f64;
        let [th, r] = arm(lerp(0.66, 1.0, f));
        let root = polar(th, r - lerp(0.45, 1.2, f));
        out.push(Feather {
            name: format!("Primary {}", k + 1),
            root,
            tip: aimed(root, lerp(186.0, 250.0, f), lerp(6.3, 9.6, f)),
            bow: 0.5,
            width: lerp(3.0, 3.3, f),
            thick: 0.95,
            under: under + 0.1 - 0.012 * k as f64,
            lift: 0.12,
            quill: 0.05,
            round: 0.42,
            follow: 0.0,
            stations: LONG,
        });
    }
    // Greater coverts over the flight feathers' roots, from the shoulder to the wrist.
    let greater = 6;
    for k in 0..greater {
        let f = k as f64 / (greater - 1) as f64;
        let [th, r] = arm(lerp(0.08, 0.96, f));
        let root = polar(th, r - 0.6);
        out.push(Feather {
            name: format!("Greater covert {}", k + 1),
            root,
            tip: aimed(root, th + 90.0 + lerp(12.0, 32.0, f), lerp(4.2, 5.1, f)),
            bow: 0.2,
            width: 3.2,
            thick: 1.2,
            under: under + 0.05 - 0.012 * k as f64,
            lift: 0.06,
            quill: 0.08,
            round: 0.34,
            follow: 0.0,
            stations: SHORT,
        });
    }
    // Lesser coverts: small scales under the leading edge.
    let lesser = 6;
    for k in 0..lesser {
        let f = k as f64 / (lesser - 1) as f64;
        let [th, r] = arm(lerp(0.05, 0.92, f));
        let root = polar(th, r - 0.15);
        out.push(Feather {
            name: format!("Lesser covert {}", k + 1),
            root,
            tip: aimed(root, th + 90.0 + lerp(18.0, 38.0, f), lerp(2.7, 3.1, f)),
            bow: 0.1,
            width: 2.8,
            thick: 1.45,
            under: under + 0.02 - 0.012 * k as f64,
            lift: 0.04,
            quill: 0.1,
            round: 0.4,
            follow: 0.0,
            stations: SHORT,
        });
    }
    // Marginal coverts: small scales shingled along the leading edge from the shoulder to the wrist.
    let marginal = 9;
    for k in 0..marginal {
        let f = k as f64 / (marginal - 1) as f64;
        let u = lerp(0.0, 0.92, f);
        let [th, r] = arm(u);
        let [th2, r2] = arm((u + 0.2).min(1.0));
        let (root, ahead) = (polar(th, r), polar(th2, r2));
        let heading = (ahead[1] - root[1]).atan2(ahead[0] - root[0]).to_degrees();
        out.push(Feather {
            name: format!("Marginal covert {}", k + 1),
            root,
            tip: aimed(root, heading, lerp(2.3, 2.9, f)),
            bow: 0.0,
            width: lerp(2.0, 2.5, f),
            thick: 1.6,
            under: under - 0.012 * k as f64,
            lift: 0.05,
            quill: 0.12,
            round: 0.45,
            follow: 0.0,
            stations: SHORT,
        });
    }
    out
}


/// Crest radius: the chart's `u` is arc here.
const CREST_R: f64 = 9.5 + 3.8;
/// Height of the painted plumage, mm.
const PLUMAGE_MM: f64 = 0.36;
/// Atlas columns round the ring.
const ATLAS_W: usize = 1536;

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

/// The tail's rectrices on the crown past `from` degrees: five overlapping feathers, the centre pair longest and highest.
fn rectrices(theta: f64, z: f64, from: f64) -> f64 {
    let along = (theta - from).rem_euclid(360.0).to_radians() * CREST_R;
    if along > 14.0 {
        return 0.0;
    }
    let mut h: f64 = 0.0;
    for k in -2i32..=2 {
        let c = k as f64 * 1.45;
        let len = 12.5 - 1.7 * (k.abs() as f64);
        let half = 0.95 * (1.0 - 0.1 * smooth(0.0, len, along));
        let tip = if along > len - 1.6 { (1.0 - ((along - (len - 1.6)) / 1.6).powi(2)).max(0.0).sqrt() } else { 1.0 };
        let dc = (z - c).abs() / (half * tip).max(1e-6);
        if dc >= 1.0 || along < 0.0 {
            continue;
        }
        let profile = (1.0 - dc * dc).powf(0.35);
        let rachis = 0.12 * (1.0 - smooth(0.08, 0.16, dc));
        let base = 1.0 - 0.16 * k.abs() as f64;
        h = h.max(base * (0.55 + 0.45 * profile) + rachis * base);
    }
    h * smooth(0.0, 1.2, along)
}

/// The harpy's plumage and scaled legs over the bare band: mantle on the crown behind the head, the tail at the palm,
/// scutes down the forward shoulder, and contour feathers on both cheeks wherever the wings leave them bare.
fn plumage_at(a: &Atlas, s: &Sample) -> f64 {
    let r = s.p[0].hypot(s.p[1]);
    let theta = s.theta;
    // Degrees round from the stone, either way.
    let phi = ((theta - 90.0 + 180.0).rem_euclid(360.0) - 180.0).abs();
    let cheek = smooth(0.55, 0.8, s.n[2].abs());
    let crown = 1.0 - cheek;
    let clear = smooth(a.bore + 0.75, a.bore + 1.35, r);
    // Mantle: contour feathers flowing from the stone toward the palm on both sides.
    let (t, w, ahead) = shingle(phi.to_radians() * CREST_R, s.p[2], 2.9, 2.2, 0.7);
    let mantle = contour(t, w, ahead, 2.2) * crown * smooth(36.0, 44.0, phi) * (1.0 - smooth(140.0, 146.0, phi));
    // Tail at the palm.
    let tail = rectrices(theta, s.p[2], 234.0) * crown * arc(theta, 232.0, 306.0, 4.0);
    // Scutes: the scaled legs under both arches, two plates across, overlapping toward the talons.
    let fore = (46.0 - phi).to_radians() * CREST_R;
    let (ts, _, ahead_s) = shingle(fore, s.p[2] + 1.8, 1.35, 3.6, 0.3);
    let scutes = (0.3 + 0.7 * ts.powf(0.7) * smooth(0.0, 0.12, ahead_s)) * crown * smooth(12.0, 17.0, phi) * (1.0 - smooth(40.0, 45.0, phi));
    // Cheek feathers flowing from the stone toward the palm, clear of the bore's edge and of the wings.
    let (tc, wc, ac) = shingle(phi.to_radians() * r, r, 2.6, 1.7, 0.7);
    let wings = smooth(128.0, 136.0, phi);
    let body = contour(tc, wc, ac, 1.7) * cheek * clear * wings;
    mantle.max(tail).max(scutes).max(body)
}

/// Paints the plumage on the bare band's atlas and adds it as one layer.
fn plumage(d: &mut RingDesign, lib: &mut AlphaLibrary) -> Result<Alpha> {
    let span = d.reference_loop().surface_len_mm;
    let height = ((ATLAS_W as f64) * span / (2.0 * PI * CREST_R)).round() as usize;
    let a = Atlas::of(d, ATLAS_W, height)?;
    let mut alpha = a.paint("Harpyia plumage", |s| plumage_at(&a, s));
    for v in &mut alpha.data {
        *v = (*v * 255.0).round() / 255.0;
    }
    lib.insert(Alpha::from_png16(alpha.name.clone(), &alpha.to_png16()?)?);
    let mut e = skin::hide_layer(d, "Harpyia plumage", PLUMAGE_MM, Window::around(90.0, 360.0));
    e.name = "Plumage".into();
    d.layers.layers.push(e);
    Ok(alpha)
}

/// The head, the legs, the seat and both wings as the design's parts.
fn parts(d: &mut RingDesign) -> Result<Vec<Feather>> {
    let gem = sapphire();
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Band".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    doc.append(builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm(CLAW, gem))))?;
    let mut talons = builders::feature_on(3, "Talons", CLAW, 2, json!({"prongs": 6, "wire_mm": 1.3, "style": "Talon", "grouping": "Feet", "tip": "Point"}));
    talons.component.blend_mm = 0.0;
    doc.append(talons)?;
    let mut legs = builders::feature_on(4, "Legs", CATHEDRAL, 2, json!({"spread_deg": 30.0, "rise": 0.9, "wire_mm": 1.3, "head": 3}));
    legs.component.blend_mm = 0.0;
    doc.append(legs)?;
    doc.append(builders::feature_on(5, "Seat bur", BUR, 2, json!({"through": true})))?;
    let feathers = wing();
    let mut id = 6u64;
    let mut sources = Vec::new();
    for f in &feathers {
        doc.append(Feature { id, name: f.name.clone(), enabled: true, operation: f.loft()?, component: part(Attach::Join, 0.0) })?;
        sources.push(id);
        id += 1;
    }
    // The west wing's far cheek, then both cheeks' wings reflected through the stone to the east.
    let mirror = |doc: &mut Document, id: &mut u64, name: &str, from: &[u64], plane: MirrorPlane| -> Result<Vec<u64>> {
        let mut made = Vec::new();
        for (k, chunk) in from.chunks(builders_limit()).enumerate() {
            doc.append(Feature {
                id: *id,
                name: format!("{name}, {}", k + 1),
                enabled: true,
                operation: Operation::Pattern { sources: Sources(chunk.to_vec()), kind: PatternKind::Mirror { plane: plane.clone() } },
                component: part(Attach::Join, 0.0),
            })?;
            made.push(*id);
            *id += 1;
        }
        Ok(made)
    };
    let far = mirror(&mut doc, &mut id, "West wing, far cheek", &sources, MirrorPlane::Band)?;
    mirror(&mut doc, &mut id, "East wing, near cheek", &sources, MirrorPlane::Section { theta_deg: 90.0 })?;
    mirror(&mut doc, &mut id, "East wing, far cheek", &far, MirrorPlane::Section { theta_deg: 90.0 })?;
    d.cad = Some(doc);
    Ok(feathers)
}

/// Most parts one mirror copies together.
fn builders_limit() -> usize {
    cad::pattern::MAX_PATTERN_SOURCES
}

fn author() -> Result<(RingDesign, AlphaLibrary, Vec<Feather>)> {
    let mut d = band();
    let mut lib = AlphaLibrary::builtin();
    plumage(&mut d, &mut lib)?;
    let feathers = parts(&mut d)?;
    d.embed_alphas(&lib);
    Ok((d, lib, feathers))
}

/// The metal round the stone, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
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

/// Studio-gold renders with the sapphire set: the named views, a close-up on the talons, and the bare band against the finished ring.
fn renders(out: &Path, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    for (name, yaw, pitch) in [("hero", 0.55, 0.95), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", -0.9, 0.62), ("reverse", 1.6, 0.8)] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let head = crop(&finished.metal, [0.0, 15.5, 0.0], 9.5);
    let mut close = vec![render::Part::metal(&head, render::GOLD)];
    close.extend(finished.parts(render::GOLD));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    let bare = mesh::try_build(&band(), &AlphaLibrary::builtin(), draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)
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

/// A lofted feather's thinnest plate: from every belly face, into the metal along its own normal, to the first face turned
/// back against it within 10 degrees; where the vane rolls into its round edge no face answers, and the edge is the
/// authored section's own diameter.
fn min_section(solid: &csg::Solid) -> f64 {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let tri = |i: usize| solid.f[i].map(|k| solid.v[k as usize]);
    let normals: Vec<[f64; 3]> = (0..solid.f.len())
        .map(|i| {
            let [a, b, c] = tri(i);
            let n = cross(sub(b, a), sub(c, a));
            let l = dot(n, n).sqrt().max(1e-30);
            n.map(|v| v / l)
        })
        .collect();
    const CELL: f64 = 0.3;
    let key = |p: [f64; 3]| p.map(|v| (v / CELL).floor() as i64);
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
    let facing = -(10f64.to_radians().cos());
    let mut thinnest = f64::MAX;
    for (i, n) in normals.iter().enumerate() {
        if n[2] > -0.5 {
            continue;
        }
        let [a, b, c] = tri(i);
        let o = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
        let d = n.map(|v| -v);
        let mut seen = std::collections::HashSet::new();
        let mut first = (f64::MAX, 0.0);
        let mut t = 0.0;
        while t < 2.5 && first.0 == f64::MAX {
            let p = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];
            if let Some(cell) = grid.get(&key(p)) {
                for &j in cell {
                    if j == i || !seen.insert(j) {
                        continue;
                    }
                    let [p0, p1, p2] = tri(j);
                    let (e1, e2) = (sub(p1, p0), sub(p2, p0));
                    let h = cross(d, e2);
                    let det = dot(e1, h);
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let s0 = sub(o, p0);
                    let u = dot(s0, h) / det;
                    let q = cross(s0, e1);
                    let v = dot(d, q) / det;
                    if u < 0.0 || v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let hit = dot(e2, q) / det;
                    if hit > 1e-6 && hit < first.0 {
                        first = (hit, dot(normals[j], *n));
                    }
                }
            }
            t += 0.1;
        }
        if first.0 < f64::MAX && first.1 < facing {
            thinnest = thinnest.min(first.0);
        }
    }
    thinnest
}

/// Where the mesh's faces cross one another: the midpoint of each crossing pair.
fn crossing_spots(m: &mesh::Mesh) -> Vec<[f64; 3]> {
    let v: Vec<[f64; 3]> = m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect();
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let seg_hits = |p: [f64; 3], q: [f64; 3], t: [[f64; 3]; 3]| {
        let d = sub(q, p);
        let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
        let h = cross(d, e2);
        let det = dot(e1, h);
        if det.abs() < 1e-18 {
            return false;
        }
        let s0 = sub(p, t[0]);
        let u = dot(s0, h) / det;
        let qq = cross(s0, e1);
        let w = dot(d, qq) / det;
        let x = dot(e2, qq) / det;
        u > 0.0 && w > 0.0 && u + w < 1.0 && x > 0.0 && x < 1.0
    };
    const CELL: f64 = 0.4;
    let key = |p: [f64; 3]| p.map(|c| (c / CELL).floor() as i64);
    let mut grid: HashMap<[i64; 3], Vec<usize>> = HashMap::new();
    for (i, f) in m.faces.iter().enumerate() {
        let t = f.map(|k| v[k as usize]);
        let lo = key([t[0][0].min(t[1][0]).min(t[2][0]), t[0][1].min(t[1][1]).min(t[2][1]), t[0][2].min(t[1][2]).min(t[2][2])]);
        let hi = key([t[0][0].max(t[1][0]).max(t[2][0]), t[0][1].max(t[1][1]).max(t[2][1]), t[0][2].max(t[1][2]).max(t[2][2])]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    grid.entry([x, y, z]).or_default().push(i);
                }
            }
        }
    }
    let mut spots = Vec::new();
    let mut stamp = vec![usize::MAX; m.faces.len()];
    for (i, fi) in m.faces.iter().enumerate() {
        let ti = fi.map(|k| v[k as usize]);
        let lo = key([ti[0][0].min(ti[1][0]).min(ti[2][0]), ti[0][1].min(ti[1][1]).min(ti[2][1]), ti[0][2].min(ti[1][2]).min(ti[2][2])]);
        let hi = key([ti[0][0].max(ti[1][0]).max(ti[2][0]), ti[0][1].max(ti[1][1]).max(ti[2][1]), ti[0][2].max(ti[1][2]).max(ti[2][2])]);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let Some(cell) = grid.get(&[x, y, z]) else { continue };
                    for &j in cell {
                        if j <= i || stamp[j] == i {
                            continue;
                        }
                        stamp[j] = i;
                        let fj = m.faces[j];
                        if fi.iter().any(|k| fj.contains(k)) {
                            continue;
                        }
                        let tj = fj.map(|k| v[k as usize]);
                        let hit = (0..3).any(|k| seg_hits(ti[k], ti[(k + 1) % 3], tj)) || (0..3).any(|k| seg_hits(tj[k], tj[(k + 1) % 3], ti));
                        if hit {
                            spots.push([(ti[0][0] + tj[0][0]) / 2.0, (ti[0][1] + tj[0][1]) / 2.0, (ti[0][2] + tj[0][2]) / 2.0]);
                        }
                    }
                }
            }
        }
    }
    spots
}

/// Each lofted part's thinnest section, measured on its own mesh.
fn free_sections(built: &mesh::BuildResult) -> Vec<(String, f64)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| c.made.as_ref().is_some_and(|m| m.key == cad::loft::LOFT))
        .map(|c| (c.name.clone(), min_section(c.made.as_ref().unwrap().solid())))
        .collect()
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

#[derive(serde::Serialize)]
struct Report {
    name: String,
    process: String,
    size: String,
    bore_mm: f64,
    build: [usize; 2],
    triangles: usize,
    build_s: f64,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    min_angle_deg: f64,
    worst_aspect: f64,
    mesh_self_crossings: usize,
    made_parts: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    parts_joined: usize,
    parts_cut: usize,
    feathers: usize,
    authored_min_section_mm: f64,
    free_part_sections_mm: Vec<(String, f64)>,
    free_part_min_section_mm: f64,
    bore_radius_mm: f64,
    nearest_to_axis_mm: f64,
    vertices_in_bore: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    parts_undercut_mm2: f64,
    undercut_percent: f64,
    thinnest_wall_mm: f64,
    thinnest_wall_theta_deg: f64,
    investment_min_section_mm: f64,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    stone_carats: f64,
    stone_warnings: Vec<String>,
    closest_stones: Option<String>,
    z_extent_mm: f64,
    radial_reach_mm: f64,
    layers: Vec<String>,
    cad_features: usize,
    design_bytes: u64,
    design_format: u64,
    embedded_alphas: usize,
    grams_18k: f64,
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
    let (d, lib, feathers) = author()?;
    let art = out.join("artwork");
    std::fs::create_dir_all(&art)?;
    if let Some(a) = lib.get("Harpyia plumage") {
        std::fs::write(art.join("plumage.png"), a.to_png16()?)?;
    }
    let authored_min = feathers
        .iter()
        .flat_map(|f| f.stations.iter().map(move |&s| f.thick.min(2.0 * f.half(s))))
        .fold(f64::MAX, f64::min);
    ensure!(authored_min >= MIN_SECTION_MM, "A feather is authored under the {MIN_SECTION_MM} mm section: {authored_min:.3}");
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let (watertight, boundary_edges, non_manifold_edges) = (built.report.validation.watertight, built.report.validation.boundary_edges, built.report.validation.non_manifold_edges);
    let (solids_notes, parts_notes, parts_joined, parts_cut) = (built.solids.notes.clone(), built.parts.notes.clone(), built.parts.joined, built.parts.cut);
    let quality = built.report.quality;
    println!(
        "  {} feathers; {} triangles in {build_s:.1} s; watertight {}; degenerate {}; sharpest corner {:.4} deg",
        feathers.len(),
        built.mesh.faces.len(),
        watertight,
        quality.degenerate_faces,
        quality.min_angle_deg
    );
    let made_parts = crossings(&built);
    let free = free_sections(&built);
    let free_min = free.iter().map(|(_, t)| *t).fold(f64::MAX, f64::min);
    let mesh_self_crossings = csg::self_crossings(&solid_of(&built.mesh));
    if mesh_self_crossings > 0 {
        for p in crossing_spots(&built.mesh) {
            println!("    crossing near ({:.3}, {:.3}, {:.3}), theta {:.1}, r {:.3}", p[0], p[1], p[2], p[1].atan2(p[0]).to_degrees(), p[0].hypot(p[1]));
        }
    }
    let (nearest, in_bore) = bore_clearance(&d, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let zs = built.mesh.vertices.iter().map(|p| p.2 as f64);
    let z_extent = zs.clone().fold(f64::MIN, f64::max) - zs.fold(f64::MAX, f64::min);
    let reach = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(0.0, f64::max);
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
        if !same {
            let first = rebuilt.mesh.vertices.iter().zip(&built.mesh.vertices).position(|(a, b)| a != b);
            println!(
                "    vertices {} against {}, faces {} against {}, normals equal {}, first differing vertex {first:?}, layers {} against {}, alphas {:?}",
                rebuilt.mesh.vertices.len(),
                built.mesh.vertices.len(),
                rebuilt.mesh.faces.len(),
                built.mesh.faces.len(),
                rebuilt.mesh.normals == built.mesh.normals,
                saved.layers.layers.len(),
                d.layers.layers.len(),
                cold_lib.names()
            );
        }
        Some(same)
    } else {
        None
    };
    let mut warnings: Vec<String> = stones_report.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
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
        setup.bench_notes = "Invest the band, talons, legs and both wings as one tree. Clear investment from between the flight feathers' free tips above the crown. Finish the six talons' bearings for the 8 mm sapphire, then set after casting.".into();
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
    let mut report = Report {
        name: d.name.clone(),
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: 2.0 * d.inner_radius_mm(),
        build: [params.theta_steps, params.profile_steps],
        triangles: finished.metal.faces.len(),
        build_s,
        watertight,
        boundary_edges,
        non_manifold_edges,
        degenerate_faces: quality.degenerate_faces,
        min_angle_deg: quality.min_angle_deg,
        worst_aspect: quality.worst_aspect,
        mesh_self_crossings,
        made_parts,
        solids_notes,
        parts_notes,
        parts_joined,
        parts_cut,
        feathers: feathers.len(),
        authored_min_section_mm: authored_min,
        free_part_sections_mm: free,
        free_part_min_section_mm: free_min,
        bore_radius_mm: d.inner_radius_mm(),
        nearest_to_axis_mm: nearest,
        vertices_in_bore: in_bore,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        parts_undercut_mm2: field.parts.iter().map(|p| p.undercut_area_mm2).sum(),
        undercut_percent: field.undercut_fraction() * 100.0,
        thinnest_wall_mm: field.thinnest_wall_mm,
        thinnest_wall_theta_deg: field.thinnest_wall_theta_deg,
        investment_min_section_mm: d.draft.min_section_mm,
        dfm_findings: findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        stones_reported: stones_report.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        stone_carats: stones_report.as_ref().map_or(0.0, |s| s.total_carats),
        stone_warnings: warnings,
        closest_stones: stones_report.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle", p.a, p.b, p.gap_mm)),
        z_extent_mm: z_extent,
        radial_reach_mm: reach,
        layers: d.layers.layers.iter().map(|e| e.name.clone()).collect(),
        cad_features: d.cad.as_ref().map_or(0, |c| c.features.len()),
        design_bytes: text.len() as u64,
        design_format,
        embedded_alphas: d.embedded.len(),
        grams_18k: grams,
        cold_reload_identical: cold,
        pattern,
        geometry_gates_passed: false,
    };
    report.geometry_gates_passed = report.watertight
        && report.degenerate_faces == 0
        && report.mesh_self_crossings == 0
        && report.made_parts.iter().all(|(_, n)| *n == 0)
        && report.solids_notes.is_empty()
        && report.parts_notes.is_empty()
        && report.vertices_in_bore == 0
        && report.free_part_min_section_mm >= MIN_SECTION_MM - 1e-6
        && report.field_verdict == castability::Verdict::Castable.label()
        && report.thinnest_wall_mm >= MIN_SECTION_MM
        && report.dfm_findings.is_empty()
        && report.stones_reported as usize == report.stones_previewed
        && report.cold_reload_identical != Some(false)
        && report.triangles <= TRIANGLE_BUDGET;
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &finished, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.4}% undercut, reported only); thinnest wall {:.2} mm at {:.0} deg; dfm {}; stones {} reported, {} previewed; crossings {} on the mesh, {} parts crossed; free sections >= {:.3} mm (authored {:.3}); nearest the axis {:.3} mm of {:.3}; z extent {:.2} mm; reach {:.2} mm; {:.2} g of 18k",
        report.field_verdict,
        report.undercut_percent,
        report.thinnest_wall_mm,
        report.thinnest_wall_theta_deg,
        report.dfm_findings.len(),
        report.stones_reported,
        report.stones_previewed,
        report.mesh_self_crossings,
        report.made_parts.iter().filter(|(_, n)| *n > 0).count(),
        report.free_part_min_section_mm,
        report.authored_min_section_mm,
        report.nearest_to_axis_mm,
        report.bore_radius_mm,
        report.z_extent_mm,
        report.radial_reach_mm,
        report.grams_18k
    );
    for f in &report.dfm_findings {
        println!("    dfm: {f}");
    }
    for n in report.parts_notes.iter().chain(&report.solids_notes).chain(&report.field_notes) {
        println!("    note: {n}");
    }
    println!("  geometry gates {}", if report.geometry_gates_passed { "passed" } else { "FAILED" });
    ensure!(report.geometry_gates_passed, "Harpyia failed its geometry gates; see {}", out.join("report.json").display());
    Ok(())
}
