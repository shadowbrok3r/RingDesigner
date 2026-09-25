//! Painting on the ring's own surface: the bare-surface atlas, the hide chart, jointed series and the sand's draft rule.
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::field::{Blend, Layer, LayerEntry, Window, smoothstep};
use crate::profile::{MAX_PROFILE_STEPS, ProfileLoop};
use crate::tiling::TilingLayer;
use crate::{Alpha, RingDesign};
use anyhow::{Result, ensure};

/// Most samples one atlas holds.
pub const MAX_ATLAS_SAMPLES: usize = 1 << 22;
/// Most joints one series carries.
pub const MAX_JOINTS: usize = 4096;
/// Share of a layer's height a texel must lose to count as cut.
pub const CLAMP_NOTICE: f32 = 0.02;

/// One point of the bare surface: where it is, which way it faces, and where it sits in the chart.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    /// World position, mm: finger axis Z, the head toward +Y.
    pub p: [f64; 3],
    /// Unit outward normal from grid differences; zero on the first and last rows.
    pub n: [f64; 3],
    /// Ring angle, degrees.
    pub theta: f64,
    /// Chart `v`, mm.
    pub v: f64,
    /// Index into the atlas, `y * width + x`.
    pub i: usize,
}

/// The bare surface on a grid over the chart: columns round the ring from 0°, rows across the section from the low bore edge.
pub struct Atlas {
    pub width: usize,
    pub height: usize,
    pub samples: Vec<Sample>,
    /// Furthest reach toward the head, mm of Y.
    pub top: f64,
    /// Bore radius, mm.
    pub bore: f64,
    /// Chart `v` the rows span, mm.
    pub span: f64,
    /// The head's face length, mm.
    pub head_length_mm: f64,
    /// The band's width, mm.
    pub band_width_mm: f64,
}

impl Atlas {
    /// The design's bare surface: imported stock through its field surface, a procedural body one modulated section per column.
    pub fn of(d: &RingDesign, width: usize, height: usize) -> Result<Self> {
        ensure!(width >= 4 && height >= 3, "An atlas needs at least 4 x 3 samples");
        ensure!(width.saturating_mul(height) <= MAX_ATLAS_SAMPLES, "An atlas holds at most {MAX_ATLAS_SAMPLES} samples");
        let span = d.reference_loop().surface_len_mm;
        let mut samples = vec![Sample::default(); width * height];
        if let Some(b) = &d.imported_base {
            let surface = b.field_surface(d)?;
            for x in 0..width {
                let theta = x as f64 / width as f64 * 360.;
                for y in 0..height {
                    let fraction = y as f64 / height as f64;
                    samples[y * width + x] = Sample { p: surface.point(theta, fraction), theta, v: fraction * span, i: y * width + x, ..Default::default() };
                }
            }
        } else {
            ensure!(d.band_is_procedural(), "The band is replaced by CAD parts and has no chart");
            let reference = d.reference_loop();
            let at = |x: usize| column(d, &reference, x as f64 / width as f64 * 360., height);
            #[cfg(feature = "parallel")]
            let columns: Result<Vec<Vec<[f64; 2]>>> = (0..width).into_par_iter().map(at).collect();
            #[cfg(not(feature = "parallel"))]
            let columns: Result<Vec<Vec<[f64; 2]>>> = (0..width).map(at).collect();
            for (x, rows) in columns?.into_iter().enumerate() {
                let theta = x as f64 / width as f64 * 360.;
                let (sin, cos) = theta.to_radians().sin_cos();
                for (y, [r, z]) in rows.into_iter().enumerate() {
                    let fraction = y as f64 / height as f64;
                    samples[y * width + x] = Sample { p: [r * cos, r * sin, z], theta, v: fraction * span, i: y * width + x, ..Default::default() };
                }
            }
        }
        let top = samples.iter().map(|s| s.p[1]).fold(0_f64, f64::max);
        for y in 1..height - 1 {
            for x in 0..width {
                let a = samples[y * width + (x + 1) % width].p;
                let b = samples[y * width + (x + width - 1) % width].p;
                let c = samples[(y + 1) * width + x].p;
                let e = samples[(y - 1) * width + x].p;
                let u: [f64; 3] = std::array::from_fn(|i| a[i] - b[i]);
                let v: [f64; 3] = std::array::from_fn(|i| c[i] - e[i]);
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                let l = n.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-9);
                samples[y * width + x].n = n.map(|a| a / l);
            }
        }
        Ok(Self {
            width,
            height,
            samples,
            top,
            bore: d.inner_radius_mm(),
            span,
            head_length_mm: d.shank.head.length_mm,
            band_width_mm: d.profile.width_mm,
        })
    }

    /// The sample at column `x`, row `y`.
    pub fn at(&self, x: usize, y: usize) -> &Sample {
        &self.samples[y.min(self.height - 1) * self.width + x % self.width]
    }

    /// The surface at a chart point, bilinear between samples; rows past the last clamp to it.
    pub fn point(&self, theta_deg: f64, v_mm: f64) -> [f64; 3] {
        let fx = theta_deg.rem_euclid(360.0) / 360.0 * self.width as f64;
        let fy = (v_mm / self.span.max(1e-9) * self.height as f64).clamp(0.0, (self.height - 1) as f64);
        let (x0, y0) = (fx.floor() as usize % self.width, (fy.floor() as usize).min(self.height - 2));
        let (tx, ty) = (fx - fx.floor(), fy - y0 as f64);
        let x1 = (x0 + 1) % self.width;
        let (a, b, c, e) = (self.at(x0, y0).p, self.at(x1, y0).p, self.at(x0, y0 + 1).p, self.at(x1, y0 + 1).p);
        std::array::from_fn(|k| (a[k] + (b[k] - a[k]) * tx) * (1.0 - ty) + (c[k] + (e[k] - c[k]) * tx) * ty)
    }

    /// An alpha the atlas's size holding `f` at every sample, clamped to 0..1.
    pub fn paint(&self, name: impl Into<String>, f: impl Fn(&Sample) -> f64 + Sync) -> Alpha {
        #[cfg(feature = "parallel")]
        let data = self.samples.par_iter().map(|s| f(s).clamp(0., 1.) as f32).collect();
        #[cfg(not(feature = "parallel"))]
        let data = self.samples.iter().map(|s| f(s).clamp(0., 1.) as f32).collect();
        Alpha::new(name, self.width, self.height, data)
    }

    /// How squarely a sample stands on a signet's table, 0..1.
    pub fn face(&self, s: &Sample) -> f64 {
        let a = s.p[0].abs() / (self.head_length_mm * 0.5);
        let b = s.p[2].abs() / (self.band_width_mm * 0.5);
        let q = a.max(b).max((a + b) / 1.68);
        smoothstep(0.91, 0.985, s.n[1]) * (1. - smoothstep(0.82, 0.90, q)) * smoothstep(self.top - 3., self.top - 2., s.p[1])
    }

    /// How squarely a sample stands on a head's wall facing the pull, 0..1.
    pub fn cheek(&self, s: &Sample) -> f64 {
        let r = s.p[0].hypot(s.p[1]);
        smoothstep(0.65, 0.9, s.n[2].abs())
            * smoothstep(self.bore + 0.8, self.bore + 1.45, r)
            * (1. - smoothstep(self.top - 1.1, self.top - 0.55, s.p[1]))
            * smoothstep(-3., 2., s.p[1])
    }

    /// How squarely a sample stands on a shoulder facing round the ring, 0..1.
    pub fn shoulder(&self, s: &Sample) -> f64 {
        smoothstep(0.38, 0.68, s.n[0].abs())
            * (1. - smoothstep(0.28, 0.48, s.n[2].abs()))
            * smoothstep(-1., 1.2, s.p[1])
            * (1. - smoothstep(self.top - 3.5, self.top - 1.9, s.p[1]))
            * smoothstep(self.bore + 0.8, self.bore + 1.3, s.p[0].hypot(s.p[1]))
    }
}

/// Radius and height of the modulated section at `theta` at each row's share of its surface arc.
fn column(d: &RingDesign, reference: &ProfileLoop, theta: f64, height: usize) -> Result<Vec<[f64; 2]>> {
    let l = d.section_at(theta, MAX_PROFILE_STEPS, None, Some(reference));
    let n = l.pts.len();
    let surface: Vec<_> = (0..n).map(|k| &l.pts[(l.surface_start + k) % n]).take_while(|p| p.surface).collect();
    ensure!(surface.len() >= 2, "The section at {theta:.1}° has no outer surface");
    Ok((0..height)
        .map(|y| {
            let target = y as f64 / height as f64 * l.surface_len_mm;
            let j = surface.partition_point(|p| p.v_mm < target).clamp(1, surface.len() - 1);
            let (a, b) = (surface[j - 1], surface[j]);
            let t = ((target - a.v_mm) / (b.v_mm - a.v_mm).max(1e-12)).clamp(0.0, 1.0);
            [a.r + (b.r - a.r) * t, a.z + (b.z - a.z) * t]
        })
        .collect())
}

/// A sample in hide millimetres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HidePoint {
    /// Along the parting line from the head's centre, signed by shoulder.
    pub along: f64,
    /// Across the section from the parting line, signed by side.
    pub across: f64,
    /// How far across the outer surface runs on this side before it faces the pull.
    pub rim: f64,
    /// How far the wall runs past the rim toward the bore edge on this side.
    pub wall: f64,
}

/// The surface measured the way a hide is: along the parting line from the head's centre, across the section from it, and where each side turns to face the pull.
pub struct Hide {
    width: usize,
    height: usize,
    /// Arc along the parting line from the head's centre per column, mm, signed by shoulder.
    pub along: Vec<f64>,
    /// Arc across the section from the parting line per sample, mm, signed by side.
    pub across: Vec<f64>,
    /// Rim per column on the low and high side, mm, averaged over 13 columns.
    pub rim: Vec<[f64; 2]>,
    /// Wall per column on the low and high side, mm, averaged over 13 columns.
    pub wall: Vec<[f64; 2]>,
    /// The atlas row nearest the parting plane in each column.
    pub crest: Vec<usize>,
}

fn distance(p: [f64; 3], q: [f64; 3]) -> f64 {
    (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt()
}

/// Degrees the path turns at the middle of three points.
fn turn(p: [[f64; 3]; 3]) -> f64 {
    let (u, w): ([f64; 3], [f64; 3]) = (std::array::from_fn(|k| p[1][k] - p[0][k]), std::array::from_fn(|k| p[2][k] - p[1][k]));
    let dot = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
    (dot / (u.iter().map(|x| x * x).sum::<f64>() * w.iter().map(|x| x * x).sum::<f64>()).sqrt().max(1e-12)).clamp(-1.0, 1.0).acos().to_degrees()
}

impl Hide {
    /// The hide chart of an atlas.
    pub fn of(a: &Atlas) -> Self {
        let (w, h) = (a.width, a.height);
        let at = |x: usize, y: usize| a.samples[y * w + x];
        let crest: Vec<usize> = (0..w)
            .map(|x| (1..h - 1).min_by(|p, q| at(x, *p).p[2].abs().total_cmp(&at(x, *q).p[2].abs())).unwrap_or(h / 2))
            .collect();
        let head = (w as f64 * 0.25).round() as usize % w;
        let mut along = vec![0.0; w];
        for dir in [1i64, -1] {
            let (mut acc, mut prev) = (0.0, head);
            for k in 1..=w / 2 {
                let x = (head as i64 + dir * k as i64).rem_euclid(w as i64) as usize;
                acc += distance(at(prev, crest[prev]).p, at(x, crest[x]).p);
                along[x] = dir as f64 * acc;
                prev = x;
            }
        }
        let mut across = vec![0.0; w * h];
        let mut rim = vec![[0.0; 2]; w];
        let mut wall = vec![[0.0; 2]; w];
        for x in 0..w {
            let c = crest[x];
            for dir in [1i64, -1] {
                let (mut acc, mut y) = (0.0, c as i64);
                let (mut edge, mut low) = (None, 0.0);
                loop {
                    let next = y + dir;
                    if next < 0 || next >= h as i64 {
                        break;
                    }
                    let (p, q) = (at(x, y as usize), at(x, next as usize));
                    acc += distance(p.p, q.p);
                    across[next as usize * w + x] = acc * q.p[2].signum();
                    if edge.is_none() && q.n[2].abs() > 0.6 {
                        edge = Some(acc);
                    }
                    if q.p[0].hypot(q.p[1]) > a.bore + 0.9 {
                        low = acc;
                    }
                    y = next;
                }
                let side = if at(x, (c as i64 + dir * 4).clamp(0, h as i64 - 1) as usize).p[2] < 0.0 { 0 } else { 1 };
                rim[x][side] = edge.unwrap_or(acc).max(0.5);
                wall[x][side] = (low - rim[x][side]).max(0.0);
            }
        }
        let smooth = |v: &Vec<[f64; 2]>| -> Vec<[f64; 2]> {
            (0..w).map(|x| std::array::from_fn(|k| (-6i64..=6).map(|o| v[(x as i64 + o).rem_euclid(w as i64) as usize][k]).sum::<f64>() / 13.0)).collect()
        };
        let (rim, wall) = (smooth(&rim), smooth(&wall));
        Self { width: w, height: h, along, across, rim, wall, crest }
    }

    /// A sample's hide coordinates, the rim and wall read on its own side.
    pub fn at(&self, s: &Sample) -> HidePoint {
        let (x, side) = (s.i % self.width, if s.p[2] < 0.0 { 0 } else { 1 });
        HidePoint { along: self.along[x], across: self.across[s.i], rim: self.rim[x][side], wall: self.wall[x][side] }
    }

    /// Furthest the parting line runs from the head's centre, mm.
    pub fn reach(&self) -> f64 {
        self.along.iter().fold(0.0_f64, |m, l| m.max(l.abs()))
    }

    /// The column whose parting line stands nearest `along`.
    fn column(&self, along: f64) -> usize {
        (0..self.width).min_by(|p, q| (self.along[*p] - along).abs().total_cmp(&(self.along[*q] - along).abs())).unwrap_or(self.width / 4)
    }

    /// The point of the parting line `along` mm from the head's centre, to the atlas's resolution.
    pub fn crest_point(&self, a: &Atlas, along: f64) -> [f64; 3] {
        let x = self.column(along);
        a.samples[self.crest[x] * self.width + x].p
    }

    /// The chart point on the parting line `along` mm from the head's centre, with `v` where the section crosses `z = 0`.
    pub fn crest_at(&self, a: &Atlas, along: f64) -> (f64, f64) {
        let x = self.column(along);
        let at = |y: usize| a.samples[y * self.width + x];
        let c = self.crest[x];
        for (y0, y1) in [(c.saturating_sub(1), c), (c, (c + 1).min(self.height - 1))] {
            let (p, q) = (at(y0), at(y1));
            if y0 != y1 && p.p[2] * q.p[2] <= 0.0 && p.p[2] != q.p[2] {
                let f = p.p[2] / (p.p[2] - q.p[2]);
                return (p.theta, p.v + (q.v - p.v) * f);
            }
        }
        (at(c).theta, at(c).v)
    }

    /// Where the parting line turns faster than `deg_per_mm`, read over 0.8 mm every 0.1 mm out along each shoulder: signed `along`, mm.
    pub fn folds(&self, a: &Atlas, deg_per_mm: f64) -> Vec<f64> {
        let mut out = Vec::new();
        for sign in [1.0_f64, -1.0] {
            let reach = self.along.iter().map(|l| l * sign).fold(0.0_f64, f64::max);
            let mut i = 0usize;
            loop {
                let l = i as f64 * 0.1;
                if l + 0.4 > reach {
                    break;
                }
                if turn([l - 0.4, l, l + 0.4].map(|o| self.crest_point(a, sign * o))) > deg_per_mm * 0.8 {
                    out.push(sign * l);
                }
                i += 1;
            }
        }
        out
    }
}

/// A deterministic hash of two integers onto 0..1, in steps of 1e-4.
pub fn hash(i: i64, j: i64) -> f64 {
    let mut h = (i.wrapping_mul(0x9E37_79B9_7F4A_7C15u64 as i64) ^ j.wrapping_mul(0xC2B2_AE3D_27D4_EB4Fu64 as i64)) as u64;
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h % 10_000) as f64 / 10_000.0
}

/// `2 atan(c tan(x/2))` in its branch-safe form.
fn eccentric_warp(x: f64, c: f64) -> f64 {
    let (sin, cos) = x.sin_cos();
    let (a, b) = (2.0 * c * sin, (1.0 + cos) - c * c * (1.0 - cos));
    a.atan2(b)
}

/// Where one series of plates is jointed along a run, mm, first joint to last.
#[derive(Clone, Debug, PartialEq)]
pub struct Joints(pub Vec<f64>);

impl Joints {
    /// Joints from `start` to `end` at lengths scattered about the local `pitch` by `seed`; a stub under half a plate joins the plate before it.
    pub fn new(start: f64, end: f64, pitch: impl Fn(f64) -> f64, seed: i64) -> Self {
        let mut g = vec![start];
        let mut j = 0;
        while *g.last().unwrap() < end && g.len() < MAX_JOINTS {
            let l = *g.last().unwrap();
            g.push(l + pitch(l).max(0.01) * (0.72 + 0.56 * hash(seed, j)));
            j += 1;
        }
        let n = g.len();
        if n >= 3 && end - g[n - 2] < 0.5 * pitch(end) {
            g.remove(n - 2);
        }
        *g.last_mut().unwrap() = end;
        Self(g)
    }

    /// Joints from `start` to `end` whose pitch runs a raised cosine from `p0` to `p1`, an integer count at the pitches' geometric mean.
    pub fn eccentric(start: f64, end: f64, p0: f64, p1: f64) -> Self {
        let len = end - start;
        if !(len > 0.0 && p0.is_finite() && p1.is_finite()) {
            return Self(vec![start, end]);
        }
        let (p0, p1) = (p0.max(0.01), p1.max(0.01));
        let n = ((len / (p0 * p1).sqrt()).round() as usize).clamp(1, MAX_JOINTS - 1);
        let c = (p0 / p1).sqrt();
        Self((0..=n).map(|k| if k == n { end } else { start + len * eccentric_warp(std::f64::consts::PI * k as f64 / n as f64, c) / std::f64::consts::PI }).collect())
    }

    /// The plate at `l`: its index, how far along it from 0 to 1, and its length.
    pub fn at(&self, l: f64) -> Option<(usize, f64, f64)> {
        let g = &self.0;
        if g.len() < 2 || l < g[0] || l > g[g.len() - 1] {
            return None;
        }
        let j = g.partition_point(|x| *x <= l).saturating_sub(1).min(g.len() - 2);
        let len = g[j + 1] - g[j];
        Some((j, (l - g[j]) / len, len))
    }

    /// How many plates the series holds.
    pub fn plates(&self) -> usize {
        self.0.len().saturating_sub(1)
    }
}

/// What the draft rule took from a painted layer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClampReport {
    /// Texels lowered by more than [`CLAMP_NOTICE`] of the layer's height.
    pub texels_cut: usize,
    /// Most any texel was lowered, mm.
    pub worst_mm: f64,
}

/// Hold a layer painted on `a` to a two-part pull: walking out from the parting line, relief rises no faster than the surface's own draft allows, and what breaks that is cut back.
pub fn draft_clamp(a: &Atlas, alpha: &mut Alpha, height_mm: f64) -> Result<ClampReport> {
    let (w, h) = (a.width, a.height);
    ensure!(alpha.width == w && alpha.height == h && alpha.data.len() == w * h, "\"{}\" is {} x {}, not painted on this {w} x {h} atlas", alpha.name, alpha.width, alpha.height);
    let before = alpha.data.clone();
    for x in 0..w {
        let at = |y: usize| a.samples[y * w + x];
        let Some(crest) = (1..h - 1).min_by(|p, q| at(*p).p[2].abs().total_cmp(&at(*q).p[2].abs())) else { continue };
        for dir in [1i64, -1] {
            let mut y = crest as i64;
            loop {
                let next = y + dir;
                if next < 1 || next >= h as i64 - 1 {
                    break;
                }
                let (here, there) = (at(y as usize), at(next as usize));
                let step = (0..3).map(|k| (there.p[k] - here.p[k]).powi(2)).sum::<f64>().sqrt();
                let lean = (there.n[2] * there.p[2].signum()).clamp(0.0, 0.9995);
                let rise = lean / (1.0 - lean * lean).sqrt() * step / height_mm.max(1e-9);
                let cap = alpha.data[y as usize * w + x] + rise as f32;
                let cell = &mut alpha.data[next as usize * w + x];
                *cell = cell.min(cap.min(1.0));
                y = next;
            }
        }
    }
    let (mut texels_cut, mut worst) = (0usize, 0.0f32);
    for (b, c) in before.iter().zip(&alpha.data) {
        if b - c > CLAMP_NOTICE {
            texels_cut += 1;
        }
        worst = worst.max(b - c);
    }
    Ok(ClampReport { texels_cut, worst_mm: worst as f64 * height_mm })
}

/// A layer showing `alpha`, painted on the design's atlas: one tile over the whole chart, joined by `Max`, gated to `window`.
pub fn hide_layer(d: &RingDesign, alpha: &str, height_mm: f64, window: Window) -> LayerEntry {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(alpha, &ctx);
    t.repeats_around = 1;
    t.rows = 1;
    t.v_center_mm = ctx.band_v_len_mm * 0.5;
    t.v_span_mm = ctx.band_v_len_mm;
    t.height_mm = height_mm;
    t.feather_mm = 0.0;
    let mut e = LayerEntry::new(alpha, Layer::Tiling(t));
    e.blend = Blend::Max;
    e.window = window;
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{ShankKey, ShankKind};
    use crate::{AlphaLibrary, BuildParams, ProfileStyle};

    fn keyframed() -> RingDesign {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::LowDome);
        d.profile.width_mm = 5.6;
        d.profile.thickness_mm = 2.4;
        d.shank.kind = ShankKind::Keyframes;
        d.shank.amount = 1.0;
        d.shank.keys = vec![
            ShankKey { theta_deg: 90.0, width_scale: 1.35, thickness_scale: 1.25, crown_scale: 1.0 },
            ShankKey { theta_deg: 180.0, width_scale: 1.05, thickness_scale: 1.0, crown_scale: 1.0 },
            ShankKey { theta_deg: 270.0, width_scale: 0.92, thickness_scale: 0.95, crown_scale: 1.0 },
        ];
        d
    }

    /// Factory stock 013 as its sand master, at its own size.
    fn master() -> RingDesign {
        let preset = crate::imported_base::PRESETS.iter().find(|p| p.id == "013").unwrap();
        let mut d = RingDesign::default();
        crate::imported_base::ImportedBase::attach(&mut d, crate::imported_base::sand_master(preset.load().unwrap()).unwrap()).unwrap();
        d.imported_base.as_mut().unwrap().sand_envelope = true;
        d
    }

    fn bypass() -> RingDesign {
        let mut d = RingDesign::default();
        d.profile.width_mm = 5.0;
        d.profile.apply_style(ProfileStyle::LowDome);
        d.shank.kind = ShankKind::Bypass;
        d.shank.amount = 1.0;
        d
    }

    /// Worst miss of the swept mesh's surface vertices against the atlas, alone, over the texel, over the texel plus a mesh row, and the count.
    fn miss(d: &RingDesign, params: BuildParams, a: &Atlas) -> (f64, f64, f64, usize) {
        let built = crate::mesh::try_build(d, &AlphaLibrary::default(), params).unwrap();
        let reference = d.reference_loop();
        let ctx = d.field_context();
        let (n_theta, n_prof) = (params.theta_steps, params.profile_steps);
        let (mut worst, mut ratio, mut rowed, mut checked) = (0.0f64, 0.0f64, 0.0f64, 0usize);
        for i in 0..n_theta {
            let theta = i as f64 / n_theta as f64 * 360.0;
            let l = d.section_at(theta, n_prof, None, Some(&reference));
            assert_eq!(l.pts.len(), n_prof);
            let row = l.surface_len_mm / l.pts.iter().filter(|p| p.surface).count() as f64;
            for (j, p) in l.pts.iter().enumerate().filter(|(_, p)| p.surface) {
                let v = p.v_mm / l.surface_len_mm * ctx.band_v_len_mm;
                if v / a.span * a.height as f64 > (a.height - 1) as f64 {
                    continue;
                }
                let m = built.mesh.vertices[i * n_prof + j];
                let got = a.point(theta, v);
                let err = distance([m.0 as f64, m.1 as f64, m.2 as f64], got);
                let (x, y) = ((theta / 360.0 * a.width as f64) as usize % a.width, (v / a.span * a.height as f64) as usize);
                let texel = distance(a.at(x, y).p, a.at(x + 1, y).p).max(distance(a.at(x, y).p, a.at(x, y + 1).p));
                worst = worst.max(err);
                ratio = ratio.max(err / texel);
                rowed = rowed.max(err / (texel + row));
                checked += 1;
            }
        }
        (worst, ratio, rowed, checked)
    }

    /// Keyframed 0.0006 mm and bypass 0.049 mm at 1024 rows, within a texel; at 160 rows a sweep's own chart runs a row fast.
    #[test]
    fn the_atlas_lands_on_the_swept_mesh() {
        let fine = BuildParams { theta_steps: 360, profile_steps: MAX_PROFILE_STEPS, refine: None, ..Default::default() };
        let preview = BuildParams { profile_steps: 160, ..fine };
        for (name, d) in [("keyframed", keyframed()), ("bypass", bypass())] {
            let a = Atlas::of(&d, 512, 256).unwrap();
            let (worst, ratio, _, checked) = miss(&d, fine, &a);
            eprintln!("{name}, {} rows: {checked} vertices, worst {worst:.5} mm, {ratio:.3} of the local texel", fine.profile_steps);
            assert!(checked > 150_000, "{name}: {checked}");
            assert!(ratio < 1.0, "{name}: {worst} mm is {ratio} of a texel");
            let (worst, ratio, rowed, _) = miss(&d, preview, &a);
            eprintln!("{name}, 160 rows: worst {worst:.5} mm, {ratio:.3} of the texel, {rowed:.3} of the texel and a row");
            assert!(rowed < 1.0, "{name}: {worst} mm is {rowed} of a texel and a row");
        }
    }

    /// 3.2 to 1.7 mm over 30 mm closes on 13 plates, grading monotonically within 2% of each end's pitch.
    #[test]
    fn a_graded_series_closes_on_its_geometric_mean() {
        let j = Joints::eccentric(0.0, 30.0, 3.2, 1.7);
        assert_eq!(j.plates(), 13, "30 / sqrt(3.2 x 1.7) = 12.86");
        assert_eq!((j.0[0], *j.0.last().unwrap()), (0.0, 30.0));
        let gaps: Vec<f64> = j.0.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.windows(2).all(|g| g[1] < g[0]), "{gaps:?}");
        assert!((gaps[0] / 3.2 - 1.0).abs() < 0.02, "first plate {}", gaps[0]);
        assert!((gaps[12] / 1.7 - 1.0).abs() < 0.02, "last plate {}", gaps[12]);
        let even = Joints::eccentric(2.0, 12.0, 2.5, 2.5);
        assert_eq!(even.plates(), 4);
        assert!(even.0.windows(2).all(|w| (w[1] - w[0] - 2.5).abs() < 1e-9), "{:?}", even.0);
        assert_eq!(Joints::eccentric(5.0, 5.0, 1.0, 1.0).0, vec![5.0, 5.0]);
        let (k, t, len) = j.at(15.0).unwrap();
        assert!((j.0[k]..=j.0[k + 1]).contains(&15.0) && (0.0..=1.0).contains(&t) && len > 1.7);
        assert!(Joints::new(0.0, 10.0, |_| 0.0, 1).0.len() < MAX_JOINTS + 1);
    }

    /// Least share of a sample's normal along the radius at its column's crest row.
    fn crest_facing(a: &Atlas) -> f64 {
        (0..a.width)
            .map(|x| {
                let y = (1..a.height - 1).min_by(|p, q| a.at(x, *p).p[2].abs().total_cmp(&a.at(x, *q).p[2].abs())).unwrap();
                let s = a.at(x, y);
                (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / s.p[0].hypot(s.p[1])
            })
            .fold(f64::MAX, f64::min)
    }

    /// Most the radius of the relieved surface climbs between two rows walking out from any column's crest, mm.
    fn worst_climb(a: &Atlas, alpha: &Alpha, height_mm: f64) -> f64 {
        let (w, h) = (a.width, a.height);
        let mut worst = f64::MIN;
        for x in 0..w {
            let r = |y: usize| {
                let (s, d) = (a.at(x, y), alpha.data[y * w + x] as f64 * height_mm);
                (s.p[0] + d * s.n[0]).hypot(s.p[1] + d * s.n[1])
            };
            let crest = (1..h - 1).min_by(|p, q| a.at(x, *p).p[2].abs().total_cmp(&a.at(x, *q).p[2].abs())).unwrap();
            for y in crest + 1..h - 1 {
                worst = worst.max(r(y) - r(y - 1));
            }
            for y in 1..crest {
                worst = worst.max(r(y) - r(y + 1));
            }
        }
        worst
    }

    /// Normals face out; a relief falling from the parting line keeps every texel; a bump 1.2 mm off it climbs 0.070 mm a row, loses 0.44 mm and then climbs under 0.0005; a second pass takes nothing.
    #[test]
    fn the_draft_rule_cuts_what_faces_back_and_nothing_else() {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::LowDome);
        let a = Atlas::of(&d, 256, 192).unwrap();
        let facing = crest_facing(&a);
        assert!(facing > 0.999, "a crest normal is only {facing} radial");
        let hide = Hide::of(&a);
        let mut falling = a.paint("falling", |s| 1.0 - hide.at(s).across.abs() / 3.0);
        assert_eq!(draft_clamp(&a, &mut falling, 0.5).unwrap(), ClampReport::default());
        let mut bump = a.paint("bump", |s| 1.0 - smoothstep(0.2, 0.6, (hide.at(s).across - 1.2).abs()));
        let raw = worst_climb(&a, &bump, 0.5);
        let cut = draft_clamp(&a, &mut bump, 0.5).unwrap();
        let (bare, falls, held) = (worst_climb(&a, &Alpha::new("bare", 256, 192, vec![0.0; 256 * 192]), 0.5), worst_climb(&a, &falling, 0.5), worst_climb(&a, &bump, 0.5));
        eprintln!("bump: {cut:?}; climb bare {bare:.6}, falling {falls:.6}, bump raw {raw:.6}, clamped {held:.6} mm");
        assert!((cut.worst_mm - 0.4408).abs() < 0.005 && cut.texels_cut.abs_diff(6144) <= 32, "{cut:?}");
        assert!(raw > 0.06, "the raw bump climbs only {raw} mm");
        assert!(bare.max(falls) <= 0.0 && held < 5e-4, "bare {bare}, falling {falls}, clamped {held}");
        assert_eq!(draft_clamp(&a, &mut bump, 0.5).unwrap(), ClampReport::default());
        let mut wrong = Alpha::new("wrong", 8, 8, vec![0.0; 64]);
        assert!(draft_clamp(&a, &mut wrong, 0.5).is_err());
    }

    /// Relief climbing a squared side face away from the parting line faces the pull and keeps every texel.
    #[test]
    fn relief_climbs_a_face_that_faces_the_pull() {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::Flat);
        d.profile.flatten_sides();
        let a = Atlas::of(&d, 256, 192).unwrap();
        let facing = crest_facing(&a);
        assert!(facing > 0.999, "a crest normal is only {facing} radial");
        let hide = Hide::of(&a);
        let mut climb = a.paint("climb", |s| {
            let p = hide.at(s);
            ((p.across.abs() - p.rim - 0.2) / 0.7).clamp(0.0, 1.0)
        });
        let rising: Vec<&Sample> = a.samples.iter().filter(|s| (0.01..0.99).contains(&climb.data[s.i])).collect();
        let square = rising.iter().map(|s| s.n[2].abs()).fold(f64::MAX, f64::min);
        let raised = climb.data.iter().filter(|t| **t > 0.99).count();
        let before = worst_climb(&a, &climb, 0.5);
        let cut = draft_clamp(&a, &mut climb, 0.5).unwrap();
        eprintln!("side face: {} texels rising, least |n.z| {square:.4}, {raised} at full height, climb {before:.6} mm, {cut:?}", rising.len());
        assert!(rising.len() > 256 * 16 && square > 0.99 && raised > 256 * 16, "{} rising at |n.z| >= {square}, {raised} raised", rising.len());
        assert!(before <= 0.0, "{before}");
        assert_eq!(cut, ClampReport::default());
    }

    /// On a plain band the hide reaches half the crest's circumference, crosses z = 0 on the parting line, and has no folds.
    #[test]
    fn the_hide_measures_a_band_in_millimetres() {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::Flat);
        let a = Atlas::of(&d, 512, 192).unwrap();
        let hide = Hide::of(&a);
        let half = std::f64::consts::PI * d.reference_loop().crest_radius_mm;
        assert!((hide.reach() / half - 1.0).abs() < 1e-3, "{} against {half}", hide.reach());
        let lo = hide.along.iter().copied().fold(f64::MAX, f64::min);
        assert!((lo + hide.reach()).abs() < 0.1, "{lo}");
        for along in [0.0, 5.0, -12.0, 20.0] {
            let (theta, v) = hide.crest_at(&a, along);
            assert!(a.point(theta, v)[2].abs() < 1e-3, "{along}: {:?}", a.point(theta, v));
        }
        assert!(hide.rim.iter().all(|r| (r[0] - r[1]).abs() < 0.05), "{:?}", &hide.rim[..4]);
        assert!(hide.folds(&a, 12.0).is_empty());
        let e = hide_layer(&d, "hide", 0.4, Window::around(90.0, 300.0));
        let Layer::Tiling(t) = &e.layer else { panic!() };
        assert_eq!((t.repeats_around, t.rows, t.v_span_mm, e.blend), (1, 1, d.field_context().band_v_len_mm, Blend::Max));
    }

    /// 013's master is the same twice, its sections mirror within 0.004 mm, its atlas faces out, and its parting line runs through the head's centre.
    #[test]
    fn a_sand_master_is_its_own_mirror() {
        let source = crate::imported_base::PRESETS.iter().find(|p| p.id == "013").unwrap().load().unwrap();
        let (a, b) = (crate::imported_base::sand_master(source.clone()).unwrap(), crate::imported_base::sand_master(source).unwrap());
        assert_eq!((&a.vertices, &a.faces), (&b.vertices, &b.faces));
        assert!(a.name.ends_with("/ drafted workshop master"));
        let d = master();
        let seg = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy).max(1e-18)).clamp(0.0, 1.0);
            (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dy * t)
        };
        let mut worst = 0.0f64;
        for k in 0..24 {
            let l = d.section_at(k as f64 * 15.0 + 0.3, 256, None, None);
            let pts: Vec<[f64; 2]> = l.pts.iter().map(|p| [p.r, p.z]).collect();
            for p in &pts {
                let m = [p[0], -p[1]];
                worst = worst.max((0..pts.len()).map(|i| seg(m, pts[i], pts[(i + 1) % pts.len()])).fold(f64::MAX, f64::min));
            }
        }
        eprintln!("013 master: {} vertices, {} faces, worst section mirror miss {worst:.4} mm", a.vertices.len(), a.faces.len());
        assert!(worst < 0.02, "{worst}");
        let atlas = Atlas::of(&d, 256, 96).unwrap();
        let facing = crest_facing(&atlas);
        eprintln!("013 master: least crest normal share along the radius {facing:.4}");
        assert!(facing > 0.7, "{facing}");
        let hide = Hide::of(&atlas);
        let head = hide.crest_point(&atlas, 0.0);
        assert!(head[0].abs() < 0.2 && head[2].abs() < 0.1 && head[1] > atlas.top - 0.2, "{head:?} against top {}", atlas.top);
        assert!((hide.reach() + hide.along.iter().copied().fold(f64::MAX, f64::min)).abs() < 0.3);
    }
}
