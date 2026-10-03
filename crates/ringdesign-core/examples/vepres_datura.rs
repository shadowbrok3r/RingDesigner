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
const SHARED_ROWS: usize = 80;

/// Black spinel, the seeds.
const SPINEL_TINT: [f32; 3] = [0.03, 0.03, 0.04];
const SEED_MM: f64 = 1.5;

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
    /// The calyx frill's reach and its five lobes' depth.
    frill_r: f64,
    frill_lobe: f64,
    /// Depth the capsule is sunk into the table.
    sunk_mm: f64,
    /// The four splits' bearing round the axis from local x, radians.
    split_at: f64,
}

const SHAPE: Shape = Shape {
    egg_r: 5.0,
    tip_z: 10.6,
    split_z: 4.6,
    gape: 0.36,
    frill_r: 5.6,
    frill_lobe: 0.6,
    sunk_mm: 0.9,
    split_at: 0.0,
};

/// The meridian of a valve's middle and of a split's floor, sampled alike: (r, z) from the buried foot to the axis
/// over the placenta. Both run the same way up the egg until the split opens.
fn meridians(s: &Shape) -> (Vec<[f64; 2]>, Vec<[f64; 2]>) {
    let (r, h) = (s.egg_r, s.tip_z);
    let floor_z = h - 1.7;
    let foot = -s.sunk_mm;
    // Shared run: the buried foot, the calyx's reflexed frill, the waist and the egg's lower half.
    let shared: Vec<[f64; 2]> = vec![
        [0.0, foot],
        [s.frill_r - 0.5, foot],
        [s.frill_r, foot + 0.35],
        [s.frill_r, 0.2],
        [s.frill_r - 0.3, 0.42],
        [r + 0.05, 0.62],
        [r - 0.5, 0.95],
        [r - 0.55, 1.35],
        [r - 0.15, 2.4],
        [r, 3.3],
        [r - 0.02, s.split_z],
    ];
    // The valve: on up the egg and over its shoulder, its lip just parted from its neighbours over the placenta.
    let valve: Vec<[f64; 2]> = vec![
        [r - 0.2, 5.9],
        [r - 0.6, 7.2],
        [r - 1.3, 8.4],
        [r - 2.2, 9.3],
        [r - 3.1, h - 0.15],
        [1.35, h],
        [0.95, h - 0.2],
        [0.75, floor_z + 0.4],
        [0.0, floor_z + 0.25],
    ];
    // The split's floor: from where the split opens, a channel climbing in under the valves' lips to the placenta.
    let split: Vec<[f64; 2]> = vec![
        [r - 0.55, s.split_z + 0.3],
        [r - 1.15, s.split_z + 1.35],
        [r - 1.8, s.split_z + 2.25],
        [r - 2.45, floor_z - 0.45],
        [r - 3.1, floor_z - 0.12],
        [1.2, floor_z + 0.1],
        [0.65, floor_z + 0.2],
        [0.0, floor_z + 0.25],
    ];
    // The shared run is sampled once, so both meridians agree on it point for point; each divergent run alike.
    let joint = *shared.last().unwrap();
    let mut a = resample(&shared, SHARED_ROWS);
    let mut b = a.clone();
    let mut v = vec![joint];
    v.extend(valve);
    let mut w = vec![joint];
    w.extend(split);
    a.extend(resample(&v, 181).into_iter().skip(1));
    b.extend(resample(&w, 181).into_iter().skip(1));
    (a, b)
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
    around: usize,
}

impl Capsule {
    fn new(shape: Shape, around: usize) -> Self {
        let (valve, split) = meridians(&shape);
        Self { shape, valve, split, around }
    }
    /// How far into its split a bearing `phi` lies at meridian sample `i`: 1 on the floor, 0 on a valve's face.
    fn into_split(&self, i: usize, phi: f64) -> f64 {
        let (v, s) = (self.valve[i], self.split[i]);
        let parted = (v[0] - s[0]).hypot(v[1] - s[1]);
        if parted < 1e-6 {
            return 0.0;
        }
        // The clear half-gap opens over the first millimetre of the split and holds; its angle grows as the floor runs
        // in, never shrinking up the meridian, so no meridian folds back on itself.
        // The split runs as a slit from where it opens and gapes toward the top, its angle never shrinking up the
        // meridian, so no meridian folds back on itself.
        let along = (i as f64 - SHARED_ROWS as f64) / (self.valve.len() - SHARED_ROWS) as f64;
        let opened = 0.25 + 0.75 * smooth(0.05, 0.42, along);
        let half = self.shape.gape * opened * smooth(0.0, 0.03, along);
        let wall = 0.06;
        let off = (0..4).map(|k| wrap(phi - self.shape.split_at - k as f64 * PI / 2.0).abs()).fold(f64::MAX, f64::min);
        1.0 - smooth(half, half + wall, off)
    }
    fn point(&self, i: usize, j: usize) -> P3 {
        let phi = 2.0 * PI * j as f64 / self.around as f64;
        let t = self.into_split(i, phi);
        let [r, z] = lerp2(self.valve[i], self.split[i], t);
        // The frill's five shallow lobes.
        let frill = smooth(0.0, 0.5, z + self.shape.sunk_mm) * (1.0 - smooth(0.5, 0.9, z));
        let r = r * (1.0 - frill * self.shape.frill_lobe / self.shape.frill_r * (0.5 - 0.5 * (5.0 * phi).cos()));
        // Each valve's top drawn to a point: heights over the shoulder fall toward the valve's edges.
        let shoulder = self.shape.tip_z - 3.6;
        let edge = 0.5 - 0.5 * (4.0 * (phi - self.shape.split_at - PI / 4.0)).cos();
        let z = if z > shoulder { shoulder + (z - shoulder) * (1.0 - 0.0 * edge) } else { z };
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
        let j = (phi / (2.0 * PI) * self.around as f64).rem_euclid(self.around as f64);
        let jf = j.floor() as usize;
        let p = self.point(i, jf);
        let du = sub3(self.point(i + 1, jf), self.point(i - 1, jf));
        let dv = sub3(self.point(i, jf + 1), self.point(i, (jf + self.around - 1) % self.around));
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
    let around = 20usize;
    let buried = 0.45;
    // Rings along the axis: (height along the axis, radius).
    let mut rings: Vec<(f64, f64)> = Vec::new();
    let steps = 14;
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
    /// The 2 mm band of height the count is taken over, from its foot.
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

/// The four seed places in each split, in the capsule frame: (point on the split floor, the floor's outward normal).
fn seed_places(c: &Capsule) -> Vec<(P3, P3, f64)> {
    let mut out = Vec::new();
    let m = c.split.len();
    // The split floor's own normal in its meridian plane.
    let floor_at = |r_want: f64| -> (usize, [f64; 2]) {
        let i = (1..m - 1)
            .filter(|&i| c.split[i][1] > c.shape.split_z + 0.2 && c.into_split(i, c.shape.split_at) > 0.99)
            .min_by(|&a, &b| (c.split[a][0] - r_want).abs().total_cmp(&(c.split[b][0] - r_want).abs()))
            .unwrap_or(m / 2);
        let d = [c.split[i + 1][0] - c.split[i - 1][0], c.split[i + 1][1] - c.split[i - 1][1]];
        // Outward: turn the run (inward and up) a quarter clockwise in (r, z).
        let n = [d[1], -d[0]];
        let l = n[0].hypot(n[1]).max(1e-9);
        (i, [-n[0] / l, -n[1] / l])
    };
    for k in 0..4 {
        let phi = c.shape.split_at + k as f64 * PI / 2.0;
        for r_want in [c.shape.egg_r - 1.75, c.shape.egg_r - 3.35] {
            let (i, n) = floor_at(r_want);
            let [r, z] = c.split[i];
            let p = [r * phi.cos(), r * phi.sin(), z];
            let normal = unit3([n[0] * phi.cos(), n[0] * phi.sin(), n[1]]);
            out.push((p, normal, phi));
        }
    }
    out
}

/// The whole capsule in its own frame: the split egg and its rows of spines, one closed solid.
fn capsule(shape: Shape, blockout: bool) -> Result<(csg::Solid, CapsuleReport, Capsule)> {
    let c = Capsule::new(shape, if blockout { 480 } else { 720 });
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
    let spacing = 1.22;
    let mut candidates: Vec<(u64, usize, f64)> = Vec::new();
    let mut z = 1.7;
    while z < shape.tip_z - 0.5 {
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
    for &(_, i, phi) in &candidates {
        let r = c.valve[i][0];
        let z = c.valve[i][1];
        let root = 0.43;
        let clear = (root + 0.22) / r;
        if [-clear, 0.0, clear].iter().any(|d| c.into_split(i, phi + d) > 0.0) {
            continue;
        }
        let u = i as f64 / (c.valve.len() - 1) as f64;
        let (p, n) = c.at(u, phi);
        if feet.iter().any(|(f, _)| dot3(sub3(*f, p), sub3(*f, p)) < spacing * spacing) {
            continue;
        }
        // Longest round the egg's waist, shorter toward the foot and the crown; varied by the shuffle.
        let reach = (1.9 - 0.6 * ((z - 4.6) / 4.0).powi(2)).max(0.9);
        let hash = ((i * 131 + (phi * 1000.0) as usize * 7) % 97) as f64 / 97.0;
        let length = reach * (0.8 + 0.35 * hash);
        let axis = unit3(add3(n, [0.0, 0.0, 1.0], 0.22));
        parts.push(spine(add3(p, n, -0.05), axis, length, root, 0.17));
        feet.push((p, z));
    }
    let mut report_rows = Vec::new();
    for band in 0..((shape.tip_z / 2.0).ceil() as usize) {
        let (lo, hi) = (2.0 * band as f64, 2.0 * band as f64 + 2.0);
        let count = feet.iter().filter(|(_, z)| *z >= lo && *z < hi).count();
        report_rows.push(SpineRow { z_mm: lo, count, length_mm: 0.0, root_mm: 0.86 });
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
    ensure!(solid.open_edges() == (0, 0), "The spined capsule does not close");
    ensure!(csg::self_crossings(&solid) == 0, "The spined capsule crosses itself");
    let report = CapsuleReport { shape, rows: report_rows, spines, triangles: solid.f.len(), volume_mm3: solid.volume(), slivers_cleaned: slivers };
    Ok((solid, report, c))
}

/// Capsule frame to the world: z up the table's normal at the top of the ring, x round the ring, y along the finger.
fn to_world(p: P3, top: f64) -> P3 {
    [p[0], top + p[2], -p[1]]
}
fn dir_to_world(v: P3) -> P3 {
    [v[0], v[2], -v[1]]
}

fn solid_to_world(s: &csg::Solid, top: f64) -> csg::Solid {
    // The map is a proper rotation, so the winding holds.
    csg::Solid { v: s.v.iter().map(|&p| to_world(p, top)).collect(), f: s.f.clone() }
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
    theta_deg: f64,
    across_mm: f64,
    height_mm: f64,
    tilt_deg: f64,
    cant_deg: f64,
    seated_error_mm: f64,
}

/// A ring placement whose seat frame stands at `target` with its z along `normal`, found on the band's own surface.
fn seat_for(d: &RingDesign, surface: &mesh::Mesh, target: P3, normal: P3) -> Result<(Placement, f64)> {
    let theta = target[1].atan2(target[0]).to_degrees();
    let across = target[2];
    let base = Placement::Ring { theta_deg: theta, across_mm: across, height_mm: 0.0, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
    let f0 = base.frame_on(d, Some(surface))?;
    // The ray's hit and the surface frame there.
    let (x, y, z) = (f0.x_axis, f0.y_axis, f0.z_axis);
    let h = dot3(sub3(target, f0.origin), z);
    // The normal in the hit's frame; with no spin the leaned z is (cos tilt sin cant, -sin tilt, cos tilt cos cant).
    let nl = [dot3(normal, x), dot3(normal, y), dot3(normal, z)];
    let tilt = (-nl[1]).clamp(-1.0, 1.0).asin();
    let cant = nl[0].atan2(nl[2]);
    let mut best = (f64::MAX, base.clone());
    // The origin stands on the hit's own normal line: walk the ray until it lands on the target.
    let mut th = theta;
    let mut ac = across;
    let mut hh = h;
    for _ in 0..12 {
        let p = Placement::Ring { theta_deg: th, across_mm: ac, height_mm: hh, spin_deg: 0.0, tilt_deg: tilt.to_degrees(), cant_deg: cant.to_degrees() };
        let f = p.frame_on(d, Some(surface))?;
        let miss = sub3(target, f.origin);
        let err = dot3(miss, miss).sqrt();
        if err < best.0 {
            best = (err, p.clone());
        }
        if err < 1e-5 {
            break;
        }
        // Correct round the ring, across it and out along the normal.
        let r = f.origin[0].hypot(f.origin[1]).max(1e-6);
        let round = [-f.origin[1] / r, f.origin[0] / r, 0.0];
        th += (dot3(miss, round) / r).to_degrees();
        ac += miss[2];
        hh += dot3(miss, z);
    }
    Ok((best.1, best.0))
}

/// The CAD parts: the capsule joined to the badge, and the eight seeds with their seats cut into the splits.
fn parts(d: &mut RingDesign, lib: &AlphaLibrary, t: Table, blockout: bool) -> Result<(CapsuleReport, Vec<SeedReport>)> {
    let (local, report, c) = capsule(SHAPE, blockout)?;
    let world = solid_to_world(&local, t.top_mm);
    let mut doc = Document::default();
    doc.append(feature(1, "Badge", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() }))?;
    doc.append(feature(
        2,
        "Thorn-apple capsule",
        stored_op(&world, "capsule", json!({ "shape": SHAPE, "rows": report.rows, "table_mm": t.top_mm }))?,
        Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, blend_mm: 0.5, ..Component::default() },
    ))?;
    let mut bare = d.clone();
    bare.cad = None;
    let surface = mesh::try_build(&bare, lib, coarse_params())?.mesh;
    let mut seeds = Vec::new();
    let mut id: Id = 3;
    for (k, (p, n, phi)) in seed_places(&c).into_iter().enumerate() {
        // The girdle sits a little proud of the split's floor, the seat cut under it.
        let g = spinel();
        let girdle = add3(p, n, 0.18);
        let (wp, wn) = (to_world(girdle, t.top_mm), unit3(dir_to_world(n)));
        let (placement, err) = seat_for(d, &surface, wp, wn)?;
        let Placement::Ring { theta_deg, across_mm, height_mm, tilt_deg, cant_deg, .. } = placement else { unreachable!() };
        let mut stone = builders::stone_feature(id, g, placement);
        stone.name = format!("Seed {} ({:.0} deg)", k + 1, phi.to_degrees());
        let stone_id = id;
        doc.append(stone)?;
        id += 1;
        doc.append(builders::feature_on(id, &format!("Seed seat {}", k + 1), builders::BUR, stone_id, json!({ "through": false })))?;
        id += 1;
        doc.append(builders::feature_on(
            id,
            &format!("Seed thorns {}", k + 1),
            builders::CLAW,
            stone_id,
            json!({ "prongs": 3, "wire_mm": 0.45, "rails": "Seat", "style": "Thorn", "tip": "Point" }),
        ))?;
        id += 1;
        seeds.push(SeedReport { name: format!("Seed {}", k + 1), world: wp, normal: wn, theta_deg, across_mm, height_mm, tilt_deg, cant_deg, seated_error_mm: err });
    }
    d.cad = Some(doc);
    Ok((report, seeds))
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
    ("hero", 0.3, 0.3),
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

fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
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

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, top: f64, edge: usize) -> Result<render::Finished> {
    let fin = render::finished_from(d, lib, built);
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
    // The seeds close up: the capsule's top.
    let close_metal = crop(&fin.metal, [0.0, top + 6.0, 0.0], 10.0);
    let mut close = vec![render::Part::metal(&close_metal, render::GOLD)];
    close.extend(fin.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.2, edge)?;
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
    if args.iter().any(|a| a == "--bare") {
        let b = mesh::try_build(&d, &lib, draft_params())?;
        let parts = vec![render::Part::metal(&b.mesh, render::GOLD)];
        for (name, yaw, pitch) in VIEWS {
            render::write_png_parts(out.join(format!("bare-{name}.png")), &parts, yaw, pitch, 800)?;
        }
        return Ok(());
    }
    let started = std::time::Instant::now();
    let (cap, seeds) = parts(&mut d, &lib, t, blockout)?;
    d.bake_all(&mut lib);
    let author_s = started.elapsed().as_secs_f64();
    println!("  capsule: {} spines, {} triangles, {:.1} mm3; authored in {author_s:.1} s", cap.spines, cap.triangles, cap.volume_mm3);
    for s in &seeds {
        println!("    {}: theta {:.2} across {:.2} height {:.2} tilt {:.1} cant {:.1}; off by {:.4} mm", s.name, s.theta_deg, s.across_mm, s.height_mm, s.tilt_deg, s.cant_deg, s.seated_error_mm);
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
        "placement_note": "Placement::Free from the capsule's seated frame (C-V1 Relative not on master); the capsule and seeds do not follow a resize.",
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
