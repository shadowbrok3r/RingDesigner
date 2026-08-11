//! Error-driven placement of the swept grid's sample lines.
//!
//! Both grid directions stay regular and wrapping, so the mesh keeps torus
//! topology and is still watertight by construction. Only the *positions* of
//! the lines change: they cluster where the height field, the shank
//! modulation, and the cross-section's own curvature carry detail, and thin
//! out over the bore and the plain stretches of shank.
//!
//! A flat field on a straight profile yields a uniform density, which places
//! samples at equal arc length — the same grid as before.
//!
//! # Off by default, and why
//!
//! Measured as the worst distance from a dense reference section to the
//! polyline actually built, on a size-7 D-shape at 96 and 144 profile steps:
//!
//! | design | equal arc | this |
//! | --- | --- | --- |
//! | plain band | 0.0038 / 0.0028 mm | 0.0027 / 0.0025 mm |
//! | tiled alpha + milgrain | 0.1411 / 0.0791 mm | 0.1788 / 0.1507 mm |
//!
//! It wins where the base profile is the only thing bending, and loses badly
//! once a layer stack displaces the surface. Two causes, both structural:
//!
//! - The densities are **separable**. `v` detail is a maximum over `u`, so 120
//!   discrete milgrain beads densify their `v` at every angle around the ring,
//!   including the angles whose section passes between beads and is flat there.
//!   Detail localized in `u` and `v` at once cannot be expressed this way.
//! - [`curvature_density`] measures the **base profile**, while the error is on
//!   the **displaced** surface. Those agree on a bare band and diverge as soon
//!   as relief is applied, which is exactly when the redistribution matters.
//!
//! Both want per-angle local refinement rather than two shared 1D densities.
//! What holds up and is worth keeping is the equidistribution itself: the sqrt
//! mapping in [`Density::finish_against`], resampling the cross-section as one
//! closed loop so corners are never chopped, and probing the field once.

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::alpha::AlphaLibrary;
use crate::field::{FieldContext, Uv};
use crate::RingDesign;

/// Probe grid. Fixed, so probing never scales with the build resolution.
const PROBE_U: usize = 128;
const PROBE_V: usize = 160;

/// Bins a density is accumulated into.
const BINS: usize = 256;

/// Widest ratio between the densest and sparsest spacing. Bounds the aspect
/// ratio of the triangles a redistribution can produce.
const MAX_CONTRAST: f64 = 10.0;

/// A non-negative sample density over a normalized parameter.
///
/// Raw feature magnitudes go in through [`Density::raise`]; [`Density::finish`]
/// rescales them to `1..=MAX_CONTRAST`, after which two densities can be
/// combined and the result asked for sample positions.
#[derive(Clone, Debug)]
pub struct Density {
    bins: Vec<f64>,
    wrap: bool,
}

impl Density {
    /// A flat density, which places samples at equal arc length.
    pub fn uniform() -> Self {
        Self { bins: vec![1.0; BINS], wrap: false }
    }

    fn zeros(wrap: bool) -> Self {
        Self { bins: vec![0.0; BINS], wrap }
    }

    /// Keep the largest magnitude seen at a normalized position.
    fn raise(&mut self, at: f64, w: f64) {
        if !at.is_finite() || !w.is_finite() || w <= 0.0 {
            return;
        }
        let n = self.bins.len();
        let i = ((at.clamp(0.0, 1.0) * n as f64) as usize).min(n - 1);
        self.bins[i] = self.bins[i].max(w);
    }

    fn get(&self, i: isize) -> f64 {
        let n = self.bins.len() as isize;
        let i = if self.wrap { i.rem_euclid(n) } else { i.clamp(0, n - 1) };
        self.bins[i as usize]
    }

    /// Spread each feature over its neighbourhood, so the samples a feature
    /// earns land on its approach as well as on the feature itself.
    fn dilate(&mut self, radius: usize) {
        if radius == 0 {
            return;
        }
        let r = radius as isize;
        let widened: Vec<f64> = (0..self.bins.len() as isize)
            .map(|i| (-r..=r).map(|d| self.get(i + d)).fold(0.0, f64::max))
            .collect();
        self.bins = widened;
        let softened: Vec<f64> = (0..self.bins.len() as isize)
            .map(|i| (self.get(i - 1) + 2.0 * self.get(i) + self.get(i + 1)) * 0.25)
            .collect();
        self.bins = softened;
    }

    /// Largest raw magnitude, for finishing two densities on one scale.
    pub fn peak(&self) -> f64 {
        self.bins.iter().cloned().fold(0.0f64, f64::max)
    }

    /// Rescale raw magnitudes onto `1..=MAX_CONTRAST` against a given peak.
    ///
    /// The square root equidistributes chord error rather than feature size:
    /// a piecewise-linear approximation's error falls with the square of the
    /// spacing, so matching error means spacing proportional to `1/sqrt(f)`.
    /// `MAX_CONTRAST` then caps how far apart the extremes may sit, and the
    /// magnitude that lands on 1 follows from it rather than being chosen: a
    /// feature `MAX_CONTRAST²` below the peak is the last one worth a sample.
    ///
    /// Interpolating up from 1 instead would compress everything below the
    /// peak — an edge fillet 60x sharper than the dome it joins would earn less
    /// than 4x the samples out of a 6x budget.
    ///
    /// Densities that will be combined must share `hi`. Self-normalizing makes
    /// a straight bore's sharpest corner look as sharp as a fillet.
    pub fn finish_against(&mut self, hi: f64) {
        if !(hi > 0.0) {
            self.bins.fill(1.0);
            return;
        }
        for b in &mut self.bins {
            *b = (MAX_CONTRAST * (*b / hi).clamp(0.0, 1.0).sqrt()).clamp(1.0, MAX_CONTRAST);
        }
    }

    fn finish(&mut self) {
        self.finish_against(self.peak());
    }

    /// Take the larger demand of two finished densities at every bin.
    pub fn combine(&mut self, other: &Density) {
        for (a, b) in self.bins.iter_mut().zip(&other.bins) {
            *a = a.max(*b);
        }
    }

    /// [`Density::combine`], with `other`'s whole range folded onto `lo..hi` of
    /// this one — for laying a density over the surface span alone when this
    /// density covers the entire closed cross-section.
    pub fn combine_range(&mut self, other: &Density, lo: f64, hi: f64) {
        let span = hi - lo;
        if !(span > 1e-9) {
            return;
        }
        let n = self.bins.len();
        let last = other.bins.len() - 1;
        for i in 0..n {
            let t = ((i as f64 + 0.5) / n as f64 - lo) / span;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let k = ((t * last as f64).round() as usize).min(last);
            self.bins[i] = self.bins[i].max(other.bins[k]);
        }
    }

    /// `count` normalized positions, each spanning equal density.
    ///
    /// Monotonically increasing and starting at zero, so the result drops
    /// straight into an arc-length resample.
    pub fn positions(&self, count: usize) -> Vec<f64> {
        if count == 0 {
            return Vec::new();
        }
        let n = self.bins.len();
        let mut cum = Vec::with_capacity(n + 1);
        let mut acc = 0.0;
        cum.push(0.0);
        for &b in &self.bins {
            acc += b.max(1e-9);
            cum.push(acc);
        }
        let total = acc.max(1e-12);

        let mut out = Vec::with_capacity(count);
        let mut k = 0usize;
        for i in 0..count {
            let target = total * i as f64 / count as f64;
            while k + 1 < n && cum[k + 1] <= target {
                k += 1;
            }
            let span = (cum[k + 1] - cum[k]).max(1e-12);
            let f = ((target - cum[k]) / span).clamp(0.0, 1.0);
            out.push(((k as f64 + f) / n as f64).clamp(0.0, 1.0));
        }
        out
    }
}

/// Sample-line placement for one build.
#[derive(Clone, Debug)]
pub struct Spacing {
    /// Normalized ring positions, 0..1, one per angular step.
    pub theta: Vec<f64>,
    /// Detail across the displaceable surface, in normalized `v`.
    pub v: Density,
}

impl Spacing {
    /// Equal spacing in both directions.
    pub fn uniform(n_theta: usize) -> Self {
        Self {
            theta: (0..n_theta).map(|i| i as f64 / n_theta.max(1) as f64).collect(),
            v: Density::uniform(),
        }
    }

    /// Probe the height field once and derive both directions' placement.
    pub fn compute(
        design: &RingDesign,
        ctx: &FieldContext,
        lib: &AlphaLibrary,
        n_theta: usize,
    ) -> Self {
        let mut theta = Density::zeros(true);
        let mut v = Density::zeros(false);

        if !design.layers.is_empty() && ctx.band_v_len_mm > 1e-9 {
            let h = probe(design, ctx, lib);
            let at = |i: usize, j: usize| h[i * PROBE_V + j];

            // Second differences, not first. A chord tracks a straight ramp
            // exactly however steep it is, so what costs samples is the field
            // *bending*, and gradient would spend them on a bead's flanks
            // instead of its crown. Divided through by the step, this is a
            // curvature in 1/mm — the same quantity, and the same scale, as
            // the cross-section's own turning.
            let du = ctx.circumference_mm / PROBE_U as f64;
            let dv = ctx.band_v_len_mm / (PROBE_V - 1) as f64;

            for i in 0..PROBE_U {
                // `u` wraps, so the end columns are each other's neighbours.
                let (i0, i2) = ((i + PROBE_U - 1) % PROBE_U, (i + 1) % PROBE_U);
                let g = (0..PROBE_V)
                    .map(|j| (at(i2, j) - 2.0 * at(i, j) + at(i0, j)).abs())
                    .fold(0.0, f64::max);
                theta.raise(i as f64 / PROBE_U as f64, g / (du * du).max(1e-12));
            }

            let last_v = (PROBE_V - 1) as f64;
            for j in 1..PROBE_V - 1 {
                let g = (0..PROBE_U)
                    .map(|i| (at(i, j + 1) - 2.0 * at(i, j) + at(i, j - 1)).abs())
                    .fold(0.0, f64::max);
                v.raise(j as f64 / last_v, g / (dv * dv).max(1e-12));
            }
        }

        theta.dilate(2);
        theta.finish();
        v.dilate(2);
        v.finish();

        let mut shank = shank_density(design, ctx);
        shank.dilate(1);
        shank.finish();
        theta.combine(&shank);

        Self { theta: theta.positions(n_theta), v }
    }
}

/// Height field on a fixed grid, row-major in `u`.
fn probe(design: &RingDesign, ctx: &FieldContext, lib: &AlphaLibrary) -> Vec<f64> {
    let row = |i: usize| {
        let u = i as f64 / PROBE_U as f64 * ctx.circumference_mm;
        (0..PROBE_V).map(move |j| {
            let v = j as f64 / (PROBE_V - 1) as f64 * ctx.band_v_len_mm;
            let h = design.layers.height(Uv { u, v }, ctx, lib);
            if h.is_finite() { h } else { 0.0 }
        })
    };
    #[cfg(feature = "parallel")]
    return (0..PROBE_U).into_par_iter().flat_map_iter(row).collect();
    #[cfg(not(feature = "parallel"))]
    (0..PROBE_U).flat_map(row).collect()
}

/// How fast the shank modulation changes the cross-section, in mm per bin.
fn shank_density(design: &RingDesign, ctx: &FieldContext) -> Density {
    let mut d = Density::zeros(true);
    let w = design.profile.width_mm;
    let t = design.profile.thickness_mm;
    let c = design.profile.effective_crown_mm();
    let inner_r = design.inner_radius_mm();
    let at =
        |i: usize| design.shank.modulation(i as f64 / BINS as f64 * 360.0, inner_r, ctx.crest_radius_mm);
    // A cap or an outright radius appearing or vanishing between two angles is
    // a full-thickness step, which is the sharpest thing the modulation can do.
    let step = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(x), Some(y)) => (x - y).abs(),
        (Some(_), None) | (None, Some(_)) => t,
        (None, None) => 0.0,
    };
    for i in 0..BINS {
        let (a, b) = (at(i), at((i + 1) % BINS));
        let g = (a.width_scale - b.width_scale).abs() * w
            + (a.thickness_scale - b.thickness_scale).abs() * t
            + (a.crown_scale - b.crown_scale).abs() * c
            + (a.z_center_frac - b.z_center_frac).abs() * w * 0.5
            + step(a.outer_max_r, b.outer_max_r)
            + step(a.outer_r, b.outer_r);
        d.raise(i as f64 / BINS as f64, g);
    }
    d
}

/// Density from a polyline's own turning, so fillets and corners keep samples
/// that equal-arc-length spacing would spend on the straights.
///
/// Returned raw, in turning per mm: the caller finishes it against a peak
/// shared with every span it will be compared against.
pub fn curvature_density(p: &[[f64; 2]]) -> Density {
    let mut d = Density::zeros(false);
    if p.len() < 3 {
        return d;
    }

    let seg = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        ((dx * dx + dy * dy).sqrt(), dx, dy)
    };
    let mut cum = Vec::with_capacity(p.len());
    let mut acc = 0.0;
    cum.push(0.0);
    for w in p.windows(2) {
        acc += seg(w[0], w[1]).0;
        cum.push(acc);
    }
    let total = acc.max(1e-12);

    for i in 1..p.len() - 1 {
        let (l0, dx0, dy0) = seg(p[i - 1], p[i]);
        let (l1, dx1, dy1) = seg(p[i], p[i + 1]);
        if l0 <= 1e-12 || l1 <= 1e-12 {
            continue;
        }
        let cos = ((dx0 * dx1 + dy0 * dy1) / (l0 * l1)).clamp(-1.0, 1.0);
        // Turning per unit length is the discrete curvature.
        d.raise(cum[i] / total, cos.acos() / (0.5 * (l0 + l1)));
    }

    d.dilate(2);
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry};

    #[test]
    fn a_uniform_density_places_samples_evenly() {
        let pos = Density::uniform().positions(8);
        assert_eq!(pos.len(), 8);
        for (i, p) in pos.iter().enumerate() {
            assert!((p - i as f64 / 8.0).abs() < 1e-6, "{pos:?}");
        }
    }

    #[test]
    fn positions_are_monotone_and_start_at_zero() {
        let mut d = Density::zeros(false);
        d.raise(0.25, 5.0);
        d.raise(0.9, 1.0);
        d.dilate(2);
        d.finish();
        let pos = d.positions(64);
        assert_eq!(pos[0], 0.0);
        for w in pos.windows(2) {
            assert!(w[1] >= w[0], "not monotone: {:?} then {:?}", w[0], w[1]);
            assert!(w[1] <= 1.0);
        }
    }

    #[test]
    fn samples_cluster_where_the_density_is_high() {
        let mut d = Density::zeros(false);
        for i in 0..24 {
            d.raise(0.5 + i as f64 * 0.001, 10.0);
        }
        d.dilate(2);
        d.finish();
        let pos = d.positions(100);
        let near = pos.iter().filter(|p| (**p - 0.5).abs() < 0.1).count();
        let far = pos.iter().filter(|p| **p < 0.2).count();
        assert!(near > far, "dense region got {near}, sparse region {far}");
    }

    #[test]
    fn a_flat_field_leaves_the_spacing_uniform() {
        let d = RingDesign::default();
        let s = Spacing::compute(&d, &d.field_context(), &AlphaLibrary::builtin(), 64);
        for (i, t) in s.theta.iter().enumerate() {
            assert!(
                (t - i as f64 / 64.0).abs() < 1e-6,
                "plain band should sweep uniformly, got {t} at {i}"
            );
        }
    }

    #[test]
    fn a_windowed_layer_pulls_samples_to_its_arc() {
        use crate::field::{SignetLayer, Window};
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let s = SignetLayer { v_mm: ctx.crest_v_mm, ..SignetLayer::default() };
        d.layers.layers.push(
            LayerEntry::new("Signet", Layer::Signet(s)).with_window(Window::around(90.0, 60.0)),
        );

        let sp = Spacing::compute(&d, &ctx, &AlphaLibrary::builtin(), 256);
        // The signet spans roughly 60 degrees about the top, a sixth of the ring.
        let inside = sp.theta.iter().filter(|t| (**t - 0.25).abs() < 1.0 / 12.0).count();
        assert!(
            inside > 256 / 6,
            "the signet's arc got {inside} of 256 steps, no better than uniform"
        );
    }

}
