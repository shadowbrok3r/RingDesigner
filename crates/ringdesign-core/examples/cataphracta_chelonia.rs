//! Cataphracta — Chelonia, the carapace seal: a hawksbill turtle swimming across the factory 007 Quatrefoil, its shell on
//! the table's middle, a flipper on each of the four lobes, its head reaching down one along-ring cusp and its tail the
//! other. The real 007 stock, unmirrored, cast in lost wax. The turtle is made parts: sculpted closed meshes fitted to the
//! stock's own table and joined to it, so every outline is true geometry.
//! cargo build --release -p ringdesign-core --example cataphracta_chelonia
//! target/release/examples/cataphracta_chelonia [OUT_DIR] [--draft] [--verify] [--block-out]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement, SurfaceKind, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    interaction::bvh::Bvh,
    manufacturing as mf, mesh,
    render::{self, Part},
    stl,
};
use serde_json::{Value, json};
use std::{
    f64::consts::PI,
    path::{Path, PathBuf},
    time::Instant,
};

const SLUG: &str = "chelonia";
const BORE_MM: f64 = 18.6;
/// The table's face: along the ring by across it, mm (0.92 of the master's 18.5 width, inside the resize guard).
const FACE: (f64, f64) = (16.0, 17.0);
/// The investment's fill floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const HERO: (f64, f64) = (0.55, 0.95);
/// The plan's along-ring scale: plan millimetres are arc millimetres at this radius, about the table's own.
const PLAN_R: f64 = 14.4;
/// How far every part's floor sinks under the stock's surface, mm.
const SINK_MM: f64 = 0.4;
/// Where a part's top meets its outline: just under the stock's surface, so the join crosses it cleanly.
const EDGE_DIP_MM: f64 = 0.05;

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

type P2 = (f64, f64);

fn seg_dist(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let k = if l2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - a.0 - k * dx).hypot(p.1 - a.1 - k * dy)
}

// --- The base ----------------------------------------------------------------------------------------------------------

/// The factory 007 Quatrefoil at 16 x 17 mm, unmirrored, in lost wax at the 0.8 mm fill.
fn band() -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "007").context("no 007 stock")?;
    let mut d = RingDesign { name: "Chelonia \u{2014} the carapace seal".into(), ..RingDesign::default() };
    ImportedBase::attach(&mut d, preset.load()?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.profile.apply_style(ProfileStyle::Flat);
    d.shank.head.length_mm = FACE.0;
    d.profile.width_mm = FACE.1;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
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
    setup.bench_notes = "Lost wax on the factory 007 Quatrefoil, unmirrored. A hawksbill turtle swims across the table: its shell on the \
        middle, a flipper on each lobe, its head down one along-ring cusp and its tail the other. Sprue from the palm. At the bench: clean the \
        scute seams with a fine graver, leave the shell's plates bright and the flippers' scales satin."
        .into();
    d.manufacturing = Some(setup);
    Ok(d)
}

// --- The stock's surface, read by rays -----------------------------------------------------------------------------------

/// The bare stock's outer surface, read along rays toward the finger axis.
struct Surface {
    mesh: mesh::Mesh,
    bvh: Bvh,
}

impl Surface {
    fn of(d: &RingDesign) -> Result<Self> {
        let built = mesh::try_build(d, &AlphaLibrary::default(), params(false))?;
        let bvh = Bvh::build(&built.mesh);
        Ok(Self { mesh: built.mesh, bvh })
    }
    /// The ring angle of a plan point: along-ring plan millimetres from the table's middle, +u (the head's way) toward +X,
    /// the side the hero camera stands.
    fn theta(u: f64) -> f64 {
        0.5 * PI - u / PLAN_R
    }
    /// The plan's along-ring millimetres of a world point.
    fn u_of(p: [f64; 3]) -> f64 {
        (0.5 * PI - p[1].atan2(p[0])) * PLAN_R
    }
    /// The radial unit at plan `u`.
    fn radial(u: f64) -> [f64; 3] {
        let (s, c) = Self::theta(u).sin_cos();
        [c, s, 0.0]
    }
    /// The surface's radius from the axis under plan point `(u, v)`.
    fn radius(&self, u: f64, v: f64) -> Option<f64> {
        let th = Self::theta(u);
        let (s, c) = th.sin_cos();
        const FAR: f64 = 40.0;
        self.bvh.ray(&self.mesh, [FAR * c, FAR * s, v], [-c, -s, 0.0]).map(|(_, t)| FAR - t)
    }
    /// The world point at radius `r` from the axis over plan point `(u, v)`.
    fn point(&self, u: f64, v: f64, r: f64) -> [f64; 3] {
        let (s, c) = Self::theta(u).sin_cos();
        [r * c, r * s, v]
    }
    /// The world point `h` mm out along the radial from the surface under `(u, v)`.
    fn at(&self, u: f64, v: f64, h: f64) -> Result<[f64; 3]> {
        let r = self.radius(u, v).with_context(|| format!("no stock under plan ({u:.2}, {v:.2})"))?;
        let (s, c) = Self::theta(u).sin_cos();
        Ok([(r + h) * c, (r + h) * s, v])
    }
}

// --- Sculpted strips: the turtle's parts ------------------------------------------------------------------------------

/// A point of a strip, as its height function reads it.
#[derive(Clone, Copy)]
struct Here {
    /// Plan millimetres: along the ring from the table's middle, and across it.
    u: f64,
    v: f64,
    /// Along the strip's spine 0..1, and across it -1..1 (+1 on the spine's left).
    s: f64,
    t: f64,
    /// Plan distance to the outline, mm.
    edge: f64,
    /// Millimetres along the spine from its start, and across it from the spine (+ on the left).
    along: f64,
    across: f64,
}

/// A closed, sculpted solid in plan: a spine, a half-width along it, and a height over the stock at every point.
struct Strip<'a> {
    name: &'static str,
    spine: Vec<P2>,
    /// Half-width, mm, by the share of the spine's length.
    width: &'a dyn Fn(f64) -> f64,
    /// The radii of the rounded tip at each end, mm.
    ends: (f64, f64),
    height: &'a dyn Fn(Here) -> f64,
    /// Rows along and columns across the top.
    rows: usize,
    cols: usize,
    /// Every `under`-th column carries the floor.
    under: usize,
    /// The half-width on the spine's right, when it differs from the left.
    right: Option<&'a dyn Fn(f64) -> f64>,
    /// A lens stands free of the stock: `height` is then its top's radius from the axis and this its floor's.
    lens: Option<&'a dyn Fn(Here) -> f64>,
}

/// A polyline resampled to `n` points by arc length, with the length.
fn resample(pts: &[P2], n: usize) -> (Vec<P2>, f64) {
    let mut acc = vec![0.0];
    for w in pts.windows(2) {
        acc.push(acc.last().unwrap() + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1));
    }
    let len = *acc.last().unwrap();
    let out = (0..n)
        .map(|i| {
            let d = len * i as f64 / (n - 1) as f64;
            let k = (0..pts.len() - 1).find(|&k| d <= acc[k + 1]).unwrap_or(pts.len() - 2);
            let f = ((d - acc[k]) / (acc[k + 1] - acc[k]).max(1e-12)).clamp(0.0, 1.0);
            (pts[k].0 + f * (pts[k + 1].0 - pts[k].0), pts[k].1 + f * (pts[k + 1].1 - pts[k].1))
        })
        .collect();
    (out, len)
}

/// Catmull-Rom through `knots`, `per` points a span.
fn smooth_path(knots: &[P2], per: usize) -> Vec<P2> {
    let n = knots.len();
    let at = |i: isize| knots[i.clamp(0, n as isize - 1) as usize];
    let mut out = Vec::new();
    for i in 0..n - 1 {
        let (p0, p1, p2, p3) = (at(i as isize - 1), at(i as isize), at(i as isize + 1), at(i as isize + 2));
        for k in 0..per {
            let t = k as f64 / per as f64;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f64, b: f64, c: f64, d: f64| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out.push(knots[n - 1]);
    out
}

struct Sculpted {
    name: &'static str,
    solid: csg::Solid,
    outline: Vec<P2>,
    tallest_mm: f64,
}

impl Strip<'_> {
    fn build(&self, surface: &Surface) -> Result<Sculpted> {
        let (n, m, k) = (self.rows, self.cols, self.under);
        ensure!(m % k == 0 && m % 2 == 0, "{}: columns must divide into the floor's", self.name);
        let (spine, len) = resample(&smooth_path(&self.spine, 16), 2001);
        let at_s = |s: f64| -> (P2, P2) {
            let f = s.clamp(0.0, 1.0) * 2000.0;
            let i = (f as usize).min(1999);
            let g = f - i as f64;
            let (a, b) = (spine[i], spine[i + 1]);
            let p = (a.0 + g * (b.0 - a.0), a.1 + g * (b.1 - a.1));
            let (j0, j1) = (i.saturating_sub(3), (i + 4).min(2000));
            let d = (spine[j1].0 - spine[j0].0, spine[j1].1 - spine[j0].1);
            let l = d.0.hypot(d.1);
            (p, (d.0 / l, d.1 / l))
        };
        let round = |x: f64| {
            let x = x.clamp(0.0, 1.0);
            (1.0 - (1.0 - x) * (1.0 - x)).sqrt()
        };
        let ends = |s: f64| round(s * len / self.ends.0) * round((1.0 - s) * len / self.ends.1);
        let half = |s: f64| (self.width)(s) * ends(s);
        // The half-width on a side: t < 0 is the spine's right.
        let side = |s: f64, t: f64| if t < 0.0 { self.right.map_or((self.width)(s), |r| r(s)) * ends(s) } else { half(s) };
        // Rows bunch toward the rounded ends, except on a long strip, whose middle needs them as much.
        let s_of = |i: usize| if len > 30.0 { i as f64 / n as f64 } else { 0.5 * (1.0 - (PI * i as f64 / n as f64).cos()) };
        let t_of = |j: usize| -(PI * j as f64 / m as f64).cos();
        let plan = |s: f64, t: f64| -> P2 {
            let (p, d) = at_s(s);
            let w = side(s, t) * t;
            (p.0 - d.1 * w, p.1 + d.0 * w)
        };
        // The outline, for the edge distance: down the right side and back up the left.
        let mut outline: Vec<P2> = (0..=n).map(|i| plan(s_of(i), -1.0)).collect();
        outline.extend((0..=n).rev().map(|i| plan(s_of(i), 1.0)));
        let edge_of = |p: P2| outline.windows(2).map(|w| seg_dist(p, w[0], w[1])).fold(f64::MAX, f64::min);
        // A long strip reads its edge distance across and along instead of off the outline.
        let fast_edge = n * m > 150_000;
        let mut v: Vec<[f64; 3]> = Vec::new();
        let mut tallest: f64 = 0.0;
        let here = |s: f64, t: f64| -> Here {
            let p = plan(s, t);
            let edge = if t.abs() >= 1.0 || s <= 0.0 || s >= 1.0 {
                0.0
            } else if fast_edge {
                ((1.0 - t.abs()) * side(s, t)).min(s * len).min((1.0 - s) * len)
            } else {
                edge_of(p)
            };
            Here { u: p.0, v: p.1, s, t, edge, along: s * len, across: t * side(s, t) }
        };
        let mut top = |s: f64, t: f64, v: &mut Vec<[f64; 3]>| -> Result<u32> {
            let h = here(s, t);
            if let Some(floor) = self.lens {
                let (r, under) = ((self.height)(h), floor(h));
                ensure!(r > under + 0.2, "{}: its top meets its floor at ({:.2}, {:.2})", self.name, h.u, h.v);
                tallest = tallest.max(r - surface.radius(h.u, h.v).unwrap_or(r));
                v.push(surface.point(h.u, h.v, r));
            } else {
                let lift = if h.edge <= 0.0 { -EDGE_DIP_MM } else { (self.height)(h) };
                ensure!(lift > -SINK_MM + 0.05, "{}: its top dips to its floor at ({:.2}, {:.2})", self.name, h.u, h.v);
                tallest = tallest.max(lift);
                v.push(surface.at(h.u, h.v, lift)?);
            }
            Ok(v.len() as u32 - 1)
        };
        // Top: a pole at each end, rows 1..n-1 of m+1 columns.
        let top0 = top(0.0, 0.0, &mut v)?;
        let mut grid = vec![vec![0u32; m + 1]; n + 1];
        for i in 1..n {
            for j in 0..=m {
                grid[i][j] = top(s_of(i), t_of(j), &mut v)?;
            }
        }
        let top1 = top(1.0, 0.0, &mut v)?;
        // Floor: the same rows, every k-th column, sunk under the surface.
        let floor = |s: f64, t: f64, v: &mut Vec<[f64; 3]>| -> Result<u32> {
            let h = here(s, t);
            match self.lens {
                Some(floor) => v.push(surface.point(h.u, h.v, floor(h))),
                None => v.push(surface.at(h.u, h.v, -SINK_MM)?),
            }
            Ok(v.len() as u32 - 1)
        };
        let bot0 = floor(0.0, 0.0, &mut v)?;
        let mb = m / k;
        let mut low = vec![vec![0u32; mb + 1]; n + 1];
        for i in 1..n {
            for j in 0..=mb {
                low[i][j] = floor(s_of(i), t_of(j * k), &mut v)?;
            }
        }
        let bot1 = floor(1.0, 0.0, &mut v)?;
        let mut f: Vec<[u32; 3]> = Vec::new();
        // Each quad is split along its shorter diagonal, so a crease running obliquely across the grid is followed rather
        // than cut into a saw.
        let quad = |f: &mut Vec<[u32; 3]>, a: u32, b: u32, c: u32, d: u32| {
            let dist = |x: u32, y: u32| {
                let (p, q) = (v[x as usize], v[y as usize]);
                (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)
            };
            if dist(a, c) <= dist(b, d) {
                f.push([a, b, c]);
                f.push([a, c, d]);
            } else {
                f.push([a, b, d]);
                f.push([b, c, d]);
            }
        };
        for i in 1..n - 1 {
            for j in 0..m {
                quad(&mut f, grid[i][j], grid[i + 1][j], grid[i + 1][j + 1], grid[i][j + 1]);
            }
            for j in 0..mb {
                quad(&mut f, low[i][j], low[i][j + 1], low[i + 1][j + 1], low[i + 1][j]);
            }
        }
        for j in 0..m {
            f.push([top0, grid[1][j], grid[1][j + 1]]);
            f.push([top1, grid[n - 1][j + 1], grid[n - 1][j]]);
        }
        for j in 0..mb {
            f.push([bot0, low[1][j + 1], low[1][j]]);
            f.push([bot1, low[n - 1][j], low[n - 1][j + 1]]);
        }
        // The walls: the right side (t = -1, column 0) and the left (t = +1, column m), pole to pole.
        let right_top: Vec<u32> = std::iter::once(top0).chain((1..n).map(|i| grid[i][0])).chain(std::iter::once(top1)).collect();
        let right_low: Vec<u32> = std::iter::once(bot0).chain((1..n).map(|i| low[i][0])).chain(std::iter::once(bot1)).collect();
        let left_top: Vec<u32> = std::iter::once(top0).chain((1..n).map(|i| grid[i][m])).chain(std::iter::once(top1)).collect();
        let left_low: Vec<u32> = std::iter::once(bot0).chain((1..n).map(|i| low[i][mb])).chain(std::iter::once(bot1)).collect();
        for i in 0..n {
            quad(&mut f, right_top[i], right_low[i], right_low[i + 1], right_top[i + 1]);
            quad(&mut f, left_top[i], left_top[i + 1], left_low[i + 1], left_low[i]);
        }
        let mut solid = csg::Solid { v, f };
        let mut directed = std::collections::HashSet::new();
        for t in &solid.f {
            for e in 0..3 {
                ensure!(directed.insert((t[e], t[(e + 1) % 3])), "{}: its faces are not consistently wound", self.name);
            }
        }
        if solid.volume() < 0.0 {
            for t in &mut solid.f {
                t.swap(1, 2);
            }
        }
        ensure!(solid.open_edges() == (0, 0), "{}: the sculpted solid does not close ({:?})", self.name, solid.open_edges());
        let crossings = csg::self_crossings(&solid);
        if crossings > 0 {
            debug_crossings(&solid, |i| {
                let (row, rest) = if i < 1 + (n - 1) * (m + 1) { ("top", i) } else { ("floor", i - 1 - (n - 1) * (m + 1)) };
                format!("{row} #{rest}")
            });
        }
        ensure!(crossings == 0, "{}: the sculpted solid crosses itself {crossings} times", self.name);
        Ok(Sculpted { name: self.name, solid, outline, tallest_mm: tallest })
    }
}

/// Print the first few pairs of crossing faces of `s`, brute force, naming vertices by `who`.
fn debug_crossings(s: &csg::Solid, who: impl Fn(usize) -> String) {
    let sub = |a: P3, b: P3| -> P3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] };
    let cross = |a: P3, b: P3| -> P3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] };
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let seg_tri = |p: P3, q: P3, t: [P3; 3]| -> bool {
        let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let (dp, dq) = (dot(n, sub(p, t[0])), dot(n, sub(q, t[0])));
        if dp * dq >= 0.0 {
            return false;
        }
        let x: P3 = std::array::from_fn(|k| p[k] + (q[k] - p[k]) * dp / (dp - dq));
        (0..3).all(|k| dot(n, cross(sub(t[(k + 1) % 3], t[k]), sub(x, t[k]))) > 0.0)
    };
    let tris: Vec<[P3; 3]> = s.f.iter().map(|f| f.map(|i| s.v[i as usize])).collect();
    let bb: Vec<(P3, P3)> = tris.iter().map(|t| (std::array::from_fn(|k| t[0][k].min(t[1][k]).min(t[2][k])), std::array::from_fn(|k| t[0][k].max(t[1][k]).max(t[2][k])))).collect();
    let mut shown = 0;
    for i in 0..tris.len() {
        for j in i + 1..tris.len() {
            if (0..3).any(|k| bb[i].1[k] < bb[j].0[k] || bb[j].1[k] < bb[i].0[k]) || s.f[i].iter().any(|v| s.f[j].contains(v)) {
                continue;
            }
            let hit = (0..3).any(|k| seg_tri(tris[i][k], tris[i][(k + 1) % 3], tris[j])) || (0..3).any(|k| seg_tri(tris[j][k], tris[j][(k + 1) % 3], tris[i]));
            if hit && shown < 6 {
                shown += 1;
                eprintln!("  crossing: {:?} x {:?} at {:?}", s.f[i].map(|v| who(v as usize)), s.f[j].map(|v| who(v as usize)), tris[i][0]);
            }
        }
    }
}

/// A seam's groove `depth` mm deep at its floor, `half` mm to its lip: C1 at both, so no crease runs along the lip for
/// the mesh to alias into a saw.
fn groove(d: f64, half: f64, depth: f64) -> f64 {
    depth * (1.0 - smoothstep(0.0, half, d))
}

/// The height a strip stands at a point: `body` mm in its interior, rising from the outline over `roll` mm on an S: a
/// concave foot that sweeps up off the stock like a fillet, then a convex shoulder.
fn footed(body: f64, edge: f64, roll: f64) -> f64 {
    -EDGE_DIP_MM + (body + EDGE_DIP_MM) * smoothstep(0.0, roll, edge)
}

/// The height a strip stands at a point: `body` mm in its interior, rolled down to the outline over `roll` mm.
fn rolled(body: f64, edge: f64, roll: f64) -> f64 {
    let x = (edge / roll).clamp(0.0, 1.0);
    let k = (1.0 - (1.0 - x) * (1.0 - x)).sqrt();
    -EDGE_DIP_MM + (body + EDGE_DIP_MM) * k
}

// --- The hawksbill ---------------------------------------------------------------------------------------------------

/// The carapace's outline: half-width by the share of its length from the rear, widest a third back from the nuchal.
/// It reaches the four waists of the quatrefoil and bridges the two across-ring cusps, as a shell overhangs its bridge.
const SHELL_FROM: f64 = -7.0;
const SHELL_TO: f64 = 6.6;
const SHELL_W: [(f64, f64); 8] = [(0.0, 2.1), (0.08, 3.3), (0.22, 4.55), (0.42, 5.3), (0.6, 5.5), (0.78, 5.15), (0.9, 4.3), (1.0, 3.3)];
/// The shell's rim stands at this radius from the finger axis, its crown this far over the rim.
const SHELL_RIM_R: f64 = 14.55;
const SHELL_CROWN_MM: f64 = 2.35;

/// The scutes: five vertebrals down the midline and four costals a side, as Voronoi seeds; the marginals are a ring
/// of their own inside the outline.
const VERTEBRALS: [f64; 5] = [4.45, 2.1, -0.3, -2.8, -5.15];
const COSTALS: [(f64, f64); 4] = [(3.25, 2.7), (0.9, 3.0), (-1.55, 2.95), (-4.0, 2.5)];
/// The vertebrals' power-diagram weight, mm²: with the costals drawn in close beside them, each vertebral meets two
/// costals a side on slanted seams, a hexagon about as long as it is wide.
const VERTEBRAL_WEIGHT: f64 = 0.0;
/// The corner rounding of every scute's seams and annuli, mm: a soft minimum over the cell's sides.
const SCUTE_ROUND_MM: f64 = 0.2;
const MARGINAL_MM: f64 = 1.05;
const MARGINAL_PITCH_MM: f64 = 1.6;
const SEAM_DEPTH_MM: f64 = 0.2;
const SEAM_HALF_MM: f64 = 0.22;
/// The growth annuli: a terrace this far in from each scute's seams, each rising this much.
/// The growth annuli as shares of each scute's own inradius: three concentric terraces, the areola inside the last.
const ANNULI: [f64; 3] = [0.3, 0.55, 0.8];
const ANNULUS_RISE_MM: f64 = 0.05;
/// How much each plate's rear edge stands proud of its front, the hawksbill's shingled scutes.
const IMBRICATE_MM: f64 = 0.09;

struct Shell {
    seeds: Vec<P2>,
    /// Each inner scute's inradius: half the distance to its nearest neighbour's seed.
    inradius: Vec<f64>,
    weights: Vec<f64>,
    marginal_seeds: Vec<P2>,
    block_out: bool,
}

impl Shell {
    fn new(outline: &[P2], block_out: bool) -> Self {
        let mut seeds: Vec<P2> = VERTEBRALS.iter().map(|&u| (u, 0.0)).collect();
        for &(u, v) in &COSTALS {
            seeds.push((u, v));
            seeds.push((u, -v));
        }
        // Marginal seeds along the outline at the marginal band's middle, an odd count: the outline starts at the rear
        // pole, so the rear falls on a seam (the paired supracaudals) and the front on a seed (the single nuchal).
        let (ring, len) = resample(outline, 4001);
        let count = (len / MARGINAL_PITCH_MM).round() as usize | 1;
        let mut marginal_seeds = Vec::new();
        for i in 0..count {
            let f = (i as f64 + 0.5) / count as f64;
            let idx = ((f * 4000.0).round() as usize).min(3999);
            let (a, b) = (ring[idx], ring[(idx + 1).min(4000)]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l = dx.hypot(dy).max(1e-9);
            // Inward is to the right of travel (the outline runs clockwise in plan: down the right, up the left).
            let inward = (dy / l, -dx / l);
            let k = 0.5 * MARGINAL_MM;
            marginal_seeds.push((a.0 + inward.0 * k, a.1 + inward.1 * k));
        }
        let weights = (0..seeds.len()).map(|i| if i < VERTEBRALS.len() { VERTEBRAL_WEIGHT } else { 0.0 }).collect();
        // Bounded too by the marginal ring where it clips the scute, so a costal's rings close inside what is left.
        let inradius = seeds
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let nb = seeds.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| 0.5 * (a.0 - b.0).hypot(a.1 - b.1)).fold(f64::MAX, f64::min);
                let to_ring = outline.windows(2).map(|w| seg_dist(*a, w[0], w[1])).fold(f64::MAX, f64::min) - MARGINAL_MM;
                nb.min(to_ring.max(0.3))
            })
            .collect();
        Self { seeds, inradius, weights, marginal_seeds, block_out }
    }
    /// The nearest of `seeds` to `p`, and the distance to its Voronoi seam.
    fn cell(seeds: &[P2], p: P2) -> (usize, f64) {
        let d2: Vec<f64> = seeds.iter().map(|s| (p.0 - s.0).powi(2) + (p.1 - s.1).powi(2)).collect();
        let i = (0..seeds.len()).min_by(|&a, &b| d2[a].total_cmp(&d2[b])).unwrap();
        let seam = (0..seeds.len())
            .filter(|&j| j != i)
            .map(|j| (d2[j] - d2[i]) / (2.0 * ((seeds[j].0 - seeds[i].0).hypot(seeds[j].1 - seeds[i].1))))
            .fold(f64::MAX, f64::min);
        (i, seam)
    }
    /// The power-diagram cell of `p` among `seeds` weighted by `w`, and a soft distance to its seams: the soft minimum
    /// over its sides rounds every corner by about `round` mm, so the scutes and their annuli turn their corners.
    fn power_cell(seeds: &[P2], w: &[f64], p: P2, round: f64) -> (usize, f64) {
        let d2: Vec<f64> = seeds.iter().zip(w).map(|(s, w)| (p.0 - s.0).powi(2) + (p.1 - s.1).powi(2) - w).collect();
        let i = (0..seeds.len()).min_by(|&a, &b| d2[a].total_cmp(&d2[b])).unwrap();
        let sides: Vec<f64> = (0..seeds.len())
            .filter(|&j| j != i)
            .map(|j| (d2[j] - d2[i]) / (2.0 * ((seeds[j].0 - seeds[i].0).hypot(seeds[j].1 - seeds[i].1))))
            .collect();
        let hard = sides.iter().cloned().fold(f64::MAX, f64::min);
        let soft = -round * sides.iter().map(|d| (-(d - hard) / round).exp()).sum::<f64>().ln() + hard;
        (i, soft.max(0.0).min(hard))
    }
    /// The distance to the nearest Voronoi seam among `seeds` at `p`.
    fn seam(seeds: &[P2], p: P2) -> f64 {
        Self::cell(seeds, p).1
    }
    /// The top's radius from the finger axis.
    fn top(&self, h: Here) -> f64 {
        let sh = 2.0 * h.s - 1.0;
        let q2 = (sh * sh + h.t * h.t * (1.0 - sh * sh)).min(1.0);
        // The dome: high over the vertebrals, the flanks falling to a raised marginal rim.
        let dome = 0.35 + (SHELL_CROWN_MM - 0.35) * (1.0 - q2).powf(0.55) + 0.18 * (-(h.v / 1.3).powi(2)).exp() * (1.0 - sh * sh);
        let p = (h.u, h.v);
        let (marginal, (cell, seam)) = if h.edge < MARGINAL_MM {
            // In the marginal ring: the seams between marginals, and the ring seam inside them.
            let zero = vec![0.0; self.marginal_seeds.len()];
            let (i, d) = Self::power_cell(&self.marginal_seeds, &zero, p, SCUTE_ROUND_MM);
            (true, (i, d.min(MARGINAL_MM - h.edge)))
        } else {
            let (i, d) = Self::power_cell(&self.seeds, &self.weights, p, SCUTE_ROUND_MM);
            (false, (i, d.min(h.edge - MARGINAL_MM)))
        };
        // Each plate pillowed off its seams, the seams cut as rounded V grooves.
        let pillow = 0.1 * smoothstep(0.0, 0.5, seam);
        let groove = groove(seam, SEAM_HALF_MM, SEAM_DEPTH_MM);
        let mut relief = pillow - groove;
        if !self.block_out {
            // The growth annuli: three terraces stepping up toward each scute's areola, each a share of the scute's own
            // inradius, so every scute's rings are concentric copies of its outline and none is cut into a letter by the
            // marginal ring; a marginal carries one.
            let (inr, own) = if marginal { (0.5 * MARGINAL_MM, seam) } else { (self.inradius[cell], seam) };
            let rings: &[f64] = if marginal { &ANNULI[1..2] } else { &ANNULI };
            relief += rings.iter().map(|&r| ANNULUS_RISE_MM * smoothstep(r * inr - 0.06, r * inr + 0.06, own)).sum::<f64>();
            // Shingled: every inner plate tips up toward its rear edge, so it steps down onto the plate behind it.
            if !marginal {
                let c = self.seeds[cell];
                relief += IMBRICATE_MM * ((c.0 - h.u) / 1.2).clamp(-1.0, 1.0);
            }
        }
        SHELL_RIM_R + rolled(dome + relief, h.edge, 0.45)
    }
    /// The floor's radius: from the rim's underside it sweeps down into the stock, so where the shell bridges a cusp
    /// its underside is a lip.
    fn floor(h: Here) -> f64 {
        SHELL_RIM_R - 0.8 - 1.9 * smoothstep(0.0, 1.5, h.edge)
    }
}

fn shell_part(surface: &Surface, block_out: bool) -> Result<Sculpted> {
    let width = |s: f64| pchip(&SHELL_W, s);
    // First the bare outline, for the marginal seeds.
    let (one, zero) = (|_: Here| 1.0, |_: Here| 0.0);
    let spine = vec![(SHELL_FROM, 0.0), (SHELL_TO, 0.0)];
    let probe = Strip { name: "Carapace", spine: spine.clone(), width: &width, ends: (1.6, 3.0), height: &one, rows: 120, cols: 80, under: 8, right: None, lens: Some(&zero) };
    let outline = probe.build(surface)?.outline;
    let shell = Shell::new(&outline, block_out);
    let top = |h: Here| shell.top(h);
    Strip { name: "Carapace", spine, width: &width, ends: (1.6, 3.0), height: &top, rows: 760, cols: 640, under: 8, right: None, lens: Some(&Shell::floor) }.build(surface)
}

// --- Sculpted blobs: closed solids star-shaped about a centre, read off a distance field ---------------------------

type P3 = [f64; 3];

fn len3(a: P3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// Signed distance (approximate, bound-preserving near the surface) to an ellipsoid of radii `r` about `c`.
fn ellipsoid(p: P3, c: P3, r: P3) -> f64 {
    let q: P3 = std::array::from_fn(|k| (p[k] - c[k]) / r[k]);
    let qq: P3 = std::array::from_fn(|k| (p[k] - c[k]) / (r[k] * r[k]));
    let (k0, k1) = (len3(q), len3(qq));
    if k1 < 1e-12 { -r.iter().cloned().fold(f64::MAX, f64::min) } else { k0 * (k0 - 1.0) / k1 }
}

/// Signed distance to a capsule from `a` to `b`, its radius running `ra` to `rb`.
fn capsule(p: P3, a: P3, b: P3, ra: f64, rb: f64) -> f64 {
    let ab: P3 = std::array::from_fn(|k| b[k] - a[k]);
    let ap: P3 = std::array::from_fn(|k| p[k] - a[k]);
    let h = ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / (ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2])).clamp(0.0, 1.0);
    let d: P3 = std::array::from_fn(|k| ap[k] - h * ab[k]);
    len3(d) - (ra + h * (rb - ra))
}

/// Polynomial smooth minimum over `k` mm.
fn smin(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// A closed solid whose surface is the zero set of `field` in a local frame, read along rays from the frame's origin:
/// `origin` world, and the frame's forward, up and side units.
struct Blob<'a> {
    name: &'static str,
    origin: P3,
    axes: [P3; 3],
    field: &'a dyn Fn(P3) -> f64,
    /// Meridians round the side axis, and parallels pole to pole.
    around: usize,
    rings: usize,
}

impl Blob<'_> {
    fn build(&self) -> Result<Sculpted> {
        ensure!((self.field)([0.0; 3]) < 0.0, "{}: its centre is outside it", self.name);
        let reach = |d: P3| -> f64 {
            let mut t = 0.0;
            while (self.field)(d.map(|x| x * (t + 0.01))) < 0.0 && t < 12.0 {
                t += 0.01;
            }
            let (mut lo, mut hi) = (t, t + 0.01);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if (self.field)(d.map(|x| x * mid)) < 0.0 { lo = mid } else { hi = mid }
            }
            0.5 * (lo + hi)
        };
        let world = |l: P3| -> P3 { std::array::from_fn(|k| self.origin[k] + l[0] * self.axes[0][k] + l[1] * self.axes[1][k] + l[2] * self.axes[2][k]) };
        let (n, m) = (self.rings, self.around);
        let mut v = Vec::new();
        // Poles on the side axis; meridian angle a round forward/up.
        let dir = |i: usize, j: usize| -> P3 {
            let phi = PI * i as f64 / n as f64;
            let a = 2.0 * PI * j as f64 / m as f64;
            [phi.sin() * a.cos(), phi.sin() * a.sin(), phi.cos()]
        };
        let mut tallest: f64 = 0.0;
        let mut push = |d: P3, v: &mut Vec<P3>| {
            let r = reach(d);
            tallest = tallest.max(r);
            v.push(world(d.map(|x| x * r)));
            v.len() as u32 - 1
        };
        let p0 = push([0.0, 0.0, 1.0], &mut v);
        let mut grid = vec![vec![0u32; m]; n];
        for i in 1..n {
            for j in 0..m {
                grid[i][j] = push(dir(i, j), &mut v);
            }
        }
        let p1 = push([0.0, 0.0, -1.0], &mut v);
        let mut f = Vec::new();
        for j in 0..m {
            let k = (j + 1) % m;
            f.push([p0, grid[1][j], grid[1][k]]);
            f.push([p1, grid[n - 1][k], grid[n - 1][j]]);
            for i in 1..n - 1 {
                f.push([grid[i][j], grid[i + 1][j], grid[i + 1][k]]);
                f.push([grid[i][j], grid[i + 1][k], grid[i][k]]);
            }
        }
        let mut solid = csg::Solid { v, f };
        if solid.volume() < 0.0 {
            for t in &mut solid.f {
                t.swap(1, 2);
            }
        }
        ensure!(solid.open_edges() == (0, 0), "{}: the blob does not close", self.name);
        let crossings = csg::self_crossings(&solid);
        ensure!(crossings == 0, "{}: the blob crosses itself {crossings} times", self.name);
        Ok(Sculpted { name: self.name, solid, outline: Vec::new(), tallest_mm: tallest })
    }
}

/// Where the skull's centre sits along the ring, plan mm, and how far its axis tips down the shank, degrees.
const HEAD_U: f64 = 8.3;
const HEAD_TILT_DEG: f64 = 10.0;
/// The crown of the head stands no higher than the marginal tier of the shell.
const HEAD_CROWN_R: f64 = SHELL_RIM_R + 0.35;
/// How far the crown stands over the head's centre, local mm.
const HEAD_CROWN_Y: f64 = 0.95;

/// The head and neck: a sea turtle's broad blunt head, 3.0 mm long and 2.6 mm wide, under three low head plates, on a
/// neck that leaves the shell under the nuchal through a collar of skin. Local mm: x forward along the ring, y up, z
/// across.
fn head_field(p: P3) -> f64 {
    let cranium = ellipsoid(p, [0.0, 0.15, 0.0], [1.5, 0.8, 1.3]);
    let snout = ellipsoid(p, [0.95, -0.05, 0.0], [0.85, 0.62, 0.98]);
    let jaw = ellipsoid(p, [0.3, -0.4, 0.0], [1.4, 0.5, 1.15]);
    let neck = capsule(p, [-3.6, -0.75, 0.0], [-0.9, -0.1, 0.0], 1.05, 0.95);
    let collar = ellipsoid(p, [-1.75, -0.3, 0.0], [0.38, 1.05, 1.18]);
    let mut d = smin(cranium, snout, 0.55);
    d = smin(d, jaw, 0.45);
    d = smin(d, neck, 0.6);
    d = smin(d, collar, 0.25);
    // The head scales: a frontal behind, a pair of prefrontals before it, each a low pillow off a soft wide seam on
    // the crown only (no scored line).
    let seg = |a: P2, b: P2| seg_dist((p[0], p[2]), a, b);
    let crown = [((0.35, -0.72), (0.35, 0.72)), ((0.35, 0.0), (1.3, 0.0)), ((-0.75, 0.72), (0.35, 0.72)), ((-0.75, -0.72), (0.35, -0.72)), ((0.35, 0.72), (1.25, 0.5)), ((0.35, -0.72), (1.25, -0.5))];
    let seam = crown.iter().map(|&(a, b)| seg(a, b)).fold(f64::MAX, f64::min);
    let on_crown = smoothstep(0.3, 0.7, p[1]);
    d += groove(seam, 0.28, 0.045) * on_crown;
    // The mouth line from the snout back to the jaw's corner, each side.
    let mouth = seg_dist((p[0], p[1]), (1.7, -0.32), (-0.2, -0.38));
    d + groove(mouth, 0.1, 0.06) * smoothstep(0.35, 0.6, p[2].abs())
}

fn head_part(surface: &Surface) -> Result<Sculpted> {
    let up = Surface::radial(HEAD_U);
    let fwd0 = [up[1], -up[0], 0.0];
    let a = HEAD_TILT_DEG.to_radians();
    let fwd: P3 = std::array::from_fn(|k| a.cos() * fwd0[k] - a.sin() * up[k]);
    let up2: P3 = std::array::from_fn(|k| a.sin() * fwd0[k] + a.cos() * up[k]);
    // The head's centre stands where its crown meets the marginal tier's height.
    let origin = surface.point(HEAD_U, 0.0, HEAD_CROWN_R - HEAD_CROWN_Y);
    ensure!(surface.radius(HEAD_U, 0.0).is_some_and(|g| g < HEAD_CROWN_R - HEAD_CROWN_Y), "the head's centre is inside the stock");
    Blob { name: "Head and neck", origin, axes: [fwd, up2, [0.0, 0.0, 1.0]], field: &head_field, around: 160, rings: 80 }.build()
}

/// The tail: a short pointed cone from under the supracaudals down the other cusp.
fn tail_part(surface: &Surface) -> Result<Sculpted> {
    // The tail's root stands up into the shell's rear rim, so it leaves from under the supracaudals with no crevice.
    const FROM: f64 = -6.0;
    const TO: f64 = -10.0;
    let width = |s: f64| 0.9 * (1.0 - 0.5 * s);
    let height = |h: Here| rolled((1.55 - 0.9 * h.s.sqrt()) * (1.0 - 0.5 * h.t * h.t), h.edge, 0.3);
    Strip { name: "Tail", spine: vec![(FROM, 0.0), (TO, 0.0)], width: &width, ends: (0.7, 0.45), height: &height, rows: 60, cols: 40, under: 8, right: None, lens: None }.build(surface)
}

/// An arc in plan from `start`, leaving at `heading` degrees, `length` mm long, turning left on `radius` mm (right when
/// negative).
fn arc(start: P2, heading: f64, length: f64, radius: f64) -> Vec<P2> {
    let h0 = heading.to_radians();
    (0..=64)
        .map(|i| {
            let l = length * i as f64 / 64.0;
            let a = h0 + l / radius;
            let (cx, cy) = (start.0 - radius * h0.sin(), start.1 + radius * h0.cos());
            (cx + radius * a.sin(), cy - radius * a.cos())
        })
        .collect()
}

/// A cheap deterministic hash to 0..1.
fn hash(i: i64, j: i64, salt: i64) -> f64 {
    let mut h = (i.wrapping_mul(73_856_093) ^ j.wrapping_mul(19_349_663) ^ salt.wrapping_mul(83_492_791)) as u64;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h % 10_000) as f64 / 10_000.0
}

/// The distance to the nearest seam of a jittered Voronoi of cells `pitch` mm along by `rows` across a strip, in the
/// strip's own along/across millimetres; the leading-edge row is larger.
fn scales(along: f64, across: f64, pitch: (f64, f64), salt: i64) -> f64 {
    let (gx, gy) = (along / pitch.0, across / pitch.1);
    let (ix, iy) = (gx.floor() as i64, gy.floor() as i64);
    let mut seeds = Vec::with_capacity(16);
    for di in -1..=2 {
        for dj in -1..=2 {
            let (i, j) = (ix + di, iy + dj);
            // Every other row staggered by half a cell, so the scales lie in courses, not a grid.
            let stagger = if j.rem_euclid(2) == 1 { 0.5 } else { 0.0 };
            let sx = (i as f64 + stagger + 0.2 + 0.6 * hash(i, j, salt)) * pitch.0;
            let sy = (j as f64 + 0.25 + 0.5 * hash(i, j, salt + 1)) * pitch.1;
            seeds.push((sx, sy));
        }
    }
    Shell::seam(&seeds, (along, across))
}

/// The fore blade's length along its spine, mm.
const FORE_LEN_MM: f64 = 9.3;
/// No blade grows wider than this either side of its spine, mm.
const FLIPPER_CAP_MM: f64 = 3.2;

/// A flipper on one lobe: `side` +1 for +Z. The fore pair long sickle blades reaching to the lobes' tips, the hind pair
/// broad rounded paddles; each scaled in courses, its leading edge the thicker.
fn flipper_part(surface: &Surface, front: bool, side: f64) -> Result<Sculpted> {
    // Constant-curvature spines down each lobe's long axis: a strip on an arc cannot fold while its half-width stays
    // under the bend radius. The fore blades reach forward and curl in at the tip; the hind paddles trail.
    // Each blade is rooted under the shell's marginals and runs down its lobe's long axis; the fore pair a quarter
    // longer than the hind, reaching the lobe's tip.
    let len = if front { FORE_LEN_MM } else { FORE_LEN_MM / 1.25 };
    let knots = if front { arc((1.6, 3.7), 14.0, len, -40.0) } else { arc((-3.0, 3.7), 166.0, len, 40.0) };
    let spine: Vec<P2> = knots.iter().map(|&(u, v)| (u, side * v)).collect();
    let hk: Vec<P2> = if front { vec![(0.0, 1.25), (0.45, 1.0), (1.0, 0.55)] } else { vec![(0.0, 1.15), (0.5, 0.95), (1.0, 0.6)] };
    // The blade fills its lobe: each side's half-width is read off the lobe's own rim, square to the spine, inset from
    // it and capped where the spine runs under the shell; the fore blade is drawn to a point.
    let n = 300;
    let (mut left, mut right) = (Vec::with_capacity(n + 1), Vec::with_capacity(n + 1));
    let (dense, _) = resample(&spine, n + 1);
    for i in 0..=n {
        let (a, b) = (dense[i.saturating_sub(1)], dense[(i + 1).min(n)]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy).max(1e-9);
        let normal = (-dy / l, dx / l);
        let p = dense[i];
        let reach = |dir: f64| {
            let mut d = 0.0;
            while d < FLIPPER_CAP_MM + LOBE_INSET_MM && surface.radius(p.0 + dir * normal.0 * (d + 0.02), p.1 + dir * normal.1 * (d + 0.02)).is_some_and(|r| r > LOBE_RIM_R) {
                d += 0.02;
            }
            (d - LOBE_INSET_MM).clamp(0.25, FLIPPER_CAP_MM)
        };
        let s = i as f64 / n as f64;
        let point = if front { 1.0 - 0.55 * smoothstep(0.62, 1.0, s) } else { 1.0 };
        left.push(reach(1.0) * point);
        right.push(reach(-1.0) * point);
    }
    let table = |t: &[f64], s: f64| {
        let f = s.clamp(0.0, 1.0) * n as f64;
        let i = (f as usize).min(n - 1);
        t[i] + (f - i as f64) * (t[i + 1] - t[i])
    };
    let width = |s: f64| table(&left, s);
    let rightw = |s: f64| table(&right, s);
    // The leading edge is the outer one: t runs to the spine's left, which is outward (+v) on the +Z side.
    let lead = side;
    let salt = if front { 11 } else { 23 } + if side > 0.0 { 0 } else { 100 };
    let height = |h: Here| {
        let w = if h.t >= 0.0 { width(h.s) } else { rightw(h.s) };
        let x = lead * h.t;
        // A blade: the bony leading edge thick and rounded, thinning to the trailing edge.
        let body = pchip(&hk, h.s) * (1.0 - 0.28 * h.t * h.t) * (1.0 + 0.3 * x);
        // Scales: small courses over the blade, a row of larger plates down the leading edge.
        let lead_row = x > 0.55;
        let pitch = if lead_row { (1.7, 0.95) } else { (1.3, 1.0) };
        let seam = scales(h.along, h.across + 3.0, pitch, salt + lead_row as i64).min((x - 0.55).abs() * w);
        let groove = groove(seam, 0.2, 0.12);
        let pillow = 0.09 * smoothstep(0.0, 0.4, seam);
        // The claw on the leading edge, a hooked cone pointing tipward.
        let cs = if front { 0.42 } else { 0.5 };
        let claw_at = (cs * len, lead * 0.78 * if lead > 0.0 { width(cs) } else { rightw(cs) });
        let dc = ((h.along - claw_at.0) / 0.42).hypot((h.across - claw_at.1) / 0.3);
        let claw = 0.32 * (1.0 - dc.min(1.0)).powf(1.4);
        footed(body + pillow - groove + claw, h.edge, 0.6)
    };
    let name = match (front, side > 0.0) {
        (true, true) => "Fore flipper, +Z",
        (true, false) => "Fore flipper, -Z",
        (false, true) => "Hind flipper, +Z",
        (false, false) => "Hind flipper, -Z",
    };
    Strip { name, spine, width: &width, ends: (0.4, if front { 0.6 } else { 1.6 }), height: &height, rows: 340, cols: 180, under: 10, right: Some(&rightw), lens: None }.build(surface)
}

// --- The shank: hawksbill plates on the shoulders, the plastron on the palm -------------------------------------------

/// The sleeve runs from behind the head round the palm to behind the tail, plan mm along the ring.
const SLEEVE_FROM: f64 = 12.4;
const SLEEVE_TO: f64 = 79.6;
/// The palm's middle, plan mm along the ring (half way round from the table).
const PALM_U: f64 = PI * PLAN_R;
/// The plastron covers the palm this far either side of its middle.
const PLASTRON_HALF: f64 = 11.0;

/// The sleeve's half-width by plan u: the shank's outer face inset from its rounded edges, wider where the shoulders
/// rise into the lobes.
fn sleeve_half(u: f64) -> f64 {
    let d = (u - PALM_U).abs();
    pchip(&[(0.0, 2.2), (24.0, 2.2), (26.0, 2.4), (28.0, 2.75), (30.0, 3.3), (32.8, 4.0)], d)
}

/// The shoulder scales' radius and their rows' pitch along the ring, mm, at the table end and at the plastron.
const SCALE_R: (f64, f64) = (2.1, 1.3);
/// How far each scale rises toward its free edge and steps down onto the scale beneath it, mm.
const SCALE_STEP_MM: f64 = 0.3;

/// A shoulder's height at `x` mm from the table end of its run (palmward positive) and `v` across: hawksbill scales
/// overlapping like roof tiles, rows staggered by half a scale, each a disc whose palmward arc is its free edge and lies
/// over the row beyond it, the scale nearest the table on top.
fn shoulder_scale(x: f64, v: f64, run: f64) -> f64 {
    let mut x0 = -0.5;
    let mut k = 0i64;
    while x0 < run + 2.0 {
        let f = (x0 / run).clamp(0.0, 1.0);
        let r = SCALE_R.0 + (SCALE_R.1 - SCALE_R.0) * f;
        let pitch = 0.62 * r;
        let across = 1.5 * r;
        let shift = if k % 2 == 1 { 0.5 * across } else { 0.0 };
        let j = ((v - shift) / across).round();
        for dj in [-1.0, 0.0, 1.0] {
            let cv = (j + dj) * across + shift;
            let (dx, dv) = (x - x0, v - cv);
            let d = dx.hypot(dv);
            // Inside this disc and on its exposed (palmward) half: this scale is the one seen.
            if d < r && dx > -0.25 * r {
                let to_edge = r - d;
                let rise = SCALE_STEP_MM * (1.0 - (to_edge / (0.9 * r)).min(1.0)).powf(0.8);
                let lip = smoothstep(0.0, 0.2, to_edge);
                let dome = 0.06 * (1.0 - (dv / r).powi(2)).max(0.0);
                // A short keel down the plate's middle toward its free edge, as on a hawksbill's plates.
                let keel = 0.07 * (1.0 - smoothstep(0.0, 0.35, dv.abs())) * smoothstep(-0.2 * r, 0.4 * r, dx);
                return 0.08 + (rise + keel) * lip + dome - 0.04 * (1.0 - lip);
            }
        }
        x0 += pitch;
        k += 1;
    }
    0.08
}

/// The plastron's transverse seams, mm from the palm's middle: three broad pairs of plates between them.
const PLASTRON_SEAMS: [f64; 2] = [-3.7, 3.7];

/// The plastron's height at `x` mm along from the palm's middle and `v` across: three broad pairs of domed plates,
/// straight transverse seams, the long central seam shallow and left for the bench to cut.
fn plastron(x: f64, v: f64, edge: f64) -> f64 {
    let across = PLASTRON_SEAMS.iter().map(|&e| (x - e).abs()).fold(f64::MAX, f64::min);
    let seam = across.min(edge);
    let mid = v.abs();
    let dome = 0.26 * (seam.min(mid + 0.4) / 1.1).min(1.0).sqrt();
    let ring = 0.03 * smoothstep(0.85, 0.95, seam.min(mid + 0.4)) * (1.0 - smoothstep(0.95, 1.05, seam.min(mid + 0.4)));
    0.06 + dome + ring - groove(seam, 0.24, 0.17) - groove(mid, 0.18, 0.06)
}

fn sleeve_part(surface: &Surface) -> Result<Sculpted> {
    let len = SLEEVE_TO - SLEEVE_FROM;
    let width = |s: f64| sleeve_half(SLEEVE_FROM + s * len);
    let run = PALM_U - PLASTRON_HALF - SLEEVE_FROM;
    let height = |h: Here| {
        let d = (h.u - PALM_U).abs();
        let relief = if d <= PLASTRON_HALF {
            plastron(h.u - PALM_U, h.v, h.edge)
        } else {
            let x = run - (d - PLASTRON_HALF);
            // The two shoulders are each other's mirror; the far one is staggered the other way.
            let v = if h.u > PALM_U { -h.v } else { h.v };
            shoulder_scale(x, v, run)
        };
        rolled(0.08 + relief, h.edge, 0.3)
    };
    Strip { name: "Hawksbill plates and plastron", spine: vec![(SLEEVE_FROM, 0.0), (SLEEVE_TO, 0.0)], width: &width, ends: (2.0, 2.0), height: &height, rows: 2200, cols: 144, under: 8, right: None, lens: None }.build(surface)
}

/// Where a lobe's table turns down into its wall, radius mm.
const LOBE_RIM_R: f64 = 13.7;
/// The blades stop this far inside the lobe's rim.
const LOBE_INSET_MM: f64 = 0.6;

fn turtle(surface: &Surface, block_out: bool) -> Result<Vec<Sculpted>> {
    let mut parts = vec![shell_part(surface, block_out)?, head_part(surface)?, tail_part(surface)?];
    let mut flippers = Vec::new();
    for front in [true, false] {
        for side in [1.0, -1.0] {
            flippers.push(flipper_part(surface, front, side)?);
        }
    }
    if !block_out {
        parts.push(sleeve_part(surface)?);
    }
    parts.extend(flippers);
    Ok(parts)
}

/// The quadric error a part's decimation may spend, mm²: about a hundredth of a millimetre off the sculpted surface.
const DECIMATE_COST: f64 = 1.5e-4;

/// A sculpted part decimated where it is flat, held to `DECIMATE_COST`, kept only if it stays closed and uncrossed.
fn decimated(p: Sculpted) -> Sculpted {
    // Backing off to gentler collapses until the result stays closed, uncrossed and of the same volume.
    for (cost, turn) in [(DECIMATE_COST, 12.0), (0.5 * DECIMATE_COST, 8.0), (0.25 * DECIMATE_COST, 5.0)] {
        let nets = ringdesign_core::sculpt::decimate(&p.solid, p.solid.f.len() / 40, cost, 2.0, turn, 25.0);
        let (open, crossings, dv) = (nets.open_edges(), csg::self_crossings(&nets), (nets.volume() - p.solid.volume()).abs() / p.solid.volume().abs());
        if std::env::var_os("CHELONIA_DIAG").is_some() {
            eprintln!("  {} at {cost:.1e}: open {open:?}, crossings {crossings}, volume off {dv:.4}", p.name);
        }
        if open == (0, 0) && crossings == 0 && dv < 0.01 {
            println!("  {}: {} triangles decimated to {} (cost {cost:.1e})", p.name, p.solid.f.len(), nets.f.len());
            return Sculpted { solid: nets, ..p };
        }
    }
    println!("  {}: decimation refused, {} triangles kept", p.name, p.solid.f.len());
    p
}

fn stored_part(p: &Sculpted) -> Result<Operation> {
    let mesh = stored::Packed::encode(&p.solid.v, &p.solid.f, &vec![0; p.solid.f.len()], &[SurfaceKind::Freeform])?;
    let recipe = stored::Recipe {
        kernel: "cataphracta_chelonia".into(),
        op: "sculpted_strip".into(),
        params: json!({ "part": p.name, "plan_radius_mm": PLAN_R, "sink_mm": SINK_MM, "edge_dip_mm": EDGE_DIP_MM }),
        digest: String::new(),
    };
    Ok(Operation::Stored { recipe, sources: Vec::new(), mesh })
}

struct Made {
    names: Vec<String>,
    parts: Vec<Value>,
}

/// The design: the stock, and the turtle joined to it.
fn design(block_out: bool) -> Result<(RingDesign, AlphaLibrary, Made)> {
    let mut d = band()?;
    let surface = Surface::of(&d)?;
    let t = Instant::now();
    // The head's distance-field mesh is left as meshed: decimating it crumpled the neck's root.
    let parts: Vec<Sculpted> = turtle(&surface, block_out)?.into_iter().map(|p| if p.name == "Head and neck" { p } else { decimated(p) }).collect();
    println!("  sculpted {} parts in {:.0} ms", parts.len(), ms(t));
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Factory 007 stock".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let mut made = Vec::new();
    for (k, p) in parts.iter().enumerate() {
        // The kernel lays one fillet round a joined cluster's whole seam, so none is asked for: the flippers carry their own
        // footed edge, and the shell's rim, the head and the tail stand crisp on the stock.
        let blend_mm = 0.0;
        let component = Component { attach: Attach::Join, placement: Placement::Free, blend_mm, ..Component::default() };
        doc.append(Feature { id: 2 + k as u64, name: p.name.into(), enabled: true, operation: stored_part(p)?, component })?;
        made.push(json!({ "name": p.name, "vertices": p.solid.v.len(), "triangles": p.solid.f.len(), "self_crossings": csg::self_crossings(&p.solid), "open_edges": p.solid.open_edges(), "tallest_over_stock_mm": p.tallest_mm, "volume_mm3": p.solid.volume() }));
    }
    let mut lib = AlphaLibrary::default();
    d.bake_all(&mut lib);
    let names = d.cad.as_ref().unwrap().features.iter().map(|f| f.name.clone()).collect();
    Ok((d, lib, Made { names, parts: made }))
}

// --- Gates -----------------------------------------------------------------------------------------------------------

fn release_json(r: &mf::release::ReleaseReport) -> Value {
    json!({ "status": format!("{:?}", r.status), "obstructions": r.obstructions.len(), "unresolved_rays": r.unresolved_rays })
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Which part of the ring a point belongs to, for naming its thin sections.
fn feature_at(p: [f64; 3]) -> &'static str {
    let r = p[0].hypot(p[1]);
    if p[1] < 0.0 || r < 12.0 {
        return "the shank";
    }
    let u = Surface::u_of(p);
    let v = p[2];
    if u > 4.6 && v.abs() < 2.2 {
        return "the head: beak, eyes, neck folds";
    }
    if u < -5.3 && v.abs() < 1.0 {
        return "the tail";
    }
    if (u / 5.4).powi(2) + (v / 4.3).powi(2) < 1.0 {
        return "the carapace: scute seams and marginal rim";
    }
    "the flippers"
}

fn treatment(feature: &str, thinnest: f64) -> String {
    let what = match feature {
        "the head: beak, eyes, neck folds" => "The beak's tip and the eyes' bulges: relief on the head, filling from the neck. Leave as cast; touch the beak with a fine file.",
        "the tail" => "The tail's point on the cusp: fills from the shell's rear. Leave as cast.",
        "the carapace: scute seams and marginal rim" => "The scute seams' groove walls and the marginal rim where it meets the table: fill from the shell. Clean the seams with a fine graver.",
        "the flippers" => "The flippers' trailing edges where they meet the lobes: fill from the flipper's root. Satin brush.",
        _ => "The factory stock's own edges and the seam fillets: as the stock casts.",
    };
    format!("{what} Thinnest single-ray section read here: {thinnest:.3} mm.")
}

/// The land-width census: every face of the finished ring read by one ray along its inward normal, the area under the
/// fill floor grouped by the feature it belongs to, each with its bench treatment.
/// Air behind a thin read shorter than this, and the read is a relief flank, not a fin.
const SUPPORT_GAP_MM: f64 = 0.3;

fn land_widths(built: &mesh::BuildResult) -> Value {
    let m = &built.mesh;
    let solid = solid_of(m);
    let (min_all, under_all) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    let bvh = Bvh::build(m);
    const IN: f64 = 1e-4;
    // Per feature: area, thinnest, faces; then the unsupported share: area and thinnest.
    let mut groups: Vec<(&'static str, f64, f64, usize, f64, f64)> = Vec::new();
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
            let name = feature_at(cen);
            // A flank of relief: past the thin read the same ray is back in metal within SUPPORT_GAP_MM, so the
            // section fills from the metal it stands on. Only a section with air behind it is a fin or a lip.
            let exit: [f64; 3] = std::array::from_fn(|k| o[k] + (t + IN) * inward[k]);
            let r = exit[0].hypot(exit[1]).max(1e-9);
            let down = [-exit[0] / r, -exit[1] / r, 0.0];
            let supported = bvh.ray(m, exit, inward).is_some_and(|(_, g)| g < SUPPORT_GAP_MM) || bvh.ray(m, exit, down).is_some_and(|(_, g)| g < SUPPORT_GAP_MM);
            let (ua, ut) = if supported { (0.0, f64::MAX) } else { (0.5 * twice, section) };
            if !supported && std::env::var_os("CHELONIA_DIAG").is_some() && section < 0.3 {
                eprintln!("unsupported {section:.3} at {cen:.2?} u {:.2} {name}", Surface::u_of(cen));
            }
            match groups.iter_mut().find(|g| g.0 == name) {
                Some(g) => {
                    g.1 += 0.5 * twice;
                    g.2 = g.2.min(section);
                    g.3 += 1;
                    g.4 += ua;
                    g.5 = g.5.min(ut);
                }
                None => groups.push((name, 0.5 * twice, section, 1, ua, ut)),
            }
        }
    }
    groups.sort_by(|a, b| b.1.total_cmp(&a.1));
    let named = groups.iter().map(|g| g.1).sum::<f64>();
    let unsupported = groups.iter().map(|g| g.4).sum::<f64>();
    let thinnest_unsupported = groups.iter().map(|g| g.5).fold(f64::MAX, f64::min);
    json!({
        "floor_mm": MIN_SECTION_MM,
        "method": "dfm::part_sections over the whole finished ring, then the same one-ray read per face grouped by the feature it lies in; a read is 'supported' when the ray, once out, re-enters metal within support_gap_mm, or metal lies within it straight beneath the exit toward the finger axis (a relief flank on the metal it fills from), else it is a fin or lip with air behind it",
        "support_gap_mm": SUPPORT_GAP_MM,
        "unsupported_area_mm2": unsupported,
        "unsupported_thinnest_mm": if thinnest_unsupported == f64::MAX { Value::Null } else { json!(thinnest_unsupported) },
        "thinnest_mm": min_all,
        "area_under_floor_mm2": under_all,
        "area_named_mm2": named,
        "all_named": (named - under_all).abs() < 1e-3 * under_all.max(1.0) + 1e-6,
        "by_feature": groups.iter().map(|g| json!({ "feature": g.0, "area_mm2": g.1, "thinnest_mm": g.2, "faces": g.3, "unsupported_area_mm2": g.4, "unsupported_thinnest_mm": if g.5 == f64::MAX { Value::Null } else { json!(g.5) }, "bench": treatment(g.0, g.2) })).collect::<Vec<_>>(),
    })
}

struct Gated {
    json: Value,
    passed: bool,
    built: mesh::BuildResult,
    pattern: mesh::BuildResult,
    inspection: mf::Inspection,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, p: BuildParams, label: &str, made: &Made) -> Result<Gated> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = ms(t);
    let v = &built.report.validation;
    let degenerate = built.report.quality.degenerate_faces;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    if crossings > 0 {
        for c in ringdesign_core::sculpt::crossing_sites(&solid_of(&built.mesh)) {
            println!("  crossing at {c:.3?}, plan u {:.2}, r {:.2}", Surface::u_of(c), c[0].hypot(c[1]));
        }
    }
    let part_crossings: usize = made.parts.iter().map(|p| p["self_crossings"].as_u64().unwrap_or(1) as usize).sum();
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    let nearest = built.mesh.vertices.iter().map(|q| (q.0 as f64).hypot(q.1 as f64) - bore).fold(f64::MAX, f64::min);
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let dfm = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report(d, 0.0).map_or(0, |r| r.stone_count as usize);
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
    let lands = land_widths(&built);
    let lands_ok = lands["all_named"].as_bool().unwrap_or(false);
    let joined = built.parts.joined;
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && degenerate == 0),
        ("0 self-crossings on the ring and on every made part", crossings == 0 && part_crossings == 0),
        ("solids and parts notes empty, every part joined", built.solids.notes.is_empty() && built.parts.notes.is_empty() && joined == made.parts.len() && built.solids.stamped == d.stamps.len()),
        ("nothing inside the finger hole", inside == 0),
        ("field Castable under lost wax at the 0.8 mm fill", field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && d.draft.min_section_mm >= MIN_SECTION_MM),
        ("land widths: every section under 0.8 mm named with its bench treatment", lands_ok),
        ("0 DFM findings", dfm.is_empty()),
        ("stones report equals the preview", stones == preview_stones),
        ("within 2 million triangles", triangles <= 2_000_000),
        ("casting pattern closed, 0 degenerate, 0 crossings", pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0),
    ];
    let passed = list.iter().all(|g| g.1);
    println!("[{label}] {triangles} triangles in {build_ms:.0} ms, {joined} parts joined");
    for (g, ok) in &list {
        println!("  {} {g}", if *ok { "pass" } else { "FAIL" });
    }
    for n in built.solids.notes.iter().chain(&built.parts.notes) {
        println!("  note: {n}");
    }
    println!("  field {} (thinnest wall {:.2} mm) {:?}", field.verdict.label(), field.thinnest_wall_mm, field.notes);
    println!("  release 0.100: {} obstructions, {} unresolved; 0.075: {} obstructions, {} unresolved", r.obstructions.len(), r.unresolved_rays, release_fine.obstructions.len(), release_fine.unresolved_rays);
    println!("  lands: thinnest {:.3} mm, {:.2} mm2 under the floor", lands["thinnest_mm"].as_f64().unwrap_or(0.0), lands["area_under_floor_mm2"].as_f64().unwrap_or(0.0));
    for f in &dfm {
        println!("  dfm: {}: {}", f.label, f.message);
    }
    let json = json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": triangles, "ms": build_ms },
        "process": format!("{:?}", d.draft.process),
        "draft_rules": { "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3 },
        "made": { "stamps": d.stamps.len(), "stamped": built.solids.stamped, "notes": built.solids.notes, "parts_notes": built.parts.notes, "made_parts": made.parts.len(), "parts_joined": joined, "parts_self_crossings": part_crossings, "parts": made.parts },
        "finger_hole": { "bore_radius_mm": bore, "vertices_inside": inside, "nearest_margin_mm": nearest },
        "field": { "verdict": field.verdict.label(), "process": format!("{:?}", field.process), "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes },
        "land_widths": lands,
        "release": {
            "note": "Lost wax: the ray release measures a two-part pull; reported, as the lost-wax rings report it.",
            "release_0100": release_json(r),
            "release_0075": release_json(&release_fine),
        },
        "two_part_numbers": {
            "note": "Lost wax: these measure a two-part sand pull and are reported, not gated.",
            "undercut_area_mm2": field.undercut_area_mm2,
            "marginal_area_mm2": field.marginal_area_mm2,
            "vertical_area_mm2": field.vertical_area_mm2,
            "total_area_mm2": field.total_area_mm2,
            "worst_draft_deg": field.worst_draft_deg,
        },
        "draft_clamp": { "note": "Lost wax and no painted layer: no draft clamp runs, so no bite.", "max_bite_mm": 0.0 },
        "dfm_findings": dfm.iter().map(|f| json!({ "label": f.label, "message": f.message })).collect::<Vec<_>>(),
        "stones": { "report_count": stones, "preview_count": preview_stones },
        "casting_pattern": { "watertight": pv.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings, "triangles": pattern.mesh.faces.len() },
        "gates": list.iter().map(|(g, ok)| json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "gates_passed": passed,
    });
    Ok(Gated { json, passed, built, pattern, inspection })
}

// --- Renders ---------------------------------------------------------------------------------------------------------

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The part of a mesh between two ring angles, degrees, reindexed.
fn arc_of(m: &mesh::Mesh, from: f64, to: f64) -> mesh::Mesh {
    let keep: Vec<bool> = m.vertices.iter().map(|v| (v.1 as f64).atan2(v.0 as f64).to_degrees().rem_euclid(360.0)).map(|a| a >= from && a <= to).collect();
    let mut map = vec![u32::MAX; m.vertices.len()];
    let mut out = mesh::Mesh::default();
    for f in &m.faces {
        if f.iter().all(|&i| keep[i as usize]) {
            let g = f.map(|i| {
                if map[i as usize] == u32::MAX {
                    map[i as usize] = out.vertices.len() as u32;
                    out.vertices.push(m.vertices[i as usize]);
                    out.normals.push(m.normals.get(i as usize).copied().unwrap_or(m.vertices[i as usize]));
                }
                map[i as usize]
            });
            out.faces.push(g);
        }
    }
    out
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
    let tiles: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3], VIEWS[2]].iter().map(|(_, y, pt)| render::render_parts_ss(&parts, *y, *pt, 300, 300, 3)).collect();
    let mut sheet = vec![0u8; 600 * 600 * 3];
    for (k, t) in tiles.iter().enumerate() {
        let (ox, oy) = ((k % 2) * 300, (k / 2) * 300);
        for y in 0..300 {
            sheet[((oy + y) * 600 + ox) * 3..((oy + y) * 600 + ox + 300) * 3].copy_from_slice(&t[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 600, 600, image::ColorType::Rgb8)?;
    // No stone: stones.png is the head and shell close up from the head's side.
    render::write_png_parts(out.join("stones.png"), &parts, -0.35, 1.2, edge)?;
    // Close-ups framed on a point of the whole ring (never a cropped mesh), so every edge keeps its crease normals.
    let at = |u: f64, v: f64, r: f64| -> [f64; 3] {
        let th = Surface::theta(u);
        [r * th.cos(), r * th.sin(), v]
    };
    let close = |name: &str, u: f64, v: f64, r: f64, half: f64, pitch: f64| {
        let th = Surface::theta(u).to_degrees();
        render::write_png_framed(out.join(name), &parts, render::yaw_facing(th), pitch, render::Framing::new(at(u, v, r), half), 1600)
    };
    close("shell-close.png", 0.0, 0.0, 15.5, 6.5, 0.5 * PI)?;
    close("head-close.png", HEAD_U - 1.0, 0.0, 14.0, 3.5, 1.0)?;
    close("flipper-close.png", 6.0, 5.5, 14.2, 4.0, 0.5 * PI)?;
    close("scales-close.png", 22.0, 0.0, 11.3, 4.5, 0.5 * PI)?;
    close("plastron-close.png", PALM_U, 0.0, 11.3, 6.0, 0.5 * PI)?;
    let bare = band()?;
    let b = mesh::try_build(&bare, lib, p)?;
    let e = if draft { 700 } else { 1000 };
    let left = render::render_parts_ss(&[Part::metal(&b.mesh, render::GOLD)], HERO.0, HERO.1, e, e, 3);
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, e, e, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &left, &right, e)?;
    Ok(())
}

fn write(out: &Path, draft: bool, verify: bool, block_out: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let (mut d, lib, made) = design(block_out)?;
    let p = params(draft);
    d.build = p;
    let mut blocks = serde_json::Map::new();
    let main = gates(&d, &lib, p, if draft { "draft 768 x 320" } else { "export 1536 x 448" }, &made)?;
    let mut passed = main.passed;
    if !draft {
        let dr = gates(&d, &lib, params(true), "draft 768 x 320", &made)?;
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
    let bytes = std::fs::metadata(out.join("design.ring.json")).map_or(0, |m| m.len());
    let report = json!({
        "ring": d.name,
        "slug": SLUG,
        "process": "lost wax",
        "base": "factory 007 Quatrefoil, unmirrored (Fallback B), face 16 x 17 mm",
        "base_decision": {
            "by": "Logan",
            "recorded": "docs/collections/cataphracta.md header (2026-09-24: 'Chelonia (007 Quatrefoil) and Phrynosoma (016 Star) go lost wax on the real factory stock, not procedural heads') and the lane's TASK.md ('Logan chose lost wax on the real factory 007 Quatrefoil stock (Fallback B), unmirrored, not a procedural head')",
            "route": "Fallback B: the real 007 stock, unmirrored, lost wax",
        },
        "release_gate": {
            "status": "not applicable under lost wax, by the doctrine's own rule; reported, never gated",
            "rule": "CLAUDE.md, 'Manufacturing analysis speaks in the sand's numbers': under lost wax the pull statistics are still measured and reported (with an explicit 'cannot move to sand as-is' note when undercut exists) but never gate; only fill and detail do, at investment floors",
            "process_decision": "Logan, docs/collections/cataphracta.md header (2026-09-24): \"Chelonia (007 Quatrefoil) and Phrynosoma (016 Star) go lost wax on the real factory stock\"",
            "note": "No explicit per-gate waiver from Logan is quoted: none exists in the repository. The counts sit in each block's release field.",
        },
        "template_class": {
            "class": "painted",
            "why": "brief.md, Template gate: 'template_class: procedural for an unpainted node-built ring, stock for bare factory stock, painted otherwise'. Chelonia is factory stock carrying eight made parts, not bare stock, so it is 'painted' by that rule (3 MB budget); it carries no painted layer.",
        },
        "stage": if block_out { "block-out" } else { "full" },
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "features": made.names,
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
    std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": [], "note": "No stone: the carapace is the jewel." }))?)?;
    renders(out, &d, &lib, &main.built, p, draft)?;
    println!("gates {}", if passed { "all pass" } else { "FAILED" });
    ensure!(passed, "gates failed");
    Ok(())
}

/// A quick look while sculpting: the draft build and a few views, no gates.
fn quick(out: &Path, block_out: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let (d, lib, _) = design(block_out)?;
    let t = Instant::now();
    let built = mesh::try_build(&d, &lib, params(true))?;
    println!("  built in {:.0} ms; notes {:?}", ms(t), built.parts.notes);
    let parts = vec![Part::metal(&built.mesh, render::GOLD)];
    render::write_png_parts(out.join("hero.png"), &parts, HERO.0, HERO.1, 900)?;
    render::write_png_parts(out.join("face.png"), &parts, 0.0, PI * 0.5, 900)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, HERO.0, HERO.1, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    render::write_png_parts(out.join("palm.png"), &parts, PI, PI * 0.5, 900)?;
    render::write_png_parts(out.join("shoulder.png"), &parts, 1.25, 1.05, 900)?;
    render::write_png_parts(out.join("side.png"), &parts, 0.0, 0.0, 900)?;
    if let Ok(arc) = std::env::var("CHELONIA_ZOOM") {
        // A close-up of one arc of the ring, degrees from..to, seen from its own outside.
        let mut it = arc.split(',').map(|x| x.parse::<f64>().unwrap_or(0.0));
        let (from, to) = (it.next().unwrap_or(0.0), it.next().unwrap_or(90.0));
        let piece = arc_of(&built.mesh, from, to);
        let mid = (0.5 * (from + to)).to_radians();
        render::write_png_parts(out.join("zoom.png"), &[Part::metal(&piece, render::GOLD)], 0.5 * PI - mid, 0.5 * PI, 1400)?;
    }
    if std::env::var_os("CHELONIA_PLAN").is_some() {
        let surface = Surface::of(&band()?)?;
        for vi in (-18..=18).rev() {
            let v = vi as f64 * 0.5;
            let mut line = format!("{v:+5.1} ");
            for ui in -26..=26 {
                let u = ui as f64 * 0.5;
                line.push(match surface.radius(u, v) {
                    Some(r) if r > 14.2 => '#',
                    Some(r) if r > 13.8 => '%',
                    Some(r) if r > 13.4 => '=',
                    Some(r) if r > 13.0 => '-',
                    Some(r) if r > 12.0 => '.',
                    Some(_) => ',',
                    None => ' ',
                });
            }
            println!("{line}");
        }
    }
    if std::env::var_os("CHELONIA_SHANK").is_some() {
        let surface = Surface::of(&band()?)?;
        for k in 0..=36 {
            let u = 6.0 + k as f64 * 2.0;
            let r0 = surface.radius(u, 0.0).unwrap_or(0.0);
            let mut edge = 0.0;
            let mut v = 0.0;
            while v < 10.0 {
                match (surface.radius(u, v), surface.radius(u, -v)) {
                    (Some(a), Some(b)) if a > r0 - 0.25 && b > r0 - 0.25 => edge = v,
                    _ => break,
                }
                v += 0.05;
            }
            let prof: Vec<String> = (0..8).map(|i| format!("{:.2}", surface.radius(u, i as f64 * 0.5).unwrap_or(0.0) - r0)).collect();
            println!("u {u:5.1} r0 {r0:6.3} face half-width {edge:.2} drop {}", prof.join(" "));
        }
    }
    if std::env::var_os("CHELONIA_SWEEP").is_some() {
        for k in 0..8 {
            let yaw = k as f64 * PI / 4.0;
            render::write_png_parts(out.join(format!("yaw{k}.png")), &parts, yaw, HERO.1, 400)?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("showcase/cataphracta/{SLUG}")));
    if flag("--quick") {
        return quick(&out, flag("--block-out"));
    }
    write(&out, flag("--draft"), flag("--verify"), flag("--block-out"))
}
