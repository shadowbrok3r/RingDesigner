//! Vepres — Viscum, the golden bough: a jointed mistletoe bough laid along the crest of factory 003 Clover, its
//! strap leaves in splayed pairs at every node and its white berries (21 moonstone cabochons) clustered in the
//! forks. Lost wax on the native stock, unmirrored, with no sand envelope.
//! cargo build --release -p ringdesign-core --example vepres_viscum
//! target/release/examples/vepres_viscum [OUT_DIR] [--draft] [--verify] [--blockout]
#![recursion_limit = "256"]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{Attach, Component, Document, Feature, Operation, Placement, SurfaceKind, measure, stored},
    castability, csg, dfm,
    alpha::{ProcRecipe, Procedural},
    field::{Blend, Layer, LayerEntry, SeatPadLayer, SeatStyle},
    tiling::{ChartSpace, TilingLayer},
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render,
    setting::{SolidKind, Stamp, StampTop},
    skin::Atlas,
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

const SLUG: &str = "viscum";
const AW: usize = 2048;
const AH: usize = 768;
const BORE_MM: f64 = 18.6;
/// The factory stock and its face, length round the ring by width across it, mm.
const STOCK_ID: &str = "001";
const STOCK_FACE: (f64, f64) = (17.0, 14.5);
/// Whether the stock comes through its sand master (the envelope on): stamps strike only there.
const STOCK_SAND_MASTER: bool = true;
/// Moonstone's milky white with a cold blue sheen: the berry.
const MOONSTONE_TINT: [f32; 3] = [0.95, 0.95, 0.96];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// Investment casting in 18k yellow gold at the lost-wax floors.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe.process = castability::CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.name = "Viscum / investment / Gold 18k".into();
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = 0.8;
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Factory 003 Clover at native 18 x 18, lost wax, sprued at the palm. The bough, its leaves and its \
        nodes are cast with the ring. After the pour: drill each berry seat on its raised mark, cut it to the measured \
        moonstone and burnish flush. Polish the leaves and the bough; leave the lobes' ground as cast and pumiced."
        .into();
    s
}

/// Factory 003 Clover at its native 18 x 18 face, not mirrored, no sand envelope, on its own Flat chart.
fn stock() -> Result<RingDesign> {
    let id = std::env::var("VISCUM_STOCK").unwrap_or_else(|_| STOCK_ID.into());
    let face: Vec<f64> = std::env::var("VISCUM_FACE").ok().map(|v| v.split('x').filter_map(|x| x.parse().ok()).collect()).unwrap_or_else(|| vec![STOCK_FACE.0, STOCK_FACE.1]);
    let preset = PRESETS.iter().find(|p| p.id == id).with_context(|| format!("no stock {id}"))?;
    let mut d = RingDesign::default();
    let sand = std::env::var("VISCUM_SAND").is_ok() || STOCK_SAND_MASTER;
    ImportedBase::attach(&mut d, if sand { ringdesign_core::imported_base::sand_master(preset.load()?)? } else { preset.load()? })?;
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    d.name = "Viscum \u{2014} the golden bough".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.shank.head.length_mm = face[0];
    d.profile.width_mm = face[1];
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    // The chart comes from THIS stock before anything is drawn on it.
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = export_params();
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.manufacturing = Some(s);
    Ok(d)
}

// --- Small vector help -------------------------------------------------------------------------------------------

fn add2(a: P2, b: P2, k: f64) -> P2 {
    [a[0] + b[0] * k, a[1] + b[1] * k]
}
fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: P2) -> f64 {
    a[0].hypot(a[1])
}
fn dir2(deg: f64) -> P2 {
    let (s, c) = deg.to_radians().sin_cos();
    [c, s]
}
fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Monotone cubic through `knots` (sorted by x), flat beyond the ends.
fn pchip(knots: &[(f64, f64)], x: f64) -> f64 {
    let n = knots.len();
    if x <= knots[0].0 {
        return knots[0].1;
    }
    if x >= knots[n - 1].0 {
        return knots[n - 1].1;
    }
    let i = knots.partition_point(|k| k.0 <= x).clamp(1, n - 1) - 1;
    let slope = |j: usize| (knots[j + 1].1 - knots[j].1) / (knots[j + 1].0 - knots[j].0);
    let tangent = |j: usize| -> f64 {
        if j == 0 {
            return slope(0);
        }
        if j == n - 1 {
            return slope(n - 2);
        }
        let (a, b) = (slope(j - 1), slope(j));
        if a * b <= 0.0 { 0.0 } else { 2.0 / (1.0 / a + 1.0 / b) }
    };
    let (x0, x1) = (knots[i].0, knots[i + 1].0);
    let h = x1 - x0;
    let t = (x - x0) / h;
    let (y0, y1) = (knots[i].1, knots[i + 1].1);
    let (m0, m1) = (tangent(i) * h, tangent(i + 1) * h);
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * y0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * y1 + (t3 - t2) * m1
}

// --- The bare surface as depth maps ------------------------------------------------------------------------------

/// How a plan point `(p, q)` and a height stand in the world.
#[derive(Clone, Copy)]
enum Frame {
    /// A flat plan: `p` along `u`, `q` along `v`, depth along `n`.
    Plane { u: P3, v: P3, n: P3 },
    /// Round the finger: `p` is arc at radius `r0` from the head's centre line (+Y), `q` is along the finger, depth is the radius.
    Cylinder { r0: f64 },
}

impl Frame {
    /// A world point as `(p, q, depth)`.
    fn project(&self, x: P3) -> [f64; 3] {
        match *self {
            Frame::Plane { u, v, n } => [dot3(x, u), dot3(x, v), dot3(x, n)],
            Frame::Cylinder { r0 } => {
                let theta = x[1].atan2(x[0]);
                let mut rel = theta - FRAC_PI_2;
                if rel <= -PI {
                    rel += TAU;
                }
                if rel > PI {
                    rel -= TAU;
                }
                [rel * r0, x[2], x[0].hypot(x[1])]
            }
        }
    }
    fn world(&self, p: f64, q: f64, depth: f64) -> P3 {
        match *self {
            Frame::Plane { u, v, n } => std::array::from_fn(|k| u[k] * p + v[k] * q + n[k] * depth),
            Frame::Cylinder { r0 } => {
                let (s, c) = (FRAC_PI_2 + p / r0).sin_cos();
                [depth * c, depth * s, q]
            }
        }
    }
}

/// The bare stock's outermost surface over a plan grid, read along the frame's depth.
struct Depth {
    frame: Frame,
    p0: f64,
    q0: f64,
    step: f64,
    w: usize,
    h: usize,
    data: Vec<f64>,
}

impl Depth {
    /// Rasterise the atlas's quads into a grid over `[p0, p1] x [q0, q1]`, keeping the furthest surface.
    fn of(a: &Atlas, frame: Frame, p: (f64, f64), q: (f64, f64), step: f64) -> Self {
        let w = ((p.1 - p.0) / step).ceil() as usize + 1;
        let h = ((q.1 - q.0) / step).ceil() as usize + 1;
        let mut data = vec![f64::NAN; w * h];
        let proj: Vec<[f64; 3]> = a.samples.iter().map(|s| frame.project(s.p)).collect();
        for y in 0..a.height - 1 {
            for x in 0..a.width {
                let x1 = (x + 1) % a.width;
                let c = [proj[y * a.width + x], proj[y * a.width + x1], proj[(y + 1) * a.width + x], proj[(y + 1) * a.width + x1]];
                for tri in [[c[0], c[1], c[3]], [c[0], c[3], c[2]]] {
                    let (lo_p, hi_p) = (tri.iter().map(|t| t[0]).fold(f64::MAX, f64::min), tri.iter().map(|t| t[0]).fold(f64::MIN, f64::max));
                    let (lo_q, hi_q) = (tri.iter().map(|t| t[1]).fold(f64::MAX, f64::min), tri.iter().map(|t| t[1]).fold(f64::MIN, f64::max));
                    if hi_p < p.0 || lo_p > p.1 || hi_q < q.0 || lo_q > q.1 || hi_p - lo_p > 2.0 || hi_q - lo_q > 2.0 {
                        continue;
                    }
                    let i0 = ((lo_p - p.0) / step).ceil().max(0.0) as usize;
                    let i1 = (((hi_p - p.0) / step).floor() as isize).min(w as isize - 1);
                    let j0 = ((lo_q - q.0) / step).ceil().max(0.0) as usize;
                    let j1 = (((hi_q - q.0) / step).floor() as isize).min(h as isize - 1);
                    if i1 < i0 as isize || j1 < j0 as isize {
                        continue;
                    }
                    let (a0, b0, c0) = (tri[0], tri[1], tri[2]);
                    let den = (b0[1] - c0[1]) * (a0[0] - c0[0]) + (c0[0] - b0[0]) * (a0[1] - c0[1]);
                    if den.abs() < 1e-14 {
                        continue;
                    }
                    for j in j0..=j1 as usize {
                        for i in i0..=i1 as usize {
                            let (pp, qq) = (p.0 + i as f64 * step, q.0 + j as f64 * step);
                            let l0 = ((b0[1] - c0[1]) * (pp - c0[0]) + (c0[0] - b0[0]) * (qq - c0[1])) / den;
                            let l1 = ((c0[1] - a0[1]) * (pp - c0[0]) + (a0[0] - c0[0]) * (qq - c0[1])) / den;
                            let l2 = 1.0 - l0 - l1;
                            if l0 < -1e-9 || l1 < -1e-9 || l2 < -1e-9 {
                                continue;
                            }
                            let dd = l0 * a0[2] + l1 * b0[2] + l2 * c0[2];
                            let cell = &mut data[j * w + i];
                            if cell.is_nan() || dd > *cell {
                                *cell = dd;
                            }
                        }
                    }
                }
            }
        }
        Self { frame, p0: p.0, q0: q.0, step, w, h, data }
    }

    fn depth(&self, p: f64, q: f64) -> Option<f64> {
        let fx = (p - self.p0) / self.step;
        let fy = (q - self.q0) / self.step;
        if fx < 0.0 || fy < 0.0 {
            return None;
        }
        let (i, j) = (fx.floor() as usize, fy.floor() as usize);
        if i + 1 >= self.w || j + 1 >= self.h {
            return None;
        }
        let (tx, ty) = (fx - i as f64, fy - j as f64);
        let g = |a: usize, b: usize| self.data[b * self.w + a];
        let (a, b, c, e) = (g(i, j), g(i + 1, j), g(i, j + 1), g(i + 1, j + 1));
        if a.is_nan() || b.is_nan() || c.is_nan() || e.is_nan() {
            return None;
        }
        Some((a + (b - a) * tx) * (1.0 - ty) + (c + (e - c) * tx) * ty)
    }

    /// The surface `h` mm out from the bare stock at a plan point.
    fn world(&self, p: f64, q: f64, h: f64) -> Option<P3> {
        self.depth(p, q).map(|d| self.frame.world(p, q, d + h))
    }
}

// --- Sculpted parts: a pillow draped on the stock ----------------------------------------------------------------

/// A plan curve by arc length.
struct Spine {
    pts: Vec<P2>,
    cum: Vec<f64>,
}

impl Spine {
    fn new(pts: Vec<P2>) -> Self {
        let mut cum = vec![0.0];
        for k in 1..pts.len() {
            cum.push(cum[k - 1] + len2(sub2(pts[k], pts[k - 1])));
        }
        Self { pts, cum }
    }
    fn len(&self) -> f64 {
        *self.cum.last().unwrap()
    }
    /// The point and unit tangent at fraction `s` of the length.
    fn at(&self, s: f64) -> (P2, P2) {
        let l = s.clamp(0.0, 1.0) * self.len();
        let k = self.cum.partition_point(|c| *c < l).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[k - 1], self.pts[k]);
        let seg = (self.cum[k] - self.cum[k - 1]).max(1e-12);
        let t = (l - self.cum[k - 1]) / seg;
        let d = sub2(b, a);
        let dl = len2(d).max(1e-12);
        (add2(a, d, t), [d[0] / dl, d[1] / dl])
    }
}

/// A quadratic Bezier from `a` through control `c` to `b`, sampled finely.
fn bezier(a: P2, c: P2, b: P2) -> Vec<P2> {
    (0..=64)
        .map(|i| {
            let t = i as f64 / 64.0;
            let u = 1.0 - t;
            [u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1]]
        })
        .collect()
}

/// What a part stands on.
enum Ground<'a> {
    /// Draped on the stock: the top over the stock, the bottom `sink` under it.
    Drape { sink: f64 },
    /// Riding over hollows: the top over the highest stock within `reach` of the point, the bottom `sink` under
    /// the stock itself, so a stem bridges a crease instead of lying in it.
    Bridge { sink: f64, reach: f64 },
    /// A free blade: the top over its own rest surface, and its own underside `under` below that wherever the
    /// stock falls away more than `gap` beneath it; elsewhere the bottom sinks into the stock.
    Free { rest: &'a dyn Fn(f64, f64) -> f64, under: f64, sink: f64, gap: f64 },
}

/// A part's plan and top: half-width along the spine (fraction 0..1), and the height over its ground at fraction
/// `s`, share `t` of the half-width out from the spine (1 at the margin), with `w` the half-width there.
struct Pillow<'a> {
    spine: Spine,
    half: &'a dyn Fn(f64) -> f64,
    top: &'a dyn Fn(f64, f64, f64) -> f64,
    ground: Ground<'a>,
    around: usize,
}

/// How far a free blade's underside rises to its rounded bottom edge, mm.
const UNDER_ROUND_MM: f64 = 0.12;

/// Rings from the spine out to the margin, top first then bottom: the outline shrunk across by `t` and along by a
/// matching share, so the contours nest like offsets. Closed by a ladder across the spine top and bottom.
fn pillow(map: &Depth, pl: &Pillow) -> Result<csg::Solid> {
    const TOP: [f64; 14] = [0.06, 0.16, 0.28, 0.4, 0.52, 0.63, 0.73, 0.81, 0.875, 0.925, 0.96, 0.982, 0.995, 1.0];
    const BOTTOM: [f64; 6] = [1.0, 0.99, 0.965, 0.9, 0.7, 0.35];
    let n = pl.around - pl.around % 2;
    let l = pl.spine.len();
    let wmax = (0..=200).map(|k| (pl.half)(k as f64 / 200.0)).fold(0.0, f64::max);
    let along_k = (wmax / (0.5 * l)).min(0.9);
    let mut v: Vec<P3> = Vec::new();
    let mut ring = |t: f64, bottom: bool| -> Result<()> {
        let e = 1.0 - (1.0 - t) * along_k;
        for i in 0..n {
            let phi = TAU * i as f64 / n as f64;
            let sf = 0.5 * (1.0 - phi.cos());
            let side = if i < n / 2 { 1.0 } else { -1.0 };
            let sp = 0.5 + (sf - 0.5) * e;
            let w = (pl.half)(sf);
            let (c, tan) = pl.spine.at(sp);
            let nrm = [-tan[1], tan[0]];
            let across = if i == 0 || i == n / 2 { 0.0 } else { side * t * w };
            let at = add2(c, nrm, across);
            let (p, q) = (at[0], at[1]);
            let depth = || map.depth(p, q).with_context(|| format!("no stock under {at:?}"));
            let level = match &pl.ground {
                Ground::Drape { sink } => {
                    let d = depth()?;
                    if bottom { d - sink } else { d + (pl.top)(sf, t, w) }
                }
                Ground::Bridge { sink, reach } => {
                    let d = depth()?;
                    if bottom {
                        d - sink
                    } else {
                        let high = (0..16)
                            .filter_map(|k| {
                                let o = add2(at, dir2(22.5 * k as f64), *reach);
                                map.depth(o[0], o[1])
                            })
                            .fold(d, f64::max);
                        high.min(d + 1.5) + (pl.top)(sf, t, w)
                    }
                }
                Ground::Free { rest, under, sink, gap } => {
                    let r = rest(p, q);
                    if bottom {
                        let m = (1.0 - t) * w;
                        let rb = UNDER_ROUND_MM.min(0.45 * w);
                        let lift = if m < rb { rb - (rb * rb - (rb - m) * (rb - m)).max(0.0).sqrt() } else { 0.0 };
                        let own = r - under - 0.15 * (1.0 - t * t) + lift;
                        match map.depth(p, q) {
                            Some(d) if d > r - under - gap => d - sink,
                            _ => own,
                        }
                    } else {
                        r + (pl.top)(sf, t, w)
                    }
                }
            };
            v.push(map.frame.world(p, q, level));
        }
        Ok(())
    };
    // Rings run from the top's centre down to the bottom's centre.
    for t in TOP {
        ring(t, false)?;
    }
    for t in BOTTOM {
        ring(t, true)?;
    }
    let rings = TOP.len() + BOTTOM.len();
    let at = |r: usize, i: usize| (r * n + i % n) as u32;
    let mut f: Vec<[u32; 3]> = Vec::new();
    for r in 0..rings - 1 {
        for i in 0..n {
            f.push([at(r, i), at(r, i + 1), at(r + 1, i + 1)]);
            f.push([at(r, i), at(r + 1, i + 1), at(r + 1, i)]);
        }
    }
    // Ladders: the first ring (top centre) and the last (bottom centre), pairs mirrored across the spine.
    let ladder = |r: usize, f: &mut Vec<[u32; 3]>, flip: bool| {
        let mut push = |t: [u32; 3]| f.push(if flip { [t[0], t[2], t[1]] } else { t });
        for i in 0..n / 2 - 1 {
            push([at(r, i), at(r, n - i - 1), at(r, i + 1)]);
        }
        for i in 1..n / 2 {
            push([at(r, i), at(r, n - i), at(r, n - i - 1)]);
        }
    };
    ladder(0, &mut f, false);
    ladder(rings - 1, &mut f, true);
    let mut s = csg::Solid { v, f };
    if s.volume() < 0.0 {
        for t in &mut s.f {
            t.swap(1, 2);
        }
    }
    ensure!(s.open_edges() == (0, 0), "a pillow does not close: {:?}", s.open_edges());
    Ok(s)
}

/// A smooth rest surface for a free blade: a quadric fitted to the stock under the blade's plan, raised so the
/// stock nowhere stands through it, so the blade lies on the lobe and runs on past its edge in the same curve.
fn rest_surface(map: &Depth, spine: &Spine, half: &dyn Fn(f64) -> f64) -> Result<impl Fn(f64, f64) -> f64 + use<>> {
    let mut pts: Vec<[f64; 3]> = Vec::new();
    for i in 0..=40 {
        let s = i as f64 / 40.0;
        let (c, tan) = spine.at(s);
        let nrm = [-tan[1], tan[0]];
        for j in -4..=4 {
            let at = add2(c, nrm, half(s) * j as f64 / 4.0);
            if let Some(d) = map.depth(at[0], at[1]) {
                pts.push([at[0], at[1], d]);
            }
        }
    }
    // Only the stock the blade truly lies on: drop samples far below the highest (a lobe's wall seen from above).
    let top = pts.iter().map(|p| p[2]).fold(f64::MIN, f64::max);
    pts.retain(|p| p[2] > top - 1.6);
    ensure!(pts.len() >= 12, "a free blade has too little stock under it");
    let (cx, cy) = (pts.iter().map(|p| p[0]).sum::<f64>() / pts.len() as f64, pts.iter().map(|p| p[1]).sum::<f64>() / pts.len() as f64);
    let basis = move |x: f64, y: f64| {
        let (x, y) = (x - cx, y - cy);
        [1.0, x, y, x * x, x * y, y * y]
    };
    let mut a = [[0.0f64; 7]; 6];
    for p in &pts {
        let b = basis(p[0], p[1]);
        for r in 0..6 {
            for c in 0..6 {
                a[r][c] += b[r] * b[c];
            }
            a[r][6] += b[r] * p[2];
        }
    }
    for r in 0..6 {
        a[r][r] += 1e-6;
    }
    for col in 0..6 {
        let piv = (col..6).max_by(|x, y| a[*x][col].abs().total_cmp(&a[*y][col].abs())).unwrap();
        a.swap(col, piv);
        let d = a[col][col];
        ensure!(d.abs() > 1e-12, "the rest surface's fit is singular");
        for c in col..7 {
            a[col][c] /= d;
        }
        for r in 0..6 {
            if r != col {
                let k = a[r][col];
                for c in col..7 {
                    a[r][c] -= k * a[col][c];
                }
            }
        }
    }
    let coef: [f64; 6] = std::array::from_fn(|r| a[r][6]);
    let fit = move |x: f64, y: f64| basis(x, y).iter().zip(coef.iter()).map(|(b, c)| b * c).sum::<f64>();
    let lift = pts.iter().map(|p| p[2] - fit(p[0], p[1])).fold(0.0, f64::max);
    Ok(move |x: f64, y: f64| fit(x, y) + lift)
}

/// A round tube along an open 3-D path, its ends closed by half-balls: a graver's cut when subtracted.
fn tube(path: &[P3], radius: impl Fn(f64) -> f64, around: usize) -> Result<csg::Solid> {
    ensure!(path.len() >= 2, "a tube needs a path");
    let n = path.len();
    let unit = |a: P3| {
        let l = dot3(a, a).sqrt().max(1e-12);
        a.map(|x| x / l)
    };
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let tan: Vec<P3> = (0..n).map(|i| unit(sub3(path[(i + 1).min(n - 1)], path[i.saturating_sub(1)]))).collect();
    // A frame turned along the path without twist.
    let seed = if tan[0][1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
    let mut nrm = vec![unit(cross(cross(tan[0], seed), tan[0]))];
    for i in 1..n {
        let prev = nrm[i - 1];
        let k = dot3(prev, tan[i]);
        nrm.push(unit([prev[0] - k * tan[i][0], prev[1] - k * tan[i][1], prev[2] - k * tan[i][2]]));
    }
    let total: f64 = (1..n).map(|i| dot3(sub3(path[i], path[i - 1]), sub3(path[i], path[i - 1])).sqrt()).sum();
    let mut along = vec![0.0];
    for i in 1..n {
        along.push(along[i - 1] + dot3(sub3(path[i], path[i - 1]), sub3(path[i], path[i - 1])).sqrt());
    }
    let m = around.max(8);
    let mut v: Vec<P3> = Vec::new();
    let mut rings: Vec<(P3, P3, P3, f64)> = Vec::new();
    const CAP: usize = 5;
    let r0 = radius(0.0);
    let r1 = radius(1.0);
    for k in (1..=CAP).rev() {
        let b = std::f64::consts::FRAC_PI_2 * k as f64 / (CAP + 1) as f64;
        let c = add3(path[0], tan[0], -r0 * b.sin());
        rings.push((c, nrm[0], cross(tan[0], nrm[0]), r0 * b.cos()));
    }
    for i in 0..n {
        rings.push((path[i], nrm[i], cross(tan[i], nrm[i]), radius(along[i] / total.max(1e-9))));
    }
    for k in 1..=CAP {
        let b = std::f64::consts::FRAC_PI_2 * k as f64 / (CAP + 1) as f64;
        let c = add3(path[n - 1], tan[n - 1], r1 * b.sin());
        rings.push((c, nrm[n - 1], cross(tan[n - 1], nrm[n - 1]), r1 * b.cos()));
    }
    for (c, a, b, r) in &rings {
        for j in 0..m {
            let ang = TAU * j as f64 / m as f64;
            v.push(std::array::from_fn(|q| c[q] + r * (ang.cos() * a[q] + ang.sin() * b[q])));
        }
    }
    let at = |r: usize, j: usize| (r * m + j % m) as u32;
    let mut f: Vec<[u32; 3]> = Vec::new();
    for r in 0..rings.len() - 1 {
        for j in 0..m {
            f.push([at(r, j), at(r + 1, j), at(r + 1, j + 1)]);
            f.push([at(r, j), at(r + 1, j + 1), at(r, j + 1)]);
        }
    }
    let start = v.len() as u32;
    v.push(add3(path[0], tan[0], -r0));
    let end = v.len() as u32;
    v.push(add3(path[n - 1], tan[n - 1], r1));
    let last = rings.len() - 1;
    for j in 0..m {
        f.push([start, at(0, j), at(0, j + 1)]);
        f.push([end, at(last, j + 1), at(last, j)]);
    }
    let mut s = csg::Solid { v, f };
    if s.volume() < 0.0 {
        for t in &mut s.f {
            t.swap(1, 2);
        }
    }
    ensure!(s.open_edges() == (0, 0), "a tube does not close: {:?}", s.open_edges());
    Ok(s)
}

fn add3(a: P3, b: P3, k: f64) -> P3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// A closed plan outline pushed out by `g` along its own outward normals.
fn offset_outline(poly: &[P2], g: f64) -> Vec<P2> {
    let n = poly.len();
    let area: f64 = (0..n).map(|i| poly[i][0] * poly[(i + 1) % n][1] - poly[(i + 1) % n][0] * poly[i][1]).sum();
    let ccw = if area > 0.0 { 1.0 } else { -1.0 };
    (0..n)
        .map(|i| {
            let (a, b) = (poly[(i + n - 1) % n], poly[(i + 1) % n]);
            let t = sub2(b, a);
            let l = len2(t).max(1e-12);
            // Outward for a counter-clockwise outline is the tangent turned clockwise.
            add2(poly[i], [ccw * t[1] / l, -ccw * t[0] / l], g)
        })
        .collect()
}

/// A plan path resampled at about `step` mm.
fn resample(path: &[P2], step: f64) -> Vec<P2> {
    let mut out = vec![path[0]];
    for w in path.windows(2) {
        let l = len2(sub2(w[1], w[0]));
        let k = (l / step).ceil().max(1.0) as usize;
        for j in 1..=k {
            out.push(add2(w[0], sub2(w[1], w[0]), j as f64 / k as f64));
        }
    }
    out
}

/// The trench round a face leaf: its radius, how far its centre stands out from the leaf's margin, and the share
/// of the leaf's length from the stalk where it begins.
const TRENCH_R_MM: f64 = 0.24;
const TRENCH_OFFSET_MM: f64 = 0.36;
const TRENCH_FROM: f64 = 0.2;
const MIDRIB_R_MM: f64 = 0.13;

/// Pack a solid as a stored part.
fn stored_op(solid: &csg::Solid, what: &str, params: Value) -> Result<Operation> {
    let mesh = stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?;
    Ok(Operation::Stored { recipe: stored::Recipe { kernel: "viscum".into(), op: what.into(), params, digest: String::new() }, sources: Vec::new(), mesh })
}

// --- Mistletoe -------------------------------------------------------------------------------------------------

/// A Viscum leaf's plan: a short stalk widening into an oblong strap with a blunt, rounded tip, widest past halfway.
fn leaf_half(width: f64) -> impl Fn(f64) -> f64 {
    move |s: f64| {
        let k = [(0.0, 0.0), (0.012, 0.16), (0.06, 0.24), (0.16, 0.5), (0.3, 0.78), (0.45, 0.93), (0.62, 1.0), (0.78, 0.98), (0.87, 0.9), (0.93, 0.75), (0.97, 0.53), (0.99, 0.3), (1.0, 0.0)];
        0.5 * width * pchip(&k, s)
    }
}

/// A fleshy leaf's top: a 0.2 mm wall rounded over 0.1 mm into a cushion that swells to its crown along the
/// spine, thinner at the stalk and the tip.
fn leaf_top(crown: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |s: f64, t: f64, w: f64| {
        let along = pchip(&[(0.0, 0.45), (0.15, 0.6), (0.5, 1.0), (0.8, 0.95), (1.0, 0.7)], s);
        let edge = 0.2;
        let r = 0.1f64.min(0.45 * w);
        let m = (1.0 - t) * w;
        let rim = if m < r { edge - r + (r * r - (r - m) * (r - m)).max(0.0).sqrt() } else { edge };
        // A low keel along the midrib, fading toward the stalk and the tip.
        let keel = 0.0 * smoothstep(0.08, 0.3, s) * (1.0 - smoothstep(0.82, 0.97, s)) * (-(t * w / 0.16).powi(2)).exp();
        rim + (crown * along - edge).max(0.0) * (1.0 - t * t).powf(0.75) + keel
    }
}

/// A round stem lying on the stock: a little wall and a half-round crown, its ends rounded.
fn stem_half(radius: f64, length: f64) -> impl Fn(f64) -> f64 {
    move |s: f64| {
        let cap = (radius / length).min(0.5);
        let e = (s.min(1.0 - s) / cap).min(1.0);
        radius * (1.0 - (1.0 - e) * (1.0 - e)).max(0.0).sqrt()
    }
}
fn stem_top(radius: f64, wall: f64) -> impl Fn(f64, f64, f64) -> f64 {
    move |_s: f64, t: f64, w: f64| wall + (radius * radius - (t * w) * (t * w)).max(0.0).sqrt() * (w / radius).sqrt()
}

/// A berry: the moonstone and where it sits.
struct Berry {
    name: String,
    world: P3,
    d_mm: f64,
    /// Where the girdle stands and which way the stone faces, for a stone set in its own collet.
    girdle: P3,
    normal: P3,
    /// Set in a thin collet sunk flush in a made part, rather than in a gypsy mound in the stock's field.
    cad: bool,
}

/// How far a collet's girdle stands above the highest metal under the stone, mm.
const GIRDLE_RISE_MM: f64 = 0.3;

/// The stock's outward normal averaged over a disc of radius `r` round a plan point, and how far the disc's
/// highest point stands above the centre along it.
fn disc_normal(map: &Depth, at: P2, r: f64) -> Option<(P3, P3, f64)> {
    let w0 = map.world(at[0], at[1], 0.0)?;
    let mut n = [0.0; 3];
    let mut pts = vec![w0];
    // Fans of triangles from the centre to each pair of neighbouring rim points the stock lies under.
    for rr in [0.45 * r, 0.9 * r] {
        let rim: Vec<Option<P3>> = (0..12).map(|k| {
            let q = add2(at, dir2(30.0 * k as f64), rr);
            map.world(q[0], q[1], 0.0)
        }).collect();
        for k in 0..12 {
            if let (Some(a), Some(b)) = (rim[k], rim[(k + 1) % 12]) {
                let e1 = [a[0] - w0[0], a[1] - w0[1], a[2] - w0[2]];
                let e2 = [b[0] - w0[0], b[1] - w0[1], b[2] - w0[2]];
                let c = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                for j in 0..3 {
                    n[j] += c[j];
                }
            }
        }
        pts.extend(rim.into_iter().flatten());
    }
    let l = dot3(n, n).sqrt();
    let mut n = n.map(|v| v / l);
    let out = match map.frame {
        Frame::Plane { n: fnrm, .. } => fnrm,
        Frame::Cylinder { .. } => {
            let r = w0[0].hypot(w0[1]);
            [w0[0] / r, w0[1] / r, 0.0]
        }
    };
    if dot3(n, out) < 0.0 {
        n = n.map(|v| -v);
    }
    let high = pts.iter().map(|p| dot3([p[0] - w0[0], p[1] - w0[1], p[2] - w0[2]], n)).fold(0.0, f64::max);
    Some((w0, n, high))
}

/// Everything laid on the stock, for the parts and the report.
#[derive(Default)]
struct Sprig {
    /// Whether the blades are engraved: a trench round each and a midrib line.
    engrave: bool,
    parts: Vec<(String, csg::Solid, f64)>,
    /// Graver's cuts subtracted from the metal: the trench round each face leaf and its midrib.
    cuts: Vec<(String, csg::Solid)>,
    berries: Vec<Berry>,
}

impl Sprig {
    /// Every part sinks to its own depth, so no two bottoms ever lie in one surface.
    fn unique(&self, sink: f64) -> f64 {
        sink + 0.0061 * self.parts.len() as f64
    }
    fn leaf(&mut self, map: &Depth, name: &str, base: P2, heading_deg: f64, bend: f64, length: f64, width: f64, crown: f64, sink: f64) -> Result<()> {
        let d = dir2(heading_deg);
        let tip = add2(base, d, length);
        let mid = add2(add2(base, d, 0.5 * length), [-d[1], d[0]], bend);
        let half = leaf_half(width);
        let top = leaf_top(crown);
        let sink = self.unique(sink);
        let solid = pillow(map, &Pillow { spine: Spine::new(bezier(base, mid, tip)), half: &half, top: &top, ground: Ground::Drape { sink }, around: 112 })?;
        self.parts.push((name.into(), solid, 0.3));
        Ok(())
    }
    /// A leaf that lies on the stock where it can and runs on past the stock's edge as a free blade.
    fn blade(&mut self, map: &Depth, name: &str, base: P2, heading_deg: f64, bend: f64, length: f64, width: f64, crown: f64, sink: f64) -> Result<()> {
        let d = dir2(heading_deg);
        let tip = add2(base, d, length);
        let mid = add2(add2(base, d, 0.5 * length), [-d[1], d[0]], bend);
        let half = leaf_half(width);
        let top = leaf_top(crown);
        let spine = Spine::new(bezier(base, mid, tip));
        let rest = rest_surface(map, &spine, &half)?;
        let sink = self.unique(sink);
        let solid = pillow(map, &Pillow { spine, half: &half, top: &top, ground: Ground::Free { rest: &rest, under: 0.62, sink, gap: 0.9 }, around: 128 })?;
        self.parts.push((name.into(), solid, 0.3));
        if self.engrave {
            // The trench: a half-round cut in the stock just outside the leaf's margin, from stalk round the tip and
            // back, broken wherever the leaf runs out past the stock.
            let mut outline = leaf_outline(base, heading_deg, bend, length, width);
            outline.dedup_by(|a, b| len2(sub2(*a, *b)) < 1e-6);
            let ring = offset_outline(&outline, TRENCH_OFFSET_MM);
            let keep: Vec<P2> = ring.iter().enumerate().filter(|(i, _)| {
                let s = if *i <= 48 { *i as f64 / 48.0 } else { 1.0 - (*i - 49) as f64 / 48.0 };
                s >= TRENCH_FROM
            }).map(|(_, p)| *p).collect();
            let fine = resample(&keep, 0.05);
            let mut run: Vec<P3> = Vec::new();
            let mut runs: Vec<Vec<P3>> = Vec::new();
            for p in &fine {
                match map.depth(p[0], p[1]) {
                    Some(d) if d > rest(p[0], p[1]) - 0.9 => run.push(map.frame.world(p[0], p[1], d - 0.04)),
                    _ => {
                        if run.len() > 6 {
                            runs.push(std::mem::take(&mut run));
                        }
                        run.clear();
                    }
                }
            }
            if run.len() > 6 {
                runs.push(run);
            }
            for (k, r) in runs.into_iter().enumerate() {
                // Ease the run's heights so the graver rides the stock without catching on its grid.
                let smooth: Vec<P3> = (0..r.len())
                    .map(|i| {
                        let (a, b) = (i.saturating_sub(4), (i + 4).min(r.len() - 1));
                        let n = (b - a + 1) as f64;
                        std::array::from_fn(|q| r[a..=b].iter().map(|p| p[q]).sum::<f64>() / n)
                    })
                    .collect();
                let cut = tube(&smooth, |_| TRENCH_R_MM, 16)?;
                if csg::self_crossings(&cut) == 0 {
                    self.cuts.push((format!("{name} trench {}", k + 1), cut));
                } else {
                    eprintln!("  {name} trench {} folds; left out", k + 1);
                }
            }
            // The midrib: a fine graver line down the blade's crown.
            let d = dir2(heading_deg);
            let mid = add2(add2(base, d, 0.5 * length), [-d[1], d[0]], bend);
            let spine = Spine::new(bezier(base, mid, add2(base, d, length)));
            let path: Vec<P3> = (0..=80)
                .map(|i| {
                    let sf = lerp(0.14, 0.86, i as f64 / 80.0);
                    let (c, _) = spine.at(sf);
                    map.frame.world(c[0], c[1], rest(c[0], c[1]) + top(sf, 0.0, half(sf)) + 0.02)
                })
                .collect();
            let cut = tube(&path, |u| MIDRIB_R_MM * (1.0 - 0.5 * u), 12)?;
            self.cuts.push((format!("{name} midrib"), cut));
        }
        Ok(())
    }
    fn stem(&mut self, map: &Depth, name: &str, pts: Vec<P2>, radius: f64, sink: f64) -> Result<()> {
        let spine = Spine::new(pts);
        let half = stem_half(radius, spine.len());
        let top = stem_top(radius, 0.12);
        let sink = self.unique(sink);
        let solid = pillow(map, &Pillow { spine, half: &half, top: &top, ground: Ground::Bridge { sink, reach: 0.9 * radius }, around: 64 })?;
        self.parts.push((name.into(), solid, 0.3));
        Ok(())
    }
    /// A swollen joint: a short fat stem across the bough.
    fn node(&mut self, map: &Depth, name: &str, at: P2, heading_deg: f64, radius: f64, sink: f64) -> Result<()> {
        let d = dir2(heading_deg);
        let pts = vec![add2(at, d, -1.1 * radius), add2(at, d, 1.1 * radius)];
        self.stem(map, name, pts, radius, sink)
    }
    fn berry(&mut self, map: &Depth, name: &str, at: P2, d_mm: f64) -> Result<()> {
        let world = map.world(at[0], at[1], 0.0).with_context(|| format!("no stock under berry {name}"))?;
        let (w0, normal, high) = disc_normal(map, at, 0.5 * d_mm + 0.4).with_context(|| format!("no stock round berry {name}"))?;
        let girdle = std::array::from_fn(|k| w0[k] + normal[k] * (high + GIRDLE_RISE_MM));
        self.berries.push(Berry { name: name.into(), world, d_mm, girdle, normal, cad: false });
        Ok(())
    }
}

impl Sprig {
    /// A berry raised on its own smooth mound part: the mound fitted to the stock under it, the moonstone flush in a
    /// thin collet at its crown.
    fn mounded_berry(&mut self, map: &Depth, name: &str, at: P2, d_mm: f64, rim: f64) -> Result<()> {
        let r = 0.5 * d_mm + rim;
        // Each mound a few microns off the last, so no two of the boolean's seams meet edge on.
        let crown = MOUND_CROWN_MM + 0.0037 * (self.berries.len() % 7) as f64;
        let half = stem_half(r, 2.0 * r);
        let spine = Spine::new(vec![[at[0] - r, at[1]], [at[0] + r, at[1]]]);
        let rest = rest_surface(map, &spine, &half)?;
        let top = |_s: f64, t: f64, w: f64| {
            let rr = 0.12f64.min(0.45 * w);
            let m = (1.0 - t) * w;
            let edge = 0.22;
            let rim = if m < rr { edge - rr + (rr * rr - (rr - m) * (rr - m)).max(0.0).sqrt() } else { edge };
            rim + (crown - edge) * (1.0 - t * t).max(0.0).sqrt()
        };
        let sink = self.unique(0.45);
        let solid = pillow(map, &Pillow { spine, half: &half, top: &top, ground: Ground::Free { rest: &rest, under: 0.5, sink, gap: 1.0 }, around: 96 })?;
        self.parts.push((format!("{name} mound"), solid, 0.0));
        let h = rest(at[0], at[1]) + crown + 0.08;
        let girdle = map.frame.world(at[0], at[1], h);
        if std::env::var("VISCUM_DEBUG_BERRY").is_ok() {
            eprintln!("  {name}: at {at:?}, rest {:.3}, depth {:?}, girdle {girdle:?}", rest(at[0], at[1]), map.depth(at[0], at[1]));
        }
        let e = 0.05;
        let dp = sub3(map.frame.world(at[0] + e, at[1], rest(at[0] + e, at[1]) + crown), map.frame.world(at[0] - e, at[1], rest(at[0] - e, at[1]) + crown));
        let dq = sub3(map.frame.world(at[0], at[1] + e, rest(at[0], at[1] + e) + crown), map.frame.world(at[0], at[1] - e, rest(at[0], at[1] - e) + crown));
        let mut n = [dp[1] * dq[2] - dp[2] * dq[1], dp[2] * dq[0] - dp[0] * dq[2], dp[0] * dq[1] - dp[1] * dq[0]];
        let l = dot3(n, n).sqrt();
        n = n.map(|x| x / l);
        let out = sub3(map.frame.world(at[0], at[1], h + 1.0), girdle);
        if dot3(n, out) < 0.0 {
            n = n.map(|x| -x);
        }
        self.berries.push(Berry { name: name.into(), world: girdle, d_mm, girdle, normal: n, cad: true });
        Ok(())
    }
}

fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// How high a berry's mound's crown stands over the stock, mm.
const MOUND_CROWN_MM: f64 = 0.6;

/// The longest a leaf may run, up to `length`, with its whole outline on the crest's outer face.
fn fit_on_crest(map: &Depth, base: P2, heading: f64, bend: f64, length: f64, width: f64) -> f64 {
    let mut len = length;
    while len > 1.2 {
        let ok = leaf_outline(base, heading, bend, len, width).iter().all(|p| {
            let top = (-4..=4).filter_map(|k| map.depth(p[0], 0.4 * k as f64)).fold(f64::MIN, f64::max);
            map.depth(p[0], p[1]).is_some_and(|d| d > top - 0.45)
        });
        if ok {
            break;
        }
        len -= 0.1;
    }
    len
}

/// A leaf's plan outline, for keeping berries clear of it.
fn leaf_outline(base: P2, heading_deg: f64, bend: f64, length: f64, width: f64) -> Vec<P2> {
    let d = dir2(heading_deg);
    let spine = Spine::new(bezier(base, add2(add2(base, d, 0.5 * length), [-d[1], d[0]], bend), add2(base, d, length)));
    let half = leaf_half(width);
    let mut out = Vec::new();
    for side in [1.0, -1.0] {
        for i in 0..=48 {
            let s = if side > 0.0 { i as f64 / 48.0 } else { 1.0 - i as f64 / 48.0 };
            let (c, tan) = spine.at(s);
            out.push(add2(c, [-tan[1], tan[0]], side * half(s)));
        }
    }
    out
}

/// Distance from a point to a closed outline, negative inside it.
fn outline_distance(p: P2, poly: &[P2]) -> f64 {
    let n = poly.len();
    let mut best = f64::MAX;
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let ab = sub2(b, a);
        let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
        best = best.min(len2(sub2(p, add2(a, ab, t))));
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            inside = !inside;
        }
    }
    if inside { -best } else { best }
}

fn moonstone(d_mm: f64) -> Gem {
    let mut g = Gem::cabochon(GemCut::Round, d_mm);
    g.preview_tint = Some(MOONSTONE_TINT);
    g
}

/// The chart point nearest a world point on the bare stock.
fn chart_at(a: &Atlas, p: P3) -> (f64, f64) {
    let s = a
        .samples
        .iter()
        .min_by(|x, y| {
            let dx = (x.p[0] - p[0]).powi(2) + (x.p[1] - p[1]).powi(2) + (x.p[2] - p[2]).powi(2);
            let dy = (y.p[0] - p[0]).powi(2) + (y.p[1] - p[1]).powi(2) + (y.p[2] - p[2]).powi(2);
            dx.total_cmp(&dy)
        })
        .expect("an atlas");
    (s.theta, s.v)
}

/// The face's sprig on the cushion's table, mm (x round the ring, z along the finger): the joint where the bough
/// from shoulder A forks, and the two joints its twigs end in.
const FACE_NODE: P2 = [-4.2, 0.0];
const FACE_TIPS: [P2; 2] = [[0.33, 3.29], [0.5, -3.2]];
/// The face's leaves: the joint they grow from (0 the fork, 1 and 2 the tips), heading (degrees), length and width,
/// mm. Narrow 3:1 straps in opposite pairs.
const FACE_LEAVES: [(usize, f64, f64, f64); 6] = [(0, 110.0, 5.2, 1.75), (0, -108.0, 5.3, 1.75), (1, 2.0, 5.2, 1.75), (1, 46.0, 3.4, 1.2), (2, -3.0, 5.3, 1.75), (2, -45.0, 3.4, 1.2)];
/// The berries: a tight triangle in the fork's crotch, and one in each tip's V, sizes in mm.
const FACE_CROTCH: [f64; 3] = [2.2, 2.0, 1.9];
const FACE_V: [f64; 3] = [1.5, 1.4, 1.35];
/// How far the leaves stand and how high their pillow crowns, mm.
const LEAF_WALL_MM: f64 = 0.35;
const LEAF_CROWN_MM: f64 = 0.6;
/// The bough's radius where it leaves the face.
const BOUGH_R: f64 = 0.6;
/// The shoulder units, from the head outward: the berries of the bunch each fork holds and the unit's scale.
const SHOULDER_UNITS: [(&[f64], f64); 2] = [(&[1.95, 1.8, 1.7], 1.0), (&[1.8, 1.65, 1.6], 0.82)];
/// How far each fork's twigs splay off the bough, and how much further out each leaf turns, degrees.
const SHOULDER_FORK_DEG: f64 = 27.0;
const SHOULDER_LEAF_TURN_DEG: f64 = 9.0;
/// Ring angle off the head's centre line where the first shoulder node stands.
const SHOULDER_START_DEG: f64 = 45.0;
/// Each cheek berry's size, mm.
const CHEEK_BERRY_MM: f64 = 1.5;

/// What was laid and where, for the report.
#[derive(Default, serde::Serialize)]
struct Placed {
    parts: Vec<String>,
    berries: Vec<String>,
    shoulder_width_mm: Vec<[f64; 3]>,
}

fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock()?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let mut placed = Placed::default();
    let face = Depth::of(&a, Frame::Plane { u: [1.0, 0.0, 0.0], v: [0.0, 0.0, 1.0], n: [0.0, 1.0, 0.0] }, (-11.0, 11.0), (-11.0, 11.0), 0.02);
    let r0 = a.top - 1.5;
    let crest = Depth::of(&a, Frame::Cylinder { r0 }, (-0.95 * PI * r0, 0.95 * PI * r0), (-10.0, 10.0), 0.03);
    // How wide the crest's outer surface is round the shoulders: z where the radius is within 0.6 mm of the most.
    for deg in [40.0, 50.0, 60.0, 70.0, 80.0, 100.0, 120.0, 150.0, 180.0f64] {
        let p = deg.to_radians() * r0;
        let col: Vec<(f64, f64)> = (-300..=300).filter_map(|k| crest.depth(p, k as f64 * 0.03).map(|r| (k as f64 * 0.03, r))).collect();
        let top = col.iter().map(|c| c.1).fold(0.0, f64::max);
        let near: Vec<f64> = col.iter().filter(|c| c.1 > top - 0.6).map(|c| c.0).collect();
        let (lo, hi) = (near.iter().copied().fold(f64::MAX, f64::min), near.iter().copied().fold(f64::MIN, f64::max));
        eprintln!("  crest {deg}: r {top:.2} outer {lo:.2}..{hi:.2}; covered {:.2}..{:.2}", col.first().map_or(0.0, |c| c.0), col.last().map_or(0.0, |c| c.0));
        placed.shoulder_width_mm.push([deg, top, hi - lo]);
    }
    let mut sprig = Sprig::default();
    // The face: one forked sprig on the quiet table. The bough comes in from shoulder A to the fork; two twigs
    // run on to joints, every joint bears an opposite pair of narrow strap leaves, and a tight triangle of
    // berries sits in the fork's crotch and in each tip's V. The leaves are struck as pillow-topped stamps with
    // their walls standing clean off the field.
    let sv = {
        let lo = a.point(90.0, a.span * 0.5 - 1.0);
        let hi = a.point(90.0, a.span * 0.5 + 1.0);
        (hi[2] - lo[2]).signum()
    };
    sprig.node(&face, "Face joint 1", FACE_NODE, 0.0, 0.9, 0.5)?;
    let mut keep_clear: Vec<(Vec<P2>, f64)> = Vec::new();
    for (k, tip) in FACE_TIPS.into_iter().enumerate() {
        let d = sub2(tip, FACE_NODE);
        let mid = add2(add2(FACE_NODE, d, 0.5), [-d[1], d[0]], if k == 0 { -0.06 } else { 0.06 });
        let twig = bezier(add2(FACE_NODE, d, 0.12), mid, tip);
        sprig.stem(&face, &format!("Face twig {}", k + 1), twig.clone(), 0.55, 0.42)?;
        sprig.node(&face, &format!("Face joint {}", k + 2), tip, d[1].atan2(d[0]).to_degrees(), 0.75, 0.5)?;
        keep_clear.push((twig, 0.55 + 0.1));
        keep_clear.push((vec![tip, tip], 0.75 + 0.1));
    }
    keep_clear.push((vec![FACE_NODE, FACE_NODE], 0.9 + 0.1));
    for (k, (from, h, len, wid)) in FACE_LEAVES.into_iter().enumerate() {
        let joint = if from == 0 { FACE_NODE } else { FACE_TIPS[from - 1] };
        let base = add2(joint, dir2(h), 0.45);
        let outline = leaf_outline(base, h, 0.0, len, wid);
        let centre = add2(base, dir2(h), 0.5 * len);
        let world = face.world(centre[0], centre[1], 0.0).context("no table under a leaf")?;
        let (theta, v) = chart_at(&a, world);
        // The stamp's frame runs along increasing theta (toward -x on the table) and across increasing v.
        let mut local: Vec<[f64; 2]> = outline.iter().map(|p| [-(p[0] - centre[0]), sv * (p[1] - centre[1])]).collect();
        local.dedup_by(|p, q| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-6);
        if local.first().zip(local.last()).is_some_and(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1]) < 1e-6) {
            local.pop();
        }
        if std::env::var("VISCUM_CIRCLE").is_ok() {
            local = ringdesign_core::outline::circle(2.0);
        }
        // A stamp's outline runs counter-clockwise in its own frame.
        let area: f64 = (0..local.len()).map(|i| { let (p, q) = (local[i], local[(i + 1) % local.len()]); p[0] * q[1] - q[0] * p[1] }).sum();
        if area < 0.0 {
            local.reverse();
        }
        if std::env::var("VISCUM_NO_STAMPS").is_err() { d.stamps.push(Stamp {
            name: format!("Face leaf {}", k + 1),
            theta_deg: theta,
            v_mm: v,
            rot_deg: 0.0,
            outline: local,
            height_mm: LEAF_WALL_MM,
            sink_mm: 0.3,
            draft_deg: 0.0,
            cut: false,
            bench: false,
            along_pull: false,
            fine_cap: std::env::var("VISCUM_COARSE").is_err(),
            tier: 0,
            top: if std::env::var("VISCUM_FLAT").is_ok() { StampTop::Flat } else { StampTop::Pillow { crown_mm: LEAF_CROWN_MM } },
        }); }
        keep_clear.push((outline, 0.05));
    }
    let clear = |c: P2, r: f64| {
        keep_clear.iter().all(|(o, extra)| {
            if o.len() > 60 {
                outline_distance(c, o) >= r + extra
            } else {
                o.windows(2).all(|w| {
                    let ab = sub2(w[1], w[0]);
                    let t = (((c[0] - w[0][0]) * ab[0] + (c[1] - w[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                    len2(sub2(c, add2(w[0], ab, t))) >= r + extra
                })
            }
        })
    };
    // A tight triangle along a bisector from a joint: one berry nearest it, two beyond side by side.
    let triangle = |from: P2, heading: f64, sizes: [f64; 3], rim: f64| -> Option<Vec<(P2, f64)>> {
        let (ra, rb) = (0.5 * sizes[0], 0.5 * sizes[1].max(sizes[2]));
        let gap = 0.3;
        let lat = rb + 0.5 * gap;
        let fwd = ((ra + rb + gap).powi(2) - lat * lat).max(0.0).sqrt();
        let (u, n) = (dir2(heading), dir2(heading + 90.0));
        let mut dist = 0.6;
        while dist < 7.0 {
            let c0 = add2(from, u, dist);
            let c1 = add2(add2(from, u, dist + fwd), n, lat);
            let c2 = add2(add2(from, u, dist + fwd), n, -lat);
            let spots = vec![(c0, sizes[0]), (c1, sizes[1]), (c2, sizes[2])];
            if spots.iter().all(|(c, d)| clear(*c, 0.5 * d + rim)) {
                return Some(spots);
            }
            dist += 0.05;
        }
        None
    };
    let mut bunches: Vec<Vec<(P2, f64)>> = vec![triangle(FACE_NODE, 0.0, FACE_CROTCH, 0.25).context("no room in the fork's crotch")?];
    for (k, tip) in FACE_TIPS.into_iter().enumerate() {
        let (h0, h1) = (FACE_LEAVES[2 + 2 * k].1, FACE_LEAVES[3 + 2 * k].1);
        bunches.push(triangle(tip, 0.5 * (h0 + h1), FACE_V, 0.25).with_context(|| format!("no room in tip {}'s V", k + 1))?);
    }
    let mut n_berry = 0;
    for bunch in &bunches {
        for (c, dmm) in bunch {
            n_berry += 1;
            sprig.mounded_berry(&face, &format!("Face berry {n_berry}"), *c, *dmm, 0.3)?;
        }
    }
    // Side B's bough comes up over the head's end and stops at the table's edge.
    let face_bunch_end = 7.4;
    // The bough, one piece a side on the crest: out from under the knot along the crease, over the head's end and
    // down the shoulder. At each node a pair of leaves splays toward the palm and a berry sits in each axil,
    // between the bough and its leaf.
    let to_crest = |x: f64| -> Result<f64> {
        let w = face.world(x, 0.0, 0.0).context("no face on the crease")?;
        Ok(crest.frame.project(w)[0])
    };
    for sign in [-1.0f64, 1.0] {
        let side = if sign < 0.0 { "A" } else { "B" };
        // Crest p grows toward -x; the palm lies further out along `out`.
        let out = -sign;
        let toward = if out > 0.0 { 0.0 } else { 180.0 };
        // Side A's bough ends under the face's first joint; side B's runs up into the head's notch between the
        // tips' leaves and ends in a joint there.
        let start = if sign < 0.0 { to_crest(FACE_NODE[0] + 0.3)? } else { to_crest(face_bunch_end - 0.6)? };
        // Side B stands a hair off side A's mirror image, so no two of the boolean's seams fall together.
        let mut node = out * (SHOULDER_START_DEG.to_radians() * r0 + if sign > 0.0 { 0.031 } else { 0.0 });
        let mut from = start;
        for (n_k, (sizes, scale)) in SHOULDER_UNITS.into_iter().enumerate() {
            let unit = format!("{side}{}", n_k + 1);
            // The stem in to the node, from under the last bunch (or from the face).
            let pts: Vec<P2> = (0..=80).map(|i| [lerp(from, node, i as f64 / 80.0), 0.0]).collect();
            sprig.stem(&crest, &format!("Shoulder bough {unit}"), pts, BOUGH_R * (1.0 - 0.12 * n_k as f64), 0.45)?;
            let knob_r = 0.62 * scale.max(0.8);
            sprig.node(&crest, &format!("Shoulder node {unit}"), [node, 0.0], 0.0, knob_r, 0.5)?;
            if sizes.len() == 2 {
                // The last joint: its two berries astride it, and its leaf pair trailing on toward the palm.
                let rim = 0.3;
                let mut berries = Vec::new();
                for (b_k, dmm) in sizes.iter().enumerate() {
                    let c = [node, [1.0, -1.0][b_k] * (knob_r + 0.08 + 0.5 * dmm + rim)];
                    sprig.mounded_berry(&crest, &format!("Shoulder berry {unit}.{}", b_k + 1), c, *dmm, rim)?;
                    berries.push((c, 0.5 * dmm + rim));
                }
                for (l_k, across) in [1.0f64, -1.0].into_iter().enumerate() {
                    let heading = toward + across * 20.0;
                    let mut off = 0.3;
                    let (len, wid) = (3.0 * scale.max(0.8), 1.4 * scale.max(0.8));
                    while !berries.iter().all(|(c, r)| outline_distance(*c, &leaf_outline(add2([node, 0.0], dir2(heading), off), heading, 0.1 * across, len, wid)) >= r + 0.05) && off < 3.0 {
                        off += 0.05;
                    }
                    let base = add2([node, 0.0], dir2(heading), off);
                    let len = fit_on_crest(&crest, base, heading, 0.1 * across, len, wid);
                    sprig.leaf(&crest, &format!("Shoulder leaf {unit}.{}", l_k + 1), base, heading, 0.1 * across, len, wid, 0.7, 0.34)?;
                }
                break;
            }
            // The fork: two short twigs splayed toward the palm, each ending in a joint that bears one leaf of the
            // pair, and the bunch of berries held in the crotch between them.
            let branch = 2.6 * scale;
            let (leaf_len, leaf_w) = (3.3 * scale, 1.5 * scale);
            let mut keep_clear: Vec<(Vec<P2>, f64)> = Vec::new();
            for (b_k, across) in [1.0f64, -1.0].into_iter().enumerate() {
                let h = toward + across * SHOULDER_FORK_DEG;
                let tip = add2([node, 0.0], dir2(h), branch);
                let twig = bezier(add2([node, 0.0], dir2(h), 0.3), add2(add2([node, 0.0], dir2(h), 0.5 * branch), dir2(h + 90.0), -0.12 * across), tip);
                sprig.stem(&crest, &format!("Shoulder twig {unit}.{}", b_k + 1), twig.clone(), 0.4 * scale.max(0.8), 0.42)?;
                sprig.node(&crest, &format!("Shoulder joint {unit}.{}", b_k + 1), tip, h, 0.48 * scale.max(0.8), 0.48)?;
                let lh = h + across * SHOULDER_LEAF_TURN_DEG;
                let base = add2(tip, dir2(lh), 0.3);
                // As long as the shank's crest holds it: a leaf stays on the outer face, never over the edge.
                let len = fit_on_crest(&crest, base, lh, 0.1 * across, leaf_len, leaf_w);
                sprig.leaf(&crest, &format!("Shoulder leaf {unit}.{}", b_k + 1), base, lh, 0.1 * across, len, leaf_w, 0.7, 0.34)?;
                keep_clear.push((leaf_outline(base, lh, 0.1 * across, len, leaf_w), 0.05));
                keep_clear.push((twig, 0.42 * scale.max(0.8) + 0.08));
            }
            // The bunch: tight, 0.3 mm between berries, as near the fork as the twigs and leaves allow.
            let gap = 0.3;
            let rim = 0.3;
            let spots = |a: f64| -> Vec<(P2, f64)> {
                let r: Vec<f64> = sizes.iter().map(|d| 0.5 * d).collect();
                if sizes.len() == 3 {
                    let l = r[1].max(r[2]) + 0.5 * gap;
                    let fwd = ((r[0] + r[1] + gap).powi(2) - l * l).max(0.0).sqrt();
                    vec![([node + out * a, 0.0], sizes[0]), ([node + out * (a + fwd), l], sizes[1]), ([node + out * (a + fwd), -l], sizes[2])]
                } else {
                    let l = r[0].max(r[1]) + 0.5 * gap;
                    vec![([node + out * a, l], sizes[0]), ([node + out * a, -l], sizes[1])]
                }
            };
            let clear = |c: P2, r: f64| {
                keep_clear.iter().all(|(o, extra)| {
                    if o.len() > 60 && extra < &0.1 {
                        outline_distance(c, o) >= r + extra
                    } else {
                        o.windows(2).all(|w| {
                            let ab = sub2(w[1], w[0]);
                            let t = (((c[0] - w[0][0]) * ab[0] + (c[1] - w[0][1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12)).clamp(0.0, 1.0);
                            len2(sub2(c, add2(w[0], ab, t))) >= r + extra
                        })
                    }
                }) && len2(sub2(c, [node, 0.0])) >= r + knob_r + 0.1
            };
            let mut a = knob_r;
            while !spots(a).iter().all(|(c, d)| clear(*c, 0.5 * d + rim)) && a < 9.0 {
                a += 0.05;
            }
            ensure!(a < 9.0, "no room for shoulder bunch {unit}");
            let bunch = spots(a + if sign > 0.0 { 0.043 } else { 0.0 });
            if std::env::var("VISCUM_DEBUG_BERRY").is_ok() {
                eprintln!("  {unit}: node {node:.3} out {out} a {a:.3} r0 {r0:.3} bunch {:?}", bunch.iter().map(|b| b.0).collect::<Vec<_>>());
            }
            let mut far = 0.0f64;
            for (b_k, (c, dmm)) in bunch.iter().enumerate() {
                sprig.mounded_berry(&crest, &format!("Shoulder berry {unit}.{}", b_k + 1), *c, *dmm, rim)?;
                far = far.max((c[0] - node) * out + 0.5 * dmm + rim);
            }
            // The bough on starts from between the bunch's far berries, clear of the first one.
            let first_r = 0.5 * sizes[0];
            from = node + out * (a + first_r + 0.3 + BOUGH_R);
            eprintln!("  unit {unit}: node at {:.1} deg, bunch from {:.2} to {:.2} mm", (node / r0).to_degrees().abs() % 360.0, a, far);
            let next_knob = 0.62 * SHOULDER_UNITS.get(n_k + 1).map_or(0.8, |u| u.1.max(0.8));
            node += out * (far + 0.9 + next_knob);
        }
    }
    let _ = blockout;
    if std::env::var("VISCUM_BARK").is_ok() {
        oak_bark(&mut d);
    }
    // Parts: every sculpted piece joined to the stock with a small seam bead.
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Factory 003 Clover".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let mut next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let skip = std::env::var("VISCUM_SKIP").unwrap_or_default();
    let skips: Vec<&str> = skip.split(',').filter(|x| !x.is_empty()).collect();
    for (name, solid, blend) in sprig.parts.iter().filter(|p| !skips.iter().any(|k| p.0.starts_with(k))) {
        ensure!(csg::self_crossings(solid) == 0, "{name} crosses itself");
        let op = stored_op(solid, "pillow", json!({ "name": name }))?;
        doc.append(Feature {
            id: next,
            name: name.clone(),
            enabled: true,
            operation: op,
            component: Component { attach: Attach::Join, placement: Placement::Free, blend_mm: *blend * 0.0, ..Component::default() },
        })?;
        next += 1;
        placed.parts.push(format!("{name}: {} triangles, {:.2} mm3", solid.f.len(), solid.volume()));
    }
    for (name, solid) in &sprig.cuts {
        ensure!(csg::self_crossings(solid) == 0, "{name} crosses itself");
        let op = stored_op(solid, "graver", json!({ "name": name }))?;
        doc.append(Feature {
            id: next,
            name: name.clone(),
            enabled: true,
            operation: op,
            component: Component { attach: Attach::Cut, placement: Placement::Free, ..Component::default() },
        })?;
        next += 1;
        placed.parts.push(format!("{name}: cut, {} triangles", solid.f.len()));
    }
    // Berries: moonstone cabochons, each in its own collet standing on the bough or the stock like the fruit in
    // its cup. A collet is a made setting and joins the metal under it whatever the ground's creases do.
    for b in sprig.berries.iter().filter(|b| !skips.iter().any(|k| b.name.starts_with(k)) && b.cad) {
        let gem = moonstone(b.d_mm);
        let stone = next;
        let mut f = ringdesign_core::cad::builders::stone_feature(stone, gem, Placement::Free);
        f.name = format!("{} (moonstone {:.1})", b.name, b.d_mm);
        doc.append(f.clone())?;
        let n = b.normal;
        let roll = -n[1].clamp(-1.0, 1.0).asin();
        let pitch = n[0].atan2(n[2]);
        let moved = stone + 1;
        doc.append(Feature {
            id: moved,
            name: b.name.clone(),
            enabled: true,
            operation: Operation::Transform { source: stone, translation: b.girdle, rotation_deg: [roll.to_degrees(), pitch.to_degrees(), 0.0] },
            component: f.component.clone(),
        })?;
        next = moved + 1;
        let mut counter = next;
        let mut take = || {
            let id = counter;
            counter += 1;
            id
        };
        for mut s in ringdesign_core::cad::builders::setting_features("bezel", moved, gem, false, &mut take)? {
            s.name = format!("{} {}", b.name, s.name.to_lowercase());
            // A thin collet sunk flush: only a bright rim shows round the berry, as in a gypsy setting.
            if let Operation::Builder { key, params, .. } = &mut s.operation {
                if key == ringdesign_core::cad::builders::BEZEL {
                    *params = json!({ "wall_mm": 0.3, "lip": 0.1 });
                }
            }
            doc.append(s)?;
        }
        next = counter;
        placed.berries.push(format!("{} {:.1} mm in a collet, girdle at [{:.2}, {:.2}, {:.2}], facing [{:.2}, {:.2}, {:.2}]", b.name, b.d_mm, b.girdle[0], b.girdle[1], b.girdle[2], n[0], n[1], n[2]));
    }
    // Any berry not in a collet sits flush in a gypsy mound.
    for b in sprig.berries.iter().filter(|b| !skips.iter().any(|k| b.name.starts_with(k)) && !b.cad) {
        let (theta, v) = chart_at(&a, b.world);
        let mut seat = SeatPadLayer {
            theta_deg: theta,
            v_mm: v,
            style: SeatStyle::GypsyMound,
            crown: 1.0,
            blend_mm: 0.35,
            metal_true: true,
            solid: SolidKind::Flush,
            through: std::env::var("VISCUM_THROUGH").is_ok(),
            mark_mm: 0.6,
            ..Default::default()
        };
        seat.fit_stone(moonstone(b.d_mm));
        seat.diameter_mm = b.d_mm + std::env::var("VISCUM_MOUND").ok().and_then(|v| v.parse().ok()).unwrap_or(0.7);
        seat.height_mm = 0.55;
        let mut e = LayerEntry::new(b.name.clone(), Layer::SeatPad(seat));
        e.blend = Blend::Max;
        d.layers.layers.push(e);
        placed.berries.push(format!("{} {:.1} mm at {:.2} deg, v {:.3}", b.name, b.d_mm, theta, v));
    }
    if std::env::var("VISCUM_NO_CAD").is_ok() {
        d.cad = None;
    }
    if std::env::var("VISCUM_STAMP_DEBUG").is_ok() {
        let ctx = d.field_context();
        for st in &d.stamps {
            let f = st.frame(&d, &ctx);
            eprintln!("  {}: theta {:.2} v {:.2} origin {:?} z {:?} x {:?}", st.name, st.theta_deg, st.v_mm, f.origin.map(|v| (v * 100.0).round() / 100.0), f.z.map(|v| (v * 100.0).round() / 100.0), f.x.map(|v| (v * 100.0).round() / 100.0));
        }
    }
    let lib = mf::source_library(&d, &lib).into_owned();
    if std::env::var("VISCUM_BARK_DEBUG").is_ok() {
        let ctx = d.field_context();
        for e in d.layers.layers.iter().filter(|e| e.name.starts_with("Oak")) {
            if let Layer::Tiling(t) = &e.layer {
                eprintln!("bark alpha in lib: {}", lib.get(&t.alpha).is_some());
                for theta in [80.0, 90.0, 100.0, 180.0] {
                    for v in [5.0, 8.0, 10.0, 12.0, 15.0] {
                        let uv = ringdesign_core::field::Uv { u: ctx.u_of_theta(theta), v };
                        eprintln!("  theta {theta} v {v}: h {:.3} hide {:?}", t.height(uv, &ctx, &lib), ctx.hide_uv(uv));
                    }
                }
            }
        }
    }
    Ok((d, lib, placed))
}

/// The host: oak bark cut into the lobes and the shank's walls in hide millimetres, its furrows running round
/// the ring like a branch's, so the bare stock reads as the oak the mistletoe grows from and not as petals.
fn oak_bark(d: &mut RingDesign) {
    let ctx = d.field_context();
    let kind = if std::env::var("VISCUM_HAMMER").is_ok() { Procedural::Hammered } else { Procedural::Bark };
    d.recipes.push(ProcRecipe { name: "Viscum oak bark".into(), kind, repeats: 1, quarter_turns: 0, gamma: 1.0, invert: false });
    let mut t = TilingLayer::default_for("Viscum oak bark", &ctx);
    t.space = if std::env::var("VISCUM_BARK_CHART").is_ok() { ChartSpace::Chart } else { ChartSpace::Hide };
    if std::env::var("VISCUM_BARK_CHART").is_ok() {
        t.v_center_mm = ctx.crest_v_mm;
    }
    t.height_mm = BARK_DEPTH_MM;
    t.repeats_around = 13;
    t.rows = 4;
    if t.space == ChartSpace::Hide {
        t.v_center_mm = 0.0;
    }
    t.v_span_mm = 26.0;
    t.rotation_deg = 90.0;
    t.feather_mm = 0.6;
    t.continuous = true;
    for (name, region) in [("Oak bark, lobes", "table")] {
        let mut e = LayerEntry::new(name, Layer::Tiling(t.clone()));
        e.blend = if std::env::var("VISCUM_BARK_ADD").is_ok() { Blend::Add } else { Blend::Subtract };
        e.bench_only = std::env::var("VISCUM_BARK_BENCH").is_ok();
        if std::env::var("VISCUM_BARK_NOMASK").is_err() {
            e.mask = Some(format!("{}{region}", ringdesign_core::skin::REGION_PREFIX));
        }
        d.layers.layers.push(e);
    }
}

/// How deep the bark's furrows cut, mm.
const BARK_DEPTH_MM: f64 = 0.2;

// --- Gates -------------------------------------------------------------------------------------------------------

fn self_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() })
}

/// The closest any vertex comes to the finger axis, less the bore radius, mm.
fn bore_margin(d: &RingDesign, m: &mesh::Mesh) -> f64 {
    let r = d.inner_radius_mm();
    m.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64) - r).fold(f64::MAX, f64::min)
}

/// Metal vertices standing inside each cabochon, 0.03 mm in from its surface.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    let m = &built.mesh;
    ringdesign_core::stones::all_stone_frames_built(d, built)
        .into_iter()
        .map(|(st, f)| {
            let (ra, rb, h) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03, st.gem.depth_mm() - 0.03);
            let n = m
                .vertices
                .iter()
                .filter(|p| {
                    let q = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
                    let (x, y, z) = (dot3(q, f.long), dot3(q, f.short), dot3(q, f.normal));
                    z > 0.03 && z < h && (x / ra).powi(2) + (y / rb).powi(2) < 1.0 - (z / h).powi(2)
                })
                .count();
            if n > 0 && std::env::var("VISCUM_XLOC").is_ok() {
                let pts: Vec<String> = m.vertices.iter().filter_map(|p| {
                    let q = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
                    let (x, y, z) = (dot3(q, f.long), dot3(q, f.short), dot3(q, f.normal));
                    (z > 0.03 && z < h && (x / ra).powi(2) + (y / rb).powi(2) < 1.0 - (z / h).powi(2)).then(|| format!("({x:.2},{y:.2},{z:.2})"))
                }).take(8).collect();
                eprintln!("  metal in {}: h {h:.2}, e.g. {}", st.label, pts.join(" "));
            }
            (st.label, n)
        })
        .collect()
}

/// Stones the gem preview draws: its triangles welded into connected pieces.
fn preview_count(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> usize {
    let v = ringdesign_core::gems::built_vertices(d, lib, built);
    let mut index = std::collections::HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let n = p[j];
            p[j] = r;
            j = n;
        }
        r
    }
    for tri in v.chunks_exact(36) {
        let ids: Vec<usize> = (0..3)
            .map(|k| {
                let key = [tri[k * 12], tri[k * 12 + 1], tri[k * 12 + 2]].map(|c| (c * 1e3).round() as i64);
                *index.entry(key).or_insert_with(|| {
                    parent.push(parent.len());
                    parent.len() - 1
                })
            })
            .collect();
        for k in 1..3 {
            let (a, b) = (find(&mut parent, ids[0]), find(&mut parent, ids[k]));
            parent[a] = b;
        }
    }
    let n = parent.len();
    (0..n).filter(|&i| find(&mut parent, i) == i).count()
}

/// Every gate at one build size, and whether all are green.
fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool, mesh::BuildResult)> {
    let started = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = self_crossings(&built.mesh);
    if q.degenerate_faces > 0 && std::env::var("VISCUM_XLOC").is_ok() {
        let m = &built.mesh;
        for f in &m.faces {
            let p: Vec<[f64; 3]> = f.iter().map(|&i| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] }).collect();
            let e1 = sub3(p[1], p[0]);
            let e2 = sub3(p[2], p[0]);
            let c = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            if 0.5 * dot3(c, c).sqrt() < 1e-10 {
                eprintln!("  degenerate face at ({:.2}, {:.2}, {:.2})", p[0][0], p[0][1], p[0][2]);
            }
        }
    }
    if crossings > 0 && std::env::var("VISCUM_XLOC").is_ok() {
        // Where they are: count inside 2 mm cubes over the ring's box.
        let m = &built.mesh;
        let mut cells: std::collections::BTreeMap<(i32, i32, i32), Vec<[u32; 3]>> = Default::default();
        for f in &m.faces {
            let c = f.iter().fold([0.0f64; 3], |a, &i| { let v = m.vertices[i as usize]; [a[0] + v.0 as f64 / 3.0, a[1] + v.1 as f64 / 3.0, a[2] + v.2 as f64 / 3.0] });
            cells.entry(((c[0] / 2.0).floor() as i32, (c[1] / 2.0).floor() as i32, (c[2] / 2.0).floor() as i32)).or_default().push(*f);
        }
        for (k, fs) in &cells {
            let mut idx = std::collections::HashMap::new();
            let mut sol = csg::Solid::default();
            for f in fs {
                let g = f.map(|i| *idx.entry(i).or_insert_with(|| { let v = m.vertices[i as usize]; sol.v.push([v.0 as f64, v.1 as f64, v.2 as f64]); (sol.v.len() - 1) as u32 }));
                sol.f.push(g);
            }
            let n = csg::self_crossings(&sol);
            if n > 0 {
                eprintln!("  crossings {n} near ({}, {}, {}) mm", k.0 * 2 + 1, k.1 * 2 + 1, k.2 * 2 + 1);
            }
        }
    }
    let margin = bore_margin(d, &built.mesh);
    if margin < -0.01 {
        let r = d.inner_radius_mm();
        let worst = built.mesh.vertices.iter().min_by(|a, b| ((a.0 as f64).hypot(a.1 as f64)).total_cmp(&(b.0 as f64).hypot(b.1 as f64))).unwrap();
        eprintln!("  into the finger hole by {:.3} mm at ({:.2}, {:.2}, {:.2}); bore r {r:.3}", -margin, worst.0, worst.1, worst.2);
    }
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    // Every made part on its own: closed, uncrossed.
    let mut part_rows = Vec::new();
    let mut parts_ok = true;
    if let Some(doc) = &d.cad {
        for f in &doc.features {
            if let Operation::Stored { mesh: packed, .. } = &f.operation {
                let u = packed.decode()?;
                let s = csg::Solid { v: u.positions, f: u.triangles };
                let (open, nm) = s.open_edges();
                let x = csg::self_crossings(&s);
                parts_ok &= open == 0 && nm == 0 && x == 0;
                part_rows.push(json!({ "part": f.name, "open_edges": open, "non_manifold": nm, "self_crossings": x }));
            }
        }
    }
    let statuses: Vec<String> = built.parts.notes.clone();
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = preview_count(d, lib, &built);
    let inside = metal_in_stones(d, &built);
    let closest_gap = stones.as_ref().and_then(|s| s.closest.as_ref()).map_or(f64::MAX, |p| p.gap_mm.min(p.gap_deep_mm));
    let closest = stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm));
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    // The wall gate: the field's thinnest wall at the 0.8 mm floor, and the kernel's ray census on a light mesh.
    let thick = if built.mesh.faces.len() <= 250_000 { Some(measure::thickness(&built.mesh, 0.8)) } else { None };
    let castable = field.verdict == castability::Verdict::Castable;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && parts_ok
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && margin >= -0.01
        && castable
        && field.thinnest_wall_mm >= 0.8
        && findings.is_empty()
        && reported == previewed
        && reported == 21
        && closest_gap >= 0.1
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
        "made_parts": part_rows,
        "solids_notes": built.solids.notes,
        "parts_notes": statuses,
        "parts_joined": built.parts.joined,
        "seam_beads": built.parts.beads,
        "stamps_struck": built.solids.stamped,
        "seats_resolved": built.solids.resolved,
        "bore_margin_mm": margin,
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "kernel_thickness": thick.as_ref().map(|t| json!({ "sampled_min_mm": t.sampled_min_mm, "rays": t.rays, "below_limit": t.below_limit, "note": t.note })),
        "undercut_percent_two_part_not_gated": field.undercut_fraction() * 100.0,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "metal_inside_stones": inside.iter().filter(|(_, n)| *n > 0).collect::<Vec<_>>(),
        "stone_carats": stones.as_ref().map_or(0.0, |s| s.total_carats),
        "stone_warnings": warnings,
        "closest_stones": closest,
        "grams_18k": built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams),
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders -----------------------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn save_rgb(path: &Path, rgb: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The 300 px read and a contact sheet of every view at that size.
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
    // Close-ups framed on whole parts, never a cropped mesh: the face's sprig, and the near shoulder's forks.
    let top = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0), 1.15, render::Framing::new([0.0, top - 1.0, 0.0], 11.0), edge)?;
    render::write_png_framed(out.join("shoulder-close.png"), &parts, render::yaw_facing(30.0), 0.9, render::Framing::new([9.5, 7.0, 0.0], 7.0), edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.stamps.clear();
    bare.layers.layers.clear();
    bare.cad = None;
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    Ok(())
}

// --- Main --------------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/viscum"));
    std::fs::create_dir_all(&out)?;
    println!("Viscum");
    if args.iter().any(|a| a == "--probe") {
        let mut d = stock()?;
        d.imported_base.as_mut().unwrap().bare = true;
        let lib = AlphaLibrary::builtin();
        let a = Atlas::of(&d, AW, AH)?;
        let face = Depth::of(&a, Frame::Plane { u: [1.0, 0.0, 0.0], v: [0.0, 0.0, 1.0], n: [0.0, 1.0, 0.0] }, (-12.0, 12.0), (-12.0, 12.0), 0.05);
        println!("  top {:.2}", a.top);
        for zz in (-6..=6).rev() {
            let z = zz as f64 * 1.5;
            let row: String = (-8..=8).map(|xx| face.depth(xx as f64 * 1.5, z).map_or("    . ".into(), |y| format!("{:6.2}", y - a.top))).collect();
            println!("  z {z:5.1} {row}");
        }
        let b = mesh::try_build(&d, &lib, draft_params())?;
        let parts = [render::Part::metal(&b.mesh, render::GOLD)];
        for (name, yaw, pitch) in [("hero", 0.48, 1.0), ("face", 0.0, PI * 0.5), ("side", 0.0, 0.05)] {
            render::write_png_parts(out.join(format!("probe-{name}.png")), &parts, yaw, pitch, 500)?;
        }
        return Ok(());
    }
    let started = std::time::Instant::now();
    let (d, lib, placed) = author(blockout)?;
    println!("  authored in {:.1} s: {} parts, {} berries", started.elapsed().as_secs_f64(), placed.parts.len(), placed.berries.len());
    for w in &placed.shoulder_width_mm {
        println!("  crest at {:.0} deg: r {:.2}, outer width {:.2}", w[0], w[1], w[2]);
    }
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: pass {draft_pass}, {} triangles", draft_gates["triangles"]);
    let (export_gates, export_pass, built) = if draft {
        (Value::Null, true, draft_built)
    } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: pass {p}, {} triangles", g["triangles"]);
        (g, p, b)
    };
    library::save_design(out.join("design.ring.json"), &d)?;
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
        let pc = self_crossings(&pattern.mesh);
        let pv = pattern.mesh.validate();
        pattern_pass = pv.watertight && pq.degenerate_faces == 0 && pc == 0;
        pattern_gates = json!({ "triangles": pattern.mesh.faces.len(), "watertight": pv.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "pass": pattern_pass });
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let fin = render::finished_from(&d, &lib, mesh::try_build(&d, &lib, params)?);
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-moonstone.stl".to_string() } else { format!("reference-moonstone-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Viscum reference stone")?;
            materials.push(json!({ "mesh": file, "name": "Moonstone", "tint": tint, "ior": 1.52, "dispersion": 0.012, "roughness": 0.18, "transmission": 0.35 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    let all = draft_pass && export_pass && pattern_pass && cold != Some(false) && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "recipe": "Lost wax (investment), 18k yellow gold; 0.8 mm section, 0.15 mm detail, no draft",
        "base": "Factory 003 Clover, native 18 x 18, not mirrored, no sand envelope",
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "placed": placed,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "design_bytes": text.len(),
        "design_format": serde_json::from_str::<Value>(&text)?.get("format_version").cloned(),
        "draft": draft_gates,
        "export": export_gates,
        "casting_pattern": pattern_gates,
        "cold_reload_identical": cold,
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {} in {:.1} s", if all { "passed" } else { "FAILED" }, started.elapsed().as_secs_f64());
    ensure!(all || draft, "Viscum failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
