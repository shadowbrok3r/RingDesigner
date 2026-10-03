//! Vepres — Datura, the thorn-apple: a spined seed capsule split into four gaping valves, black spinel seeds in the
//! splits, standing on factory 011 Badge as on its calyx; cast in lost wax.
//! cargo build --release -p ringdesign-core --example vepres_datura
//! target/release/examples/vepres_datura [OUT_DIR] [--draft] [--verify] [--blockout] [--bare]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, Feature, Operation, Placement, Stage, SurfaceKind, builders, stored},
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render,
    setting::{Stamp, StampTop},
    skin,
    sketch::Id,
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const BORE_MM: f64 = 18.6;
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;
/// Meridian samples the valve and the split share, foot to where the split opens.
/// The template carries every stored triangle (about ten bytes each in the graph), so the sculpture is meshed to a
/// budget of about 90k triangles in all.
const SHARED_ROWS: usize = 34;
const DIVERGENT_ROWS: usize = 76;
const CAPSULE_AROUND: usize = 120;
/// The stem: its width, how far it stands proud of the skin, where it ends round the palm, and its leaf nodes (degrees).
const STEM_W_MM: f64 = 2.2;
const STEM_PROUD_MM: f64 = 0.9;
const STEM_END_DEG: f64 = -165.0;
const STEM_NODES_DEG: [f64; 2] = [-28.0, -100.0];
/// The two small leaves hanging from the stem round the shank: bearing (degrees), across offset, turn off the stem.
/// The shank leaves' length and width, mm.
const SHANK_LEAF_MM: (f64, f64) = (5.4, 3.2);
const SHANK_LEAVES: [(f64, f64, f64); 2] = [(-28.0, 0.25, 10.0), (-100.0, -0.25, -10.0)];
/// Which way the capsule leans in the table's frame: straight back, away from the hero.
const LEAN_BEARING_DEG: f64 = 90.0;
/// The seeds in the open splits: which split, how far up its floor (share of its run) and how far off its middle
/// (share of its half-angle).
const IN_SPLIT: [(usize, f64, f64); 4] = [(0, 0.4, 0.0), (1, 0.46, 0.0), (2, 0.4, 0.0), (3, 0.46, 0.0)];
/// The leaves: where each springs from on the table (table frame), its bearing (degrees) and its length wanted.
const LEAVES: [([f64; 2], f64, f64); 4] = [([0.8, 5.0], 172.0, 8.5), ([-1.0, 1.0], 205.0, 7.5), ([-1.6, -2.2], 228.0, 6.2), ([4.6, -1.2], 272.0, 6.0)];
/// The trumpet flower: length, mouth across, and any bend of its bell off the table (0 lies straight).
const FLOWER_LEN_MM: f64 = 18.5;
const FLOWER_MOUTH_MM: f64 = 10.0;
const FLOWER_BEND_DEG: f64 = 0.0;
/// The flower's ribs, how far its mouth stands off the plate, and the curled tooth at each of its five points.
const RIB_MM: f64 = 0.3;
const MOUTH_LIFT_MM: f64 = 1.7;
const TOOTH_MM: f64 = 1.3;
/// Where the flower runs out from under the capsule, degrees from the table frame's x round the ring.
const FLOWER_BEARING_DEG: f64 = 205.0;
/// The capsule's axis on the table, in the table's frame (x round the ring, y along the finger away from the hero).
const CAPSULE_AT: [f64; 2] = [3.5, 3.5];
/// Where the flower's stalk starts on the table, in the table's frame: under the capsule, the two on one stem.
const FLOWER_FROM: [f64; 2] = CAPSULE_AT;
/// How deep the splits run under the crown, and half the clear gap they hold between the valves' lips.
const SPLIT_DEPTH_MM: f64 = 4.0;
const HALF_GAP_MM: f64 = 1.0;

/// Black spinel, the seeds.
const SPINEL_TINT: [f32; 3] = [0.03, 0.03, 0.04];
const SEED_MM: f64 = 1.5;
/// The seed pads: radius, height, how far each stands proud of its split's floor, and the girdle over its top.
const PAD_R_MM: f64 = 0.98;
const PAD_H_MM: f64 = 1.0;
const PAD_PROUD_MM: f64 = 0.05;
const SEED_OVER_PAD_MM: f64 = 0.25;
/// The seeds spilled on the table in front of the capsule, in the table's frame, and how far their pads stand proud.
const SPILLED: [[f64; 2]; 4] = [[3.0, -0.8], [1.0, -2.0], [0.9, -4.4], [2.8, -6.1]];
const SPILLED_PAD_PROUD_MM: f64 = 0.35;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The investment pour: 18k yellow gold, the wax floors.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = MIN_DETAIL_MM;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.name = "Datura / investment / Gold 18k".into();
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(1.3, |m| m.shrink_pct);
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    s.sample_pitch_mm = 0.1;
    s.bench_notes = "Invest the badge with the capsule, its spines and the seed seats in one tree, sprued at the palm. After the \
        pour: clean the spines with a fine graver, open each seed seat to the measured stone, set the eight black spinels \
        and close a bead over each girdle. Polish the palm and the badge's walls; leave the valves' splits dark."
        .into();
    s
}

/// Factory 011 Badge at its native 18 x 20 face, not mirrored, on its own Flat chart.
fn stock() -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "011").context("no stock 011")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, preset.load()?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.name = "Datura \u{2014} the thorn-apple".into();
    let (length, width) = (d.shank.head.length_mm, d.profile.width_mm);
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.shank.head.length_mm = length;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = export_params();
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = None;
    d.draft.min_detail_mm = MIN_DETAIL_MM;
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d.manufacturing = Some(s);
    Ok(d)
}

// --- Small vector helpers ------------------------------------------------------------------------------------------

fn add3(a: P3, b: P3, k: f64) -> P3 {
    std::array::from_fn(|i| a[i] + b[i] * k)
}
fn sub3(a: P3, b: P3) -> P3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit3(a: P3) -> P3 {
    let l = dot3(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}
fn smooth(lo: f64, hi: f64, x: f64) -> f64 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lerp2(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}
/// The angle from `b` to `a` folded into (-pi, pi].
fn wrap(a: f64) -> f64 {
    let x = (a + PI).rem_euclid(2.0 * PI) - PI;
    if x <= -PI { x + 2.0 * PI } else { x }
}

// --- The badge's table ---------------------------------------------------------------------------------------------

/// Where the capsule stands: the table's height over the finger axis at its centre, and its extent round and across.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Table {
    top_mm: f64,
    round_mm: f64,
    across_mm: f64,
}

fn table(d: &RingDesign, lib: &AlphaLibrary) -> Result<Table> {
    let mut bare = d.clone();
    bare.cad = None;
    let b = mesh::try_build(&bare, lib, coarse_params())?;
    let (hit, _) = ringdesign_core::cad::surface_hit(&b.mesh, 90.0, 0.0).context("no table at 90 deg")?;
    let top = hit[1];
    let (x, z) = b
        .mesh
        .vertices
        .iter()
        .filter(|v| v.1 as f64 > top - 0.25)
        .fold((0.0f64, 0.0f64), |(x, z), v| (x.max(v.0.abs() as f64), z.max(v.2.abs() as f64)));
    Ok(Table { top_mm: top, round_mm: 2.0 * x, across_mm: 2.0 * z })
}

// --- The badge's outer skin, for draping ---------------------------------------------------------------------------

/// The bare band's outer radius over (theta, across) on a fine grid: the largest radius any triangle reaches there.
struct Skin {
    theta0: f64,
    dtheta: f64,
    z0: f64,
    dz: f64,
    nt: usize,
    nz: usize,
    r: Vec<f64>,
}

impl Skin {
    fn of(m: &mesh::Mesh, theta_range: (f64, f64), z_range: (f64, f64)) -> Self {
        let (dtheta, dz) = (0.1f64.to_radians(), 0.02);
        let nt = ((theta_range.1 - theta_range.0).to_radians() / dtheta).ceil() as usize + 1;
        let nz = ((z_range.1 - z_range.0) / dz).ceil() as usize + 1;
        let mut skin = Self { theta0: theta_range.0.to_radians(), dtheta, z0: z_range.0, dz, nt, nz, r: vec![f64::NAN; nt * nz] };
        for f in &m.faces {
            let q: Vec<[f64; 3]> = f
                .iter()
                .map(|&i| {
                    let v = m.vertices[i as usize];
                    let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
                    let t = y.atan2(x).rem_euclid(2.0 * PI);
                    [if t < theta_range.0.to_radians() { t + 2.0 * PI } else { t }, z, x.hypot(y)]
                })
                .collect();
            if q.iter().map(|p| p[0]).fold(f64::MIN, f64::max) - q.iter().map(|p| p[0]).fold(f64::MAX, f64::min) > PI {
                continue;
            }
            let gi = |t: f64| (t - skin.theta0) / skin.dtheta;
            let gj = |z: f64| (z - skin.z0) / skin.dz;
            let (i0, i1) = (q.iter().map(|p| gi(p[0])).fold(f64::MAX, f64::min).ceil().max(0.0) as isize, q.iter().map(|p| gi(p[0])).fold(f64::MIN, f64::max).floor() as isize);
            let (j0, j1) = (q.iter().map(|p| gj(p[1])).fold(f64::MAX, f64::min).ceil().max(0.0) as isize, q.iter().map(|p| gj(p[1])).fold(f64::MIN, f64::max).floor() as isize);
            let tri: Vec<P3> = f.iter().map(|&i| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] }).collect();
            let (e1, e2) = (sub3(tri[1], tri[0]), sub3(tri[2], tri[0]));
            for i in i0..=i1.min(nt as isize - 1) {
                for j in j0..=j1.min(nz as isize - 1) {
                    // The radial ray from the finger's axis at this height, met exactly.
                    let (t, z) = (skin.theta0 + i as f64 * dtheta, skin.z0 + j as f64 * dz);
                    let (o, dir) = ([0.0, 0.0, z], [t.cos(), t.sin(), 0.0]);
                    let h = cross3(dir, e2);
                    let det = dot3(e1, h);
                    if det.abs() < 1e-12 {
                        continue;
                    }
                    let sv = sub3(o, tri[0]);
                    let a = dot3(sv, h) / det;
                    let qv = cross3(sv, e1);
                    let b = dot3(dir, qv) / det;
                    if a < -1e-9 || b < -1e-9 || a + b > 1.0 + 1e-9 {
                        continue;
                    }
                    let r = dot3(e2, qv) / det;
                    if r <= 0.0 {
                        continue;
                    }
                    let cell = &mut skin.r[i as usize * nz + j as usize];
                    if cell.is_nan() || r > *cell {
                        *cell = r;
                    }
                }
            }
        }
        skin
    }
    /// The outer radius at `theta` (radians) and `z`, bilinear; None off the band.
    fn radius(&self, theta: f64, z: f64) -> Option<f64> {
        let theta = self.theta0 + (theta - self.theta0).rem_euclid(2.0 * PI);
        let (u, v) = ((theta - self.theta0) / self.dtheta, (z - self.z0) / self.dz);
        if u < 0.0 || v < 0.0 || u >= (self.nt - 1) as f64 || v >= (self.nz - 1) as f64 {
            return None;
        }
        let (i, j) = (u.floor() as usize, v.floor() as usize);
        let (fu, fv) = (u - i as f64, v - j as f64);
        let at = |i: usize, j: usize| self.r[i * self.nz + j];
        let r = at(i, j) * (1.0 - fu) * (1.0 - fv) + at(i + 1, j) * fu * (1.0 - fv) + at(i, j + 1) * (1.0 - fu) * fv + at(i + 1, j + 1) * fu * fv;
        (!r.is_nan()).then_some(r)
    }
    /// How far across the band reaches at `theta` (degrees): the least and greatest z with skin.
    fn across(&self, theta_deg: f64) -> (f64, f64) {
        let t = theta_deg.to_radians();
        let zs: Vec<f64> = (0..self.nz).map(|j| self.z0 + j as f64 * self.dz).filter(|&z| self.radius(t, z).is_some()).collect();
        (zs.first().copied().unwrap_or(0.0), zs.last().copied().unwrap_or(0.0))
    }
}

// --- The capsule ---------------------------------------------------------------------------------------------------

/// The thorn-apple's measures, mm, in the capsule's own frame: z up its axis from the table, x round the ring.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Shape {
    /// Widest radius of the egg.
    egg_r: f64,
    /// Height of the valves' tips over the table.
    tip_z: f64,
    /// Height where the splits open on the side.
    split_z: f64,
    /// Half the split's angle at its widest, radians.
    gape: f64,
    /// How far the valves' lips flare out along the splits.
    lip_mm: f64,
    /// The calyx frill's reach and its five lobes' depth.
    frill_r: f64,
    frill_lobe: f64,
    /// Depth the capsule is sunk into the table.
    sunk_mm: f64,
    /// The four splits' bearing round the axis from local x, radians.
    split_at: f64,
    /// The whole capsule scaled from these reference measures; the splits keep their clear gap in millimetres.
    scale: f64,
    /// How many valves have split, a quarter turn apart from `split_at`.
    splits: usize,
    /// The egg's height over its reference, after `scale`: above 1 it stands taller than round.
    stretch: f64,
    /// How far the egg leans, degrees, sheared over its base so its foot stays flat.
    lean_deg: f64,
}

const SHAPE: Shape = Shape {
    egg_r: 4.5,
    tip_z: 9.0,
    split_z: 6.0,
    gape: 0.8,
    lip_mm: 0.5,
    frill_r: 4.9,
    frill_lobe: 0.45,
    sunk_mm: 0.9,
    split_at: 0.25 * PI + 0.44,
    scale: 0.78,
    splits: 4,
    stretch: 1.3,
    lean_deg: 22.0,
};

/// The meridian of a valve's middle and of a split's floor, sampled alike: (r, z) from the buried foot to the axis
/// over the placenta. Both run the same way up the egg until the split opens.
fn meridians(s: &Shape) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let (r, h) = (s.egg_r, s.tip_z);
    let floor_z = h - SPLIT_DEPTH_MM;
    let foot = -s.sunk_mm;
    // Shared run: the buried foot, the calyx's reflexed frill, the waist and the egg's lower half.
    let shared: Vec<[f64; 2]> = vec![
        [0.0, foot],
        [s.frill_r + 0.3, foot],
        [s.frill_r + 0.55, -0.25],
        [s.frill_r + 0.35, 0.0],
        [s.frill_r + 0.05, 0.14],
        [s.frill_r - 0.15, 0.3],
        [s.frill_r - 0.35, 0.44],
        [r + 0.1, 0.62],
        [r - 0.45, 0.95],
        [r - 0.5, 1.45],
    ];
    // The egg's outline from its waist over its blunt crown to the axis; the splits open from `split_z` up.
    let egg: Vec<[f64; 2]> = vec![[r - 0.4, 2.0], [r, 3.5], [r - 0.3, 4.8], [r - 0.85, 6.0], [r - 1.6, 7.1], [r - 2.4, h - 0.62], [r - 3.2, h - 0.16], [r - 3.9, h - 0.02], [0.0, h]];
    let k = egg.iter().position(|p| p[1] > s.split_z).unwrap_or(egg.len() - 1).max(1);
    let t = (s.split_z - egg[k - 1][1]) / (egg[k][1] - egg[k - 1][1]);
    let r_s = egg[k - 1][0] + (egg[k][0] - egg[k - 1][0]) * t;
    let mut shared = shared;
    shared.extend(egg.iter().filter(|p| p[1] < s.split_z - 0.25));
    shared.push([r_s, s.split_z]);
    // The valve: on up the egg and over its crown.
    let valve: Vec<[f64; 2]> = egg.iter().copied().filter(|p| p[1] > s.split_z + 0.25).collect();
    // The split's floor: in and down from where the valves part to a level floor four millimetres under the crown,
    // where the seeds lie on the placenta's arms.
    let split: Vec<[f64; 2]> = vec![
        [r_s - 0.35, s.split_z - 0.05],
        [r_s - 0.9, 0.5 * (s.split_z + floor_z)],
        [0.45 * r_s, floor_z - 0.1],
        [0.7, floor_z],
        [0.35, floor_z + 0.03],
        [0.0, floor_z + 0.05],
    ];
    // The shared run is sampled once, so both meridians agree on it point for point; each divergent run alike.
    let joint = *shared.last().unwrap();
    let mut a = resample(&shared, SHARED_ROWS);
    let mut b = a.clone();
    let mut v = vec![joint];
    v.extend(valve);
    let mut w = vec![joint];
    w.extend(split);
    a.extend(resample(&v, DIVERGENT_ROWS).into_iter().skip(1));
    b.extend(resample(&w, DIVERGENT_ROWS).into_iter().skip(1));
    let k = s.scale;
    let z = k * s.stretch;
    (a.into_iter().map(|p| [p[0] * k, p[1] * z]).collect(), b.into_iter().map(|p| [p[0] * k, p[1] * z]).collect())
}

/// A polyline smoothed by Catmull-Rom and resampled to `n` points by its own length, ends kept.
fn resample(pts: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let mut dense = Vec::new();
    for i in 0..pts.len() - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(pts.len() - 1)];
        for k in 0..24 {
            let t = k as f64 / 24.0;
            let (t2, t3) = (t * t, t * t * t);
            dense.push(std::array::from_fn(|j| {
                0.5 * (2.0 * p1[j] + (-p0[j] + p2[j]) * t + (2.0 * p0[j] - 5.0 * p1[j] + 4.0 * p2[j] - p3[j]) * t2 + (-p0[j] + 3.0 * p1[j] - 3.0 * p2[j] + p3[j]) * t3)
            }));
        }
    }
    dense.push(*pts.last().unwrap());
    let mut arc = vec![0.0];
    for w in dense.windows(2) {
        arc.push(arc.last().unwrap() + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let total = *arc.last().unwrap();
    (0..n)
        .map(|i| {
            let s = total * i as f64 / (n - 1) as f64;
            let k = arc.partition_point(|&a| a < s).clamp(1, dense.len() - 1);
            let t = (s - arc[k - 1]) / (arc[k] - arc[k - 1]).max(1e-12);
            lerp2(dense[k - 1], dense[k], t)
        })
        .collect()
}

/// The capsule as a closed surface in its own frame, before its spines: each meridian blends the valve's and the
/// split's by how near its bearing lies to a split, the blend opening as the split widens toward the top.
struct Capsule {
    shape: Shape,
    valve: Vec<[f64; 2]>,
    split: Vec<[f64; 2]>,
    /// Each meridian sample's split half-angle: the clear gap held at the valve's own radius, never shrinking upward.
    halves: Vec<f64>,
    around: usize,
}

impl Capsule {
    fn new(shape: Shape, around: usize) -> Self {
        let (valve, split) = meridians(&shape);
        let mut halves = vec![0.0; valve.len()];
        let mut most: f64 = 0.0;
        for i in SHARED_ROWS..valve.len() {
            let along = (i - SHARED_ROWS) as f64 / (valve.len() - SHARED_ROWS) as f64;
            let opened = 0.35 + 0.65 * smooth(0.02, 0.35, along);
            let raw = (HALF_GAP_MM / valve[i][0].max(1e-3)).min(1.0).asin().min(shape.gape) * opened * smooth(0.0, 0.02, along);
            most = most.max(raw);
            halves[i] = most;
        }
        Self { shape, valve, split, halves, around }
    }
    /// How far into its split a bearing `phi` lies at meridian sample `i`: 1 on the floor, 0 on a valve's face; the
    /// split's floor runs flat across its middle half and rises to the lips.
    fn into_split(&self, i: usize, phi: f64) -> f64 {
        let (half, _) = self.half(i);
        if half <= 0.0 {
            return 0.0;
        }
        1.0 - smooth(0.5 * half, half, self.off(phi))
    }
    /// A split's half-angle at meridian sample `i`, radians, and the band its lip flares over.
    fn half(&self, i: usize) -> (f64, f64) {
        (self.halves[i], 0.0)
    }
    /// How far up the split meridian sample `i` lies: 0 where it opens, 1 at the placenta.
    fn along(&self, i: usize) -> f64 {
        (i as f64 - SHARED_ROWS as f64).max(0.0) / (self.valve.len() - SHARED_ROWS) as f64
    }
    /// How far a bearing lies from the nearest split, radians.
    fn off(&self, phi: f64) -> f64 {
        (0..self.shape.splits).map(|k| wrap(phi - self.shape.split_at - k as f64 * PI / 2.0).abs()).fold(f64::MAX, f64::min)
    }
    /// The bearing of column `j` on meridian row `i`. Where four splits are open the columns are warped row by row, so
    /// each split's walls fall on the same columns all the way up and their edges run clean, not stepped.
    fn phi_of(&self, i: usize, j: usize) -> f64 {
        let base = 2.0 * PI * j as f64 / self.around as f64;
        let (half, _) = self.half(i);
        if self.shape.splits != 4 || half <= 0.0 {
            return base;
        }
        let quarter = PI / 2.0;
        let q = (base - self.shape.split_at).rem_euclid(2.0 * PI) / quarter;
        let (k, u) = (q.floor(), q - q.floor());
        let (h, a) = ((half / quarter).min(0.42), 0.22);
        let warp = if u < a { u / a * h } else if u > 1.0 - a { 1.0 - (1.0 - u) / a * h } else { h + (u - a) / (1.0 - 2.0 * a) * (1.0 - 2.0 * h) };
        let blend = smooth(0.0, 0.25, half / self.shape.gape);
        self.shape.split_at + (k + (1.0 - blend) * u + blend * warp) * quarter
    }
    fn point(&self, i: usize, j: usize) -> P3 {
        self.point_at(i, self.phi_of(i, j))
    }
    fn point_at(&self, i: usize, phi: f64) -> P3 {
        let t = self.into_split(i, phi);
        let [r, z] = lerp2(self.valve[i], self.split[i], t);
        // The valves' lips flare out along each split, fading over a millimetre of the valve.
        let (half, wall) = self.half(i);
        let lip = if half > 0.0 {
            let beyond = (self.off(phi) - half - wall) * r.max(1.0);
            self.shape.lip_mm * smooth(0.02, 0.35, self.along(i)) * smooth(0.8, 2.0, self.valve[i][0]) * (1.0 - smooth(0.0, 1.1, beyond)) * (1.0 - t)
        } else {
            0.0
        };
        let r = r + lip;
        // The frill's five shallow lobes.
        let frill = smooth(0.0, 0.5, z + self.shape.sunk_mm) * (1.0 - smooth(0.5, 0.9, z));
        let r = r * (1.0 - frill * self.shape.frill_lobe / self.shape.frill_r * (0.5 - 0.5 * (5.0 * phi).cos()));
        [r * phi.cos(), r * phi.sin(), z]
    }
    fn solid(&self) -> csg::Solid {
        let m = self.valve.len();
        let n = self.around;
        let mut s = csg::Solid::default();
        // Rows 1..m-1 are rings; rows 0 and m-1 are the poles on the axis.
        let bottom = [0.0, 0.0, self.valve[0][1]];
        let top = [0.0, 0.0, self.valve[m - 1][1]];
        s.v.push(bottom);
        for i in 1..m - 1 {
            for j in 0..n {
                s.v.push(self.point(i, j));
            }
        }
        s.v.push(top);
        let last = (s.v.len() - 1) as u32;
        let at = |i: usize, j: usize| (1 + (i - 1) * n + j % n) as u32;
        for j in 0..n {
            s.f.push([0, at(1, j + 1), at(1, j)]);
            s.f.push([last, at(m - 2, j), at(m - 2, j + 1)]);
        }
        for i in 1..m - 2 {
            for j in 0..n {
                s.f.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
                s.f.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
            }
        }
        if s.volume() < 0.0 {
            for f in &mut s.f {
                f.swap(1, 2);
            }
        }
        s
    }
    /// The surface point and outward normal at a fractional meridian place `u` (0 foot, 1 placenta) and bearing `phi`.
    fn at(&self, u: f64, phi: f64) -> (P3, P3) {
        let m = self.valve.len();
        let i = ((u * (m - 1) as f64).round() as usize).clamp(1, m - 2);
        let p = self.point_at(i, phi);
        let du = sub3(self.point_at(i + 1, phi), self.point_at(i - 1, phi));
        let dv = sub3(self.point_at(i, phi + 0.01), self.point_at(i, phi - 0.01));
        (p, unit3(cross3(dv, du)))
    }
    /// The meridian index whose valve point stands nearest height `z` on the egg's outer run.
    fn row_at(&self, z: f64) -> usize {
        let top = self.valve.iter().enumerate().max_by(|a, b| a.1[1].total_cmp(&b.1[1])).map_or(0, |(i, _)| i);
        (1..top).min_by(|&a, &b| (self.valve[a][1] - z).abs().total_cmp(&(self.valve[b][1] - z).abs())).unwrap_or(1)
    }
}

/// A spine: a cone from a flared, buried root to a rounded point, along `axis` from `foot`.
fn spine(foot: P3, axis: P3, length: f64, root_r: f64, tip_r: f64) -> csg::Solid {
    let axis = unit3(axis);
    let side = unit3(cross3(axis, if axis[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }));
    let up = cross3(axis, side);
    let around = 10usize;
    let buried = 0.45;
    // Rings along the axis: (height along the axis, radius).
    let mut rings: Vec<(f64, f64)> = Vec::new();
    let steps = 8;
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        let h = -buried + (length - tip_r + buried) * t;
        let along = (h.max(0.0) / (length - tip_r)).clamp(0.0, 1.0);
        let cone = tip_r + (root_r - tip_r) * (1.0 - along).powf(1.15);
        let flare = 0.12 * (1.0 - smooth(0.0, 0.3, h)).powi(2);
        rings.push((h, cone + flare));
    }
    // The rounded point.
    for k in 1..5 {
        let a = PI * 0.5 * k as f64 / 5.0;
        rings.push((length - tip_r + tip_r * a.sin(), tip_r * a.cos()));
    }
    let mut s = csg::Solid::default();
    s.v.push(add3(foot, axis, -buried));
    for &(h, r) in &rings {
        for j in 0..around {
            let a = 2.0 * PI * j as f64 / around as f64;
            let p = add3(add3(add3(foot, axis, h), side, r * a.cos()), up, r * a.sin());
            s.v.push(p);
        }
    }
    s.v.push(add3(foot, axis, length));
    let last = (s.v.len() - 1) as u32;
    let n = rings.len();
    let at = |i: usize, j: usize| (1 + i * around + j % around) as u32;
    for j in 0..around {
        s.f.push([0, at(0, j + 1), at(0, j)]);
        s.f.push([last, at(n - 1, j), at(n - 1, j + 1)]);
    }
    for i in 0..n - 1 {
        for j in 0..around {
            s.f.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
            s.f.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
        }
    }
    if s.volume() < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

#[derive(Clone, Debug, serde::Serialize)]
struct SpineRow {
    /// The 2 mm band of height the count and mean length are taken over, from its foot.
    z_mm: f64,
    count: usize,
    length_mm: f64,
    root_mm: f64,
}

#[derive(Debug, serde::Serialize)]
struct CapsuleReport {
    shape: Shape,
    rows: Vec<SpineRow>,
    spines: usize,
    triangles: usize,
    volume_mm3: f64,
    slivers_cleaned: usize,
}

/// The seeds inside the capsule's open splits, in its frame as it leans: (point on a split's floor, the floor's
/// outward normal there, bearing).
fn seed_places(c: &Capsule) -> Vec<(P3, P3, f64)> {
    let m = c.valve.len();
    let mut out = Vec::new();
    for (split, along, side) in IN_SPLIT {
        let i = SHARED_ROWS + ((m - SHARED_ROWS) as f64 * along) as usize;
        let (half, _) = c.half(i);
        let phi = c.shape.split_at + split as f64 * PI / 2.0 + side * half;
        let (p, n) = c.at(i as f64 / (m - 1) as f64, phi);
        out.push((leaned(&c.shape, p), leaned_normal(&c.shape, p, n), phi));
    }
    out
}

/// The capsule's lean: heights over its waist slide toward `LEAN_BEARING_DEG` by a shear that eases in over 2 mm, so the foot
/// stays flat on the table and no vertical line folds.
fn lean_by(shape: &Shape, z: f64) -> (f64, f64) {
    let (z0, w, t) = (0.9 * shape.scale, 2.0, shape.lean_deg.to_radians().tan());
    let h = (z - z0).max(0.0);
    if h < w { (t * h * h / (2.0 * w), t * h / w) } else { (t * (h - 0.5 * w), t) }
}

fn leaned(shape: &Shape, p: P3) -> P3 {
    let (d, b) = (lean_by(shape, p[2]).0, LEAN_BEARING_DEG.to_radians());
    [p[0] + d * b.cos(), p[1] + d * b.sin(), p[2]]
}

fn leaned_normal(shape: &Shape, p: P3, n: P3) -> P3 {
    let (g, b) = (lean_by(shape, p[2]).1, LEAN_BEARING_DEG.to_radians());
    unit3([n[0], n[1], n[2] - g * (n[0] * b.cos() + n[1] * b.sin())])
}

/// The whole capsule in its own frame: the split egg and its rows of spines, one closed solid.
fn capsule(shape: Shape, blockout: bool) -> Result<(csg::Solid, CapsuleReport, Capsule)> {
    let c = Capsule::new(shape, if blockout { 96 } else { CAPSULE_AROUND });
    if std::env::var("DATURA_DEBUG").is_ok() {
        for jj in 0..c.around / 4 {
            let t = jj as f64;
            let line: Vec<[f64; 2]> = (0..c.valve.len()).map(|i| { let p = c.point(i, jj); [p[0].hypot(p[1]), p[2]] }).collect();
            let seg = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
                let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
                o(a, b, c) * o(a, b, d) < 0.0 && o(c, d, a) * o(c, d, b) < 0.0
            };
            for i in 0..line.len() - 1 {
                for j in i + 2..line.len() - 1 {
                    if seg(line[i], line[i + 1], line[j], line[j + 1]) {
                        eprintln!("t {t}: segments {i} and {j} cross at {:?} {:?}", line[i], line[j]);
                    }
                }
            }
        }
    }
    let body = c.solid();
    if std::env::var("DATURA_DEBUG").is_ok() {
        let n = c.around;
        let m = c.valve.len();
        let rows = |a: usize, b: usize| -> usize {
            let f: Vec<[u32; 3]> = (a..b).flat_map(|i| body.f[2 * n + (i - 1) * 2 * n..2 * n + i * 2 * n].to_vec()).collect();
            csg::self_crossings(&csg::Solid { v: body.v.clone(), f })
        };
        eprintln!("poles {}", csg::self_crossings(&csg::Solid { v: body.v.clone(), f: body.f[..2 * n].to_vec() }));
        for a in (1..m - 2).step_by(10) {
            let b = (a + 12).min(m - 2);
            let k = rows(a, b);
            if k > 0 {
                eprintln!("rows {a}..{b}: {k} crossings; valve {:?} split {:?}", c.valve[a], c.split[a]);
            }
        }
    }
    ensure!(body.open_edges() == (0, 0), "The capsule's body does not close");
    let body_crossings = csg::self_crossings(&body);
    ensure!(body_crossings == 0, "The capsule's body crosses itself {body_crossings} times");
    // An even, unruled coat of spines: candidates on a fine grid over the egg, taken in a fixed shuffled order and
    // kept where no kept spine stands nearer than the spacing, never in or at the lip of a split.
    let mut parts = vec![body];
    let spacing = 1.25;
    let mut candidates: Vec<(u64, usize, f64)> = Vec::new();
    let mut z = 1.7 * shape.scale;
    while z < shape.tip_z * shape.scale - 0.4 {
        let i = c.row_at(z);
        let r = c.valve[i][0];
        let n = (2.0 * PI * r / 0.22).round() as usize;
        for k in 0..n {
            let phi = 2.0 * PI * k as f64 / n as f64;
            let key = (i as u64 * 2_654_435_761 ^ k as u64 * 40_503).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 17;
            candidates.push((key, i, phi));
        }
        z += 0.22;
    }
    candidates.sort_by_key(|c| c.0);
    let mut feet: Vec<(P3, f64)> = Vec::new();
    let mut lengths: Vec<(f64, f64)> = Vec::new();
    for &(_, i, phi) in &candidates {
        let r = c.valve[i][0];
        let z = c.valve[i][1];
        let root = 0.42;
        let clear = (root + 0.35) / r;
        if [-clear, 0.0, clear].iter().any(|d| c.into_split(i, phi + d) > 0.0) {
            continue;
        }
        let u = i as f64 / (c.valve.len() - 1) as f64;
        let (p, n) = c.at(u, phi);
        if feet.iter().any(|(f, _)| dot3(sub3(*f, p), sub3(*f, p)) < spacing * spacing) {
            continue;
        }
        // Longest round the egg's waist, shorter toward the foot and the crown; varied by the shuffle.
        let hash = ((i * 131 + (phi * 1000.0) as usize * 7) % 97) as f64 / 97.0;
        let length = 1.0 + 0.8 * hash;
        let axis = unit3(add3(n, [0.0, 0.0, 1.0], 0.08));
        parts.push(spine(add3(p, n, -0.05), axis, length, root, 0.16));
        feet.push((p, z));
        lengths.push((z, length));
    }
    let mut report_rows = Vec::new();
    for band in 0..((shape.tip_z * shape.scale / 2.0).ceil() as usize) {
        let (lo, hi) = (2.0 * band as f64, 2.0 * band as f64 + 2.0);
        let band: Vec<f64> = lengths.iter().filter(|(z, _)| *z >= lo && *z < hi).map(|(_, l)| *l).collect();
        let mean = if band.is_empty() { 0.0 } else { band.iter().sum::<f64>() / band.len() as f64 };
        report_rows.push(SpineRow { z_mm: lo, count: band.len(), length_mm: mean, root_mm: 0.84 });
    }
    let spines = parts.len() - 1;
    let mut solid = match csg::union_all(&parts) {
        Ok(s) => s,
        Err(e) => {
            // Name the spine that will not join, for the record.
            let bad: Vec<usize> = (1..parts.len()).filter(|&k| csg::union_all(&[parts[0].clone(), parts[k].clone()]).is_err()).collect();
            anyhow::bail!("The spines do not join the capsule: {e:?}; alone, spines {bad:?} fail");
        }
    };
    let slivers = csg::clean(&mut solid, 2e-5);
    for v in &mut solid.v {
        *v = leaned(&shape, *v);
    }
    ensure!(solid.open_edges() == (0, 0), "The spined capsule does not close");
    ensure!(csg::self_crossings(&solid) == 0, "The spined capsule crosses itself");
    let report = CapsuleReport { shape, rows: report_rows, spines, triangles: solid.f.len(), volume_mm3: solid.volume(), slivers_cleaned: slivers };
    Ok((solid, report, c))
}

// --- Leaves and buds on the table ------------------------------------------------------------------------------------

/// A closed solid from a top sheet and a bottom sheet over the same (x, v) grid, sharing their v = -1 and v = 1 rows,
/// closed at each end by a point: `top` and `bottom` give the sheets' points, `ends` the two end points.
fn pillow(nx: usize, ny: usize, top: &dyn Fn(f64, f64) -> P3, bottom: &dyn Fn(f64, f64) -> P3, ends: [P3; 2]) -> csg::Solid {
    let mut s = csg::Solid::default();
    let cols: Vec<f64> = (1..nx).map(|i| i as f64 / nx as f64).collect();
    let vs: Vec<f64> = (0..=2 * ny).map(|j| -1.0 + j as f64 / ny as f64).collect();
    let rows = vs.len();
    // Top sheet: every v; bottom sheet: the inner v only, its rims taken from the top's.
    let mut top_id = vec![vec![0u32; rows]; cols.len()];
    let mut bot_id = vec![vec![0u32; rows]; cols.len()];
    for (i, &u) in cols.iter().enumerate() {
        for (j, &v) in vs.iter().enumerate() {
            top_id[i][j] = s.v.len() as u32;
            s.v.push(top(u, v));
        }
        for (j, &v) in vs.iter().enumerate() {
            bot_id[i][j] = if j == 0 || j == rows - 1 {
                top_id[i][j]
            } else {
                s.v.push(bottom(u, v));
                (s.v.len() - 1) as u32
            };
        }
    }
    let (a, b) = (s.v.len() as u32, s.v.len() as u32 + 1);
    s.v.push(ends[0]);
    s.v.push(ends[1]);
    for i in 0..cols.len() - 1 {
        for j in 0..rows - 1 {
            s.f.push([top_id[i][j], top_id[i + 1][j], top_id[i + 1][j + 1]]);
            s.f.push([top_id[i][j], top_id[i + 1][j + 1], top_id[i][j + 1]]);
            s.f.push([bot_id[i][j], bot_id[i + 1][j + 1], bot_id[i + 1][j]]);
            s.f.push([bot_id[i][j], bot_id[i][j + 1], bot_id[i + 1][j + 1]]);
        }
    }
    let last = cols.len() - 1;
    for j in 0..rows - 1 {
        s.f.push([a, top_id[0][j], top_id[0][j + 1]]);
        s.f.push([a, bot_id[0][j + 1], bot_id[0][j]]);
        s.f.push([b, top_id[last][j + 1], top_id[last][j]]);
        s.f.push([b, bot_id[last][j], bot_id[last][j + 1]]);
    }
    if s.volume() < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// A datura leaf lying on the table, stalk at x = 0 and point at x = `len`, in its own frame (z up off the table): a
/// sinuate blade whose three big teeth a side lean toward the point, domed higher on the midrib and falling to a
/// crisp margin, a raised midrib and a vein from it into every tooth; its underside sunk into the table.
fn leaf(len: f64, wid: f64, twist: f64, edge: f64, grid: (usize, usize)) -> csg::Solid {
    let teeth = 2.6;
    // Ovate, widest a little behind the middle, narrowing to an acute point.
    let env = |u: f64| { let u = u.clamp(0.0, 1.0); (u.powf(0.55) * (1.0 - u).powf(0.85) / 0.4865).min(1.0) };
    // Each side's sinuate lobes: broad, angular at their crests, irregular in size, the sides out of step; no saw
    // teeth.
    let tooth = move |u: f64, side: f64| {
        let k = u * teeth + if side > 0.0 { 0.1 } else { 0.55 } + twist;
        let f = k - k.floor();
        let size = 0.75 + 0.25 * (2.3 * k.floor() + 1.7 * side).sin().abs();
        let fade = smooth(0.08, 0.25, u) * (1.0 - smooth(0.82, 0.97, u));
        1.0 - fade * 0.42 * size * (2.0 * f - 1.0).abs().powf(1.05)
    };
    let half = move |u: f64, v: f64| 0.5 * wid * env(u) * tooth(u, v.signum());
    // Vein stations: where each tooth's point lies, and where its vein leaves the midrib.
    let veins: Vec<(f64, f64, f64)> = (0..6)
        .map(|k| {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let off = if side > 0.0 { 0.1 } else { 0.55 } + twist;
            let n = (k / 2) as f64 + 1.0;
            let tip = (n - off + 0.5) / teeth;
            (tip, (tip - 0.16).max(0.04), side)
        })
        .filter(|(tip, _, _)| *tip > 0.15 && *tip < 0.92)
        .collect();
    let height = move |u: f64, v: f64| {
        let w = half(u, v).max(1e-6);
        let (x, y) = (u * len, v * w);
        // The dome follows the blade's untoothed envelope, so the teeth cut its margin without creasing it.
        let e = (y / (0.5 * wid * env(u)).max(1e-6)).clamp(-1.0, 1.0);
        let dome = 0.34 * (1.0 - e * e).max(0.0).powf(0.7) * env(u).powf(0.4);
        let midrib = 0.2 * (-(y / 0.22).powi(2)).exp() * (1.0 - 0.7 * u) * smooth(0.0, 0.08, u);
        let mut vein: f64 = 0.0;
        for &(tip, from, side) in &veins {
            let (a, b) = ([from * len, 0.0], [tip * len, side * half(tip, side)]);
            let d = [b[0] - a[0], b[1] - a[1]];
            let t = (((x - a[0]) * d[0] + (y - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1])).clamp(0.0, 1.0);
            let dist = (x - a[0] - d[0] * t).hypot(y - a[1] - d[1] * t);
            vein = vein.max(0.1 * (-(dist / 0.32).powi(2)).exp() * (1.0 - 0.6 * t));
        }
        0.16 + (dome + midrib + vein * (1.0 - v.abs().powi(4)))
    };
    // Past the table's edge the blade curls down over it.
    let droop = move |x: f64| -0.4 * (x - edge).max(0.0).powi(2);
    // The blade cups a little up at its margins, so it is never a flat plate.
    let wave = move |u: f64, v: f64| 0.16 * v * v * (PI * u).sin();
    let top = move |u: f64, v: f64| [u * len, v * half(u, v), height(u, v) + droop(u * len) + wave(u, v)];
    let bottom = move |u: f64, v: f64| [u * len, v * half(u, v), -0.35 + 0.51 * smooth(0.82, 1.0, v.abs()) + droop(u * len) + wave(u, v)];
    pillow(grid.0, grid.1, &top, &bottom, [[-0.05, 0.0, -0.1], [len + 0.04, 0.0, 0.16 + droop(len + 0.04)]])
}

/// The open datura flower lying on the table from its stalk at x = 0: a five-ribbed calyx tube, the corolla's
/// throat, then a trumpet flaring to a pleated mouth sharpened at its rim into five points. The bell is a
/// shell `wall` thick open at the mouth; past `lift_from` along its length the flower bends up off the table, so the
/// mouth looks up and out.
fn trumpet(len: f64, mouth_r: f64, girth: f64, wall: f64, lift_from: f64, lift_deg: f64) -> csg::Solid {
    let (no, nf) = (70usize, 70usize);
    let throat = 0.6;
    // The outer meridian at bearing phi: (along, radius) for the share s of its own length.
    let outer = |s: f64, phi: f64| -> [f64; 2] {
        let point = (0.5 + 0.5 * (5.0 * phi).cos()).powi(3);
        let base = if s < 0.06 {
            girth * 0.5 * (s / 0.06).sqrt().max(0.05)
        } else if s < 0.38 {
            girth * (0.5 + 0.42 * smooth(0.06, 0.32, s))
        } else if s < throat {
            girth * (0.92 + 0.22 * smooth(0.38, throat, s))
        } else {
            let t = ((s - throat) / (1.0 - throat)).min(1.15);
            girth * 1.14 + (mouth_r - girth * 1.14) * t.powf(2.1)
        };
        // Five ribs down the tube, deepening to the mouth's pleats, which sharpen at the rim into five points.
        let ribs = 0.07 + 0.06 * smooth(throat, 1.0, s);
        let star = 0.3 * smooth(0.82, 1.0, s) * (point - 0.3);
        // A raised rib down the tube into each point.
        let ridge = RIB_MM * (0.5 + 0.5 * (5.0 * phi).cos()).powi(10) * smooth(0.08, 0.3, s);
        [s * len, base * (1.0 + ribs * (5.0 * phi).cos() + star) + ridge]
    };
    let mut sol = csg::Solid::default();
    sol.v.push([-0.06, 0.0, 0.0]);
    let inner_rows = 26usize;
    let rows = no + 1 + inner_rows;
    for j in 0..nf {
        let phi = 2.0 * PI * j as f64 / nf as f64;
        let line: Vec<[f64; 2]> = (1..=no).map(|k| outer(k as f64 / no as f64, phi)).collect();
        // The lip: half a turn of radius wall/2 over the rim.
        let rim = line[no - 1];
        let d = sub3([line[no - 1][0], line[no - 1][1], 0.0], [line[no - 3][0], line[no - 3][1], 0.0]);
        let tan = unit3(d);
        let inward = [tan[1], -tan[0]];
        let lip = [rim[0] + tan[0] * 0.5 * wall + inward[0] * 0.5 * wall, rim[1] + tan[1] * 0.5 * wall + inward[1] * 0.5 * wall];
        // The inside: the outer meridian offset in by the wall, back from the rim to the throat.
        let inner: Vec<[f64; 2]> = (0..inner_rows)
            .map(|k| {
                let s = 1.0 - (1.0 - throat) * k as f64 / (inner_rows - 1) as f64;
                let (a, b) = (outer(s, phi), outer((s - 0.004).max(0.0), phi));
                let t = unit3([a[0] - b[0], a[1] - b[1], 0.0]);
                [a[0] + t[1] * wall, (a[1] - t[0] * wall).max(0.05)]
            })
            .collect();
        let mut all = line;
        all.push(lip);
        all.extend(inner);
        if std::env::var("DATURA_DEBUG").is_ok() && j % 8 == 0 {
            let o = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
            for a in 0..all.len() - 1 {
                for b in a + 2..all.len() - 1 {
                    let (p1, p2, q1, q2) = (all[a], all[a + 1], all[b], all[b + 1]);
                    if o(p1, p2, q1) * o(p1, p2, q2) < 0.0 && o(q1, q2, p1) * o(q1, q2, p2) < 0.0 {
                        eprintln!("flower j {j}: rows {a} and {b} cross at {p1:?} {q1:?}");
                    }
                }
            }
        }
        for [x, r] in all {
            sol.v.push([x, r * phi.cos(), r * phi.sin()]);
        }
    }
    let x_end = outer(throat, 0.0)[0] + wall;
    sol.v.push([x_end, 0.0, 0.0]);
    let last = (sol.v.len() - 1) as u32;
    let at = |j: usize, k: usize| (1 + (j % nf) * rows + k) as u32;
    for j in 0..nf {
        sol.f.push([0, at(j + 1, 0), at(j, 0)]);
        sol.f.push([last, at(j, rows - 1), at(j + 1, rows - 1)]);
        for k in 0..rows - 1 {
            sol.f.push([at(j, k), at(j + 1, k), at(j + 1, k + 1)]);
            sol.f.push([at(j, k), at(j + 1, k + 1), at(j, k + 1)]);
        }
    }
    // A short tooth at each of the mouth's five points, curling back from its rim.
    let mut pieces = vec![sol];
    for k in 0..5 {
        let phi = 2.0 * PI * k as f64 / 5.0;
        let (a, b) = (outer(1.0, phi), outer(0.99, phi));
        let tan = unit3([a[0] - b[0], a[1] - b[1], 0.0]);
        let back = [-tan[1], tan[0]];
        let path: Vec<[f64; 2]> = (0..=8)
            .map(|i| {
                let t = i as f64 / 8.0;
                let ang = 1.9 * t;
                let r = TOOTH_MM / 1.9;
                [a[0] - tan[0] * 0.35 + r * (tan[0] * ang.sin() + back[0] * (1.0 - ang.cos())) + tan[0] * 0.35 * (1.0 - t), a[1] - tan[1] * 0.35 + r * (tan[1] * ang.sin() + back[1] * (1.0 - ang.cos())) + tan[1] * 0.35 * (1.0 - t)]
            })
            .collect();
        let pts: Vec<P3> = path.iter().map(|q| [q[0], q[1] * phi.cos(), q[1] * phi.sin()]).collect();
        pieces.push(tube(&pts, 0.46, 0.4));
    }
    let mut sol = csg::union_all(&pieces).expect("the flower's teeth join it");
    csg::clean(&mut sol, 2e-5);
    // Lying on the table: its axis a little over it, then bent about a centre over the table (up) or under it (down).
    let bend = lift_deg.to_radians() / (len - lift_from).max(1.0);
    let lie = 0.62 * girth;
    // Tipped up about its stalk so the bell's underside comes down onto the table.
    let tip = ((mouth_r + MOUTH_LIFT_MM - lie) / len).atan();
    for p in &mut sol.v {
        let (x, y, z) = (p[0], p[1], p[2]);
        let (x, z) = (x * tip.cos() - z * tip.sin(), x * tip.sin() + z * tip.cos() + lie);
        *p = if x <= lift_from || bend == 0.0 {
            [x, y, z]
        } else {
            let radius = 1.0 / bend;
            let a = (x - lift_from) * bend;
            [lift_from + (radius - z) * a.sin(), y, radius - (radius - z) * a.cos()]
        };
    }
    if sol.volume() < 0.0 {
        for f in &mut sol.f {
            f.swap(1, 2);
        }
    }
    sol
}

/// A closed tube along `pts`, its radius tapering from `r0` to `r1`, its far end rounded to a point.
fn tube(pts: &[P3], r0: f64, r1: f64) -> csg::Solid {
    let n = pts.len();
    let radii: Vec<f64> = (0..n).map(|i| r0 + (r1 - r0) * i as f64 / (n - 1) as f64).collect();
    tube_with(pts, &radii, 10)
}

/// A closed tube along `pts` with its own radius at each point, `around` facets round, both ends rounded to a point.
fn tube_with(pts: &[P3], radii: &[f64], around: usize) -> csg::Solid {
    let n = pts.len();
    let (r0, r1) = (radii[0], radii[n - 1]);
    let mut s = csg::Solid::default();
    s.v.push(add3(pts[0], unit3(sub3(pts[0], pts[1])), r0));
    let mut normal = {
        let t = unit3(sub3(pts[1], pts[0]));
        let a = if t[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        unit3(cross3(t, a))
    };
    for i in 0..n {
        let t = unit3(sub3(pts[(i + 1).min(n - 1)], pts[i.saturating_sub(1)]));
        normal = unit3(sub3(normal, t.map(|v| v * dot3(normal, t))));
        let bi = cross3(t, normal);
        let r = radii[i];
        for j in 0..around {
            let a = 2.0 * PI * j as f64 / around as f64;
            s.v.push(add3(add3(pts[i], normal, r * a.cos()), bi, r * a.sin()));
        }
    }
    s.v.push(add3(pts[n - 1], unit3(sub3(pts[n - 1], pts[n - 2])), r1));
    let last = (s.v.len() - 1) as u32;
    let at = |i: usize, j: usize| (1 + i * around + j % around) as u32;
    for j in 0..around {
        s.f.push([0, at(0, j + 1), at(0, j)]);
        s.f.push([last, at(n - 1, j), at(n - 1, j + 1)]);
    }
    for i in 0..n - 1 {
        for j in 0..around {
            s.f.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
            s.f.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
        }
    }
    if s.volume() < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// The plant's stem from under the capsule over the badge's right edge, down the shoulder and round the palm: a cane
/// standing `STEM_PROUD_MM` proud of the skin, riding a smoothed upper envelope so it bridges the step under the head,
/// tapering and wandering, swelling softly at its two leaf nodes; and the tendril curled off its end, a part of its own. Also the bearings and across-offsets it
/// passes, for the leaves that hang from it.
fn stem(skin: &Skin, top: f64) -> Result<(csg::Solid, csg::Solid, Vec<(f64, f64, f64)>)> {
    let r = STEM_W_MM / 2.0;
    let (from, to) = (top.atan2(CAPSULE_AT[0]).to_degrees(), STEM_END_DEG);
    let n = ((from - to) / 1.0).ceil() as usize;
    let thetas: Vec<f64> = (0..=n).map(|k| from + (to - from) * k as f64 / n as f64).collect();
    // Across the finger: from under the capsule to the shank's middle, then wandering half a millimetre either way.
    let across = |t: f64| -CAPSULE_AT[1] * smooth(32.0, 68.0, t) + 0.5 * (1.0 - smooth(20.0, 40.0, t)) * (t.to_radians() * 3.0).sin();
    let surf: Vec<f64> = thetas.iter().map(|&t| skin.radius(t.to_radians(), across(t)).unwrap_or(0.0)).collect();
    ensure!(surf.iter().all(|r| *r > 1.0), "The stem leaves the band");
    let w = 4;
    let env: Vec<f64> = (0..surf.len()).map(|i| surf[i.saturating_sub(w)..(i + w + 1).min(surf.len())].iter().copied().fold(0.0, f64::max)).collect();
    let smooth_env: Vec<f64> = (0..env.len()).map(|i| { let s = &env[i.saturating_sub(w)..(i + w + 1).min(env.len())]; s.iter().sum::<f64>() / s.len() as f64 }).collect();
    let mut pts = Vec::new();
    let mut radii = Vec::new();
    let mut path = Vec::new();
    for (k, &t) in thetas.iter().enumerate() {
        let rc = smooth_env[k].max(surf[k]) + STEM_PROUD_MM - r;
        let a = t.to_radians();
        pts.push([rc * a.cos(), rc * a.sin(), across(t)]);
        // Soft swellings at the two leaf nodes, a taper from 2.2 to 1.6 mm toward the palm, and a rounded end.
        let node: f64 = STEM_NODES_DEG.iter().map(|nd| 0.16 * (-((t - nd) / 6.0).powi(2)).exp()).sum();
        let taper = 1.0 - (0.3 / 1.1) * ((from - t) / (from - to)).clamp(0.0, 1.0);
        let end = 0.15 * smooth(to + 5.0, to, t);
        radii.push(r * taper + node + end);
        path.push((t, across(t), rc + r));
    }
    let mut parts = vec![tube_with(&pts, &radii, 10)];
    // The tendril: a curl off the end node, lying on the skin round the palm.
    let e = pts[pts.len() - 1];
    let a = to.to_radians();
    let out = [a.cos(), a.sin(), 0.0];
    let along = unit3(sub3(e, pts[pts.len() - 3]));
    let side = unit3(cross3(out, along));
    let curl = |jitter: f64| -> Vec<P3> {
        (0..=40)
            .map(|k| {
                let u = k as f64 / 40.0;
                let ang = 1.45 * PI * u;
                let rho = 1.5 * (1.0 - 0.4 * u);
                add3(add3(add3(e, along, 0.9 + jitter + rho * ang.sin()), side, rho * (1.0 - ang.cos())), out, -0.25 + 0.5 * jitter)
            })
            .collect()
    };
    let tendril = tube(&curl(0.0), 0.45, 0.4);
    let solid = parts.remove(0);
    Ok((solid, tendril, path))
}

/// A datura leaf's plan, stalk at x = 0 and point at x = `len`: its outline (counter-clockwise) and the paired lateral
/// veins from the midrib out into its lobes.
fn leaf_plan(len: f64, wid: f64, twist: f64) -> (Vec<[f64; 2]>, Vec<([f64; 2], [f64; 2])>) {
    let teeth = 2.6;
    let env = |u: f64| { let u = u.clamp(0.0, 1.0); (u.powf(0.55) * (1.0 - u).powf(0.85) / 0.4865).min(1.0) };
    let tooth = |u: f64, side: f64| {
        let k = u * teeth + if side > 0.0 { 0.1 } else { 0.55 } + twist;
        let f = k - k.floor();
        let size = 0.75 + 0.25 * (2.3 * k.floor() + 1.7 * side).sin().abs();
        let fade = smooth(0.08, 0.25, u) * (1.0 - smooth(0.82, 0.97, u));
        1.0 - fade * 0.42 * size * (2.0 * f - 1.0).abs().powf(1.05)
    };
    // A short blunt stalk at the base, never a zero-width spike.
    let half = |u: f64, side: f64| (0.5 * wid * env(u) * tooth(u, side)).max(0.2 * (1.0 - smooth(0.04, 0.12, u)));
    let n = 110;
    let mut outline: Vec<[f64; 2]> = vec![[-0.16, 0.0]];
    outline.extend((0..n).map(|i| i as f64 / n as f64).map(|u| [u * len, -half(u, -1.0)]).filter(|p| p[1].abs() > 0.02 || p[0] < 0.5 * len));
    outline.push([len, 0.0]);
    outline.extend((0..n).rev().map(|i| i as f64 / n as f64).map(|u| [u * len, half(u, 1.0)]).filter(|p| p[1].abs() > 0.02 || p[0] < 0.5 * len));
    let veins = (0..6)
        .filter_map(|k| {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let off = if side > 0.0 { 0.1 } else { 0.55 } + twist;
            let tip = ((k / 2) as f64 + 1.0 - off + 0.5) / teeth;
            (tip > 0.18 && tip < 0.9).then(|| ([(tip - 0.17).max(0.06) * len, 0.0], [tip * len, side * (half(tip, side) - 0.75).max(0.25)]))
        })
        .collect();
    (outline, veins)
}

/// A strip from `a` to `b`, `w0` wide at `a` narrowing to `w1` at `b`, its ends rounded: a vein or a midrib's plan,
/// centred on the strip's middle.
fn strip(a: [f64; 2], b: [f64; 2], w0: f64, w1: f64) -> ([f64; 2], f64, Vec<[f64; 2]>) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx.hypot(dy);
    let n = 14;
    let mut pts = Vec::new();
    for i in 0..=n {
        let t = i as f64 / n as f64;
        pts.push([t * l - 0.5 * l, -0.5 * (w0 + (w1 - w0) * t)]);
    }
    for k in 1..6 {
        let a = -PI / 2.0 + PI * k as f64 / 6.0;
        pts.push([0.5 * l + 0.5 * w1 * a.cos(), 0.5 * w1 * a.sin()]);
    }
    for i in (0..=n).rev() {
        let t = i as f64 / n as f64;
        pts.push([t * l - 0.5 * l, 0.5 * (w0 + (w1 - w0) * t)]);
    }
    for k in 1..6 {
        let a = PI / 2.0 + PI * k as f64 / 6.0;
        pts.push([-0.5 * l + 0.5 * w0 * a.cos(), 0.5 * w0 * a.sin()]);
    }
    ([0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])], dy.atan2(dx), pts)
}

/// A datura leaf struck as stamps: a pillowed blade on the sinuate outline, then a raised midrib and paired lateral
/// veins as a second tier on it. `place` gives each piece's world centre and world direction for its local x, from
/// a point and a direction in the leaf's plan.
fn leaf_stamps(name: &str, atlas: &skin::Atlas, len: f64, wid: f64, twist: f64, place: &dyn Fn([f64; 2], f64) -> (P3, P3)) -> Vec<Stamp> {
    let (outline, veins) = leaf_plan(len, wid, twist);
    let stamp = |label: String, at: [f64; 2], angle: f64, pts: Vec<[f64; 2]>, tier: u8, height: f64, crown: f64| {
        let (p, dir) = place(at, angle);
        let (theta, v, rot) = chart_at(atlas, p, dir);
        Stamp { name: label, theta_deg: theta, v_mm: v, rot_deg: rot, outline: pts, height_mm: height, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, fine_cap: true, tier, top: StampTop::Pillow { crown_mm: crown } }
    };
    let centre = [0.5 * len, 0.0];
    let mut out = vec![stamp(name.to_string(), centre, 0.0, outline.iter().map(|p| [p[0] - centre[0], p[1]]).collect(), 0, 0.22, 0.42)];
    let (c, a, pts) = strip([0.06 * len, 0.0], [0.86 * len, 0.0], 0.42, 0.12);
    out.push(stamp(format!("{name}, midrib"), c, a, pts, 1, 0.14, 0.1));
    for (k, (from, to)) in veins.into_iter().enumerate() {
        let (c, a, pts) = strip(from, to, 0.26, 0.12);
        out.push(stamp(format!("{name}, vein {}", k + 1), c, a, pts, 1, 0.09, 0.06));
    }
    out
}

/// The stamp chart's own place for a world point: the nearest sample of the bare skin's atlas, its `(theta, v)`, and
/// the turn that sets a stamp's x along world direction `along` there (degrees from the ring's tangent, about the
/// outward normal).
fn chart_at(a: &skin::Atlas, p: P3, along: P3) -> (f64, f64, f64) {
    let s = a.samples.iter().filter(|s| s.n != [0.0; 3]).min_by(|x, y| dot3(sub3(x.p, p), sub3(x.p, p)).total_cmp(&dot3(sub3(y.p, p), sub3(y.p, p)))).expect("an atlas sample");
    let th = s.theta.to_radians();
    let n = s.n;
    let tangent = unit3(sub3([-th.sin(), th.cos(), 0.0], n.map(|v| v * dot3(n, [-th.sin(), th.cos(), 0.0]))));
    let side = cross3(n, tangent);
    let rot = dot3(along, side).atan2(dot3(along, tangent)).to_degrees();
    (s.theta, s.v, rot)
}

/// A solid in its own frame turned `angle` about the capsule's axis and set `from` out from it.
fn laid(s: &csg::Solid, angle: f64, from: f64) -> csg::Solid {
    let (c, si) = (angle.cos(), angle.sin());
    csg::Solid { v: s.v.iter().map(|p| { let x = p[0] + from; [x * c - p[1] * si, x * si + p[1] * c, p[2]] }).collect(), f: s.f.clone() }
}

#[derive(Clone, Debug, serde::Serialize)]
struct Laid {
    name: String,
    bearing_deg: f64,
    from_mm: f64,
    length_mm: f64,
    width_mm: f64,
    triangles: usize,
}

/// Whether a capsule-frame point on the table lies on the badge's table, `inset` in from its edge.
fn on_table(skin: &Skin, top: f64, x: f64, y: f64, inset: f64) -> bool {
    (0..8).all(|k| {
        let a = PI * k as f64 / 4.0;
        let (px, py) = (x + inset * a.cos(), y + inset * a.sin());
        let theta = top.atan2(px);
        skin.radius(theta, -py).is_some_and(|r| (r * theta.sin() - top).abs() < 0.02)
    })
}

/// The longest run out from `origin` on the table along `bearing`, from `from`, that stays on the table `inset` in.
fn table_run_from(skin: &Skin, top: f64, origin: [f64; 2], bearing: f64, from: f64, inset: f64) -> f64 {
    let mut run = 0.0;
    while run < 20.0 && on_table(skin, top, origin[0] + (from + run + 0.1) * bearing.cos(), origin[1] + (from + run + 0.1) * bearing.sin(), inset) {
        run += 0.1;
    }
    run
}

/// Capsule frame to the world: z up the table's normal at the top of the ring, x round the ring, y along the finger.
fn to_world(p: P3, top: f64) -> P3 {
    [p[0], top + p[2], -p[1]]
}
fn dir_to_world(v: P3) -> P3 {
    [v[0], v[2], -v[1]]
}

/// Capsule frame to the frame `Placement::Ring` seats the capsule by at the table's centre: there x runs along the
/// finger (world -z), y round the ring backward (world -x) and z out of the table.
fn to_part(p: P3) -> P3 {
    [p[1], -p[0], p[2]]
}

fn in_part(s: &csg::Solid) -> csg::Solid {
    // A quarter turn about z, so the winding holds.
    csg::Solid { v: s.v.iter().map(|&p| to_part(p)).collect(), f: s.f.clone() }
}

/// The turns `Placement::Relative` takes (tilt about x, cant about y, no spin) to stand a part's z along `n`.
fn leans_to(n: P3) -> [f64; 3] {
    [(-n[1]).clamp(-1.0, 1.0).asin().to_degrees(), n[0].atan2(n[2]).to_degrees(), 0.0]
}

fn stored_op(solid: &csg::Solid, op: &str, params: Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_datura".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}

fn spinel() -> Gem {
    let mut g = Gem::calibrated(GemCut::Round, SEED_MM);
    g.preview_tint = Some(SPINEL_TINT);
    g
}

#[derive(Debug, serde::Serialize)]
struct SeedReport {
    name: String,
    world: P3,
    normal: P3,
    pad_face: u32,
    pad_mm: [f64; 2],
}

/// The CAD parts: the capsule joined to the badge, and the eight seeds with their seats cut into the splits.
fn parts(d: &mut RingDesign, lib: &AlphaLibrary, t: Table, blockout: bool) -> Result<(CapsuleReport, Vec<SeedReport>, Vec<Laid>)> {
    let (local, report, c) = capsule(SHAPE, blockout)?;
    let mut doc = Document::default();
    doc.append(feature(1, "Badge", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() }))?;
    let mut bare = d.clone();
    bare.cad = None;
    let surface = mesh::try_build(&bare, lib, coarse_params())?.mesh;
    let skin = Skin::of(&surface, (20.0, 160.0), (-12.5, 12.5));
    if std::env::var("DATURA_DEBUG").is_ok() {
        for y in (-11..=11).rev() {
            let row: String = (-10..=10).map(|x| if on_table(&skin, t.top_mm, x as f64, y as f64, 1.2) { '#' } else if on_table(&skin, t.top_mm, x as f64, y as f64, 0.0) { '+' } else { '.' }).collect();
            eprintln!("table y {y:3} {row}");
        }
        for k in 0..24 {
            let b = 2.0 * PI * k as f64 / 24.0;
            eprintln!("bearing {:5.1}: run {:.2}", b.to_degrees(), table_run_from(&skin, t.top_mm, CAPSULE_AT, b, 0.0, 0.0));
        }
    }
    let mut id: Id = 2;
    let mut laid_out = Vec::new();
    let joined = |blend: f64| Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, blend_mm: blend, ..Component::default() };
    // The capsule stands off to one side of the table; the parts on the table stand in the frame the capsule's seat
    // gives at the table's centre, so the whole head follows a resize.
    let shift = |p: P3| [p[0] + CAPSULE_AT[0], p[1] + CAPSULE_AT[1], p[2]];
    let moved = |s: &csg::Solid| csg::Solid { v: s.v.iter().map(|&p| shift(p)).collect(), f: s.f.clone() };
    let capsule_id = id;
    doc.append(feature(
        capsule_id,
        "Thorn-apple capsule",
        stored_op(&in_part(&moved(&local)), "capsule", json!({ "shape": SHAPE, "at_mm": CAPSULE_AT, "rows": report.rows }))?,
        Component { placement: Placement::ring(90.0, 0.0), ..joined(0.0) },
    ))?;
    id += 1;
    let on_capsule = |at: P3, n: P3| Placement::Relative { part: capsule_id, at: to_part(at), rotation_deg: leans_to(to_part(n)) };
    let flat = on_capsule([0.0; 3], [0.0, 0.0, 1.0]);
    let at_stalk = |s: &csg::Solid| csg::Solid { v: s.v.iter().map(|p| [p[0] + FLOWER_FROM[0], p[1] + FLOWER_FROM[1], p[2]]).collect(), f: s.f.clone() };
    // The trumpet flower lies corner to corner across the table, its bell resting on the table and its mouth
    // flaring just past the badge's edge on the hero's side, so the hero sees it three-quarter on.
    {
        let bearing = FLOWER_BEARING_DEG.to_radians();
        let run = table_run_from(&skin, t.top_mm, FLOWER_FROM, bearing, 0.0, 0.0);
        let (len, mouth, girth) = (FLOWER_LEN_MM, FLOWER_MOUTH_MM, 1.25);
        let lift_from = (run - 0.8).min(len - 5.0);
        let solid = at_stalk(&laid(&trumpet(len, 0.5 * mouth, girth, 0.85, lift_from, FLOWER_BEND_DEG), bearing, 0.0));
        laid_out.push(Laid { name: "Datura flower: table run".into(), bearing_deg: FLOWER_BEARING_DEG, from_mm: 0.0, length_mm: run, width_mm: 0.0, triangles: 0 });
        // How far the mouth stands past the table's outline along the flower, and the curled teeth's section.
        laid_out.push(Laid { name: "Datura flower: overhang past the table".into(), bearing_deg: FLOWER_BEARING_DEG, from_mm: run, length_mm: (len - run).max(0.0), width_mm: 0.0, triangles: 0 });
        laid_out.push(Laid { name: "Datura flower: tooth section".into(), bearing_deg: 0.0, from_mm: 0.0, length_mm: TOOTH_MM, width_mm: 0.8, triangles: 0 });
        ensure!(solid.open_edges() == (0, 0), "The flower does not close");
        let crossings = csg::self_crossings(&solid);
        ensure!(crossings == 0, "The flower crosses itself {crossings} times");
        let name = "Datura flower".to_string();
        laid_out.push(Laid { name: name.clone(), bearing_deg: FLOWER_BEARING_DEG, from_mm: 0.0, length_mm: len, width_mm: mouth, triangles: solid.f.len() });
        doc.append(feature(id, &name, stored_op(&in_part(&solid), "flower", json!({ "bearing_deg": FLOWER_BEARING_DEG, "length_mm": len, "mouth_mm": mouth, "girth": girth, "wall_mm": 0.85, "bend_from_mm": lift_from, "bend_deg": FLOWER_BEND_DEG }))?, Component { placement: flat.clone(), ..joined(0.0) }))?;
        id += 1;
    }
    // Four datura leaves struck on the table under and round the flower and the capsule, at uneven angles, each kept
    // inside the table's edge.
    let mut bare_d = d.clone();
    bare_d.cad = None;
    let atlas = skin::Atlas::of(&bare_d, 2048, 384)?;
    let mut stamps = Vec::new();
    for (k, (at, deg, want)) in LEAVES.into_iter().enumerate() {
        let bearing = deg.to_radians();
        let (c, sn) = (bearing.cos(), bearing.sin());
        let to_table = |q: [f64; 2]| [at[0] + q[0] * c - q[1] * sn, at[1] + q[0] * sn + q[1] * c];
        let mut len = want;
        while len > 3.0 && !leaf_plan(len, (0.66 * len).min(7.0), 0.21 * k as f64).0.iter().all(|q| { let p = to_table(*q); on_table(&skin, t.top_mm, p[0], p[1], 0.3) }) {
            len -= 0.2;
        }
        let wid = (0.66 * len).min(7.0);
        let place = |q: [f64; 2], angle: f64| {
            let p = to_table(q);
            let a = bearing + angle;
            ([p[0], t.top_mm, -p[1]], [a.cos(), 0.0, -a.sin()])
        };
        let name = format!("Datura leaf {}", k + 1);
        stamps.extend(leaf_stamps(&name, &atlas, len, wid, 0.21 * k as f64, &place));
        laid_out.push(Laid { name, bearing_deg: deg, from_mm: 0.0, length_mm: len, width_mm: wid, triangles: 0 });
    }
    // The stem down the right shoulder and round the palm, and the two small leaves that hang from it.
    let shank_skin = Skin::of(&surface, (190.0, 440.0), (-12.5, 12.5));
    {
        let (solid, tendril, path) = stem(&shank_skin, t.top_mm)?;
        ensure!(solid.open_edges() == (0, 0) && csg::self_crossings(&solid) == 0, "The stem does not close cleanly");
        ensure!(tendril.open_edges() == (0, 0) && csg::self_crossings(&tendril) == 0, "The tendril does not close cleanly");
        doc.append(feature(id, "Datura tendril", stored_op(&tendril, "tendril", json!({ "turns": 0.725, "wire_mm": 0.9 }))?, joined(0.0)))?;
        id += 1;
        laid_out.push(Laid { name: "Datura stem".into(), bearing_deg: STEM_END_DEG, from_mm: 0.0, length_mm: path.windows(2).map(|w| ((w[1].2 * w[1].0.to_radians() - w[0].2 * w[0].0.to_radians()).abs()).hypot(w[1].1 - w[0].1)).sum(), width_mm: STEM_W_MM, triangles: solid.f.len() });
        doc.append(feature(id, "Datura stem", stored_op(&solid, "stem", json!({ "width_mm": STEM_W_MM, "proud_mm": STEM_PROUD_MM, "end_deg": STEM_END_DEG, "nodes_deg": STEM_NODES_DEG }))?, joined(0.0)))?;
        id += 1;
        for (k, (theta, across, turn)) in SHANK_LEAVES.into_iter().enumerate() {
            // Laid on the shank's skin: its plan carried round the ring by arc and along the finger.
            let r_ref = shank_skin.radius(theta.to_radians(), across).unwrap_or(12.0);
            let (c, sn) = (turn.to_radians().cos(), turn.to_radians().sin());
            let place = |q: [f64; 2], angle: f64| {
                let (arc, off) = (q[0] * c - q[1] * sn, q[0] * sn + q[1] * c);
                let th = theta.to_radians() - arc / r_ref;
                let z = across + off;
                let r = shank_skin.radius(th, z).unwrap_or(r_ref);
                let a = turn.to_radians() + angle;
                let (da, dz) = (a.cos(), a.sin());
                ([r * th.cos(), r * th.sin(), z], unit3([da * th.sin(), -da * th.cos(), dz]))
            };
            let name = format!("Datura shank leaf {}", k + 1);
            stamps.extend(leaf_stamps(&name, &atlas, SHANK_LEAF_MM.0, SHANK_LEAF_MM.1, 0.4 + 0.3 * k as f64, &place));
            laid_out.push(Laid { name, bearing_deg: theta, from_mm: across, length_mm: SHANK_LEAF_MM.0, width_mm: SHANK_LEAF_MM.1, triangles: 0 });
        }
    }
    if let Ok(keep) = std::env::var("DATURA_STAMPS") {
        stamps.retain(|s| s.name.contains(keep.as_str()));
    }
    d.stamps = stamps;
    // Four seeds in the capsule's cracked crown, one in each split, each on a pad sunk in its split's floor, where the bur opens its seat and
    // three thorn claws stand.
    let mut places: Vec<(P3, P3, f64, f64)> = seed_places(&c).into_iter().map(|(p, n, phi)| (shift(p), n, phi, PAD_PROUD_MM)).collect();
    // The rest spilled on the table, one against the capsule's foot and three beyond, on pads.
    for (k, [x, y]) in SPILLED.into_iter().enumerate() {
        ensure!(on_table(&skin, t.top_mm, x, y, 1.2), "Spilled seed {} is off the table", k + 1);
        places.push(([x, y, 0.0], [0.0, 0.0, 1.0], 0.0, SPILLED_PAD_PROUD_MM));
    }
    let mut pads = Vec::new();
    for (k, (p, n, _, proud)) in places.iter().enumerate() {
        // The kernel's cylinder is centred on its origin.
        let top = add3(*p, *n, *proud);
        let origin = add3(top, *n, -0.5 * PAD_H_MM);
        doc.append(feature(
            id,
            &format!("Seed pad {}", k + 1),
            Operation::Cylinder { radius_mm: PAD_R_MM, height_mm: PAD_H_MM },
            Component { placement: on_capsule(origin, *n), ..joined(0.0) },
        ))?;
        pads.push(id);
        id += 1;
    }
    // The pads' top faces, read off a trial build.
    let mut trial = d.clone();
    trial.cad = Some(doc.clone());
    let probe = mesh::try_build(&trial, lib, coarse_params())?;
    let evaluated = probe.parts.evaluated.as_ref().context("no evaluated parts")?;
    let mut seeds = Vec::new();
    for (k, ((p, n, phi, proud), pad)) in places.into_iter().zip(pads).enumerate() {
        let c_pad = evaluated.components.iter().find(|c| c.id == pad).context("pad not built")?;
        let wn = unit3(dir_to_world(n));
        let mut best: Option<(f64, u32)> = None;
        for f in 0..c_pad.body.faces.len() as u32 {
            if c_pad.trace.face_kind.get(f as usize) != Some(&SurfaceKind::Plane) {
                continue;
            }
            let pts: Vec<P3> = c_pad.trace.tri_face.iter().enumerate().filter(|(_, ff)| **ff == f).flat_map(|(tri, _)| c_pad.mesh.faces[tri]).map(|i| { let v = c_pad.mesh.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] }).collect();
            if pts.is_empty() {
                continue;
            }
            let along = pts.iter().map(|q| dot3(*q, wn)).sum::<f64>() / pts.len() as f64;
            if best.is_none_or(|(b, _)| along > b) {
                best = Some((along, f));
            }
        }
        let (top_along, face) = best.context("pad has no planar top")?;
        if std::env::var("DATURA_DEBUG").is_ok() {
            let vs: Vec<P3> = c_pad.mesh.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
            let lo = vs.iter().map(|q| dot3(*q, wn)).fold(f64::MAX, f64::min);
            let centre = vs.iter().fold([0.0; 3], |a, q| add3(a, *q, 1.0 / vs.len() as f64));
            let want = to_world(add3(p, n, PAD_PROUD_MM), t.top_mm);
            eprintln!("pad {k}: face {face}, along n top {top_along:.3} bottom {lo:.3}, want top {:.3}; centre {:?} want top point {:?}; frame z {:?} n {:?}", dot3(want, wn), centre.map(|v| (v * 100.0).round() / 100.0), want.map(|v| (v * 100.0).round() / 100.0), c_pad.frame.z_axis.map(|v| (v * 1000.0).round() / 1000.0), wn.map(|v| (v * 1000.0).round() / 1000.0));
        }
        let seat = ringdesign_core::cad::FaceSeat::on(c_pad, face, None, SEED_OVER_PAD_MM)?;
        let mut params = builders::stone_params(spinel());
        seat.write(&mut params);
        let name = if k < IN_SPLIT.len() { format!("Seed {} (in the pod, {:.0} deg)", k + 1, phi.to_degrees()) } else { format!("Seed {} (spilled)", k + 1) };
        let stone_id = id;
        doc.append(Feature {
            id,
            name: name.clone(),
            enabled: true,
            operation: Operation::Builder { key: builders::STONE.into(), on: Some(pad), params },
            component: Component { stone_id: Some(name.clone()), role: ComponentRole::Stone, reference: true, ..Component::default() },
        })?;
        id += 1;
        doc.append(builders::feature_on(id, &format!("Seed seat {}", k + 1), builders::BUR, stone_id, json!({ "through": false })))?;
        id += 1;
        doc.append(builders::feature_on(
            id,
            &format!("Seed thorns {}", k + 1),
            builders::CLAW,
            stone_id,
            json!({ "prongs": 3, "wire_mm": 0.42, "rails": "None", "style": "Thorn", "tip": "Point" }),
        ))?;
        id += 1;
        let world = to_world(add3(p, n, proud + SEED_OVER_PAD_MM), t.top_mm);
        seeds.push(SeedReport { name, world, normal: wn, pad_face: face, pad_mm: [2.0 * PAD_R_MM, PAD_H_MM] });
    }
    d.cad = Some(doc);
    Ok((report, seeds, laid_out))
}

// --- Gates ---------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn shells(m: &mesh::Mesh) -> usize {
    let mut parent: Vec<usize> = (0..m.vertices.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for f in &m.faces {
        let a = root(&mut parent, f[0] as usize);
        for &j in &f[1..] {
            let b = root(&mut parent, j as usize);
            parent[b] = a;
        }
    }
    let mut roots = std::collections::BTreeSet::new();
    for f in &m.faces {
        roots.insert(root(&mut parent, f[0] as usize));
    }
    roots.len()
}

/// Every CAD part's self-crossings, as placed.
fn made_parts(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

/// The preview stones welded, and how many separate stones they make.
fn preview_stone_count(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> usize {
    ringdesign_core::gems::built_meshes(d, lib, built)
        .into_iter()
        .map(|(m, _)| {
            let mut out = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for f in m.faces {
                let face = f.map(|i| {
                    let p = m.vertices[i as usize];
                    let key = [p.0, p.1, p.2].map(|x| (x * 1e5).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        out.vertices.push(p);
                        out.normals.push(m.normals[i as usize]);
                        (out.vertices.len() - 1) as u32
                    })
                });
                out.faces.push(face);
            }
            shells(&out)
        })
        .sum()
}

/// Metal vertices inside each stone, its girdle eased 0.03 mm.
fn metal_in_stones(d: &RingDesign, m: &mesh::Mesh) -> Vec<(String, usize)> {
    ringdesign_core::stones::stone_frames(d)
        .into_iter()
        .map(|(st, f)| {
            let (ra, rb) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03);
            let (crown, pav) = (st.gem.crown_mm() - 0.03, st.gem.depth_mm() - st.gem.crown_mm() - 0.03);
            let n = m
                .vertices
                .iter()
                .filter(|p| {
                    let q = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
                    let (x, y, z) = (dot3(q, f.long), dot3(q, f.short), dot3(q, f.normal));
                    let e = (x / ra).powi(2) + (y / rb).powi(2);
                    (z > 0.0 && z < crown && e < (1.0 - z / crown).max(0.0)) || (z <= 0.0 && -z < pav && e < (1.0 + z / pav).max(0.0))
                })
                .count();
            (st.label, n)
        })
        .collect()
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool, mesh::BuildResult)> {
    let started = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    let made = made_parts(&built);
    let bore = d.inner_radius_mm();
    let margin = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64) - bore).fold(f64::MAX, f64::min);
    if std::env::var("DATURA_DEBUG").is_ok() {
        let worst = built.mesh.vertices.iter().min_by(|a, b| (a.0.hypot(a.1)).total_cmp(&b.0.hypot(b.1))).unwrap();
        eprintln!("deepest vertex {:?} at {:.1} deg", worst, (worst.1 as f64).atan2(worst.0 as f64).to_degrees());
    }
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = preview_stone_count(d, lib, &built);
    let inside = metal_in_stones(d, &built.mesh);
    let crowding: Vec<String> = stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect()).unwrap_or_default();
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().map(|w| format!("{}: {w}", seat.label)))).collect();
    warnings.dedup();
    let statuses: Vec<String> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .filter(|r| !matches!(r.status, ringdesign_core::cad::FeatureStatus::Ok))
        .map(|r| format!("#{}: {:?}", r.id, r.status))
        .collect();
    let castable = field.verdict == castability::Verdict::Castable;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && made.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && statuses.is_empty()
        && margin >= -0.01
        && castable
        && field.thinnest_wall_mm >= MIN_SECTION_MM
        && findings.is_empty()
        && reported == previewed
        && inside.iter().all(|(_, n)| *n == 0)
        && built.mesh.faces.len() <= 2_000_000;
    let g = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "min_angle_deg": q.min_angle_deg,
        "self_crossings": crossings,
        "shells": shells(&built.mesh),
        "made_parts": made,
        "feature_status_not_ok": statuses,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "parts_joined": built.parts.joined,
        "parts_cut": built.parts.cut,
        "stamps_struck": built.solids.stamped,
        "stamps": d.stamps.len(),
        "bore_margin_mm": margin,
        "field_verdict": field.verdict.label(),
        "field_process": field.process.label(),
        "field_notes": field.notes,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg,
        "undercut_percent_reported_only": field.undercut_fraction() * 100.0,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "metal_in_stones": inside,
        "stone_warnings": warnings,
        "crowding": crowding,
        "grams_18k": built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams),
        "volume_mm3": built.report.volume_mm3,
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders -------------------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger's axis, pitch toward it.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.3, 0.62),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.95, 0.5),
    ("reverse", PI - 0.6, 0.35),
];

fn save_rgb(path: &Path, rgb: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}


/// A copy of `m` for the camera with its normals averaged only across edges gentler than `crease_deg`, each face's
/// share weighted by its area: flat faces render flat beside the parts joined to them.
fn creased(m: &mesh::Mesh, crease_deg: f64) -> mesh::Mesh {
    let tri = |f: &[u32; 3]| f.map(|i| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] });
    let normals: Vec<P3> = m.faces.iter().map(|f| { let [a, b, c] = tri(f); cross3(sub3(b, a), sub3(c, a)) }).collect();
    let mut around: Vec<Vec<usize>> = vec![Vec::new(); m.vertices.len()];
    for (k, f) in m.faces.iter().enumerate() {
        for &i in f {
            around[i as usize].push(k);
        }
    }
    let cos = crease_deg.to_radians().cos();
    let mut out = mesh::Mesh::default();
    let mut index: std::collections::HashMap<(u32, [i64; 3]), u32> = Default::default();
    for (k, f) in m.faces.iter().enumerate() {
        let nk = unit3(normals[k]);
        let g = f.map(|i| {
            let mut sum = [0.0; 3];
            for &o in &around[i as usize] {
                if dot3(unit3(normals[o]), nk) >= cos {
                    sum = add3(sum, normals[o], 1.0);
                }
            }
            let n = unit3(sum);
            let key = (i, n.map(|x| (x * 1e4).round() as i64));
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(mesh::Vec3(n[0] as f32, n[1] as f32, n[2] as f32));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, top: f64, edge: usize) -> Result<render::Finished> {
    let mut fin = render::finished_from(d, lib, built);
    fin.metal = creased(&fin.metal, 40.0);
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let small: Vec<Vec<u8>> = VIEWS.iter().map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3)).collect();
    save_rgb(&out.join("hero-300.png"), &small[0], 300, 300)?;
    save_rgb(&out.join("face-300.png"), &small[1], 300, 300)?;
    let (cols, rows) = (3, 2);
    let mut sheet = vec![0u8; cols * 300 * rows * 300 * 3];
    for (i, img) in small.iter().enumerate() {
        let (cx, cy) = (i % cols, i / cols);
        for y in 0..300 {
            let dst = ((cy * 300 + y) * cols * 300 + cx * 300) * 3;
            sheet[dst..dst + 900].copy_from_slice(&img[y * 900..(y + 1) * 900]);
        }
    }
    save_rgb(&out.join("contact-300.png"), &sheet, cols * 300, rows * 300)?;
    // The seeds close up over the capsule's crown, and the table at twice the face view's scale, framed whole.
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0) + 0.35, 1.2, render::Framing::new([0.0, top + 7.0, 0.0], 6.0), edge)?;
    render::write_png_framed(out.join("face-zoom.png"), &parts, 0.0, PI * 0.5, render::Framing::new([0.0, top, 0.0], 11.0), edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.cad = None;
    bare.stamps.clear();
    bare.layers.layers.clear();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let right = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    Ok(fin)
}

// --- Main ----------------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/datura"));
    std::fs::create_dir_all(&out)?;
    println!("Datura");
    let mut d = stock()?;
    let mut lib = AlphaLibrary::builtin();
    let t = table(&d, &lib)?;
    println!("  table {t:?}; bore {:.2} mm", 2.0 * d.inner_radius_mm());
    if args.iter().any(|a| a == "--stamp-probe") {
        // An arrow pointing along its own +x, at theta 80 and v +4, turned 0 and then 90 degrees.
        let arrow: Vec<[f64; 2]> = vec![[-2.0, -0.4], [1.0, -0.4], [1.0, -1.0], [2.5, 0.0], [1.0, 1.0], [1.0, 0.4], [-2.0, 0.4]];
        let atlas = skin::Atlas::of(&d, 2048, 384)?;
        for (k, (x, y, dir)) in [(2.0, 3.0, [1.0, 0.0, 0.0]), (-3.0, -4.0, [0.0, 0.0, 1.0])].into_iter().enumerate() {
            let (theta, v, rot) = chart_at(&atlas, [x, t.top_mm, -y], dir);
            println!("  probe {k}: theta {theta:.2} v {v:.2} rot {rot:.1}");
            d.stamps.push(Stamp { name: format!("probe {k}"), theta_deg: theta, v_mm: v, rot_deg: rot, outline: arrow.clone(), height_mm: 0.6, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, fine_cap: false, tier: 0, top: StampTop::Flat });
        }
        d.bake_all(&mut lib);
        let b = mesh::try_build(&d, &lib, draft_params())?;
        let mut bare = d.clone();
        bare.stamps.clear();
        let b0 = mesh::try_build(&bare, &lib, draft_params())?;
        let top = |m: &mesh::Mesh| m.vertices.iter().map(|v| v.1).fold(f32::MIN, f32::max);
        println!("  struck {} notes {:?}; faces {} vs bare {}; top {} vs {}", b.solids.stamped, b.solids.notes, b.mesh.faces.len(), b0.mesh.faces.len(), top(&b.mesh), top(&b0.mesh));
        render::write_png_parts(out.join("stamp-probe.png"), &[render::Part::metal(&creased(&b.mesh, 40.0), render::GOLD)], 0.0, 1.1, 800)?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--leaf") {
        let s = leaf(9.0, 6.0, 0.2, 99.0, (48, 12));
        let mut m = mesh::Mesh::default();
        m.vertices = s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[2] as f32, -p[1] as f32)).collect();
        m.faces = s.f.clone();
        let m = creased(&m, 60.0);
        render::write_png_parts(out.join("leaf-alone.png"), &[render::Part::metal(&m, render::GOLD)], 0.0, 1.2, 800)?;
        return Ok(());
    }
    if args.iter().any(|a| a == "--bare") {
        let mut bare = d.clone();
        bare.cad = None;
        let b = mesh::try_build(&bare, &lib, draft_params())?;
        let skin = Skin::of(&b.mesh, (10.0, 170.0), (-12.0, 12.0));
        // The +z flank: the greatest z over (x, y).
        let mut zmax = std::collections::BTreeMap::new();
        for f in &b.mesh.faces {
            for &i in f {
                let v = b.mesh.vertices[i as usize];
                let key = ((v.0 / 1.0).round() as i32, (v.1 / 1.0).round() as i32);
                let e = zmax.entry(key).or_insert(f32::MIN);
                *e = e.max(v.2);
            }
        }
        for y in (-2..=16).rev() {
            let row: Vec<String> = (-14..=14).step_by(2).map(|x| zmax.get(&(x, y)).map_or("   . ".into(), |z| format!("{z:5.1}"))).collect();
            println!("  y {y:3}: {}", row.join(""));
        }
        for th in (20..=90).step_by(5) {
            let (lo, hi) = skin.across(th as f64);
            println!("  theta {th}: across {lo:.2}..{hi:.2}, r at 0 {:?}", skin.radius((th as f64).to_radians(), 0.0));
        }
        let b = mesh::try_build(&d, &lib, draft_params())?;
        let parts = vec![render::Part::metal(&b.mesh, render::GOLD)];
        for (name, yaw, pitch) in VIEWS {
            render::write_png_parts(out.join(format!("bare-{name}.png")), &parts, yaw, pitch, 800)?;
        }
        return Ok(());
    }
    let started = std::time::Instant::now();
    let (cap, seeds, foliage) = parts(&mut d, &lib, t, blockout)?;
    d.bake_all(&mut lib);
    let author_s = started.elapsed().as_secs_f64();
    println!("  capsule: {} spines, {} triangles, {:.1} mm3; authored in {author_s:.1} s", cap.spines, cap.triangles, cap.volume_mm3);
    for s in &seeds {
        println!("    {}: on pad face {} at {:?}", s.name, s.pad_face, s.world.map(|v| (v * 100.0).round() / 100.0));
    }
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: {}", serde_json::to_string(&draft_gates)?);
    let (export_gates, export_pass, built) = if draft {
        (Value::Null, true, draft_built)
    } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: {}", serde_json::to_string(&g)?);
        (g, p, b)
    };
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
    let mut pattern_gates = Value::Null;
    let mut pattern_pass = true;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.mesh.quality();
        let pc = csg::self_crossings(&solid_of(&pattern.mesh));
        let pv = pattern.mesh.validate();
        pattern_pass = pv.watertight && pq.degenerate_faces == 0 && pc == 0;
        pattern_gates = json!({ "triangles": pattern.mesh.faces.len(), "watertight": pv.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "pass": pattern_pass });
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Datura / investment pattern")?;
    }
    let all = draft_pass && export_pass && pattern_pass && cold != Some(false) && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": "datura",
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "process_note": "Lost wax: spines radiate from a dome, so in two-part sand only about a quarter of them would release.",
        "base": "Factory 011 Badge, native 18 x 20, not mirrored",
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "table": t,
        "capsule": cap,
        "seeds": seeds,
        "foliage": foliage,
        "placement_note": "The capsule is seated by Placement::Ring at the table's centre; the flower, leaves and every seed pad stand Placement::Relative to it (C-V1), so the head follows a resize. The stored meshes keep their own size.",
        "design_bytes": text.len(),
        "design_format": serde_json::from_str::<Value>(&text)?.get("format_version").cloned(),
        "draft": draft_gates,
        "export": export_gates,
        "casting_pattern": pattern_gates,
        "cold_reload_identical": cold,
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let fin = renders(&out, &d, &lib, built, t.top_mm, if draft { 1000 } else { 1600 })?;
    if !draft {
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-spinel.stl".to_string() } else { format!("reference-spinel-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Datura reference black spinel")?;
            materials.push(json!({ "mesh": file, "name": "Black spinel", "tint": tint, "ior": 1.72, "dispersion": 0.020, "roughness": 0.065, "transmission": 0.0 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    println!("  gates {}", if all { "passed" } else { "FAILED" });
    ensure!(all || draft, "Datura failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
