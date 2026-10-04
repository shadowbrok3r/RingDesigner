//! Cataphracta — Phrynosoma, the horned crown: a horned lizard's skull as the table of the factory 004 Shield, poured in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_phrynosoma
//! target/release/examples/cataphracta_phrynosoma [OUT_DIR] [--draft] [--verify]
//!
//! The shield's table is the skull: a mirror-true mosaic of cephalic plates in three tiers, brows and eyes on its
//! widest part, nostrils at the point, and a jaw fringe along the tapering sides. Its flat back edge carries the comb:
//! two long occipital horns and two temporals a side, swept back over the back wall. The cheeks carry tubercles
//! among granules, and each shoulder a crest of tubercles, a palm-tipped fringe along its rims and granules to the
//! palm. Every part is a distance field meshed by `sculpt` and joined to the factory stock.
use anyhow::{Result, ensure};
use ringdesign_core::{
    Alpha, AlphaLibrary, BuildParams, RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render,
    sculpt::{self, ellipsoid, round_cone, smax, smin},
    field::{Layer, LayerEntry, Window},
    outline,
    setting::{Stamp, StampTop},
    skin::{Atlas, Sample},
    svg::SvgAlpha,
    tiling::TilingLayer,
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
const FACE_MM: (f64, f64) = (18.0, 18.0);
/// The factory stock: 004 Shield, whose flat back edge and tapering sides are the horned lizard's skull plan.
const STOCK: &str = "004";
/// The investment's fill floor and detail floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;

/// The floor the part is cut at under the table, mm.
const FLOOR_MM: f64 = -1.0;
/// The meshing step, and the face budgets of the head's sculpt and of each shoulder's hide.
const STEP_MM: f64 = 0.055;
const HEAD_FACES: usize = 215_000;
const SEAM_FACES: usize = 14_000;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The factory 004 Shield at its own 18 x 18 face on an 18.6 mm bore, poured in lost wax.
fn base() -> Result<RingDesign> {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    let source = PRESETS.iter().find(|p| p.id == STOCK).expect("the stock is bundled").load()?;
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
    setup.bench_notes = "Factory 004 Shield, poured in lost wax. The table is a horned lizard's skull: cephalic plates cut in three tiers, \
        the comb of six horns on the flat back edge, a jaw fringe down the tapering sides to the snout at the shield's point. Feed from the \
        palm; invest horn tips up. At the bench: tack and file any short-filled horn tip; polish the plates' tops and the horns, leave the \
        joints, granules and fringe satin."
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
    /// Signed distance in plan to the table's edge over the same grid, negative on the table, mm.
    e: Vec<f64>,
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
        // The table is where the stock stands within a hair of the crown; its edge's distance by brute force over
        // the edge cells.
        let on = |i: usize, j: usize| h[j * n[0] + i] > top - 0.08;
        let mut rim = Vec::new();
        for j in 1..n[1] - 1 {
            for i in 1..n[0] - 1 {
                if on(i, j) != on(i + 1, j) || on(i, j) != on(i, j + 1) {
                    rim.push([lo[0] + (i as f64 + 0.5) * TABLE_CELL_MM, lo[1] + (j as f64 + 0.5) * TABLE_CELL_MM]);
                }
            }
        }
        let e: Vec<f64> = (0..n[0] * n[1])
            .map(|k| {
                let (i, j) = (k % n[0], k / n[0]);
                let (x, z) = (lo[0] + i as f64 * TABLE_CELL_MM, lo[1] + j as f64 * TABLE_CELL_MM);
                let d = rim.iter().map(|r| (r[0] - x).hypot(r[1] - z)).fold(f64::MAX, f64::min);
                if on(i, j) { -d } else { d }
            })
            .collect();
        Ok(Self { top, dome_mm: top - low, lo, n, h, e })
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
    /// The table edge's signed distance at a world (x, z), bilinear.
    fn edge(&self, x: f64, z: f64) -> f64 {
        let fx = ((x - self.lo[0]) / TABLE_CELL_MM).clamp(0.0, (self.n[0] - 2) as f64);
        let fz = ((z - self.lo[1]) / TABLE_CELL_MM).clamp(0.0, (self.n[1] - 2) as f64);
        let (i, j) = (fx.floor() as usize, fz.floor() as usize);
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let g = |a: usize, b: usize| self.e[(j + b) * self.n[0] + i + a];
        let a = g(0, 0) + (g(1, 0) - g(0, 0)) * tx;
        let b = g(0, 1) + (g(1, 1) - g(0, 1)) * tx;
        a + (b - a) * tz
    }
}

// --- The head --------------------------------------------------------------------------------------------------------
//
// The shield's table is the skull. Its flat back edge (world -z) carries the comb of horns, its sides taper to the
// snout at the shield's point (+z), and the whole table is cut into a bilaterally symmetric mosaic of cephalic plates.
// The frame is the world's: x round the ring across the skull, z along the finger from the crown to the snout, and
// `h` over the table.

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
enum Kind {
    Occipital,
    Temporal,
    JawFringe,
    Granule,
}

/// One horn of the comb: its root on the table's back edge (x, z), its bearing outward from straight back (degrees),
/// its rise over level (degrees), its length and its root and tip radii, mm.
struct HornSpec {
    kind: Kind,
    root: [f64; 2],
    splay_deg: f64,
    rise_deg: f64,
    len: f64,
    ra: f64,
    rb: f64,
}

/// The crown, one side (x > 0), mirrored across the skull's midline: six horns rooted along the shield's straight back
/// edge, each clear of the next, all swept back and out over the back wall. The two occipitals beside the midline
/// are the longest, 1.6 times the temporals, which step shorter outboard.
const CROWN: [HornSpec; 3] = [
    HornSpec { kind: Kind::Occipital, root: [1.6, -6.25], splay_deg: 12.0, rise_deg: 34.0, len: 2.7, ra: 0.28 + THORN_TAPER * 2.7, rb: 0.28 },
    HornSpec { kind: Kind::Temporal, root: [4.2, -6.25], splay_deg: 19.0, rise_deg: 32.0, len: 1.7, ra: 0.28 + THORN_TAPER * 1.7, rb: 0.28 },
    HornSpec { kind: Kind::Temporal, root: [6.6, -6.15], splay_deg: 25.0, rise_deg: 30.0, len: 1.5, ra: 0.28 + THORN_TAPER * 1.5, rb: 0.28 },
];
/// Every horn and thorn ends in a 0.2 mm round and widens by this much radius per mm of length, so its point closes
/// to the 0.8 mm floor within one floor of its tip: the wall census reads it as an edge, not a wall.
const THORN_TAPER: f64 = 0.36;
/// The snout: the skull's outline leaves the shield's tapering sides at `SNOUT_FROM_Z` and closes in a half-ellipse,
/// so the nose is blunt and rounded, about 39% of the back's width, overhanging the factory point as the snout's lip.
const SNOUT_FROM_Z: f64 = 4.0;
const SNOUT_HALF_MM: f64 = 5.25;
const SNOUT_REACH_MM: f64 = 5.0;
/// The wedge: half-width at the back edge and where the snout begins, at those z, mm.
const WEDGE_HALF_MM: [f64; 2] = [8.9, 5.25];
/// The skull keeps this far inside the factory table's edge everywhere, mm.
const SNOUT_INSET_MM: f64 = 0.8;
/// The skull's foot is drafted: over its lowest `SKIRT_H_MM` its wall leans out by `SKIRT_LEAN` mm per mm.
const SKIRT_H_MM: f64 = 0.6;
const SKIRT_LEAN: f64 = 0.6;
const WEDGE_Z: [f64; 2] = [-6.2, 4.0];
/// A horn's root stands this high over the table, and is blended into the plates over this radius, mm.
const HORN_ROOT_H_MM: f64 = 0.45;
const HORN_BLEND_MM: f64 = 0.25;

/// The cephalic plates: rows along the skull, (x, pitch along z, tier), one side and the midline, staggered by half
/// a pitch row to row; seeds are jittered the same on both sides, so the mosaic is mirror-true. The midline row
/// carries the largest plates, and they shrink toward the edge. Each tier stands this high over the table; the
/// joints' floor stays a skin over it, so the plates never meet the table face to face.
const PLATE_ROWS: [(f64, f64, usize); 6] = [(0.0, 2.6, 0), (1.95, 1.7, 1), (3.55, 1.45, 1), (5.0, 1.25, 2), (6.35, 1.1, 2), (7.6, 1.0, 2)];
const TIER_MM: [f64; 3] = [0.6, 0.42, 0.24];
const JOINT_FLOOR_MM: f64 = 0.14;
/// Each plate's top is crowned this much toward its middle, mm.
const PLATE_CROWN_MM: f64 = 0.07;
const PLATE_JITTER: [f64; 2] = [0.16, 0.26];
/// The joints between plates: each plate rounds down to the joint's floor over this much of its edge, mm.
const PLATE_ROUND_MM: f64 = 0.42;
/// The plates stop this far inside the factory table's edge, so its hard wall-to-face angle stays.
const PLATE_INSET_MM: f64 = 0.32;
/// The eyes on the skull's widest part, each a dome sunk in a socket: centre (x, h, z) and semi-axes, mm.
const EYE_AT: P3 = [6.1, 0.72, -2.4];
const EYE_R: P3 = [0.78, 0.55, 0.92];
/// The socket the eye sits in: a recess cut into the plates round it, its radius past the eye's and its depth, mm.
const SOCKET_GROW_MM: f64 = 0.3;

/// The skull is domed: its plates stand on a crown rising this high from the table's edge to the midline, mm.
const SKULL_DOME_MM: f64 = 0.5;
const SKULL_DOME_RUN_MM: f64 = 4.0;
/// The nostrils: two pits near the snout, (x, z) and radius, mm.
const NOSTRIL: [f64; 3] = [0.95, 9.0, 0.22];
/// The jaw fringe along the tapering sides: how many a side, the span along z, and the scale's length, half-width
/// and thickness at the back and at the snout, mm.
const JAW_SCALES: usize = 9;
const JAW_SPAN_Z: [f64; 2] = [-4.9, 5.6];
const JAW_LEN_MM: [f64; 2] = [1.5, 0.85];
/// A jaw scale's point turns back from straight out by this angle, degrees.
const JAW_SWEEP_DEG: f64 = 28.0;
/// A jaw scale's point rises off the table by this angle, degrees.
const JAW_LIFT_DEG: f64 = 9.0;

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
    a: P3,
    b: P3,
    ra: f64,
    rb: f64,
}

struct Head {
    plates: Vec<([f64; 2], usize)>,
    horns: Vec<Horn>,
    jaw: Vec<Stud>,
    top: f64,
}

/// The skull's half-width along z: widest at the back edge, narrowing straight to the snout, so the plan is a flat
/// wedge 1.7 times as wide at the horn row as at the nose.
fn wedge_half(z: f64) -> f64 {
    WEDGE_HALF_MM[0] + (WEDGE_HALF_MM[1] - WEDGE_HALF_MM[0]) * ((z - WEDGE_Z[0]) / (WEDGE_Z[1] - WEDGE_Z[0])).clamp(0.0, 1.5)
}

/// The skull's outline in plan: the wedge within the table back of the snout, then the snout's half-ellipse.
fn skull_edge(table: &Table, x: f64, z: f64) -> f64 {
    // The wedge's flank leans in by its slope; the two parts overlap by 1.5 mm, so their union has no seam.
    let slope = (WEDGE_HALF_MM[0] - WEDGE_HALF_MM[1]) / (WEDGE_Z[1] - WEDGE_Z[0]);
    let wedge = (x.abs() - wedge_half(z)) / (1.0 + slope * slope).sqrt();
    let back = table.edge(x, z).max(wedge).max(z - SNOUT_FROM_Z - 1.5);
    let (a, b) = (SNOUT_HALF_MM, SNOUT_REACH_MM);
    let k = ((x / a).powi(2) + ((z - SNOUT_FROM_Z) / b).powi(2)).sqrt();
    let cap = ((k - 1.0) * a.min(b)).max(SNOUT_FROM_Z - 1.5 - z);
    // Never nearer the factory table's edge than SNOUT_INSET_MM, so the shield's point stays crisp past the nose.
    back.min(cap).max(table.edge(x, z) + SNOUT_INSET_MM)
}

impl Head {
    fn new(table: &Table) -> Self {
        let mut plates = Vec::new();
        for (r, &(x, pitch, tier)) in PLATE_ROWS.iter().enumerate() {
            let mut z = 9.4 - 0.5 * pitch * (r % 2) as f64;
            let mut k = 0;
            while z > -7.2 {
                let salt = (r * 97 + k * 13) as u64;
                let jx = (hash(salt, 1) - 0.5) * 2.0 * PLATE_JITTER[0] * pitch;
                let jz = (hash(salt, 2) - 0.5) * 2.0 * PLATE_JITTER[1] * pitch;
                for s in if x > 0.0 { vec![-1.0, 1.0] } else { vec![1.0] } {
                    let c = [s * (x + jx), z + jz];
                    if skull_edge(table, c[0], c[1]) < -0.45 {
                        plates.push((if x > 0.0 { c } else { [0.0, c[1]] }, tier));
                    }
                }
                z -= pitch;
                k += 1;
            }
        }
        let mut horns = Vec::new();
        for h in &CROWN {
            for s in [-1.0, 1.0] {
                let (x, z) = (h.root[0] * s, h.root[1]);
                let a = [x, table.surface(x, z) + HORN_ROOT_H_MM, z];
                let (sp, cp) = h.splay_deg.to_radians().sin_cos();
                let (sr, cr) = h.rise_deg.to_radians().sin_cos();
                let dir = [sp * cr * s, sr, -cp * cr];
                horns.push(Horn { a, b: add(a, mul(dir, h.len)), ra: h.ra, rb: h.rb });
            }
        }
        // The jaw fringe: on each tapering side, a row of blades lying on the table's edge, tipped out and back.
        let mut jaw = Vec::new();
        for s in [-1.0, 1.0] {
            for k in 0..JAW_SCALES {
                let g = k as f64 / (JAW_SCALES - 1) as f64;
                let z = JAW_SPAN_Z[0] + (JAW_SPAN_Z[1] - JAW_SPAN_Z[0]) * g;
                // The edge's x at this z, found by bisection inward from outside the table.
                let (mut lo, mut hi) = (0.0, 12.0);
                for _ in 0..40 {
                    let m = 0.5 * (lo + hi);
                    if skull_edge(table, s * m, z) < 0.0 { lo = m } else { hi = m }
                }
                let xe = s * lo;
                // The edge's outward normal in plan, turned back toward the crown.
                let e = 0.05;
                let grad = [skull_edge(table, xe + e, z) - skull_edge(table, xe - e, z), skull_edge(table, xe, z + e) - skull_edge(table, xe, z - e)];
                let out = norm([grad[0], 0.0, grad[1]]);
                let (sb, cb) = JAW_SWEEP_DEG.to_radians().sin_cos();
                // Each point lifts a little off the table, so its tip stands free instead of grazing it.
                let t = norm([out[0] * cb, JAW_LIFT_DEG.to_radians().tan(), out[2] * cb - sb]);
                let b = norm(cross([0.0, 1.0, 0.0], t));
                let n = norm(cross(t, b));
                let len = JAW_LEN_MM[0] + (JAW_LEN_MM[1] - JAW_LEN_MM[0]) * g;
                let half = 0.2 + THORN_TAPER * len;
                let root = [xe - out[0] * 0.12, table.top + 0.12, z - out[2] * 0.12];
                jaw.push(Stud { kind: Kind::JawFringe, c: root, t, n, b, size: [len, half, half] });
            }
        }
        Self { plates, horns, jaw, top: table.top }
    }
    /// The plates' height over the table at (x, z): the nearest plate's tier, falling in a V to the joint's floor along
    /// its edge with the next, with a faint crown.
    fn plates_at(&self, x: f64, z: f64) -> f64 {
        let (mut i1, mut i2, mut d1, mut d2) = (0, 0, f64::MAX, f64::MAX);
        for (i, (c, _)) in self.plates.iter().enumerate() {
            let d = (x - c[0]).powi(2) + (z - c[1]).powi(2);
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
        // Each plate is a pillow: it rises from the joint's floor and rounds over within its first half millimetre.
        let k = 1.0 - (1.0 - (edge / PLATE_ROUND_MM).min(1.0)).powi(2);
        // The crown is radial about the plate's seed, so the top is a smooth dome with no ridge along its medial axis.
        let pitch = PLATE_ROWS[self.plates[i1].1.min(PLATE_ROWS.len() - 1)].1;
        JOINT_FLOOR_MM + (t - JOINT_FLOOR_MM) * k + PLATE_CROWN_MM * (1.0 - d1 / (0.36 * pitch * pitch)).max(0.0)
    }
    /// The skull's crown under the plates at a point `edge` from the table's edge.
    fn dome(&self, edge: f64) -> f64 {
        SKULL_DOME_MM * smoothstep(0.0, SKULL_DOME_RUN_MM, -edge)
    }
    /// The skull's relief over the table at a world point: the plates inside the table's edge, the brows and eyes on
    /// its sides, the nostrils pitted into the snout.
    fn skull_at(&self, table: &Table, p: P3) -> f64 {
        // The table is flat (it falls 0.01 mm), so the skull stands on its plane; the snout's lip runs on over the point.
        let h = p[1] - table.top;
        let edge = skull_edge(table, p[0], p[2]);
        // Far off the table's relief the plates cannot be nearest: a bound is enough.
        let far = (h - 2.0).max(FLOOR_MM - 0.3 - h).max(edge - 1.4);
        if far > 0.0 {
            return far;
        }
        let top = self.plates_at(p[0], p[2]) + self.dome(edge);
        let mut f = (h - top).max(edge + PLATE_INSET_MM - SKIRT_LEAN * (SKIRT_H_MM - h).clamp(0.0, SKIRT_H_MM)).max(FLOOR_MM - h);
        for s in [-1.0, 1.0] {
            let q = [p[0], h, p[2]];
            let eye = [s * EYE_AT[0], EYE_AT[1], EYE_AT[2]];
            let socket = ellipsoid(sub(q, [eye[0], top, eye[2]]), [EYE_R[0] + SOCKET_GROW_MM, 0.45, EYE_R[2] + SOCKET_GROW_MM]);
            f = smax(f, -socket, 0.12);
            f = f.min(ellipsoid(sub(q, eye), EYE_R));
            let n = [s * NOSTRIL[0], top, NOSTRIL[1]];
            f = smax(f, -(dot(sub(q, n), sub(q, n)).sqrt() - NOSTRIL[2]), 0.05);
        }
        f
    }
    /// The head's field at a world point: the skull's relief, the horns blended into it, the jaw fringe.
    fn field(&self, table: &Table, p: P3) -> f64 {
        let mut f = self.skull_at(table, p);
        for h in &self.horns {
            f = smin(f, round_cone(p, h.a, h.b, h.ra, h.rb), HORN_BLEND_MM);
        }
        for j in &self.jaw {
            f = smin(f, j.eval(p), 0.35);
        }
        f
    }
    fn world_box(&self, _table: &Table) -> (P3, P3) {
        let (mut lo, mut hi) = ([-10.5, self.top + FLOOR_MM, -8.0], [10.5, self.top + TIER_MM[0] + SKULL_DOME_MM + 0.8, 10.5]);
        for h in &self.horns {
            for (c, r) in [(h.a, h.ra), (h.b, h.rb)] {
                for k in 0..3 {
                    lo[k] = lo[k].min(c[k] - r);
                    hi[k] = hi[k].max(c[k] + r);
                }
            }
        }
        (sub(lo, [0.3, 0.3, 0.3]), add(hi, [0.3, 0.3, 0.3]))
    }
}

// --- The hide: granules, tubercles, the crest and the fringe ---------------------------------------------------------

/// The head's ground beads (the table's margin round the skull), sized by their pitch: a granule's radius and dome.
const GROUND_PITCH_MM: f64 = 0.5;
const GRANULE_R: f64 = 0.4;
const GRANULE_DOME: f64 = 0.3;
/// A bead's base is sunk this far under the stock's surface, mm.
const BEAD_SINK_MM: f64 = 0.2;
/// The head's own region of the stock, degrees off the head's centre either way; the shoulders run from there to the
/// palm.
const HEAD_REGION_DEG: f64 = 40.0;
/// The granule field: pitch and height, mm.
const GRANULE_PITCH_MM: f64 = 0.8;
const GRANULE_MM: f64 = 0.28;
/// The granule field stays this far off the bore, mm, and this far either side of the factory mesh's seams at 0 and
/// 180 degrees, where relief laid across the source's long triangles folds; the seams' strips carry sculpted granules.
const GRANULE_BORE_CLEAR_MM: f64 = 1.2;
const SEAM_DEG: f64 = 6.0;
/// The granules keep off any edge where the stock turns more than this within two atlas rows or columns, degrees.
const EDGE_TURN_DEG: f64 = 22.0;
/// The crest: how many tubercles a shoulder, their diameter and dome at the head's end, and the scale the last loses.
const CREST_COUNT: u32 = 11;
const CREST_SPAN_DEG: [f64; 2] = [HEAD_REGION_DEG + 9.3, 170.0];
const CREST_DIA_MM: f64 = 1.6;
const CREST_DOME_MM: f64 = 0.65;
const CREST_TAPER: f64 = 0.44;
/// The fringe: how many scales a side face, their length and width at the head's end, the cone's rise, where on the
/// side face they sit (0 low, 1 high) and the scale the last loses.
const FRINGE_COUNT: u32 = 16;
const FRINGE_SPAN_DEG: [f64; 2] = [HEAD_REGION_DEG + 4.0, 172.0];
const FRINGE_L_MM: f64 = 1.5;
const FRINGE_W_MM: f64 = 1.05;
const FRINGE_APEX_MM: f64 = 0.55;
const FRINGE_TAPER: f64 = 0.5;
/// The fringe sits on the outer face just inside each rim: where the outward normal leans this far from radial.
const RIM_RADIAL: f64 = 0.88;

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
            Kind::JawFringe => {
                // A flattened cone from a broad round root to a fine point.
                let (len, half, thick) = (self.size[0], self.size[1], self.size[2]);
                let k = half / thick;
                round_cone([l[0], l[1] * k, l[2]], [-0.25 * len, 0.0, 0.0], [0.75 * len, 0.0, 0.0], half, 0.2) / k
            }
            _ => ellipsoid(l, [self.size[0], self.size[1], self.size[0]]),
        }
    }
    fn reach(&self) -> f64 {
        match self.kind {
            Kind::JawFringe => 0.75 * self.size[0] + self.size[1],
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
        // Nothing comes within the bore's reach: a scale whose reach would cross it is left out.
        let studs: Vec<Stud> = studs.into_iter().filter(|s| s.c[0].hypot(s.c[1]) - s.reach() > BORE_MM * 0.5 + 0.6).collect();
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
        // On the table, clear of its edge, the horns and the jaw fringe.
        let on_table = s.p[1] > table.top - 0.1 && table.edge(s.p[0], s.p[2]) < -0.42;
        // The cheeks carry the granule field; this ground is the table's margin round the skull.
        let below = on_table && skull_edge(table, s.p[0], s.p[2]) > 0.22;
        let horn = head.horns.iter().map(|h| round_cone(s.p, h.a, h.b, h.ra, h.rb)).fold(f64::MAX, f64::min);
        let jaw = head.jaw.iter().map(|j| j.eval(s.p)).fold(f64::MAX, f64::min);
        Surface::off(s).abs() < HEAD_REGION_DEG && below && horn > 0.35 && jaw > 0.6
    };
    let mut studs = Vec::new();
    let granules = scatter(surf, &mut taken, 12, &|s| free(s).then_some(GROUND_PITCH_MM), &|s, n, pitch| Stud::bead(Kind::Granule, s.p, n, GRANULE_R * pitch, GRANULE_DOME * pitch));
    studs.extend(granules);
    Studs::new("Head ground", studs)
}

#[derive(Default, serde::Serialize)]
struct Composition {
    table_top_mm: f64,
    table_dome_mm: f64,
    horns: usize,
    plates: usize,
    crown: Vec<serde_json::Value>,
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
        let sites = sculpt::crossing_sites(&d);
        if sites.is_empty() {
            nets = Some(d);
            break;
        }
        notes.push(format!("decimation at cap {cap} crossed at {:?}", sites.iter().take(4).map(|p| p.map(|x| (x * 100.0).round() / 100.0)).collect::<Vec<_>>()));
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
    table: Table,
    head: Head,
    ground: Studs,
}

/// A bead's height profile, (1 - r^2)^1.2: a round top meeting its ground at a low angle, as SVG gradient stops, black ink at the opacity of the height: one bead per tile.
fn bead_svg(w: f64, h: f64, r_frac: f64) -> String {
    let stops: String = [(0.0, 1.0), (0.15, 0.973), (0.3, 0.893), (0.45, 0.762), (0.55, 0.649), (0.65, 0.517), (0.75, 0.371), (0.85, 0.215), (0.92, 0.106), (1.0, 0.0)]
        .iter()
        .map(|(o, a)| format!(r##"<stop offset="{o}" stop-color="#000" stop-opacity="{a}"/>"##))
        .collect();
    // An ellipse filling the cell's share: the chart squeezes the cell, the surface stretches it back to a circle.
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs><radialGradient id="b" cx="0.5" cy="0.5" r="0.5">{stops}</radialGradient></defs><ellipse cx="{:.4}" cy="{:.4}" rx="{:.4}" ry="{:.4}" fill="url(#b)"/></svg>"##,
        w * 0.5,
        h * 0.5,
        r_frac * w * 0.5,
        r_frac * h * 0.5
    )
}

/// The hide off the head, all of it live layers and struck stamps: one granule field from the head's cheeks round
/// both shoulders to the palm, a graded crest of domed tubercles on the parting line, and a palm-tipped fringe of
/// pointed scales along the top of each side face.
fn hide_layers(d: &mut RingDesign, lib: &mut AlphaLibrary, comp: &mut Composition, surf: &Surface, table: &Table) -> Result<Vec<(P3, f64)>> {
    let ctx = d.field_context();
    // The granules: one bead a tile, staggered, over the whole band off the head's table: a painted mask holds them
    // under the table's edge by 0.9 mm, so none sits on the factory's hard edge or under the skull's plates.
    let envf = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
    let a = Atlas::of(d, 1024, 256)?;
    let bore = d.inner_radius_mm();
    let clear = envf("PHRYNO_BCLEAR", GRANULE_BORE_CLEAR_MM);
    let seam = envf("PHRYNO_SEAM", SEAM_DEG);
    // The crest and the fringe, struck at atlas samples: per column the crest point (the outer surface at z = 0) and
    // the two rims (where the outward normal leans RIM_RADIAL from radial).
    let stamp = |name: String, s: &Sample, rot: f64, outline: Vec<[f64; 2]>, top: StampTop| Stamp {
        name, theta_deg: s.theta, v_mm: s.v, rot_deg: rot, outline, height_mm: 0.08, sink_mm: 0.25, draft_deg: 4.0, cut: false, bench: false, along_pull: false, fine_cap: false, tier: 0, top,
    };
    let column = |theta: f64| -> Option<(Sample, Sample, Sample)> {
        let x = ((theta.rem_euclid(360.0)) / 360.0 * surf.a.width as f64).round() as usize % surf.a.width;
        let col: Vec<&Sample> = (1..surf.a.height - 1).map(|y| surf.a.at(x, y)).filter(|s| surf.outer(s) && surf.radial(s) > 0.3).collect();
        let crest = **col.iter().min_by(|p, q| p.p[2].abs().total_cmp(&q.p[2].abs()))?;
        let rim = |sign: f64| col.iter().filter(|s| s.p[2] * sign > 0.0).min_by(|p, q| (surf.radial(p) - RIM_RADIAL).abs().total_cmp(&(surf.radial(q) - RIM_RADIAL).abs())).map(|s| **s);
        Some((crest, rim(-1.0)?, rim(1.0)?))
    };
    let mut crest = 0;
    let mut fringe = 0;
    let mut sites = Vec::new();
    for side in [1.0, -1.0] {
        let rot = if side > 0.0 { 0.0 } else { 180.0 };
        let label = if side > 0.0 { "crown side" } else { "snout side" };
        for k in 0..CREST_COUNT {
            let g = k as f64 / (CREST_COUNT - 1) as f64;
            let off = CREST_SPAN_DEG[0] + (CREST_SPAN_DEG[1] - CREST_SPAN_DEG[0]) * g;
            let Some((c, _, _)) = column(90.0 + side * off) else { continue };
            let k_size = 1.0 - CREST_TAPER * g;
            d.stamps.push(stamp(format!("Crest tubercle, {label} {}", k + 1), &c, rot, outline::circle(CREST_DIA_MM * k_size), StampTop::Dome { crown_mm: CREST_DOME_MM * k_size }));
            sites.push((c.p, 0.5 * CREST_DIA_MM * k_size));
            crest += 1;
        }
        for k in 0..FRINGE_COUNT {
            let g = k as f64 / (FRINGE_COUNT - 1) as f64;
            let off = FRINGE_SPAN_DEG[0] + (FRINGE_SPAN_DEG[1] - FRINGE_SPAN_DEG[0]) * g;
            let Some((_, lo, hi)) = column(90.0 + side * off) else { continue };
            let k_size = 1.0 - FRINGE_TAPER * g;
            for (r, rim) in [("low", lo), ("high", hi)] {
                let (l, w) = (FRINGE_L_MM * k_size, FRINGE_W_MM * k_size);
                d.stamps.push(stamp(
                    format!("Fringe, {label}, {r} rim {}", k + 1),
                    &rim,
                    rot,
                    outline::rounded_triangle(w, l, 0.12),
                    StampTop::Cone { apex_mm: FRINGE_APEX_MM * k_size, at: [l * 0.2, 0.0], tip_mm: 0.05 },
                ));
                fringe += 1;
                sites.push((rim.p, 0.6 * l));
            }
        }
    }
    let off_table = a.paint("Off the table", |s| {
        // The factory mesh's seams at 0 and 180 degrees fold any relief laid across them: a bead column is left out.
        let t = s.p[1].atan2(s.p[0]).to_degrees();
        let from_seam = t.abs().min(180.0 - t.abs());
        // Off the stock's hard edges: where the surface turns more than EDGE_TURN_DEG within two rows or columns, a
        // bead would hang over the corner as a thin lip.
        let (x, y) = (s.i % a.width, s.i / a.width);
        let turn = [(2i64, 0i64), (-2, 0), (0, 2), (0, -2)]
            .iter()
            .filter_map(|(dx, dy)| {
                let (nx, ny) = ((x as i64 + dx).rem_euclid(a.width as i64) as usize, y as i64 + dy);
                (ny > 0 && ny < a.height as i64 - 1).then(|| a.at(nx, ny as usize).n)
            })
            .map(|n| dot(n, s.n).clamp(-1.0, 1.0).acos().to_degrees())
            .fold(0.0, f64::max);
        // Clear under every struck stamp, so a tubercle or a fringe scale stands on smooth ground, ringed by beads.
        let under = sites.iter().map(|(c, r)| dot(sub(s.p, *c), sub(s.p, *c)).sqrt() - r).fold(f64::MAX, f64::min);
        smoothstep(0.05, 0.25, under)
            * (1.0 - smoothstep(table.top - 1.15, table.top - 0.9, s.p[1]))
            * smoothstep(bore + clear - 0.3, bore + clear, s.p[0].hypot(s.p[1]))
            * smoothstep(seam - 1.0, seam, from_seam)
            * (1.0 - smoothstep(EDGE_TURN_DEG - 6.0, EDGE_TURN_DEG, turn))
    });
    lib.insert(Alpha::from_png16(off_table.name.clone(), &off_table.to_png16()?)?);
    let (pitch, height) = (envf("PHRYNO_GPITCH", GRANULE_PITCH_MM), envf("PHRYNO_GHEIGHT", GRANULE_MM));
    // The chart squeezes a section into the band's span, so each stretch of the ring has its own layer, its rows
    // counted for that stretch's section: the head (sections 19-26 mm), the shoulders (9-19) and the shank (7.3-8.6).
    let mut cells = Vec::new();
    for (name, section_mm, window) in [
        ("Granules, head", 23.0, Window { enabled: true, theta_deg: 90.0, span_deg: 90.0, fade_deg: 4.0, ..Window::default() }),
        ("Granules, shoulder, crown side", 13.0, Window { enabled: true, theta_deg: 150.0, span_deg: 30.0, fade_deg: 4.0, ..Window::default() }),
        ("Granules, shoulder, snout side", 13.0, Window { enabled: true, theta_deg: 30.0, span_deg: 30.0, fade_deg: 4.0, ..Window::default() }),
        ("Granules, shank", 7.6, Window { enabled: true, theta_deg: 270.0, span_deg: 210.0, fade_deg: 4.0, ..Window::default() }),
    ] {
        let mut t = TilingLayer::default_for(name, &ctx);
        t.v_center_mm = 0.5 * ctx.band_v_len_mm;
        t.v_span_mm = ctx.band_v_len_mm;
        t.rows = (section_mm / pitch).round().max(1.0) as u32;
        t.repeats_around = (ctx.circumference_mm / pitch).round() as u32;
        t.stagger = 0.5;
        t.height_mm = height;
        t.feather_mm = 0.0;
        let (cw, ch) = t.cell_size(&ctx);
        d.svgs.push(SvgAlpha { name: name.into(), svg: bead_svg(cw, ch, envf("PHRYNO_GFRAC", 0.86)), invert: false });
        cells.push(json!({"layer": name, "rows": t.rows, "repeats_around": t.repeats_around, "section_mm": section_mm}));
        let mut e = LayerEntry::new(name, Layer::Tiling(t));
        e.mask = Some("Off the table".into());
        e.window = window;
        if std::env::var("PHRYNO_NO_GRANULES").is_err() {
            d.layers.layers.push(e);
        }
    }
    d.bake_all(lib);
    let (cw, ch) = (pitch, pitch);
    comp.hide.push(json!({"layer": "Granules", "pitch_mm": [cw, ch], "height_mm": GRANULE_MM, "mask": "Off the table: everywhere 0.9 mm or more under the table"}));
    if std::env::var("PHRYNO_NO_STAMPS").is_ok() {
        d.stamps.clear();
    }
    comp.hide.push(json!({"stamps": "Crest tubercle", "count": crest, "diameter_mm": CREST_DIA_MM, "dome_mm": CREST_DOME_MM, "taper": CREST_TAPER}));
    comp.hide.push(json!({"stamps": "Fringe", "count": fringe, "length_mm": FRINGE_L_MM, "width_mm": FRINGE_W_MM, "apex_mm": FRINGE_APEX_MM, "taper": FRINGE_TAPER}));
    Ok(sites)
}

/// The granules on the strip the field leaves at one of the factory mesh's seams: beads the field's size, sculpted,
/// clear of the stamps struck there.
fn seam_granules(surf: &Surface, table: &Table, sites: &[(P3, f64)], centre_deg: f64, name: &'static str) -> Studs {
    let mut taken = Taken::new();
    for (p, r) in sites {
        taken.put(*p, (*r - 0.15).max(0.1));
    }
    let r = 0.43 * GRANULE_PITCH_MM;
    let keep = |s: &Sample| {
        let t = s.p[1].atan2(s.p[0]).to_degrees();
        let off = ((t - centre_deg + 540.0).rem_euclid(360.0) - 180.0).abs();
        (off < SEAM_DEG + 0.2 && s.p[0].hypot(s.p[1]) > surf.bore + GRANULE_BORE_CLEAR_MM && s.p[1] < table.top - 0.9).then_some(GRANULE_PITCH_MM)
    };
    let studs = scatter(surf, &mut taken, 31 + centre_deg as u64, &keep, &|s, n, _| Stud::bead(Kind::Granule, s.p, n, r, GRANULE_MM));
    Studs::new(name, studs)
}

fn author() -> Result<Authored> {
    let mut d = base()?;
    let mut lib = AlphaLibrary::builtin();
    let table = Table::of(&d)?;
    let head = Head::new(&table);
    let surf = Surface::of(&d)?;
    let ground = head_hide(&surf, &head, &table);
    let mut comp = Composition { table_top_mm: table.top, table_dome_mm: table.dome_mm, horns: head.horns.len(), plates: head.plates.len(), ..Composition::default() };
    comp.crown = CROWN.iter().map(|h| json!({"kind": h.kind, "root_xz_mm": h.root, "length_mm": h.len, "root_radius_mm": h.ra, "tip_radius_mm": h.rb, "rise_deg": h.rise_deg, "splay_deg": h.splay_deg, "pairs": 1})).collect();
    comp.hide.push(json!({"part": ground.name, "granules": ground.count(Kind::Granule)}));
    let sites = hide_layers(&mut d, &mut lib, &mut comp, &surf, &table)?;
    let seams = [seam_granules(&surf, &table, &sites, 0.0, "Granules, seam at 0"), seam_granules(&surf, &table, &sites, 180.0, "Granules, seam at 180")];
    for s in &seams {
        comp.hide.push(json!({"part": s.name, "granules": s.count(Kind::Granule)}));
    }
    let mut made = Vec::new();
    if std::env::var("PHRYNO_NO_SCULPT").is_err() {
        let head_field = |p: P3| head.field(&table, p).min(ground.field(p));
        let (mut hlo, mut hhi) = head.world_box(&table);
        let (glo, ghi) = ground.world_box();
        for k in 0..3 {
            hlo[k] = hlo[k].min(glo[k]);
            hhi[k] = hhi[k].max(ghi[k]);
        }
        let (head_made, seam_made) = std::thread::scope(|scope| {
            let h = scope.spawn(|| sculpt_solid("Horned head", hlo, hhi, HEAD_FACES, &head_field));
            let parts: Vec<_> = seams
                .iter()
                .map(|s| {
                    scope.spawn(move || {
                        let (lo, hi) = s.world_box();
                        let f = |p: P3| s.field(p);
                        sculpt_solid(s.name, lo, hi, SEAM_FACES, &f)
                    })
                })
                .collect();
            (h.join().unwrap(), parts.into_iter().map(|p| p.join().unwrap()).collect::<Vec<_>>())
        });
        made.push(Made { name: "Horned head".into(), solid: head_made.0 });
        comp.sculpts.push(head_made.1);
        for (s, (solid, info)) in seams.iter().zip(seam_made) {
            made.push(Made { name: s.name.into(), solid });
            comp.sculpts.push(info);
        }
    }
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Factory stock".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    for m in &made {
        let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
        let recipe = stored::Recipe {
            kernel: "sculpt".into(),
            op: if m.name == "Horned head" { "horned lizard head".into() } else { "seam granules".into() },
            params: json!({"stock": STOCK, "step_mm": STEP_MM, "horns": head.horns.len()}),
            digest: String::new(),
        };
        doc.append(Feature { id: next, name: m.name.clone(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: sculpt::packed(&m.solid)? }, component: joined() })?;
    }
    Ok(Authored { d, lib, comp, made, table, head, ground })
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
    let centre = [0.0, table.top, 1.0];
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
        if std::env::var("PHRYNO_SECTIONS").is_ok() {
            let ctx = d.field_context();
            for t in (0..360).step_by(15) {
                let x = (t as f64 / 360.0 * a.width as f64).round() as usize % a.width;
                let len: f64 = (1..a.height).map(|y| { let (p, q) = (a.at(x, y).p, a.at(x, y - 1).p); ((p[0]-q[0]).powi(2)+(p[1]-q[1]).powi(2)+(p[2]-q[2]).powi(2)).sqrt() }).sum();
                println!("theta {t}: section {len:.2} mm (band_v_len {:.2})", ctx.band_v_len_mm);
            }
            return Ok(());
        }
        if let Ok(t) = std::env::var("PHRYNO_COLUMN") {
            let t: f64 = t.parse()?;
            let x = (t.rem_euclid(360.0) / 360.0 * a.width as f64).round() as usize % a.width;
            for y in 0..a.height {
                let s = a.at(x, y);
                println!("row {y}: r {:.3} z {:.3} v {:.3} n [{:.2} {:.2} {:.2}]", s.p[0].hypot(s.p[1]), s.p[2], s.v, s.n[0], s.n[1], s.n[2]);
            }
            return Ok(());
        }
        for z in [5.0, 6.0, 7.0, 8.0, 9.0, 9.8] {
            let ys: Vec<f64> = a.samples.iter().filter(|s| s.p[0].abs() < 0.4 && (s.p[2] - z).abs() < 0.25).map(|s| s.p[1]).collect();
            let lo = ys.iter().copied().fold(f64::MAX, f64::min);
            let hi = ys.iter().copied().fold(f64::MIN, f64::max);
            println!("x=0 z {z}: y from {lo:.2} to {hi:.2} ({} samples)", ys.len());
        }
        for z in (-10..=10).map(|i| i as f64) {
            let mut row = String::new();
            for x in (-10..=10).map(|i| i as f64) {
                let y = a.samples.iter().filter(|s| (s.p[0] - x).abs() < 0.3 && (s.p[2] - z).abs() < 0.3 && s.n[1] > 0.2).map(|s| s.p[1]).fold(f64::NAN, f64::max);
                row += &format!(" {:5.1}", y - 14.0);
            }
            println!("z {z}: {row}");
        }
        return Ok(());
    }
    let Authored { d, lib, comp, made, table, head, ground } = author()?;
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
    if let Ok(at) = std::env::var("PHRYNO_LOOK") {
        let v: Vec<f64> = at.split(',').filter_map(|x| x.parse().ok()).collect();
        let t = v[0].to_radians();
        let c = [v[1] * t.cos(), v[1] * t.sin(), v[2]];
        let parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
        render::write_png_framed(out.join("look.png"), &parts, render::yaw_facing(v[0]), 0.35, render::Framing::new(c, v[3]), 1000)?;
    }
    if std::env::var("PHRYNO_SITES").is_ok() {
        for p in sculpt::crossing_sites(&solid_of(&built.mesh)).iter().take(40) {
            let r = p[0].hypot(p[1]);
            println!("    crossing at theta {:.1} r {:.2} z {:.2}", p[1].atan2(p[0]).to_degrees(), r, p[2]);
        }
    }
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
    // The lost-wax wall gate: the whole-mesh census of the finished ring at the 0.8 mm floor. Every sub-floor reading
    // is classed edge (a tip or lip closing within a floor of its free edge) or wall; the gate reads walls. The horns'
    // and thorns' points are also read with a 2 mm edge reach, named here, for the record.
    let walls = ringdesign_core::cad::measure::thickness(&built.mesh, MIN_SECTION_MM);
    let walls_reach = ringdesign_core::cad::measure::census(&built.mesh, &ringdesign_core::cad::measure::CensusOptions { floor_mm: MIN_SECTION_MM, pitch_mm: None, edge_reach_mm: Some(2.0) });
    // The lead's interim gate (2026-10-03): wall zones with a real section, 0.05 to 0.8 mm, are fixed; zones under
    // 0.05 mm are listed as suspected census artifacts and never reshape the ring.
    // A speck does not block, by the lead's ruling of 2026-10-04: "a single sample or two, a speck about 0.15 mm
    // across, which fills from the metal round it", under 0.02 mm2. A zone under 0.02 mm2, or under 0.15 mm across, is
    // listed with its point, area and span.
    let sample_mm2 = 0.02;
    let speck = |z: &&ringdesign_core::cad::measure::ThinZone| z.area_mm2 < sample_mm2 || z.span_mm < 0.15;
    let real_walls: Vec<&ringdesign_core::cad::measure::ThinZone> = walls.walls.iter().filter(|z| z.thinnest_mm >= 0.05 && !speck(z)).collect();
    let sub_sample: Vec<serde_json::Value> = walls.walls.iter().filter(|z| z.thinnest_mm >= 0.05 && speck(z)).map(|z| json!({"point": z.point, "area_mm2": z.area_mm2, "span_mm": z.span_mm, "thinnest_mm": z.thinnest_mm, "samples": z.samples})).collect();
    let artifacts: Vec<serde_json::Value> = walls.walls.iter().filter(|z| z.thinnest_mm < 0.05).map(|z| json!({"point": z.point, "area_mm2": z.area_mm2, "thinnest_mm": z.thinnest_mm})).collect();
    let wall_json = |t: &ringdesign_core::cad::measure::Thickness| json!({
        "clean": t.clean(), "assessed": t.assessed, "unresolved": t.unresolved, "wall_samples": t.below_limit, "wall_area_mm2": t.wall_area_mm2,
        "edge_samples": t.edge_below_limit, "edge_area_mm2": t.edge_area_mm2, "sampled_min_mm": t.sampled_min_mm, "rays": t.rays,
        "pitch_mm": t.pitch_mm, "edge_reach_mm": t.edge_reach_mm, "walls": t.walls, "edges": t.edges,
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
        ("lost-wax wall census: assessed, 0 unresolved, no wall zone past a speck (0.02 mm2 and 0.15 mm across) with a real section of 0.05-0.8 mm", walls.assessed && walls.unresolved == 0 && real_walls.is_empty()),
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
        "base": {"stock": "004 Shield", "face_mm": [FACE_MM.0, FACE_MM.1], "bore_mm": BORE_MM, "mirrored": false, "sand_envelope": false},
        "gates_that_apply": "lost wax (Logan, Fallback B): geometry, bore, field fill verdict at 0.8 mm, land widths, DFM, stones, 384 x 192, pattern, triangles, cold reload. The sand gates (ray release, draft-clamp bites, two-part Castable) do not apply; the two-part undercut is reported as a number.",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": main_pass.triangles, "build_s": build_s, "author_s": author_s},
        "main": main_pass.json(),
        "coarse_384x192": coarse_pass.json(),
        "sculpt": {"open_edges": open_edges, "self_crossings": sculpt_crossings, "parts": made.len()},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "band_verdict": band_field.verdict.label(), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": field.notes},
        "wall_census": wall_json(&walls),
        "wall_census_real_walls": real_walls.iter().map(|z| json!({"point": z.point, "area_mm2": z.area_mm2, "thinnest_mm": z.thinnest_mm, "span_mm": z.span_mm})).collect::<Vec<_>>(),
        "wall_census_suspected_artifacts": artifacts,
        "wall_census_specks": {"rule": "under 0.02 mm2, or under 0.15 mm across (the lead, 2026-10-04)", "zones": sub_sample},
        "wall_census_edge_reach_2mm": wall_json(&walls_reach),
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
        "  field {} (band {}, thinnest {:.2} mm); dfm {}; pattern {pw}/{pd}/{px}; bore nearest {least_r:.3} of {:.3}; walls clean {} ({} wall, {} edge samples, {} unresolved; reach 2 mm: {} wall)",
        field.verdict.label(),
        band_field.verdict.label(),
        band_field.thinnest_wall_mm,
        findings.len(),
        d.inner_radius_mm(),
        walls.clean(),
        walls.below_limit,
        walls.edge_below_limit,
        walls.unresolved,
        walls_reach.below_limit
    );
    for w in &real_walls {
        let p = w.point;
        let jaw = head.jaw.iter().map(|j| j.eval(p)).fold(f64::MAX, f64::min);
        let horn = head.horns.iter().map(|h| round_cone(p, h.a, h.b, h.ra, h.rb)).fold(f64::MAX, f64::min);
        println!(
            "    real wall: {:.3} mm2, thinnest {:.3} at {:?}; nearest jaw {jaw:.2}, horn {horn:.2}, ground {:.2}, skull {:.2}, table edge {:.2}, h {:.2}",
            w.area_mm2, w.thinnest_mm, p.map(|x| (x * 100.0).round() / 100.0), ground.field(p), head.skull_at(&table, p), table.edge(p[0], p[2]), p[1] - table.top
        );
    }
    println!("    suspected census artifacts (< 0.05 mm): {}; specks under {sample_mm2} mm2: {}", artifacts.len(), sub_sample.len());
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
