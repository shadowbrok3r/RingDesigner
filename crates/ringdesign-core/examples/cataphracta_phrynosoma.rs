//! Cataphracta — Phrynosoma, the horned crown: a horned lizard's head on the factory 016 Star, poured in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_phrynosoma
//! target/release/examples/cataphracta_phrynosoma [OUT_DIR] [--draft] [--verify]
//!
//! The head is one sculpted part lying on the star's table, snout toward one along-ring point and its crown of horns
//! fanned over the points behind it: the two great occipital horns over the back point, three temporal horns a side
//! over the diagonals and the across-band points. The sculpt is a distance field meshed by `sculpt` and joined to the
//! unmirrored factory stock.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render,
    sculpt::{self, ellipsoid, round_cone, smax, smin},
    skin::{Atlas, Sample},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const NAME: &str = "Phrynosoma \u{2014} the horned crown";
const SLUG: &str = "phrynosoma";
/// Bore diameter, and the star's face (along the ring, across the band), mm.
const BORE_MM: f64 = 18.6;
const FACE_MM: (f64, f64) = (17.0, 17.0);
/// The investment's fill floor and detail floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;

/// The head's frame: `u` along the ring (world X, the snout toward +X), `h` over the table (world Y), `w` across the
/// band (world Z). The head's centre sits this far toward the snout of the table's centre, mm.
const HEAD_U0: f64 = 0.0;
/// The floor the part is cut at under the table, mm.
const FLOOR_MM: f64 = -1.0;
/// The meshing step, and the face budgets of the head's sculpt and of each shoulder's hide.
const STEP_MM: f64 = 0.05;
const HEAD_FACES: usize = 450_000;
const HIDE_FACES: usize = 380_000;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The factory 016 Star at 17 x 17 on an 18.6 mm bore, unmirrored, poured in lost wax.
fn base() -> Result<RingDesign> {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    let source = PRESETS.iter().find(|p| p.id == "016").expect("016 is bundled").load()?;
    ImportedBase::attach(&mut d, source)?;
    d.profile.width_mm = FACE_MM.1;
    d.shank.head.length_mm = FACE_MM.0;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM)?;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..Default::default() };
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.sand = None;
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_detail_mm = MIN_DETAIL_MM;
    d.draft.min_draft_deg = 0.0;
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = format!("{NAME} / investment / Silver 925");
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
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
    setup.bench_notes = "Factory 016 Star, unmirrored, poured in lost wax. A horned lizard's head lies on the table, its snout toward one \
        along-ring point and its crown of horns fanned over the points behind it. Feed from the palm; invest horn tips up. At the bench: \
        tack and file any short-filled horn tip; polish the horns and the skull's plates, leave the hollows satin."
        .into();
    d.manufacturing = Some(setup);
    Ok(d)
}

// --- Geometry helpers ------------------------------------------------------------------------------------------------

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

/// The star's table, read off the bare atlas: its crown's height, and the pillowed surface as a height map the head's
/// frame stands on.
struct Table {
    /// World Y of the table's crown, mm.
    top: f64,
    /// How far the table falls from its crown to the edge of its face, mm.
    dome_mm: f64,
    /// The outer surface's height over a grid in world (x, z), clamped no lower than `top - TABLE_FLOOR_MM`.
    lo: [f64; 2],
    n: [usize; 2],
    h: Vec<f64>,
}

/// The height map's cell, and how far under the crown it follows the stock before it levels off, mm.
const TABLE_CELL_MM: f64 = 0.12;
const TABLE_FLOOR_MM: f64 = 1.6;

impl Table {
    fn of(d: &RingDesign) -> Result<Self> {
        let a = Atlas::of(d, 2048, 640)?;
        let centre: Vec<f64> = a.samples.iter().filter(|s| s.p[0].abs() < 2.0 && s.p[2].abs() < 2.0 && s.n[1].abs() > 0.9).map(|s| s.p[1]).collect();
        let edge: Vec<f64> = a.samples.iter().filter(|s| a.face(s) > 0.5).map(|s| s.p[1]).collect();
        let top = centre.iter().copied().fold(f64::MIN, f64::max);
        let low = edge.iter().copied().fold(f64::MAX, f64::min);
        let floor = top - TABLE_FLOOR_MM;
        let lo = [-13.0, -10.0];
        let n = [(26.0 / TABLE_CELL_MM) as usize + 1, (20.0 / TABLE_CELL_MM) as usize + 1];
        let mut h = vec![f64::NAN; n[0] * n[1]];
        for s in a.samples.iter().filter(|s| s.p[1] > floor - 1.0) {
            let i = ((s.p[0] - lo[0]) / TABLE_CELL_MM).round();
            let j = ((s.p[2] - lo[1]) / TABLE_CELL_MM).round();
            if i < 0.0 || j < 0.0 || i as usize >= n[0] || j as usize >= n[1] {
                continue;
            }
            let k = j as usize * n[0] + i as usize;
            if !(h[k] >= s.p[1]) {
                h[k] = s.p[1];
            }
        }
        // Fill the cells no sample fell in from their neighbours, level the map off at the floor, and soften it once.
        for _ in 0..6 {
            let prev = h.clone();
            for j in 0..n[1] {
                for i in 0..n[0] {
                    if prev[j * n[0] + i].is_nan() {
                        let mut acc = (0.0, 0);
                        for (di, dj) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                            let (x, y) = (i as i64 + di, j as i64 + dj);
                            if x >= 0 && y >= 0 && (x as usize) < n[0] && (y as usize) < n[1] {
                                let v = prev[y as usize * n[0] + x as usize];
                                if !v.is_nan() {
                                    acc = (acc.0 + v, acc.1 + 1);
                                }
                            }
                        }
                        if acc.1 > 0 {
                            h[j * n[0] + i] = acc.0 / acc.1 as f64;
                        }
                    }
                }
            }
        }
        for v in h.iter_mut() {
            *v = if v.is_nan() { floor } else { v.max(floor) };
        }
        for _ in 0..2 {
            let prev = h.clone();
            for j in 1..n[1] - 1 {
                for i in 1..n[0] - 1 {
                    let k = j * n[0] + i;
                    h[k] = 0.5 * prev[k] + 0.125 * (prev[k - 1] + prev[k + 1] + prev[k - n[0]] + prev[k + n[0]]);
                }
            }
        }
        Ok(Self { top, dome_mm: top - low, lo, n, h })
    }
    /// The pillowed table's height at a world (x, z), read bilinear off the map.
    fn surface(&self, x: f64, z: f64) -> f64 {
        let fx = ((x - self.lo[0]) / TABLE_CELL_MM).clamp(0.0, (self.n[0] - 2) as f64);
        let fz = ((z - self.lo[1]) / TABLE_CELL_MM).clamp(0.0, (self.n[1] - 2) as f64);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let g = |a: usize, b: usize| self.h[(j + b) * self.n[0] + i + a];
        let a = g(0, 0) + (g(1, 0) - g(0, 0)) * tx;
        let b = g(0, 1) + (g(1, 1) - g(0, 1)) * tx;
        a + (b - a) * tz
    }
    fn local(&self, p: P3) -> P3 {
        [p[0] - HEAD_U0, p[1] - self.surface(p[0], p[2]), p[2]]
    }
}

// --- The head --------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
enum Kind {
    Skull,
    Eye,
    Occipital,
    Temporal,
    Granule,
    Tubercle,
    CrestBead,
    Fringe,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Skull => "skull plates",
            Kind::Eye => "eyes",
            Kind::Occipital => "occipital horns",
            Kind::Temporal => "temporal horns",
            Kind::Granule => "granules",
            Kind::Tubercle => "enlarged tubercles",
            Kind::CrestBead => "crest tubercles",
            Kind::Fringe => "lateral fringe",
        }
    }
    fn treatment(self) -> Option<&'static str> {
        match self {
            Kind::Occipital | Kind::Temporal => Some(
                "horn point: the cone tapers under the fill floor over its outer part to a 0.24 mm rounded point; fed through its root \
                 (over the floor) from the skull and invested point up; a short-filled point is built back with a laser tack and filed to shape",
            ),
            Kind::Eye => Some("the eye: a 1 mm dome of relief on the skull's edge, judged at the 0.15 mm detail floor and left as cast"),
            Kind::Granule | Kind::Tubercle | Kind::CrestBead => Some(
                "a domed bead of hide relief fused to the stock along its whole base: the part-alone reading crosses the bead's own flank \
                 and is not a section the metal fills; judged at the 0.15 mm detail floor and left as cast",
            ),
            Kind::Fringe => Some(
                "a fringe scale: a pointed blade of relief 0.25-0.32 mm thick lying on the shoulder's rim, fused to the shank over its root; \
                 its free point is judged at the 0.15 mm detail floor; a short-filled point is dressed with a file",
            ),
            Kind::Skull => None,
        }
    }
}

/// One horn of the comb: its root on the skull's back edge (u, w in the head frame), its bearing outward from straight
/// back (degrees, toward +w), its rise over level (degrees), its length and its root and tip radii, mm.
struct HornSpec {
    kind: Kind,
    root: [f64; 2],
    splay_deg: f64,
    rise_deg: f64,
    len: f64,
    ra: f64,
    rb: f64,
}

/// The crown, one side (w > 0), mirrored across the head's midline: six horns rooted along one straight back edge,
/// each clear of the next, all swept back and out. The two occipitals beside the midline are the longest, at 1.7
/// times the temporals, which step shorter outboard.
const CROWN: [HornSpec; 3] = [
    HornSpec { kind: Kind::Occipital, root: [-5.3, 0.95], splay_deg: 10.0, rise_deg: 14.0, len: 4.2, ra: 0.62, rb: 0.12 },
    HornSpec { kind: Kind::Temporal, root: [-5.3, 2.65], splay_deg: 27.0, rise_deg: 11.0, len: 2.5, ra: 0.52, rb: 0.12 },
    HornSpec { kind: Kind::Temporal, root: [-5.25, 4.25], splay_deg: 44.0, rise_deg: 9.0, len: 2.0, ra: 0.45, rb: 0.12 },
];
/// A horn's root stands this high over the table, and is blended into the skull over this radius, mm.
const HORN_ROOT_H_MM: f64 = 0.62;
const HORN_BLEND_MM: f64 = 0.25;

/// The skull's plan, one side (w >= 0), snout to back edge: a flat wedge as wide as it is long, its blunt rounded
/// snout 40% of the back's width, the back edge straight across under the comb.
const PLAN: [[f64; 2]; 10] = [[6.0, 0.0], [5.85, 0.9], [5.4, 1.65], [4.7, 2.2], [1.0, 4.35], [-3.0, 5.4], [-4.8, 5.55], [-5.4, 5.1], [-5.6, 3.0], [-5.6, 0.0]];
/// The skull is a low plaque on the table: its ground under the plates, the round on its edge, and its flank's lean
/// inward per mm of height, mm.
const PLAQUE_BASE_MM: f64 = 0.45;
const PLAQUE_ROUND_MM: f64 = 0.3;
const FLANK_LEAN: f64 = 0.6;
/// The cephalic plates: rows across the skull, (w, pitch along u, tier), one side and the midline; seeds staggered by
/// half a pitch row to row and jittered. Each tier stands this high over the plaque's ground: the midline row
/// highest, the flanking rows a step down, the outer rows lowest.
const PLATE_ROWS: [(f64, f64, usize); 4] = [(0.0, 2.0, 0), (1.85, 1.75, 1), (3.45, 1.55, 2), (4.75, 1.4, 2)];
const TIER_MM: [f64; 3] = [0.55, 0.35, 0.15];
/// Seed jitter as a share of the row's pitch, along and across.
const PLATE_JITTER: [f64; 2] = [0.3, 0.16];
/// The V-joints between plates: half their width at the top, mm; their floor is the plaque's ground.
const JOINT_HALF_MM: f64 = 0.175;
/// The eyes: centre (u, h, inset from the plan's edge) and semi-axes, mm.
const EYE_U: f64 = 1.5;
const EYE_H_MM: f64 = 0.55;
const EYE_R: P3 = [0.62, 0.5, 0.5];
/// The nostrils: two pits on the snout, (u, w) and radius, mm.
const NOSTRIL: [f64; 3] = [5.15, 0.85, 0.2];

/// Signed distance to the closed polygon `poly` in the plane.
fn polygon_sdf(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    let mut d = f64::MAX;
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (e, w) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-12)).clamp(0.0, 1.0);
        let q = [w[0] - e[0] * t, w[1] - e[1] * t];
        d = d.min(q[0] * q[0] + q[1] * q[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            inside = !inside;
        }
    }
    if inside { -d.sqrt() } else { d.sqrt() }
}

/// A repeatable number in 0..1 for a key and a salt.
fn hash(k: u64, salt: u64) -> f64 {
    let mut x = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 29;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn norm(a: P3) -> P3 {
    mul(a, 1.0 / dot(a, a).sqrt().max(1e-12))
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// A horn in world millimetres.
#[derive(Clone, Copy, Debug)]
struct Horn {
    kind: Kind,
    a: P3,
    b: P3,
    ra: f64,
    rb: f64,
}

struct Head {
    plan: Vec<[f64; 2]>,
    plates: Vec<([f64; 2], usize)>,
    horns: Vec<Horn>,
}

impl Head {
    fn new(table: &Table) -> Self {
        let mut plan: Vec<[f64; 2]> = PLAN.to_vec();
        plan.extend(PLAN.iter().rev().filter(|p| p[1] > 0.0).map(|p| [p[0], -p[1]]));
        let mut plates = Vec::new();
        for (r, &(w, pitch, tier)) in PLATE_ROWS.iter().enumerate() {
            let mut u = 6.2 - 0.5 * pitch * (r % 2) as f64;
            let mut k = 0;
            while u > -6.2 {
                for s in if w > 0.0 { vec![-1.0, 1.0] } else { vec![1.0] } {
                    let salt = (r * 97 + k * 13) as u64 + if s < 0.0 { 5000 } else { 0 };
                    let ju = (hash(salt, 1) - 0.5) * 2.0 * PLATE_JITTER[0] * pitch;
                    let jw = (hash(salt, 2) - 0.5) * 2.0 * PLATE_JITTER[1] * pitch;
                    plates.push(([u + ju, s * w + jw], tier));
                }
                u -= pitch;
                k += 1;
            }
        }
        // The horns are straight in the world: each root stands on the table where the back edge crosses it.
        let mut horns = Vec::new();
        for h in &CROWN {
            for s in [-1.0, 1.0] {
                let (x, z) = (h.root[0] + HEAD_U0, h.root[1] * s);
                let a = [x, table.surface(x, z) + HORN_ROOT_H_MM, z];
                let (sp, cp) = h.splay_deg.to_radians().sin_cos();
                let (sr, cr) = h.rise_deg.to_radians().sin_cos();
                let dir = [-cp * cr, sr, sp * cr * s];
                horns.push(Horn { kind: h.kind, a, b: add(a, mul(dir, h.len)), ra: h.ra, rb: h.rb });
            }
        }
        Self { plan, plates, horns }
    }
    /// The skull's flank: the plan's distance, leaning in with height.
    fn wall_at(&self, q: P3) -> f64 {
        polygon_sdf([q[0], q[2]], &self.plan) + FLANK_LEAN * q[1].max(0.0)
    }
    /// The plates' height over the plaque's ground at a point of the plan: the nearest plate's tier, falling to the
    /// ground in a V along its joint with the next. Two tiers meet at the joint's floor, so every step falls inside a
    /// joint.
    fn plates_at(&self, u: f64, w: f64) -> (f64, usize) {
        let (mut i1, mut i2, mut d1, mut d2) = (0, 0, f64::MAX, f64::MAX);
        for (i, (c, _)) in self.plates.iter().enumerate() {
            let d = (u - c[0]).powi(2) + (w - c[1]).powi(2);
            if d < d1 {
                (i2, d2) = (i1, d1);
                (i1, d1) = (i, d);
            } else if d < d2 {
                (i2, d2) = (i, d);
            }
        }
        let (a, b) = (self.plates[i1].0, self.plates[i2].0);
        let edge = (d2 - d1) / (2.0 * (a[0] - b[0]).hypot(a[1] - b[1]).max(1e-9));
        let t = TIER_MM[self.plates[i1].1];
        (t * (edge / JOINT_HALF_MM).min(1.0) + 0.04 * smoothstep(0.25, 0.9, edge), i1)
    }
    /// The skull in its frame over the table: the plaque's plan extruded under its plates' tops, its edge rounded,
    /// then the eyes on its edge and the nostrils pitted into the snout.
    fn skull_at(&self, q: P3) -> f64 {
        let r = PLAQUE_ROUND_MM;
        let wall = self.wall_at(q);
        // Toward the edge the plates sink to the outer tier, so the rounded edge stays whole.
        let (plates, _) = self.plates_at(q[0], q[2]);
        let top = PLAQUE_BASE_MM + plates.min(TIER_MM[2] + (TIER_MM[0] - TIER_MM[2]) * smoothstep(0.1, 0.7, -polygon_sdf([q[0], q[2]], &self.plan)));
        let e = [wall + r, q[1] - (top - r)];
        let mut f = e[0].max(e[1]).min(0.0) + e[0].max(0.0).hypot(e[1].max(0.0)) - r;
        for s in [-1.0, 1.0] {
            let half = -polygon_sdf([EYE_U, 0.0], &self.plan);
            let c = [EYE_U, EYE_H_MM, s * (half - 0.12)];
            f = smin(f, ellipsoid(sub(q, c), EYE_R), 0.15);
            let n = [NOSTRIL[0], 0.62, s * NOSTRIL[1]];
            f = smax(f, -(dot(sub(q, n), sub(q, n)).sqrt() - NOSTRIL[2]), 0.05);
        }
        f
    }
    /// The head's field at a world point: the skull over the table, the horns blended into it, cut at the floor.
    fn field(&self, table: &Table, p: P3) -> f64 {
        let q = table.local(p);
        let mut f = self.skull_at(q);
        for h in &self.horns {
            f = smin(f, round_cone(p, h.a, h.b, h.ra, h.rb), HORN_BLEND_MM);
        }
        f.max(FLOOR_MM - q[1])
    }
    fn kind_at(&self, table: &Table, p: P3) -> Kind {
        let q = table.local(p);
        let horn = self.horns.iter().map(|h| (round_cone(p, h.a, h.b, h.ra, h.rb), h.kind)).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((d, k)) = horn {
            if d < 0.05 && d < self.skull_at(q) {
                return k;
            }
        }
        let half = -polygon_sdf([EYE_U, 0.0], &self.plan);
        let c = [EYE_U, EYE_H_MM, q[2].signum() * (half - 0.12)];
        if ellipsoid(sub(q, c), EYE_R) < 0.05 {
            return Kind::Eye;
        }
        Kind::Skull
    }
    fn world_box(&self, table: &Table) -> (P3, P3) {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in &self.plan {
            lo[0] = lo[0].min(p[0] + HEAD_U0);
            hi[0] = hi[0].max(p[0] + HEAD_U0);
            lo[2] = lo[2].min(p[1]);
            hi[2] = hi[2].max(p[1]);
        }
        for h in &self.horns {
            for (c, r) in [(h.a, h.ra), (h.b, h.rb)] {
                for k in [0, 2] {
                    lo[k] = lo[k].min(c[k] - r);
                    hi[k] = hi[k].max(c[k] + r);
                }
                hi[1] = hi[1].max(c[1] + r);
            }
        }
        lo[1] = table.top - TABLE_FLOOR_MM + FLOOR_MM;
        hi[1] = hi[1].max(table.top + PLAQUE_BASE_MM + TIER_MM[0] + 0.3);
        (sub(lo, [0.3, 0.3, 0.3]), add(hi, [0.3, 0.3, 0.3]))
    }
}

// --- The hide: granules, tubercles, the crest and the fringe ---------------------------------------------------------

/// The hide's beads over the stock, sized by their pitch: a granule's radius and dome, and an enlarged tubercle's.
const GRANULE_PITCH_MM: [f64; 2] = [0.72, 0.6];
const GRANULE_R: f64 = 0.4;
const GRANULE_DOME: f64 = 0.3;
const TUBERCLE_PITCH_MM: f64 = 2.1;
const TUBERCLE_R_MM: f64 = 0.55;
const TUBERCLE_DOME_MM: f64 = 0.4;
/// A bead's base is sunk this far under the stock's surface, mm.
const BEAD_SINK_MM: f64 = 0.2;
/// The head's own region of the stock, degrees off the head's centre either way; the shoulders run from there to the
/// palm.
const HEAD_REGION_DEG: f64 = 48.0;
/// The crest tubercles: how many a shoulder, the span of ring angle they run over (degrees off the head's centre),
/// their radius and dome at the head's end and at the palm's, mm.
const CREST_BEADS: usize = 12;
const CREST_SPAN_DEG: [f64; 2] = [56.0, 168.0];
const CREST_R_MM: [f64; 2] = [0.6, 0.35];
const CREST_DOME_MM: [f64; 2] = [0.45, 0.28];
/// The fringe: pitch along the rim, scale length, half-width and thickness, at the head's end and at the palm's, mm.
const FRINGE_FROM_DEG: f64 = 64.0;
const FRINGE_PITCH_MM: [f64; 2] = [1.25, 0.8];
const FRINGE_LEN_MM: [f64; 2] = [1.35, 0.7];
const FRINGE_HALF_MM: [f64; 2] = [0.45, 0.26];
const FRINGE_THICK_MM: [f64; 2] = [0.32, 0.25];
/// A fringe scale's point leans off the rim, out past the wall, by this angle from the ring's direction, degrees.
const FRINGE_LEAN_DEG: f64 = 58.0;
/// The fringe's rim: where the shank's outward normal leans this far from radial toward the finger axis.
const RIM_RADIAL: f64 = 0.62;

/// One scale of the hide in its own surface frame: `t` along its length, `n` out of the surface, `b` across.
#[derive(Clone, Copy, Debug)]
struct Stud {
    kind: Kind,
    c: P3,
    t: P3,
    n: P3,
    b: P3,
    /// A bead: radius across, the dome's semi-axis along `n`. Fringe: length, half-width, thickness.
    size: P3,
}

impl Stud {
    fn eval(&self, p: P3) -> f64 {
        let d = sub(p, self.c);
        let l = [dot(d, self.t), dot(d, self.n), dot(d, self.b)];
        match self.kind {
            Kind::Fringe => {
                // A flattened cone from a broad round root to a fine point.
                let (len, half, thick) = (self.size[0], self.size[1], self.size[2]);
                let k = half / thick;
                round_cone([l[0], l[1] * k, l[2]], [-0.25 * len, 0.0, 0.0], [0.75 * len, 0.0, 0.0], half, 0.05) / k
            }
            _ => ellipsoid(l, [self.size[0], self.size[1], self.size[0]]),
        }
    }
    fn reach(&self) -> f64 {
        match self.kind {
            Kind::Fringe => 0.75 * self.size[0] + self.size[1],
            _ => self.size[0].max(self.size[1]),
        }
    }
    /// A bead of radius `r` domed `dome` over the surface point `p` facing `n`.
    fn bead(kind: Kind, p: P3, n: P3, r: f64, dome: f64) -> Self {
        let t = norm(cross(n, if n[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }));
        let b = cross(n, t);
        Stud { kind, c: sub(p, mul(n, BEAD_SINK_MM)), t, n, b, size: [r, dome + BEAD_SINK_MM, 0.0] }
    }
}

/// A set of scales found through a grid of world cells.
struct Studs {
    name: &'static str,
    studs: Vec<Stud>,
    cell: f64,
    grid: std::collections::HashMap<[i32; 3], Vec<u32>>,
}

impl Studs {
    fn new(name: &'static str, studs: Vec<Stud>) -> Self {
        let cell = 1.0;
        let mut grid: std::collections::HashMap<[i32; 3], Vec<u32>> = Default::default();
        for (i, s) in studs.iter().enumerate() {
            let r = s.reach() + 0.3;
            let lo: [i32; 3] = std::array::from_fn(|k| ((s.c[k] - r) / cell).floor() as i32);
            let hi: [i32; 3] = std::array::from_fn(|k| ((s.c[k] + r) / cell).floor() as i32);
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        grid.entry([x, y, z]).or_default().push(i as u32);
                    }
                }
            }
        }
        Self { name, studs, cell, grid }
    }
    fn field(&self, p: P3) -> f64 {
        let key: [i32; 3] = std::array::from_fn(|k| (p[k] / self.cell).floor() as i32);
        self.grid.get(&key).map_or(self.cell, |list| list.iter().map(|&i| self.studs[i as usize].eval(p)).fold(self.cell, f64::min))
    }
    fn nearest(&self, p: P3) -> Option<(f64, Kind)> {
        let key: [i32; 3] = std::array::from_fn(|k| (p[k] / self.cell).floor() as i32);
        self.grid.get(&key).and_then(|list| list.iter().map(|&i| (self.studs[i as usize].eval(p), self.studs[i as usize].kind)).min_by(|a, b| a.0.total_cmp(&b.0)))
    }
    fn world_box(&self) -> (P3, P3) {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for s in &self.studs {
            let r = s.reach() + 0.3;
            for k in 0..3 {
                lo[k] = lo[k].min(s.c[k] - r);
                hi[k] = hi[k].max(s.c[k] + r);
            }
        }
        (lo, hi)
    }
    fn count(&self, k: Kind) -> usize {
        self.studs.iter().filter(|s| s.kind == k).count()
    }
}

/// Discs already laid on the stock, so the next bead keeps clear of them.
struct Taken {
    cell: f64,
    grid: std::collections::HashMap<[i32; 3], Vec<(P3, f64)>>,
}

impl Taken {
    fn new() -> Self {
        Self { cell: 1.0, grid: Default::default() }
    }
    fn key(&self, p: P3) -> [i32; 3] {
        std::array::from_fn(|k| (p[k] / self.cell).floor() as i32)
    }
    fn clear(&self, p: P3, r: f64) -> bool {
        let k = self.key(p);
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    if let Some(list) = self.grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        if list.iter().any(|(c, rc)| dot(sub(p, *c), sub(p, *c)) < (r + rc).powi(2)) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }
    fn put(&mut self, p: P3, r: f64) {
        let k = self.key(p);
        self.grid.entry(k).or_default().push((p, r));
    }
}

/// The bare stock's surface for placing the hide: the atlas with its normals turned outward, and where each sample
/// sits: degrees off the head's centre, which side of the head, and whether it faces out (not the bore).
struct Surface {
    a: Atlas,
    flip: f64,
    bore: f64,
}

impl Surface {
    fn of(d: &RingDesign) -> Result<Self> {
        let a = Atlas::of(d, 2048, 640)?;
        let flip = {
            let s = a.samples.iter().filter(|s| s.p[2].abs() < 0.5 && s.p[1] < -5.0).max_by(|x, y| x.p[0].hypot(x.p[1]).total_cmp(&y.p[0].hypot(y.p[1]))).copied().unwrap_or_default();
            if dot(s.n, norm([s.p[0], s.p[1], 0.0])) < 0.0 { -1.0 } else { 1.0 }
        };
        Ok(Self { bore: d.inner_radius_mm(), a, flip })
    }
    fn n(&self, s: &Sample) -> P3 {
        mul(s.n, self.flip)
    }
    fn radial(&self, s: &Sample) -> f64 {
        dot(self.n(s), norm([s.p[0], s.p[1], 0.0]))
    }
    /// Degrees off the head's centre round the ring, 0 to 180.
    fn off(s: &Sample) -> f64 {
        let t = s.p[1].atan2(s.p[0]).to_degrees();
        (t - 90.0 + 180.0).rem_euclid(360.0) - 180.0
    }
    /// A sample the hide may cover: on the outside of the ring, clear of the bore's comfort roll.
    fn outer(&self, s: &Sample) -> bool {
        dot(s.n, s.n) > 0.5 && self.radial(s) > -0.25 && s.p[0].hypot(s.p[1]) > self.bore + 0.55
    }
    /// The samples in a repeatable shuffled order.
    fn shuffled(&self, salt: u64) -> Vec<usize> {
        let mut order: Vec<(f64, usize)> = (0..self.a.samples.len()).map(|i| (hash(i as u64, salt), i)).collect();
        order.sort_by(|a, b| a.0.total_cmp(&b.0));
        order.into_iter().map(|x| x.1).collect()
    }
}

/// Beads scattered over the samples `keep` passes, at the pitch it gives, each clear of every disc already taken.
fn scatter(surf: &Surface, taken: &mut Taken, salt: u64, keep: &dyn Fn(&Sample) -> Option<f64>, make: &dyn Fn(&Sample, P3, f64) -> Stud) -> Vec<Stud> {
    let mut out = Vec::new();
    for i in surf.shuffled(salt) {
        let s = &surf.a.samples[i];
        if !surf.outer(s) {
            continue;
        }
        let Some(pitch) = keep(s) else { continue };
        let r = 0.5 * pitch;
        if !taken.clear(s.p, r) {
            continue;
        }
        taken.put(s.p, r);
        out.push(make(s, norm(surf.n(s)), pitch));
    }
    out
}

/// The head's ground: enlarged tubercles among granules over the table and the cheeks round the skull, up to the
/// skull's foot.
fn head_hide(surf: &Surface, head: &Head, table: &Table) -> Studs {
    let mut taken = Taken::new();
    let free = |s: &Sample| {
        let q = table.local(s.p);
        // Clear of the skull's foot by a granule, and of the horns' roots.
        let skull = polygon_sdf([q[0], q[2]], &head.plan);
        let horn = head.horns.iter().map(|h| round_cone(s.p, h.a, h.b, h.ra, h.rb)).fold(f64::MAX, f64::min);
        Surface::off(s).abs() < HEAD_REGION_DEG && !(skull < 0.28 && q[1] > -0.9) && horn > 0.12
    };
    // The tubercles are spaced at their own pitch, then the granules keep a land clear of each.
    let studs = scatter(surf, &mut taken, 11, &|s| free(s).then_some(TUBERCLE_PITCH_MM), &|s, n, _| Stud::bead(Kind::Tubercle, s.p, n, TUBERCLE_R_MM, TUBERCLE_DOME_MM));
    let mut studs = studs;
    let mut taken = Taken::new();
    for t in &studs {
        taken.put(add(t.c, mul(t.n, BEAD_SINK_MM)), TUBERCLE_R_MM + 0.12 - 0.5 * GRANULE_PITCH_MM[0]);
    }
    let granules = scatter(surf, &mut taken, 12, &|s| free(s).then_some(GRANULE_PITCH_MM[0]), &|s, n, pitch| Stud::bead(Kind::Granule, s.p, n, GRANULE_R * pitch, GRANULE_DOME * pitch));
    studs.extend(granules);
    Studs::new("Head ground", studs)
}

/// Each shoulder's hide: a graded row of domed tubercles on the crest line, a fringe of pointed scales along each rim
/// tipped toward the palm and out past the wall, and granules over the rest down to the palm.
fn shoulder_hides(surf: &Surface) -> Vec<Studs> {
    let a = &surf.a;
    // Per column: the crest point (the outer surface at z = 0) and the two rims.
    struct Col {
        off: f64,
        crest: (P3, P3),
        rims: [(P3, P3); 2],
    }
    let mut cols = Vec::new();
    for x in 0..a.width {
        let col: Vec<&Sample> = (1..a.height - 1).map(|y| a.at(x, y)).filter(|s| surf.outer(s)).collect();
        let outer: Vec<&&Sample> = col.iter().filter(|s| surf.radial(s) > 0.3).collect();
        let Some(crest) = outer.iter().min_by(|p, q| p.p[2].abs().total_cmp(&q.p[2].abs())) else { continue };
        let rim = |sign: f64| outer.iter().filter(|s| s.p[2] * sign > 0.0).min_by(|p, q| (surf.radial(p) - RIM_RADIAL).abs().total_cmp(&(surf.radial(q) - RIM_RADIAL).abs())).map(|s| (s.p, norm(surf.n(s))));
        let (Some(r0), Some(r1)) = (rim(-1.0), rim(1.0)) else { continue };
        cols.push(Col { off: Surface::off(crest), crest: (crest.p, norm(surf.n(crest))), rims: [r0, r1] });
    }
    let mut out = Vec::new();
    for (side, name) in [(-1.0, "Shoulder hide, crown side"), (1.0, "Shoulder hide, snout side")] {
        // The columns on this shoulder, ordered from the head toward the palm; off is signed toward -x as negative.
        let on_side = |off: f64| if side < 0.0 { off > 0.0 } else { off < 0.0 };
        let mut line: Vec<(f64, &Col)> = cols.iter().filter(|c| on_side(c.off)).map(|c| (c.off.abs(), c)).filter(|(o, _)| *o >= HEAD_REGION_DEG - 4.0).collect();
        line.sort_by(|p, q| p.0.total_cmp(&q.0));
        let grade = |o: f64| ((o - HEAD_REGION_DEG) / (180.0 - HEAD_REGION_DEG)).clamp(0.0, 1.0);
        let lerp2 = |v: [f64; 2], g: f64| v[0] + (v[1] - v[0]) * g;
        let frame = |i: usize, pick: &dyn Fn(&Col) -> (P3, P3)| -> (P3, P3, P3, P3) {
            let (p, n) = pick(line[i].1);
            let (q, _) = pick(line[(i + 1).min(line.len() - 1)].1);
            let (o, _) = pick(line[i.saturating_sub(1)].1);
            let t = sub(q, o);
            let t = norm(sub(t, mul(n, dot(t, n))));
            let b = norm(cross(n, t));
            (p, t, n, b)
        };
        let mut studs = Vec::new();
        let mut taken = Taken::new();
        // The crest tubercles at even angles.
        for k in 0..CREST_BEADS {
            let o = CREST_SPAN_DEG[0] + (CREST_SPAN_DEG[1] - CREST_SPAN_DEG[0]) * k as f64 / (CREST_BEADS - 1) as f64;
            let i = line.iter().position(|(x, _)| *x >= o).unwrap_or(line.len() - 1);
            let (p, _, n, _) = frame(i, &|c: &Col| c.crest);
            let g = ((o - CREST_SPAN_DEG[0]) / (CREST_SPAN_DEG[1] - CREST_SPAN_DEG[0])).clamp(0.0, 1.0);
            let r = lerp2(CREST_R_MM, g);
            studs.push(Stud::bead(Kind::CrestBead, p, n, r, lerp2(CREST_DOME_MM, g)));
            taken.put(p, r + 0.1);
        }
        // The fringe along each rim, stepped by its graded pitch.
        for rim in 0..2 {
            let pick = move |c: &Col| c.rims[rim];
            let mut next = 0.0;
            let mut walked = 0.0;
            for i in 0..line.len() {
                if i > 0 {
                    let (p, q) = (pick(line[i].1).0, pick(line[i - 1].1).0);
                    walked += dot(sub(p, q), sub(p, q)).sqrt();
                }
                let o = line[i].0;
                if o < FRINGE_FROM_DEG || o > 176.0 || walked < next {
                    continue;
                }
                let g = grade(o);
                next = walked + lerp2(FRINGE_PITCH_MM, g);
                let (p, t, n, mut b) = frame(i, &pick);
                // `b` points off the crest, toward the wall the rim falls to.
                if dot(b, [0.0, 0.0, p[2].signum()]) < 0.0 {
                    b = mul(b, -1.0);
                }
                // The fringe grows in over its first scales where the shoulder leaves the star.
                let grow = smoothstep(FRINGE_FROM_DEG - 1.0, FRINGE_FROM_DEG + 12.0, o).max(0.5);
                let (len, half, thick) = (lerp2(FRINGE_LEN_MM, g) * grow, lerp2(FRINGE_HALF_MM, g) * grow.sqrt(), lerp2(FRINGE_THICK_MM, g));
                let c = add(sub(p, mul(n, 0.4 * thick)), mul(b, 0.1));
                // The point runs palmward and out past the wall, so the rim's silhouette is serrated.
                let (sl, cl) = FRINGE_LEAN_DEG.to_radians().sin_cos();
                let tip = norm(add(mul(t, cl), mul(b, sl)));
                let across = norm(cross(n, tip));
                studs.push(Stud { kind: Kind::Fringe, c, t: tip, n, b: across, size: [len, half, thick] });
                taken.put(p, 0.55 * len);
                taken.put(add(p, mul(tip, 0.45 * len)), 0.35 * len);
            }
        }
        // Granules over the rest of the shoulder, finer toward the palm.
        let granules = scatter(
            surf,
            &mut taken,
            if side < 0.0 { 21 } else { 22 },
            &|s| {
                let o = Surface::off(s);
                (on_side(o) && o.abs() >= HEAD_REGION_DEG).then(|| lerp2(GRANULE_PITCH_MM, grade(o.abs())))
            },
            &|s, n, pitch| Stud::bead(Kind::Granule, s.p, n, GRANULE_R * pitch, GRANULE_DOME * pitch),
        );
        studs.extend(granules);
        out.push(Studs::new(name, studs));
    }
    out
}

#[derive(Default, serde::Serialize)]
struct Composition {
    table_top_mm: f64,
    table_dome_mm: f64,
    horns: usize,
    plates: usize,
    hide: Vec<serde_json::Value>,
    sculpts: Vec<serde_json::Value>,
}

/// A sculpt: the field meshed over `lo..hi`, relaxed, decimated to `faces` and settled.
fn sculpt_solid(name: &str, lo: P3, hi: P3, faces: usize, field: &(dyn Fn(P3) -> f64 + Sync)) -> (csg::Solid, serde_json::Value) {
    let t = std::time::Instant::now();
    let mut notes = Vec::new();
    let mut raw = sculpt::tetra_mesh(lo, hi, STEP_MM, field);
    let raw_faces = raw.f.len();
    let unrelaxed = raw.clone();
    sculpt::relax(&mut raw, field, 3);
    for reach in [0.3, 0.6, 1.2, 1e9] {
        let sites = sculpt::crossing_sites(&raw);
        if sites.is_empty() {
            break;
        }
        notes.push(format!("relax undone within {reach} mm of {} crossings", sites.len()));
        for (v, orig) in raw.v.iter_mut().zip(&unrelaxed.v) {
            if sites.iter().any(|s| dot(sub(*v, *s), sub(*v, *s)) < reach * reach) {
                *v = *orig;
            }
        }
    }
    let mut nets = None;
    for (k, cap) in [2e-3, 1e-3, 5e-4].into_iter().enumerate() {
        let d = sculpt::decimate(&raw, faces, cap, 2.0 + k as f64, 18.0, 35.0);
        if sculpt::crossing_sites(&d).is_empty() {
            nets = Some(d);
            break;
        }
    }
    let nets = nets.unwrap_or_else(|| {
        notes.push("decimated by the clean fallback".into());
        sculpt::clean_decimate(&raw, faces)
    });
    let s = sculpt::settle(nets, field, &|_| false);
    let info = json!({"part": name, "box_mm": [lo, hi], "raw_faces": raw_faces, "faces": s.f.len(), "volume_mm3": sculpt::closure(&s).1, "seconds": t.elapsed().as_secs_f64(), "notes": notes});
    (s, info)
}

fn joined() -> Component {
    Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, ..Component::default() }
}

/// A made part, as meshed.
struct Made {
    name: String,
    solid: csg::Solid,
}

struct Authored {
    d: RingDesign,
    lib: AlphaLibrary,
    comp: Composition,
    made: Vec<Made>,
    head: Head,
    table: Table,
    ground: Studs,
    shoulders: Vec<Studs>,
}

fn author() -> Result<Authored> {
    let mut d = base()?;
    let lib = AlphaLibrary::builtin();
    let table = Table::of(&d)?;
    let head = Head::new(&table);
    let surf = Surface::of(&d)?;
    let ground = head_hide(&surf, &head, &table);
    let shoulders = shoulder_hides(&surf);
    let mut comp = Composition { table_top_mm: table.top, table_dome_mm: table.dome_mm, horns: head.horns.len(), plates: head.plates.len(), ..Composition::default() };
    for s in std::iter::once(&ground).chain(shoulders.iter()) {
        comp.hide.push(json!({"part": s.name, "granules": s.count(Kind::Granule), "enlarged_tubercles": s.count(Kind::Tubercle), "crest_tubercles": s.count(Kind::CrestBead), "fringe_scales": s.count(Kind::Fringe)}));
    }
    let head_field = |p: P3| head.field(&table, p).min(ground.field(p));
    let (mut hlo, mut hhi) = head.world_box(&table);
    let (glo, ghi) = ground.world_box();
    for k in 0..3 {
        hlo[k] = hlo[k].min(glo[k]);
        hhi[k] = hhi[k].max(ghi[k]);
    }
    // The three sculpts mesh side by side.
    let (made_head, made_hide) = std::thread::scope(|scope| {
        let h = scope.spawn(|| sculpt_solid("Horned head", hlo, hhi, HEAD_FACES, &head_field));
        let parts: Vec<_> = shoulders
            .iter()
            .map(|s| {
                scope.spawn(move || {
                    let (lo, hi) = s.world_box();
                    let f = |p: P3| s.field(p);
                    sculpt_solid(s.name, lo, hi, HIDE_FACES, &f)
                })
            })
            .collect();
        (h.join().unwrap(), parts.into_iter().map(|p| p.join().unwrap()).collect::<Vec<_>>())
    });
    let mut made = vec![Made { name: "Horned head".into(), solid: made_head.0 }];
    comp.sculpts.push(made_head.1);
    for (s, (solid, info)) in shoulders.iter().zip(made_hide) {
        made.push(Made { name: s.name.into(), solid });
        comp.sculpts.push(info);
    }
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Factory stock".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    for m in &made {
        let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
        let recipe = stored::Recipe {
            kernel: "sculpt".into(),
            op: if m.name == "Horned head" { "horned lizard head and its ground".into() } else { "shoulder hide".into() },
            params: json!({"head_u0_mm": HEAD_U0, "step_mm": STEP_MM, "horns": head.horns.len()}),
            digest: String::new(),
        };
        doc.append(Feature { id: next, name: m.name.clone(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: sculpt::packed(&m.solid)? }, component: joined() })?;
    }
    Ok(Authored { d, lib, comp, made, head, table, ground, shoulders })
}

// --- Gates -----------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

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

/// A sculpt's sections face by face, gathered by the shape nearest each face: the thinnest, and the area under the
/// fill floor. Faces `skip` holds (buried in the stock) are left out.
fn land_census(solid: &csg::Solid, kind: &dyn Fn(P3) -> Kind, skip: &dyn Fn(P3) -> bool) -> Vec<(Kind, f64, f64, P3)> {
    use ringdesign_core::interaction::bvh::Bvh;
    let m = mesh::Mesh {
        vertices: solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: solid.f.clone(),
        ..Default::default()
    };
    let bvh = Bvh::build(&m);
    const IN: f64 = 1e-4;
    let mut out: Vec<(Kind, f64, f64, P3)> = Vec::new();
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let (e1, e2) = (sub(b, a), sub(c, a));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = dot(n, n).sqrt();
        if !(twice > 1e-14) {
            continue;
        }
        let centre = mul(add(add(a, b), c), 1.0 / 3.0);
        if skip(centre) {
            continue;
        }
        let inward = mul(n, -1.0 / twice);
        let o = add(centre, mul(inward, IN));
        let Some((_, t)) = bvh.ray(&m, o, inward) else { continue };
        let section = t + IN;
        let kind = kind(centre);
        let i = match out.iter().position(|e| e.0 == kind) {
            Some(i) => i,
            None => {
                out.push((kind, f64::MAX, 0.0, [0.0; 3]));
                out.len() - 1
            }
        };
        if section < out[i].1 {
            out[i].1 = section;
            out[i].3 = centre;
        }
        if section < MIN_SECTION_MM {
            out[i].2 += 0.5 * twice;
        }
    }
    out
}

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.15, 0.85),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.62),
    ("reverse", PI - 0.5, 0.35),
];

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

fn renders(out: &Path, lib: &AlphaLibrary, built: &mesh::BuildResult, table: &Table, edge: usize, params: BuildParams) -> Result<()> {
    let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The stones view on a ring without stones: the head close from its snout's three-quarter, and from above.
    let centre = [HEAD_U0 - 1.0, table.top, 0.0];
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0) + 0.45, 0.8, render::Framing::new(centre, 9.5), edge)?;
    render::write_png_framed(out.join("head.png"), &parts, 0.0, PI * 0.5, render::Framing::new(centre, 9.0), edge)?;
    let bare = mesh::try_build(&base()?, lib, params)?;
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

struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    notes: Vec<String>,
    parts: Vec<(String, usize)>,
    joined: usize,
}

impl Pass {
    fn of(built: &mesh::BuildResult) -> Self {
        let (watertight, degenerate, crossings) = geometry(&built.mesh);
        let mut notes = built.solids.notes.clone();
        notes.extend(built.parts.notes.iter().cloned());
        Self { triangles: built.mesh.faces.len(), watertight, degenerate, crossings, notes, parts: part_crossings(built), joined: built.parts.joined }
    }
    fn ok(&self, parts: usize) -> bool {
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.notes.is_empty() && self.parts.iter().all(|p| p.1 == 0) && self.joined == parts
    }
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "notes": self.notes, "made_part_crossings": self.parts, "parts_joined": self.joined})
    }
    fn line(&self) -> String {
        format!("{} tris, watertight {}, degenerate {}, crossings {}, notes {:?}, parts {:?}, joined {}", self.triangles, self.watertight, self.degenerate, self.crossings, self.notes, self.parts, self.joined)
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
    if std::env::var("PHRYNO_TABLE").is_ok() {
        let d = base()?;
        let a = Atlas::of(&d, 1440, 320)?;
        for z in [0.0, 3.0, 5.0, 7.0] {
            let mut row = String::new();
            for x in (-9..=9).map(|i| i as f64) {
                let y = a.samples.iter().filter(|s| (s.p[0] - x).abs() < 0.3 && (s.p[2] - z).abs() < 0.3 && s.n[1] > 0.2).map(|s| s.p[1]).fold(f64::NAN, f64::max);
                row += &format!(" {y:6.2}");
            }
            println!("z {z}: {row}");
        }
        return Ok(());
    }
    let Authored { d, lib, comp, made, head, table, ground, shoulders } = author()?;
    let author_s = started.elapsed().as_secs_f64();
    println!("  table top {:.2} (dome {:.2}); {} horns, {} plates", comp.table_top_mm, comp.table_dome_mm, comp.horns, comp.plates);
    for h in &comp.hide {
        println!("  hide {h}");
    }
    for s in &comp.sculpts {
        println!("  sculpt {s}");
    }
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let main_pass = Pass::of(&built);
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, main_pass.line());
    let skip_gates = std::env::var("PHRYNO_RENDER_ONLY").is_ok();
    if skip_gates {
        renders(&out, &lib, &built, &table, if draft { 1000 } else { 1600 }, params)?;
        return Ok(());
    }
    let coarse_pass = Pass::of(&mesh::try_build(&d, &lib, coarse_params())?);
    println!("  384x192: {}", coarse_pass.line());
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let sculpt_crossings: usize = made.iter().map(|m| csg::self_crossings(&m.solid)).sum();
    let open_edges: usize = made.iter().map(|m| sculpt::closure(&m.solid).0).sum();
    let band_field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let sections: Vec<(String, f64, f64)> = made.iter().map(|m| {
        let (a, b) = dfm::part_sections(&m.solid, None, MIN_SECTION_MM);
        (m.name.clone(), a, b)
    }).collect();
    let (part_min, part_under) = (sections.iter().map(|s| s.1).fold(f64::MAX, f64::min), sections.iter().map(|s| s.2).sum::<f64>());
    // On the head's part, a face nearer a bead of the ground than the skull is the ground's; the skull's faces buried
    // under the table are left out.
    let head_kind = |c: P3| match ground.nearest(c) {
        Some((d, k)) if d < head.field(&table, c) => k,
        _ => head.kind_at(&table, c),
    };
    let mut census = land_census(&made[0].solid, &head_kind, &|c| table.local(c)[1] < 0.05 && ground.nearest(c).is_none_or(|(d, _)| d > 0.05));
    for (m, s) in made[1..].iter().zip(&shoulders) {
        census.extend(land_census(&m.solid, &|c| s.nearest(c).map_or(Kind::Granule, |x| x.1), &|_| false));
    }
    let unnamed: Vec<String> = census.iter().filter(|c| c.2 > 0.0 && c.0.treatment().is_none()).map(|c| format!("{}: {:.3} mm2 under, thinnest {:.2}", c.0.label(), c.2, c.1)).collect();
    let lands = json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": MIN_DETAIL_MM,
        "method": "one ray per face of the sculpted part along its inward normal (the buried floor skipped), gathered by the shape nearest each face; dfm::part_sections on the whole part for comparison",
        "part_sections": sections.iter().map(|s| json!({"part": s.0, "thinnest_mm": s.1, "under_floor_mm2": s.2})).collect::<Vec<_>>(),
        "by_kind": census.iter().map(|c| json!({"kind": c.0.label(), "thinnest_mm": c.1, "thinnest_at_xyz": c.3, "under_floor_mm2": c.2, "treatment": if c.2 > 0.0 { c.0.treatment().map(|t| format!("{t}. Measured thinnest section: {:.3} mm.", c.1)) } else { None }})).collect::<Vec<_>>(),
        "unnamed_under_floor": unnamed,
        "band_thinnest_wall_mm": band_field.thinnest_wall_mm,
    });
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
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", main_pass.watertight && main_pass.degenerate == 0 && main_pass.crossings == 0),
        ("the sculpted parts closed and uncrossed, as made and as placed", open_edges == 0 && sculpt_crossings == 0 && main_pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, every part joined", main_pass.notes.is_empty() && main_pass.joined == made.len()),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax field verdict Castable with the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable),
        ("every section under 0.8 mm named with its bench treatment", unnamed.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("gates hold at 384 x 192", coarse_pass.ok(made.len())),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within 2 million triangles", main_pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "draft": {"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "base": {"stock": "016 Star", "face_mm": [FACE_MM.0, FACE_MM.1], "bore_mm": BORE_MM, "mirrored": false, "sand_envelope": false},
        "gates_that_apply": "lost wax (Logan, Fallback B): geometry, bore, field fill verdict at 0.8 mm, land widths, DFM, stones, 384 x 192, pattern, triangles, cold reload. The sand gates (ray release, draft-clamp bites, two-part Castable) do not apply; the two-part undercut is reported as a number.",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.json(),
        "sculpt": {"open_edges": open_edges, "self_crossings": sculpt_crossings, "parts": made.len()},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "band_verdict": band_field.verdict.label(), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": field.notes},
        "land_widths": lands,
        "two_part_undercut": {
            "band_percent": band_field.undercut_fraction() * 100.0,
            "with_parts_percent": field.undercut_fraction() * 100.0,
            "note": "reported only: lost wax judges fill and detail, never the pull",
        },
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "composition": comp,
        "design": {"bytes": text.len()},
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let name = if draft { "report-draft.json" } else { "report.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Phrynosoma / lost-wax pattern")?;
    }
    renders(&out, &lib, &built, &table, if draft { 1000 } else { 1600 }, params)?;
    println!(
        "  field {} (band {}, thinnest {:.2} mm); dfm {}; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}; part sections min {part_min:.3}, {part_under:.2} mm2 under",
        field.verdict.label(),
        band_field.verdict.label(),
        band_field.thinnest_wall_mm,
        findings.len(),
        d.inner_radius_mm()
    );
    for c in &census {
        println!("    lands {}: thinnest {:.3}, {:.3} mm2 under", c.0.label(), c.1, c.2);
    }
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
