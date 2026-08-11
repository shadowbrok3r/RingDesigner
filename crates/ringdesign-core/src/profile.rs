//! Band cross-section profiles and shank modulation.
//!
//! The cross-section is a closed loop in the (r, z) plane, traversed
//! counter-clockwise so the 2D outward normal of a tangent `(dr, dz)` is
//! `(dz, -dr)`. Sweeping it about Z yields the ring.
//!
//! The outer surface is a superellipse drop from a single crest,
//! `d(x) = 1 - (1 - x^a)^(1/b)` with `x` the normalized distance from the
//! crest toward the nearer edge. `d` is monotonically non-decreasing for any
//! `a, b > 0`, so the outer surface can never undercut a ±Z mold pull.

use serde::{Deserialize, Serialize};

use crate::adaptive::{self, Density};
use crate::field::SignetOutline;

/// Thinnest castable outer edge; feather edges will not fill in sand.
pub const MIN_EDGE_MM: f64 = 0.2;

/// Distance over which displacement fades to zero at the bore corners.
pub const EDGE_FADE_MM: f64 = 0.35;

/// Ring angle of the top of the ring (where a head or focal motif sits).
pub const TOP_DEG: f64 = 90.0;

/// Bounds on the cross-section vertex count. `sample_mod` builds polylines at
/// `DENSE` times the requested count, so an unclamped value read from a design
/// file would allocate without limit. `mesh::build` clamps to the same range,
/// which keeps the swept grid's indexing aligned with what it gets back.
pub const MIN_PROFILE_STEPS: usize = 24;
pub const MAX_PROFILE_STEPS: usize = 1024;

/// Vertices in the reference cross-section that parameterizes the height
/// field. Fixed so the `v` span does not move with the build resolution.
pub const REFERENCE_PROFILE_STEPS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileStyle {
    Flat,
    LowDome,
    HalfRound,
    HighDome,
    CushionDome,
    DShape,
    Beveled,
    KnifeEdge,
    Custom,
}

impl ProfileStyle {
    pub const ALL: &'static [ProfileStyle] = &[
        ProfileStyle::Flat,
        ProfileStyle::LowDome,
        ProfileStyle::HalfRound,
        ProfileStyle::HighDome,
        ProfileStyle::CushionDome,
        ProfileStyle::DShape,
        ProfileStyle::Beveled,
        ProfileStyle::KnifeEdge,
        ProfileStyle::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProfileStyle::Flat => "Flat",
            ProfileStyle::LowDome => "Low Dome",
            ProfileStyle::HalfRound => "Half Round",
            ProfileStyle::HighDome => "High Dome",
            ProfileStyle::CushionDome => "Cushion Dome",
            ProfileStyle::DShape => "D-Shape",
            ProfileStyle::Beveled => "Beveled",
            ProfileStyle::KnifeEdge => "Knife Edge",
            ProfileStyle::Custom => "Custom",
        }
    }

    /// One-line note on how this profile behaves in a ±Z sand mold.
    pub fn casting_note(self) -> &'static str {
        match self {
            ProfileStyle::Flat => "Near-vertical outer wall; relief must sit on the side faces.",
            ProfileStyle::LowDome => "Gentle crown; shallow relief pulls, deep relief drags.",
            ProfileStyle::HalfRound => "Semicircular crown; good draft away from the crest line.",
            ProfileStyle::HighDome => "Tall crown; the best surface for carved relief.",
            ProfileStyle::CushionDome => "Flat centre with fast edge falloff; wide crest, tight edges.",
            ProfileStyle::DShape => "Flat sides, rounded crown; classic castable band.",
            ProfileStyle::Beveled => "Straight-sloped faces; uniform draft over the whole crown.",
            ProfileStyle::KnifeEdge => "Sharp crest; edges clamp to the minimum castable thickness.",
            ProfileStyle::Custom => "Hand-tuned exponents, or a drawn crown.",
        }
    }

    /// `(shape_a, shape_b, crown_fraction_of_thickness, edge_round_fraction)`.
    pub fn preset(self) -> (f64, f64, f64, f64) {
        match self {
            ProfileStyle::Flat => (8.0, 1.0, 0.15, 0.35),
            ProfileStyle::LowDome => (2.0, 1.0, 0.40, 0.25),
            ProfileStyle::HalfRound => (2.0, 2.0, 0.90, 0.05),
            ProfileStyle::HighDome => (2.0, 2.0, 1.00, 0.02),
            ProfileStyle::CushionDome => (4.0, 2.0, 0.70, 0.10),
            ProfileStyle::DShape => (2.5, 1.6, 0.65, 0.08),
            ProfileStyle::Beveled => (1.0, 1.0, 0.55, 0.10),
            ProfileStyle::KnifeEdge => (1.0, 1.0, 1.00, 0.0),
            ProfileStyle::Custom => (2.0, 2.0, 0.75, 0.10),
        }
    }
}


/// A flat annular face standing proud of the dome.
///
/// One control covers two shapes: at a band edge (`v_pos` 0 or 1) it widens the
/// side face into a broad flat rim; in the middle it becomes a flange, a thin
/// disc around the circumference. Both give a face perpendicular to the mould
/// pull, which is the best surface on the ring for ornament.
///
/// Castability: the mould parts at the widest silhouette, which is the flange
/// rim. Everything must fall away from there monotonically, so a flange sitting
/// *above* the dome crest leaves the stretch between them leaning back under the
/// rim — an undercut. At a band edge there is no dome beyond it, so an edge
/// flange is always safe. [`Flange::is_castable_at`] answers this, and a flange
/// at any position it calls safe takes the crest with it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Flange {
    pub enabled: bool,
    /// Position across the band width, 0 = bottom edge, 0.5 = middle, 1 = top.
    pub v_pos: f64,
    /// Radial projection beyond the dome at that height, mm.
    pub extent_mm: f64,
    /// Axial thickness of the flat disc, mm.
    pub thickness_mm: f64,
    /// Fillet where the flange meets the dome, mm.
    pub edge_round_mm: f64,
}

impl Default for Flange {
    fn default() -> Self {
        Self {
            enabled: false,
            v_pos: 0.0,
            extent_mm: 1.2,
            thickness_mm: 0.8,
            edge_round_mm: 0.15,
        }
    }
}

impl Flange {
    /// Whether this position releases from a two-part pull, given the crest.
    ///
    /// Safe at a band edge, or level with the crest. In between, the dome
    /// between crest and rim undercuts.
    pub fn is_castable_at(&self, crest_t: f64) -> bool {
        if !self.enabled {
            return true;
        }
        let v = self.v_pos.clamp(0.0, 1.0);
        v <= EDGE_FLANGE_T || v >= 1.0 - EDGE_FLANGE_T || (v - crest_t).abs() <= CREST_FLANGE_T
    }

    /// Nearest position that does release, for a snap-to-safe affordance.
    pub fn nearest_castable(&self, crest_t: f64) -> f64 {
        let v = self.v_pos.clamp(0.0, 1.0);
        [0.0, crest_t, 1.0]
            .into_iter()
            .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
            .unwrap_or(crest_t)
    }
}

/// Control points a hand-drawn drop may carry. Fixed, so [`BandProfile`] stays
/// `Copy` and a design file cannot ask for an unbounded curve.
pub const MAX_DROP_POINTS: usize = 16;

/// A hand-drawn drop from the crest, replacing the superellipse.
///
/// Control points are `(x, d)`: `x` the normalized distance from the crest
/// toward the nearer edge, `d` the normalized drop, both 0..1. That is the same
/// parameterization the superellipse uses, so the bore, side faces, fillets and
/// flange machinery are untouched — only the shape of the crown changes.
///
/// # Why any drawable shape is still castable
///
/// The castability guarantee is that `d` never falls, so the outer surface
/// never turns back under itself. That is a property of the *curve*, not of the
/// superellipse, so it survives being drawn by hand as long as the curve stays
/// monotone non-decreasing — which [`DropCurve::monotone`] enforces on every
/// edit. Interpolation is monotone cubic, so the smoothing between control
/// points cannot overshoot into a dip either.
///
/// Clearing `monotone` is the deliberate way to model an undercut; the profile
/// is then no better guaranteed than any other and `castability::analyze` is
/// the only word on it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DropCurve {
    points: [[f64; 2]; MAX_DROP_POINTS],
    len: u8,
    /// Keep the drop from ever falling back, which is the no-undercut
    /// guarantee. Clear it deliberately to draw an undercut.
    pub monotone: bool,
}

impl Default for DropCurve {
    fn default() -> Self {
        Self { points: [[0.0; 2]; MAX_DROP_POINTS], len: 0, monotone: true }
    }
}

impl DropCurve {
    /// Whether this curve, rather than the superellipse, shapes the crown.
    pub fn is_active(&self) -> bool {
        self.len >= 2
    }

    pub fn points(&self) -> &[[f64; 2]] {
        &self.points[..self.len as usize]
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Sample a superellipse into control points, so switching to a hand-drawn
    /// curve starts from the shape that was already there instead of a ramp.
    pub fn from_superellipse(a: f64, b: f64, n: usize) -> Self {
        let n = n.clamp(2, MAX_DROP_POINTS);
        let (a, b) = (a.max(0.05), b.max(0.05));
        let mut c = Self::default();
        for i in 0..n {
            let x = i as f64 / (n - 1) as f64;
            let d = (1.0 - (1.0 - x.powf(a)).max(0.0).powf(1.0 / b)).clamp(0.0, 1.0);
            c.points[i] = [x, d];
        }
        c.len = n as u8;
        c.sanitize();
        c
    }

    /// Sort, clamp, drop coincident points, and pin the ends. Monotone curves
    /// additionally get each `d` floored by the one before it.
    pub fn sanitize(&mut self) {
        let n = self.len as usize;
        if n == 0 {
            return;
        }
        let mut p: Vec<[f64; 2]> = self.points[..n]
            .iter()
            .map(|q| [q[0].clamp(0.0, 1.0), q[1].clamp(0.0, 1.0)])
            .filter(|q| q[0].is_finite() && q[1].is_finite())
            .collect();
        p.sort_by(|a, b| a[0].total_cmp(&b[0]));
        p.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-4);

        // The crown is measured from crest to edge, so the span is the whole
        // 0..1 either way; without pinned ends a drag could shorten it.
        if p.first().is_some_and(|q| q[0] > 1e-9) {
            p.insert(0, [0.0, p[0][1]]);
        }
        if p.last().is_some_and(|q| q[0] < 1.0 - 1e-9) {
            p.push([1.0, p[p.len() - 1][1]]);
        }
        p.truncate(MAX_DROP_POINTS);

        if self.monotone {
            for i in 1..p.len() {
                p[i][1] = p[i][1].max(p[i - 1][1]);
            }
        }

        self.len = p.len() as u8;
        for (slot, q) in self.points.iter_mut().zip(p) {
            *slot = q;
        }
    }

    /// Move one control point, keeping the curve legal.
    pub fn set(&mut self, i: usize, x: f64, d: f64) {
        if i >= self.len as usize {
            return;
        }
        // The ends stay at 0 and 1 so the crown always spans the full drop.
        let x = if i == 0 {
            0.0
        } else if i + 1 == self.len as usize {
            1.0
        } else {
            x.clamp(0.0, 1.0)
        };
        self.points[i] = [x, d.clamp(0.0, 1.0)];
        self.sanitize();
    }

    /// A curve through the given control points, sanitized once at the end.
    /// Inserting one at a time pins a provisional end after every point, and
    /// that pin later wins the dedup against a real point at the same `x`.
    pub fn from_points(pts: &[[f64; 2]]) -> Self {
        let mut c = Self::default();
        for &[x, d] in pts.iter().take(MAX_DROP_POINTS) {
            let n = c.len as usize;
            c.points[n] = [x.clamp(0.0, 1.0), d.clamp(0.0, 1.0)];
            c.len = (n + 1) as u8;
        }
        c.sanitize();
        c
    }

    /// Add a control point, ignored once the curve is full.
    pub fn insert(&mut self, x: f64, d: f64) {
        let n = self.len as usize;
        if n >= MAX_DROP_POINTS {
            return;
        }
        self.points[n] = [x.clamp(0.0, 1.0), d.clamp(0.0, 1.0)];
        self.len = (n + 1) as u8;
        self.sanitize();
    }

    /// Remove a control point. The two ends are never removable.
    pub fn remove(&mut self, i: usize) {
        let n = self.len as usize;
        if n <= 2 || i == 0 || i + 1 >= n {
            return;
        }
        for k in i..n - 1 {
            self.points[k] = self.points[k + 1];
        }
        self.len = (n - 1) as u8;
        self.sanitize();
    }

    /// Drop at a normalized distance from the crest.
    ///
    /// Monotone cubic Hermite (Fritsch–Carlson): a plain cubic spline would
    /// overshoot between control points and dip back, which on this curve means
    /// an undercut the control points never asked for.
    pub fn eval(&self, x: f64) -> f64 {
        let p = self.points();
        let n = p.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return p[0][1].clamp(0.0, 1.0);
        }
        let x = x.clamp(0.0, 1.0);
        if x <= p[0][0] {
            return p[0][1].clamp(0.0, 1.0);
        }
        if x >= p[n - 1][0] {
            return p[n - 1][1].clamp(0.0, 1.0);
        }

        let secant = |i: usize| (p[i + 1][1] - p[i][1]) / (p[i + 1][0] - p[i][0]).max(1e-12);

        // Tangents: the average of the neighbouring secants, zeroed wherever
        // they disagree in sign so a local extreme stays put.
        let tangent = |i: usize| -> f64 {
            if i == 0 {
                return secant(0);
            }
            if i + 1 == n {
                return secant(n - 2);
            }
            let (a, b) = (secant(i - 1), secant(i));
            if a * b <= 0.0 { 0.0 } else { 0.5 * (a + b) }
        };

        let k = p.partition_point(|q| q[0] <= x).saturating_sub(1).min(n - 2);
        let h = (p[k + 1][0] - p[k][0]).max(1e-12);
        let d = secant(k);
        let (mut m0, mut m1) = (tangent(k), tangent(k + 1));

        // Fritsch–Carlson: pull the tangents inside the circle of radius 3|d|,
        // which is what keeps the segment from overshooting.
        if d.abs() <= 1e-12 {
            m0 = 0.0;
            m1 = 0.0;
        } else {
            let (a, b) = (m0 / d, m1 / d);
            let s = a.hypot(b);
            if s > 3.0 {
                m0 = 3.0 * a / s * d;
                m1 = 3.0 * b / s * d;
            }
        }

        let t = (x - p[k][0]) / h;
        let (t2, t3) = (t * t, t * t * t);
        let y = (2.0 * t3 - 3.0 * t2 + 1.0) * p[k][1]
            + (t3 - 2.0 * t2 + t) * h * m0
            + (-2.0 * t3 + 3.0 * t2) * p[k + 1][1]
            + (t3 - t2) * h * m1;
        y.clamp(0.0, 1.0)
    }

    /// How far the curve ever falls back below its own running peak, in
    /// normalized drop. Zero on a curve that cannot undercut.
    ///
    /// Measured against the peak rather than the previous sample: an undercut
    /// is the total depth the surface comes back in by, and per-step slope
    /// would report a long shallow reversal as harmless.
    pub fn worst_reversal(&self) -> f64 {
        const STEPS: usize = 256;
        let mut peak = self.eval(0.0);
        let mut worst = 0.0f64;
        for i in 1..=STEPS {
            let y = self.eval(i as f64 / STEPS as f64);
            peak = peak.max(y);
            worst = worst.max(peak - y);
        }
        worst
    }
}

/// Fillet left on a side face squared up by [`BandProfile::flatten_sides`], mm.
pub const SQUARED_SIDE_FILLET_MM: f64 = 0.05;

/// How close to a band edge counts as an edge flange.
pub const EDGE_FLANGE_T: f64 = 0.04;
/// How close to the crest a mid-band flange must sit.
pub const CREST_FLANGE_T: f64 = 0.04;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BandProfile {
    pub style: ProfileStyle,
    /// Axial extent at the bore, mm.
    pub width_mm: f64,
    /// Maximum radial thickness, at the crest, mm.
    pub thickness_mm: f64,
    /// Radial drop from the crest to the outer edge, mm. Clamped so the edge
    /// keeps at least [`MIN_EDGE_MM`] of thickness.
    pub crown_mm: f64,
    /// Superellipse edge exponent: higher flattens the crown and sharpens the
    /// falloff at the edges.
    pub shape_a: f64,
    /// Superellipse crest exponent: higher fills the crest out.
    pub shape_b: f64,
    /// Crest position across the width, -1 (bottom edge) to 1 (top edge).
    pub crest_bias: f64,
    /// Fillet radius where the side faces meet the outer surface, mm.
    pub edge_round_mm: f64,
    /// Inward dome of the bore, mm. The stated size is measured at the crown
    /// of the dome, so the ring rides on a narrow contact band.
    pub comfort_fit_mm: f64,
    /// Taper of the side faces, degrees. Positive narrows the band outward,
    /// which adds draft to the side faces.
    pub side_draft_deg: f64,
    /// Optional flat annular face standing proud of the dome.
    #[serde(default)]
    pub flange: Flange,
    /// Hand-drawn crown, used in place of the superellipse when it carries
    /// points and the style is [`ProfileStyle::Custom`].
    #[serde(default)]
    pub drop_curve: DropCurve,
    /// Second crown the profile morphs toward around the top of the ring.
    #[serde(default)]
    pub morph: Option<ProfileMorph>,
}

/// A target crown for per-angle profile morphing: D-shape at the palm easing
/// to a flat top, dome to knife, whatever the two styles are. The blend of two
/// monotone drops is monotone, so the base surface stays undercut-free at
/// every angle in between.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProfileMorph {
    pub shape_a: f64,
    pub shape_b: f64,
    pub crown_mm: f64,
    pub edge_round_mm: f64,
    /// How tightly the morph hugs the top of the ring: 1 spreads over the
    /// whole upper half, 6 holds it to the crown.
    pub focus: f64,
}

impl ProfileMorph {
    /// A morph target taken from a style's preset shape, sized to a profile.
    pub fn from_style(style: ProfileStyle, profile: &BandProfile) -> Self {
        let (a, b, crown_frac, edge_frac) = style.preset();
        Self {
            shape_a: a,
            shape_b: b,
            crown_mm: profile.thickness_mm * crown_frac,
            edge_round_mm: profile.thickness_mm * edge_frac * 0.5,
            focus: 2.0,
        }
    }
}

impl Default for BandProfile {
    fn default() -> Self {
        let mut p = Self {
            style: ProfileStyle::HalfRound,
            width_mm: 6.0,
            thickness_mm: 2.0,
            crown_mm: 1.8,
            shape_a: 2.0,
            shape_b: 2.0,
            crest_bias: 0.0,
            edge_round_mm: 0.1,
            comfort_fit_mm: 0.25,
            side_draft_deg: 2.0,
            flange: Flange::default(),
            drop_curve: DropCurve::default(),
            morph: None,
        };
        p.apply_style(ProfileStyle::HalfRound);
        p
    }
}

impl BandProfile {
    /// Overwrite the shape parameters from a style preset, keeping the overall
    /// width and thickness.
    pub fn apply_style(&mut self, style: ProfileStyle) {
        self.style = style;
        if style == ProfileStyle::Custom {
            return;
        }
        let (a, b, crown_frac, round_frac) = style.preset();
        self.shape_a = a;
        self.shape_b = b;
        self.crown_mm = self.thickness_mm * crown_frac;
        self.edge_round_mm = self.thickness_mm * round_frac;
    }

    /// Square the two side faces to the mould pull and shrink the fillet that
    /// eats them, which is what lets those faces hold deep relief.
    ///
    /// The face this leaves is `thickness - crown` wide, so it is only worth
    /// having on a style that keeps some of its thickness at the edge — a
    /// half-round spends 90% of it on the crown and has no side to square up.
    pub fn flatten_sides(&mut self) {
        self.side_draft_deg = 0.0;
        self.edge_round_mm = self.edge_round_mm.min(SQUARED_SIDE_FILLET_MM);
    }

    /// Crown clamped so the outer edge stays castably thick.
    pub fn effective_crown_mm(&self) -> f64 {
        self.crown_mm
            .clamp(0.0, (self.thickness_mm - MIN_EDGE_MM).max(0.0))
    }

    /// Radial thickness remaining at the outer edges.
    pub fn edge_thickness_mm(&self) -> f64 {
        (self.thickness_mm - self.effective_crown_mm()).max(MIN_EDGE_MM)
    }

    /// How far toward the morph target the section at a ring angle sits, 0..1.
    pub fn morph_weight(&self, theta_deg: f64) -> f64 {
        match &self.morph {
            None => 0.0,
            Some(m) => {
                let c = (theta_deg - TOP_DEG).to_radians().cos() * 0.5 + 0.5;
                c.powf(m.focus.clamp(0.5, 8.0))
            }
        }
    }

    /// Normalized drop from the crest, `x` in 0..1.
    ///
    /// Monotonically non-decreasing, and so undercut-free, whether it comes
    /// from the superellipse or from a monotone hand-drawn curve.
    pub fn drop(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        if self.style == ProfileStyle::Custom && self.drop_curve.is_active() {
            return self.drop_curve.eval(x);
        }
        superellipse_drop(x, self.shape_a, self.shape_b)
    }

    /// Seed the hand-drawn crown from the exponents currently in force, so
    /// picking up the pencil starts from the shape already on screen.
    pub fn adopt_drop_curve(&mut self, points: usize) {
        self.style = ProfileStyle::Custom;
        self.drop_curve = DropCurve::from_superellipse(self.shape_a, self.shape_b, points);
    }

    /// Go back to the superellipse, keeping the exponents.
    pub fn clear_drop_curve(&mut self) {
        self.drop_curve = DropCurve::default();
    }

    /// Sample the unmodulated cross-section loop.
    pub fn sample(&self, inner_r: f64, n: usize) -> ProfileLoop {
        self.sample_mod(inner_r, n, &ShankMod::identity())
    }

    /// Sample the cross-section loop with shank modulation applied.
    ///
    /// An enabled flange replaces the outer surface across its axial band with
    /// two flat faces and a rim, and its rim becomes the crest.
    pub fn sample_mod(&self, inner_r: f64, n: usize, m: &ShankMod) -> ProfileLoop {
        self.sample_spaced(inner_r, n, m, None, None)
    }

    /// Sample the cross-section, optionally placing the vertices by height
    /// field detail across `v` rather than at equal arc length.
    ///
    /// `field_v` is indexed by normalized `v`, which is what the layer stack is
    /// evaluated against, so one density computed from the reference profile
    /// applies to every modulated cross-section.
    ///
    /// `reference` pins everything about the row layout that must not vary
    /// per slice. A sweep must pass its reference loop, and the same one to
    /// every slice, for two reasons with the same shape:
    ///
    /// - the **feature fractions** rows snap onto: each slice's own features
    ///   drift with the modulation, and rows snapping to drifting targets
    ///   tear the grid along theta — measured as a 0.013% phantom undercut
    ///   on a bare signet head;
    /// - the **bore/surface row split**: rounded per slice it steps by one
    ///   wherever the surface's share of the loop crosses a half-row, every
    ///   surface row renumbers, and the whole grid tears a vertical zipper —
    ///   measured as 60-82 degree folds down a signet's shoulder at exactly
    ///   the slices where the split stepped.
    pub fn sample_spaced(
        &self,
        inner_r: f64,
        n: usize,
        m: &ShankMod,
        field_v: Option<&Density>,
        reference: Option<&ProfileLoop>,
    ) -> ProfileLoop {
        let n = n.clamp(MIN_PROFILE_STEPS, MAX_PROFILE_STEPS);
        let width = (self.width_mm * m.width_scale).max(0.4);
        let thickness = match m.outer_r {
            Some(r) => (r - inner_r).max(0.3),
            None => (self.thickness_mm * m.thickness_scale).max(0.3),
        };

        let hw = width * 0.5;
        // The section spans two intervals, not one symmetric half-width: the
        // **bore** carries the band's body and the **crest** carries whatever is
        // faceted onto it. On a signet those are different shapes — the face is
        // a flat facet cut across the crown of a wider swell — and the draft
        // between them is what makes a head read as a head. Both are absolute in
        // the section's own frame, which is what lets a face that stands upright
        // sit off-centre by a different amount at its crest than at its bore.
        let half_w = self.width_mm * 0.5;
        // Sanitised: both spans feed every corner of the section now, so a
        // non-finite one would put NaN into the loop rather than a bad shape.
        let ok = |v: f64| if v.is_finite() { v } else { 0.0 };
        let b_c = ok(m.z_center_frac * half_w);
        let (b_lo, b_hi) = (b_c - hw, b_c + hw);

        let comfort = self.comfort_fit_mm.clamp(0.0, hw * 0.8);
        // Morph the crown parameters toward the profile's target. A blend of
        // two monotone drops is monotone, so castability survives the ride.
        let mt = if self.morph.is_some() { m.drop_blend.clamp(0.0, 1.0) } else { 0.0 };
        let mlerp = |a: f64, b: f64| a + (b - a) * mt;
        let (crown_src, edge_src, morph_ab) = match (&self.morph, mt > 1e-9) {
            (Some(mo), true) => (
                mlerp(self.crown_mm, mo.crown_mm),
                mlerp(self.edge_round_mm, mo.edge_round_mm),
                Some((mo.shape_a, mo.shape_b)),
            ),
            _ => (self.crown_mm, self.edge_round_mm, None),
        };
        // The comfort dome eats into the band from the inside, so the crown may
        // only take what it leaves. Without this the bore reaches past the outer
        // surface at the band edge and the cross-section folds over itself — a
        // 0.25 mm comfort fit does not fit inside a 0.2 mm edge.
        let crown = (crown_src * m.thickness_scale * m.crown_scale)
            .clamp(0.0, (thickness - comfort - MIN_EDGE_MM).max(0.0));
        let edge_t = (thickness - crown).max(MIN_EDGE_MM + comfort);
        let draft = self.side_draft_deg.clamp(-20.0, 30.0).to_radians();
        let head_w = m.head.clamp(0.0, 1.0);
        // Side faces slope inward over the edge thickness, narrowing the band.
        // Not under a head: the face span already carries the head's own
        // draft, and the band's on top of it pulls the table in from the
        // silhouette the outline drew. Sanitised because the smooth clamps
        // below propagate a NaN where the hard ones used to swallow it.
        let side_inset = ok((edge_t * draft.tan()).clamp(-hw * 0.4, hw * 0.4)) * (1.0 - head_w);

        // The crest is drafted in from whatever span it was handed, so a mod
        // that hands it the bore's own span behaves exactly as it always did.
        let (t_lo, t_hi) = match m.crest_span {
            Some((lo, hi)) => (ok(lo * half_w), ok(hi * half_w)),
            None => (b_lo, b_hi),
        };
        let (t_lo, t_hi) = if t_lo < t_hi { (t_lo, t_hi) } else { (b_lo, b_hi) };
        let keep0 = (hw * 0.075).max(0.05);
        // The rim fillet takes its radius out of the crest span's ends, so
        // the flat that must straddle the parting plane is measured past it:
        // a 0.6 mm rim on a 0.28 mm straddle rounded the crest away from the
        // plane, and the span-must-reach-the-plane ceiling came back through
        // the fillet at -54 degrees over 0.18% of a Draft heart.
        let keep = keep0 + m.head_rim_mm.clamp(0.0, 2.0) * head_w;
        // The straddle clamps engage and release mid-sweep on an upright
        // outline — a heart's lobe is a patch on one side of the band — and a
        // hard clamp is a slope step in theta that sweeps a crease down the
        // wall at the locus where it bites. Rounded, they cost a whisker of
        // the held flat run and nothing else.
        let kr = keep0 * 0.5;
        let t_mid = 0.5 * (t_lo + t_hi);
        let c_lo = smin(t_lo + side_inset, t_mid - keep, kr);
        let c_hi = smax(t_hi - side_inset, t_mid + keep, kr);

        // The crest sits where the mould parts, plus whatever bias was asked
        // for. A section pushed along the finger has to keep it there: let the
        // crest ride down with the section and the flank between the two leans
        // back over the mould half it sits in — measured at -19 degrees over
        // 0.67% of the surface on a shield head, a real undercut and not facet
        // noise.
        //
        // Which means the crest span has to **reach** the parting plane. An
        // upright outline at its last station does not straddle it — a heart's
        // lobe is a patch on one side of the band and nothing on the other — so
        // a span taken as drawn puts the crest below the plane and turns
        // everything between the two into a ceiling. Measured on a heart:
        // 0.24% of the surface at -77 degrees, and it did not fall with the
        // sweep, which is what tells a real undercut from the crest-line
        // phantom. Widening costs the head's end a flat run of `keep`.
        let want = 0.5 * self.crest_bias.clamp(-1.0, 1.0) * (c_hi - c_lo);
        // The span edge can plunge through this floor at millimetres per
        // degree — a heart's lobe boundary moves 0.86 mm/deg — and a
        // crossfade with a small value-space radius transits in under a
        // degree at that speed, which is still a kink at any sweep
        // resolution: measured as 100 degree grid folds at exactly the two
        // slices where the edge crossed the floor. So the radius follows the
        // station: wide near a face's along-ring ends, where the plunge
        // lives, and tight over the plate's middle, where a wide one drags
        // boundaries that sit legitimately close to the floor — it pulled a
        // heart's cleft half shut. Biased up by the crossfade's worst
        // undershoot of a true max — 0.087 of the radius, at 0.4 radii of
        // separation — so the crest still reaches the parting plane.
        let r_w = kr + ((hw * 0.25).max(keep) - kr) * m.straddle_soft.clamp(0.0, 1.0);
        let (c_lo, c_hi) = (
            smin(c_lo, want - keep - 0.1 * r_w, r_w),
            smax(c_hi, want + keep + 0.1 * r_w, r_w),
        );
        let c_span = (c_hi - c_lo).max(1e-9);
        let margin = (keep / c_span).min(0.45);
        let base_crest_t = ((want - c_lo) / c_span).clamp(margin, 1.0 - margin);
        let flange_v = self.flange.v_pos.clamp(0.0, 1.0);
        // A castable flange position takes the crest with it.
        let crest_t = match self.flange.enabled {
            true if flange_v <= EDGE_FLANGE_T => 0.0,
            true if flange_v >= 1.0 - EDGE_FLANGE_T => 1.0,
            true if (flange_v - base_crest_t).abs() <= CREST_FLANGE_T => flange_v,
            _ => base_crest_t,
        };
        let crest_z = c_lo + c_span * crest_t;

        let cap_r = |r: f64| match m.outer_max_r {
            Some(cap) => r.min(cap.max(inner_r + MIN_EDGE_MM)),
            None => r,
        };
        let drop_at = |x: f64| -> f64 {
            match morph_ab {
                None => self.drop(x),
                Some((a2, b2)) => mlerp(self.drop(x), superellipse_drop(x, a2, b2)),
            }
        };
        // The flank skew is a power on the normalized distance: monotone in,
        // monotone out, so a skewed flank is still a drop from a single crest.
        let bias = m.flank_bias.clamp(-1.0, 1.0);
        let gamma = |low: bool| {
            let sign = if low { 1.0 } else { -1.0 };
            (1.0 + 0.5 * bias * sign).clamp(0.45, 2.2)
        };
        let r_at = |z: f64| -> f64 {
            let z = z.clamp(c_lo, c_hi);
            let (x, low) = if z <= crest_z {
                ((crest_z - z) / (crest_z - c_lo).max(1e-9), true)
            } else {
                ((z - crest_z) / (c_hi - crest_z).max(1e-9), false)
            };
            let x = if bias.abs() > 1e-9 { x.powf(gamma(low)) } else { x };
            cap_r(inner_r + thickness - crown * drop_at(x))
        };
        let bore_r = |z: f64| -> f64 { inner_r + comfort * ((z - b_c) / hw.max(1e-9)).powi(2) };

        // --- Flange band, clamped to sit inside the outer profile. ---
        let flange = self.flange.enabled.then(|| {
            let max_t = c_span * 0.8;
            let t = self.flange.thickness_mm.clamp(MIN_EDGE_MM.min(max_t), max_t);
            let z_c = c_lo + c_span * flange_v;
            let z_lo = (z_c - 0.5 * t).clamp(c_lo, c_hi - t);
            let extent = self.flange.extent_mm.clamp(0.0, width.max(thickness));
            FlangeBand {
                z_lo,
                z_hi: z_lo + t,
                // Floored clear of the dome crest, so the rim is the crest.
                rim_r: cap_r((r_at(z_c) + extent).max(inner_r + thickness + MIN_EDGE_MM)),
            }
        });

        const DENSE: usize = 12;

        // --- Bore span: +hw down to -hw, outward normal facing the finger. ---
        let nb = n * DENSE / 3;
        let bore: Vec<[f64; 2]> = (0..=nb)
            .map(|i| {
                let z = b_hi - (b_hi - b_lo) * (i as f64 / nb as f64);
                [bore_r(z), z]
            })
            .collect();

        // --- Surface span: bottom side face, over the crown, top side face. ---
        let corner_b = [inner_r + edge_t, c_lo];
        let corner_t = [inner_r + edge_t, c_hi];
        let side_b_start = [bore_r(b_lo), b_lo];
        let side_t_end = [bore_r(b_hi), b_hi];

        let ns = n * DENSE * 2 / 3;
        let dome = |z0: f64, z1: f64, steps: usize| -> Vec<[f64; 2]> {
            let steps = steps.max(1);
            (0..=steps)
                .map(|i| {
                    let z = z0 + (z1 - z0) * (i as f64 / steps as f64);
                    [r_at(z), z]
                })
                .collect()
        };

        // Points where the section's slope is discontinuous; a sample row is
        // snapped onto each so the facets meet at the feature instead of
        // chording across it.
        let mut feats: Vec<[f64; 2]> = vec![[r_at(crest_z), crest_z]];

        let outer: Vec<[f64; 2]> = match &flange {
            None => dome(c_lo, c_hi, ns),
            Some(f) => {
                let lo_span = f.z_lo - c_lo;
                let hi_span = c_hi - f.z_hi;
                let span = (lo_span + hi_span).max(1e-9);
                let steps = |s: f64| ((ns as f64 * s / span) as usize).max(2);
                let fillet = |dome_span: f64, flat: f64| {
                    self.flange.edge_round_mm.clamp(0.0, (dome_span.min(flat) * 0.4).max(0.0))
                };
                let mut o: Vec<[f64; 2]> = Vec::with_capacity(ns + 4 * ARC_STEPS);
                if lo_span > 1e-9 {
                    let fr = fillet(lo_span, f.rim_r - r_at(f.z_lo));
                    let d = dome(c_lo, f.z_lo - fr, steps(lo_span));
                    let p0 = *d.last().unwrap_or(&corner_b);
                    o.extend(d);
                    push_arc(&mut o, p0, [r_at(f.z_lo), f.z_lo], [r_at(f.z_lo) + fr, f.z_lo]);
                    feats.push(p0);
                    feats.push([r_at(f.z_lo) + fr, f.z_lo]);
                } else {
                    o.push(corner_b);
                }
                o.push([f.rim_r, f.z_lo]);
                o.push([f.rim_r, f.z_hi]);
                feats.push([f.rim_r, f.z_lo]);
                feats.push([f.rim_r, f.z_hi]);
                if hi_span > 1e-9 {
                    let fr = fillet(hi_span, f.rim_r - r_at(f.z_hi));
                    let d = dome(f.z_hi + fr, c_hi, steps(hi_span));
                    let p0 = [r_at(f.z_hi) + fr, f.z_hi];
                    o.push(p0);
                    push_arc(&mut o, p0, [r_at(f.z_hi), f.z_hi], d[0]);
                    feats.push(p0);
                    feats.push(d[0]);
                    o.extend(d.into_iter().skip(1));
                } else {
                    o.push(corner_t);
                }
                o
            }
        };

        let cap = thickness.min(c_span * 0.5) * 0.45;
        // Under a head the rim rounding is the head's own, not the band's
        // edge fillet: the plate rim is the one edge a signet has, and how
        // hard it reads should not depend on how the shank's edges are broken.
        let er = (edge_src + (m.head_rim_mm - edge_src) * head_w).clamp(0.0, cap);
        // Each end fillet is capped by the dome the flange leaves at that edge.
        let (er_b, er_t) = match &flange {
            Some(f) => (er.min((f.z_lo - c_lo) * 0.4), er.min((c_hi - f.z_hi) * 0.4)),
            None => (er, er),
        };
        // Where the straddle floor holds the span past the outline's own
        // reach, the forced run rolls as one fillet instead of flat plus
        // corner: without it the plate's end is a hard corner migrating
        // across fixed sample rows, which tessellates as 130 degree folds at
        // the face's end. The fillet takes at most 0.85 of the forced run,
        // so the flat still reaches the outline everywhere, and it stops
        // short of the parting plane: the crest has to reach that flat.
        let roll = |er0: f64, w_f: f64, room: f64| {
            if w_f <= 1e-9 {
                er0
            } else {
                er0.max((0.85 * w_f).min(room.max(0.0))).min(cap)
            }
        };
        let w_f_lo = (t_lo + side_inset - c_lo).max(0.0) * head_w;
        let w_f_hi = (c_hi - (t_hi - side_inset)).max(0.0) * head_w;
        let er_b = roll(er_b, w_f_lo, -c_lo * 0.9);
        let er_t = roll(er_t, w_f_hi, c_hi * 0.9);

        // A head's wall is one convex C² curve: vertical into the rim fillet
        // so the plate holds the outline's own shape, vertical again into the
        // bore corner, the whole bore-to-crest offset carried in the belly
        // between — the inflated near-prism the reference heads are, with the
        // heart's cleft riding down it as a smooth cove. Off the head it is
        // the straight chord of the band's own draft. `head` crossfades the
        // two blend weights; both are monotone in `z`, so the wall can never
        // fold back into a ceiling at any mix.
        let wall = |z_a: f64, z_b: f64, r0: f64, r1: f64| -> Vec<[f64; 2]> {
            (1..FLANK_STEPS)
                .map(|i| {
                    let t = i as f64 / FLANK_STEPS as f64;
                    let s = crate::field::smootherstep(0.0, 1.0, t);
                    [r0 + (r1 - r0) * t, z_a + (z_b - z_a) * (t + (s - t) * head_w)]
                })
                .collect()
        };
        let flank_b = wall(b_lo, c_lo, side_b_start[0], corner_b[0]);
        let flank_t = {
            let mut v = wall(b_hi, c_hi, side_t_end[0], corner_t[0]);
            v.reverse();
            v
        };
        // The fillet's wall-side tangency sits a full radius back along the
        // wall, so the wall's last `er` of arc belongs to the fillet. On a
        // straight wall the split point is exactly where the fillet's own
        // back-off used to land, so ordinary bands are unchanged.
        let split_at = |pts: &[[f64; 2]], corner: [f64; 2], er: f64, from_end: bool| {
            let mut left = er.max(0.0);
            let mut prev = corner;
            let order: Vec<usize> =
                if from_end { (0..pts.len()).rev().collect() } else { (0..pts.len()).collect() };
            for k in order {
                let p = pts[k];
                let d = dist(prev, p);
                if d >= left {
                    let t = left / d.max(1e-12);
                    let at = [prev[0] + (p[0] - prev[0]) * t, prev[1] + (p[1] - prev[1]) * t];
                    return (if from_end { k + 1 } else { k }, at);
                }
                left -= d;
                prev = p;
            }
            if from_end {
                (0, *pts.first().unwrap_or(&corner))
            } else {
                (pts.len(), *pts.last().unwrap_or(&corner))
            }
        };

        let mut surface: Vec<[f64; 2]> =
            Vec::with_capacity(outer.len() + 2 * FLANK_STEPS + 4 * DENSE + 4);
        surface.push(side_b_start);
        let (kb, side_b_at) = split_at(&flank_b, corner_b, er_b, true);
        surface.extend_from_slice(&flank_b[..kb]);
        let (fb0, fb1) = push_fillet(&mut surface, side_b_at, corner_b, &outer, false, er_b);
        surface.extend_from_slice(trim_outer(&outer, er_b, er_t));
        let (kt, side_t_at) = split_at(&flank_t, corner_t, er_t, false);
        let (ft0, ft1) = push_fillet(&mut surface, side_t_at, corner_t, &outer, true, er_t);
        surface.extend_from_slice(&flank_t[kt..]);
        surface.push(side_t_end);
        dedup(&mut surface);
        feats.extend([fb0, fb1, ft0, ft1]);
        let feature_v = project_fractions(&surface, &feats);

        // --- Place the vertex budget. ---
        let len_b = polyline_len(&bore);
        let len_s = polyline_len(&surface);

        let pts: Vec<ProfileSample> = match field_v {
            // Equal arc length: the budget goes by span length and the samples
            // sit evenly along each, except that a sample is snapped onto
            // every feature so no facet chords across a slope discontinuity.
            None => {
                let total = (len_b + len_s).max(1e-9);
                // The split comes from the reference loop when there is one:
                // rounding it per slice renumbers every surface row at the
                // slices where it steps, and the grid tears there.
                let n_s = match reference {
                    Some(rl) if !rl.pts.is_empty() => {
                        let frac =
                            (rl.pts.len() - rl.surface_start) as f64 / rl.pts.len() as f64;
                        ((n as f64 * frac).round() as usize).clamp(12, n - 12)
                    }
                    _ => ((n as f64 * len_s / total).round() as usize).clamp(12, n - 12),
                };
                let even = |c: usize| (0..c).map(|i| i as f64 / c as f64).collect::<Vec<f64>>();
                let bore_pts = resample_at(&bore, &even(n - n_s));
                let surf_pts = resample_at(
                    &surface,
                    &snap_positions(n_s, reference.map_or(&feature_v[..], |rl| &rl.feature_v)),
                );
                bore_pts
                    .into_iter()
                    .map(|p| ProfileSample::bare(p[0], p[1], false))
                    .chain(surf_pts.into_iter().map(|p| ProfileSample::bare(p[0], p[1], true)))
                    .collect()
            }
            // By detail. Resampled as one closed polyline rather than two spans:
            // the corners where the bore meets the side faces are then interior
            // to the curvature density and always earn a sample. Sampling the
            // spans apart leaves those corners cut across by a chord, and a
            // chopped corner shrinks only as 1/n where a chorded curve shrinks
            // as 1/n² — so it comes to dominate however large the budget gets.
            Some(f) => {
                let mut closed = bore.clone();
                closed.extend_from_slice(&surface[1..]);
                let mut d = adaptive::curvature_density(&closed);
                d.finish_against(d.peak());
                // The field only displaces the surface span, which is the tail
                // of the loop.
                let total = (len_b + len_s).max(1e-9);
                d.combine_range(f, len_b / total, 1.0);

                resample_at(&closed, &d.positions(n))
                    .into_iter()
                    .zip(d.positions(n))
                    .map(|(p, t)| ProfileSample::bare(p[0], p[1], t * total >= len_b))
                    .collect()
            }
        };

        finish_loop(pts, feature_v)
    }
}

/// One vertex of the swept cross-section.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProfileSample {
    /// Radius from the ring axis, mm.
    pub r: f64,
    /// Axial position, mm.
    pub z: f64,
    /// Outward 2D normal, radial component.
    pub nr: f64,
    /// Outward 2D normal, axial component.
    pub nz: f64,
    /// Arc distance along the non-bore surface, mm.
    pub v_mm: f64,
    /// Whether this vertex lies on the displaceable outer surface.
    pub surface: bool,
    /// Displacement weight, fading to zero at the bore corners.
    pub weight: f64,
}

impl ProfileSample {
    fn bare(r: f64, z: f64, surface: bool) -> Self {
        Self { r, z, surface, ..Default::default() }
    }
}

/// A closed cross-section loop plus the parameterization the height field uses.
#[derive(Clone, Debug, Default)]
pub struct ProfileLoop {
    pub pts: Vec<ProfileSample>,
    /// Total arc length of the displaceable surface, mm. This is the `v` span.
    pub surface_len_mm: f64,
    /// `v` of the crest (maximum radius), mm.
    pub crest_v_mm: f64,
    /// Maximum radius, mm.
    pub crest_radius_mm: f64,
    /// Index of the first displaceable vertex.
    pub surface_start: usize,
    /// Normalized positions (0..1 along the surface span) of the section's
    /// slope discontinuities: crest, fillet tangencies, flange corners. A
    /// sample row sits on each, and refinement can split at them.
    pub feature_v: Vec<f64>,
}

impl ProfileLoop {
    pub fn len(&self) -> usize {
        self.pts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pts.is_empty()
    }

    /// Axial extent (min z, max z), mm.
    pub fn z_range(&self) -> (f64, f64) {
        self.pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.z), hi.max(p.z))
        })
    }
}

// --- Shank modulation ------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShankKind {
    Uniform,
    Tapered,
    ReverseTaper,
    Pinched,
    Bombe,
    Saddle,
    Cathedral,
    Wave,
    Twist,
    EuroFlat,
    FlatTop,
    Signet,
}

impl ShankKind {
    pub const ALL: &'static [ShankKind] = &[
        ShankKind::Uniform,
        ShankKind::Tapered,
        ShankKind::ReverseTaper,
        ShankKind::Pinched,
        ShankKind::Bombe,
        ShankKind::Saddle,
        ShankKind::Cathedral,
        ShankKind::Wave,
        ShankKind::Twist,
        ShankKind::EuroFlat,
        ShankKind::FlatTop,
        ShankKind::Signet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShankKind::Uniform => "Uniform",
            ShankKind::Tapered => "Tapered",
            ShankKind::ReverseTaper => "Reverse Taper",
            ShankKind::Pinched => "Pinched",
            ShankKind::Bombe => "Bombé",
            ShankKind::Saddle => "Saddle",
            ShankKind::Cathedral => "Cathedral",
            ShankKind::Wave => "Wave",
            ShankKind::Twist => "Twist",
            ShankKind::EuroFlat => "Euro (flat bottom)",
            ShankKind::FlatTop => "Flat top",
            ShankKind::Signet => "Signet",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ShankKind::Uniform => "Constant section all the way around.",
            ShankKind::Tapered => "Narrows toward the bottom of the finger.",
            ShankKind::ReverseTaper => "Narrows toward the top, widening at the palm.",
            ShankKind::Pinched => "Waists in just below the top, so the crown reads set-off.",
            ShankKind::Bombe => "Swells full and round at the top, slimming to the palm.",
            ShankKind::Saddle => "Low, wide top hugging the finger, round through the palm.",
            ShankKind::Cathedral => "Shoulders swell toward the top of the ring.",
            ShankKind::Wave => {
                "The band's edges wave along the finger while the crest stays level — one \
                 wave is the curved band that hugs a solitaire."
            }
            ShankKind::Twist => {
                "Reads as a twisted band: the edges wave while the steep flank alternates \
                 sides, so the light-line spirals — and everything still pulls. A true helix \
                 locks in the sand."
            }
            ShankKind::EuroFlat => "Flat chord across the bottom so the ring will not spin.",
            ShankKind::FlatTop => "Flat chord faceted across the top of the ring.",
            ShankKind::Signet => {
                "Narrow shank swelling into a broad, flat-topped head. The head is the band \
                 itself — the face outline is the band's own silhouette and the table is its \
                 crest — so set Width to the head and let the taper make the rest."
            }
        }
    }
}

/// A signet head: the band's own swell into a broad, flat-topped face.
///
/// Not a pad standing on the band — the head *is* the band over its arc. The
/// **body** is the band's plan silhouette, the union of the face's outline and
/// a long swell that carries the width back to the shank; the **table** is the
/// band's crest, solved onto a plane; and the **shoulder** is the arc over
/// which that crest falls back to the shank. Everything is one continuous
/// sweep, which is why there is no seam where a head meets a shoulder.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SignetHead {
    /// Plan silhouette of the face. The band's width follows it over the head.
    pub outline: SignetOutline,
    /// Where the head sits round the ring, degrees. 90 is the top.
    pub theta_deg: f64,
    /// Extent of the face around the ring, mm, measured on the table plane.
    /// The extent *across* the band is the profile's own width.
    pub length_mm: f64,
    /// How far the centre of the table stands above the band's crest, mm. The
    /// ends of a flat table stand higher still — that is what a plane does over
    /// a curve, and it is the chunk a signet head reads as.
    pub rise_mm: f64,
    /// Arc over which the crest falls from the head back to the shank, degrees.
    pub shoulder_deg: f64,
    /// Arc over which the *width* comes back to the shank, degrees. Much longer
    /// than the face or the shoulder: this is what a signet reads as from the
    /// side. `None` takes it from the head's own half-angle, which is what
    /// makes a big head flare further than a small one.
    #[serde(default)]
    pub swell_deg: Option<f64>,
    /// How far the body under the table rounds away from the face's outline.
    /// 0 extrudes the face straight down to the finger; 1 leaves the shape on
    /// the table and fairs everything beneath it.
    #[serde(default = "default_body_fair")]
    pub body_fair: f64,
    /// How flat the table is: 1 is a true plane, 0 keeps the profile's own
    /// crown so the head stays domed.
    pub table_flat: f64,
    /// Dome standing on the table's centre, mm — a cabochon or buff-top head.
    /// Besides the look, a domed table has real draft everywhere, where a
    /// dead-flat one is the zero-draft plane behind the refined-build phantom.
    #[serde(default)]
    pub table_dome_mm: f64,
    /// Rounding between the table and the head's walls, mm. The face outline
    /// is the head's one real edge, and this is how hard it reads. Measured on
    /// the reference heart: the rim's rounding reaches 0.9 mm below the plane,
    /// and it is the only place on the whole head with any edge at all.
    #[serde(default = "default_rim_round")]
    pub rim_round_mm: f64,
}

fn default_body_fair() -> f64 {
    HEAD_BODY_FAIR
}

fn default_rim_round() -> f64 {
    HEAD_RIM_ROUND
}

/// Extent of a signet face around the ring, mm.
pub const HEAD_LENGTH_MM: f64 = 12.0;
/// How far the centre of a signet table stands above the band's crest, mm.
pub const HEAD_RISE_MM: f64 = 0.3;
/// Arc a signet shoulder takes to fall from the head to the shank, degrees.
///
/// Measured off `BlankSignet.obj`: its crest follows the table plane out to the
/// face's edge at 31.6 degrees, peaks at the table's corner, and is back on the
/// shank by 75 — 43 degrees of fall.
pub const HEAD_SHOULDER_DEG: f64 = 43.0;
/// Shape of that fall as `(1 - s)^p`.
///
/// The reference leaves the table's rim **already diving** and eases into the
/// shank, which is the opposite of a Hermite: 0.85 of the drop left 3 degrees
/// past the rim against a Hermite's 0.97, and 0.14 at 23 degrees against 0.23.
/// A rim is an edge, and the flank under it is a fillet running out flat.
pub const HEAD_SHOULDER_POW: f64 = 2.4;
/// Arc a signet's *width* takes to come back to the shank, as a multiple of the
/// head's own half-angle.
///
/// A **ratio** and not an angle, because the swell is the head's influence and a
/// bigger head reaches further. Two real signets, measured off their meshes and
/// agreeing to within a tenth:
///
/// | | half-angle | swell | ratio |
/// | --- | --- | --- | --- |
/// | `BlankSignet.obj`, 14.7 mm round face | 32.4° | 78° | 2.4 |
/// | the heart signet, 18.5 mm face on a size 7 | 41.7° | 100° | 2.4 |
///
/// A fixed 75° came from the first alone, and on the second it ran the swell out
/// by 80 degrees where the ring is still flaring past 90.
pub const HEAD_SWELL_RATIO: f64 = 2.4;
/// How far the body under a signet's table rounds away from the face outline.
pub const HEAD_BODY_FAIR: f64 = 1.0;
/// Half-angle a head may reach before a table plane runs away from the band:
/// the plane's radius goes as `1/cos`, so this is what bounds the length.
pub const HEAD_MAX_HALF_DEG: f64 = 70.0;
/// Shank width as a fraction of the head at full strength.
pub const SIGNET_MIN_SHANK_FRAC: f64 = 0.16;
/// How much a signet shank rounds off as it narrows. The crown clamp caps it.
pub const SIGNET_SHANK_ROUNDING: f64 = 9.0;
/// How much of its thickness a signet shank gives up behind the head. It keeps
/// most of it, which is what leaves a round wire at the back rather than a
/// ribbon.
pub const SIGNET_SHANK_THIN: f64 = 0.10;
/// How much narrower the face is at the table than where it meets the band, as
/// a share of the head's width.
///
/// A signet's head is not a straight-sided slab: its flanks are drafted, so the
/// table is a slightly smaller copy of the outline that carries it. Measured on
/// `BlankSignet.obj`, the body is 16.0 mm across where its table is 14.7 — and
/// those flanks are the surface a two-part mould has to slide off, so drafting
/// them is worth more than the look.
pub const HEAD_FACE_DRAFT: f64 = 0.02;
/// Taper strength a fresh signet head starts at.
pub const SIGNET_TAPER: f64 = 0.85;
/// Default rounding between a head's table and its walls, mm.
///
/// The reference heart has **no sharp edges at all** outside its bore break —
/// a dihedral census over its mesh finds 0.0 mm of >=15 degree creases
/// anywhere else, with the rim's rounding reaching 0.9 mm below the plane.
/// The face outline still reads as an edge because the fillet is small
/// against the head; it is just never a corner.
pub const HEAD_RIM_ROUND: f64 = 0.6;
/// Fillet where the outline crosses the swell, as a share of the width the
/// taper takes away. A bare crossing is a corner in the silhouette.
pub const HEAD_SHANK_FILLET: f64 = 0.12;
/// Arc over which the body hands the head's outline over to the swell, as a
/// share of the face's own half-angle, ending at the face's end.
pub const HEAD_EDGE_BREAK: f64 = 0.45;
/// How far into the swell the outline-against-swell fillet opens to full size,
/// as a share of the swell's arc.
///
/// It has to open somewhere: at the head the two are equal, and a fillet there
/// would round the tie upward and push the band past full width. Opening it over
/// the whole swell instead leaves it too small where a straight-sided outline
/// crosses — a rectangle steps 0.25 of full width per degree against 0.10.
pub const HEAD_FILLET_ON: f64 = 0.12;
/// How far inside its end the face's outline read is held, as a share of the
/// half-length.
///
/// Zero: the outline is read to its true end, which is what closes a heart's
/// lobes instead of chopping them. The fin this hold once prevented — a table
/// read at a point wedging the section to nothing — is prevented at the source
/// now: the straddle floor keeps every crest span a real strip and the forced
/// run rolls as one fillet. The approach to the hold is still smoothed
/// ([`ShankStyle::head_at`]), so raising this again cannot kink the sweep.
pub const HEAD_TAKEOFF: f64 = 0.0;

// Tie-exact smooth maximum; the design rationale lives on its definition. Two
// curves meeting tangentially — the outline and the swell — are a tie
// everywhere they touch, and a quadratic smooth-max rounding that tie outward
// once fattened the whole shank by 4%.
use crate::field::smax;

/// Minimum with the same rounded corner.
fn smin(a: f64, b: f64, r: f64) -> f64 {
    -smax(-a, -b, r)
}

impl Default for SignetHead {
    fn default() -> Self {
        Self {
            outline: SignetOutline::Oval,
            theta_deg: TOP_DEG,
            length_mm: HEAD_LENGTH_MM,
            rise_mm: HEAD_RISE_MM,
            shoulder_deg: HEAD_SHOULDER_DEG,
            swell_deg: None,
            body_fair: HEAD_BODY_FAIR,
            table_flat: 1.0,
            table_dome_mm: 0.0,
            rim_round_mm: HEAD_RIM_ROUND,
        }
    }
}

impl SignetHead {
    /// The arc the width takes to come back to the shank, given the head's own
    /// half-angle. Set outright if asked, and otherwise scaled with the head.
    pub fn swell_arc_deg(&self, face_half_deg: f64) -> f64 {
        self.swell_deg
            .unwrap_or(face_half_deg * HEAD_SWELL_RATIO)
            .clamp(1.0, 170.0)
    }

    /// Size the face to the shape, so picking an outline gives that shape
    /// rather than the last one stretched to a new silhouette.
    pub fn fit_length_to(&mut self, band_width_mm: f64) {
        self.length_mm = (band_width_mm.max(1.0) * self.outline.head_aspect()).clamp(2.0, 40.0);
    }
}

/// Where one ring angle falls on a signet head.
#[derive(Clone, Copy, Debug)]
pub struct HeadAt {
    /// Position along the table plane in half-lengths: 0 at the centre, ±1 at
    /// the two ends, saturating there. Signed, because an outline need not be
    /// symmetric — a shield's flat top and its point are opposite ends of the
    /// head, and folding them together would leave two flat tops.
    pub x: f64,
    /// How far the **body** reaches across the band here, as `(low, high)`
    /// fractions of the head's half-width. An interval rather than a width: an
    /// upright outline reaches further one way than the other, and that is what
    /// moves the band off its own mid-plane.
    pub reach: (f64, f64),
    /// How far it reaches at the **crest**, in the same units — the table, a
    /// drafted copy of the same shape. `reach` is what the band spans at its
    /// bore; the difference between the two is the draft on the head's flanks.
    pub face: (f64, f64),
    /// 1 anywhere on the table, falling to 0 where the shank is plain again.
    pub on_head: f64,
    /// Radius the crest reaches here, mm.
    pub outer_r: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ShankStyle {
    pub kind: ShankKind,
    /// Strength of the modulation, 0..1. On a signet this is how far the shank
    /// narrows behind the head.
    pub amount: f64,
    /// Wave only: waves per revolution. Integer, so the band closes on itself.
    #[serde(default = "default_waves")]
    pub waves: u32,
    /// Signet only: the head the band swells into.
    #[serde(default)]
    pub head: SignetHead,
}

fn default_waves() -> u32 {
    1
}

impl Default for ShankStyle {
    fn default() -> Self {
        Self {
            kind: ShankKind::Uniform,
            amount: 0.5,
            waves: default_waves(),
            head: SignetHead::default(),
        }
    }
}

impl ShankStyle {
    /// Switch to a signet and give the head proportions that read as one,
    /// rather than leaving it on whatever the last style used.
    pub fn apply_signet(&mut self, band_width_mm: f64) {
        self.kind = ShankKind::Signet;
        self.amount = SIGNET_TAPER;
        self.head.fit_length_to(band_width_mm);
    }

    /// Share of the head width left at a ring angle, and where that width sits
    /// across the band — the band is the **union** of two strips, the shank
    /// running the whole way round and the face standing where it stands.
    ///
    /// A union, not a blend. The outline is followed as drawn wherever it
    /// stands clear: easing it into the shank fattens the shape, and the whole
    /// point of the head is that its silhouette *is* the face. What the two
    /// need is a fillet where they cross, which is what [`smax`] and [`smin`]
    /// give.
    ///
    /// Returns `(width_frac, centre_frac)`, both against the unmodulated width.
    pub fn signet_band(&self, theta_deg: f64, inner_r: f64, base_outer_r: f64) -> (f64, f64) {
        let (lo, hi) = self.signet_span(theta_deg, inner_r, base_outer_r);
        ((hi - lo) * 0.5, (hi + lo) * 0.5)
    }

    /// The band's span at its bore, as `(low, high)` fractions of the head's
    /// half-width.
    pub fn signet_span(&self, theta_deg: f64, inner_r: f64, base_outer_r: f64) -> (f64, f64) {
        let shank = self.signet_shank_frac(theta_deg);
        let a = self.head_at(theta_deg, inner_r, base_outer_r);
        // A floor, not a blend. The swell already lands on the shank, so this
        // only catches an outline narrower across the band than the shank it
        // stands on — a heart's dimple against a barely-tapered band. A hard
        // floor, deliberately: a crossfaded one dips below `reach` near the
        // swell's tail, past the crest span's 2% draft margin, and the crest
        // pokes through the body as a thin ceiling ring — measured 0.18% on a
        // Draft heart. Behind the head the floor is an exact tie and costs
        // nothing.
        (a.reach.0.min(-shank), a.reach.1.max(shank))
    }

    /// Fraction of the band width the head leaves at a ring angle.
    pub fn signet_width_frac(&self, theta_deg: f64, inner_r: f64, base_outer_r: f64) -> f64 {
        self.signet_band(theta_deg, inner_r, base_outer_r).0
    }

    /// Half-width of the shank strip, against the head's own.
    ///
    /// A constant. The reference signet's shank varies by 1% over the whole
    /// 215 degrees behind its head — what makes it read as tapering is the
    /// length of the swell in front, not any taper in the strip itself.
    pub fn signet_shank_frac(&self, _theta_deg: f64) -> f64 {
        let k = self.amount.clamp(0.0, 1.0);
        1.0 - (1.0 - SIGNET_MIN_SHANK_FRAC) * k
    }

    /// 0 beneath the head, 1 opposite it.
    fn away_from_head(&self, theta_deg: f64) -> f64 {
        (1.0 - (theta_deg - self.head.theta_deg).to_radians().cos()) * 0.5
    }

    /// Where a ring angle falls on the signet head: what the **body** spans
    /// there, what the **table** spans, and where the crest sits.
    ///
    /// The body is the union of two things that run out at different rates, and
    /// keeping them apart is what makes the head read as a head.
    ///
    /// - The **face** is the outline, read at the position this angle projects
    ///   to *on the table plane* rather than at the angle itself. It runs out
    ///   where the plane does, which for a 12 mm head on a size 7 is about 30
    ///   degrees off the top.
    /// - The **swell** is the head's own span at its centre, faded to the shank
    ///   over [`SignetHead::swell_deg`] — two and a half times as far.
    ///
    /// Take the union and the body follows the outline wherever the outline
    /// stands clear of the swell — a heart's lobes, a shield's shoulders — and
    /// follows the swell everywhere else, which is the whole tail. That is the
    /// thing a signet reads as from the side, and it is what the face alone
    /// could not do: the band's silhouette *was* the face outline, so the swell
    /// was over the moment the face was, 30 degrees where the reference takes
    /// 75.
    ///
    /// The table is the same outline drafted in by [`HEAD_FACE_DRAFT`], handed
    /// to the section as its crest span. Past the face it eases to the body's
    /// own drafted span, which is an ordinary dome.
    ///
    /// **The crest cannot leave the face flat.** With a plain ramp off the end
    /// of the table, measured on a cushion head, the crest went from climbing at
    /// 0.14 mm per degree to nothing in one step: a lip standing 2.9 mm proud,
    /// which is the thing a real signet does not have. So the fall is a Hermite
    /// landing flat on the shank, and it takes [`HEAD_SHOULDER_DEG`] rather than
    /// the whole swell — the width goes on widening under a crest that has
    /// already come down, which is exactly the broad thin shoulder of the
    /// reference.
    pub fn head_at(&self, theta_deg: f64, inner_r: f64, base_outer_r: f64) -> HeadAt {
        let k = self.amount.clamp(0.0, 1.0);
        let t0 = (base_outer_r - inner_r).max(0.05);
        let r_shank =
            inner_r + t0 * (1.0 - SIGNET_SHANK_THIN * k * self.away_from_head(theta_deg));

        let plane_r = base_outer_r + self.head.rise_mm.max(0.0);
        let half_l = (self.head.length_mm.max(0.5) * 0.5)
            .min(plane_r * HEAD_MAX_HALF_DEG.to_radians().tan());
        let signed =
            crate::field::wrap_delta(theta_deg - self.head.theta_deg, 360.0).to_radians();
        let d = signed.abs();
        let end = if signed < 0.0 { -1.0 } else { 1.0 };
        let face_edge = (half_l / plane_r).atan();

        // --- Body: the outline faired out, at the station this angle lands on.
        //
        // The band follows the **body**, not the face. They are different shapes
        // and have to be: extruding a face down to the finger gives a prism, and
        // a heart's dimple then runs the whole depth of the ring while its lobes
        // leave a crease down each flank. `body_extent` is the face's own reach
        // dilated and blurred, so it holds the head's proportions and none of
        // its detail — and contains the face, so the flank stays drafted.
        let x = (plane_r * d.min(face_edge).tan() / half_l).clamp(0.0, 1.0);
        let k_fair = self.head.body_fair.clamp(0.0, 1.0);
        let face_at = |s: f64| self.head.outline.extent(s);
        let body_at =
            |s: f64| blend_span(face_at(s), self.head.outline.body_extent(s), k_fair);
        let body = body_at(end * x);

        // --- Swell: the head's span at its centre, faded to the shank. ---
        let shank = self.signet_shank_frac(theta_deg);
        let arc = self.head.swell_arc_deg(face_edge.to_degrees());
        let g = 1.0 - crate::field::smoothstep(0.0, 1.0, d.to_degrees() / arc);
        let swell = blend_span(body_at(0.0), (-shank, shank), 1.0 - g);

        // The outline hands the band over to the swell across its end, rather
        // than stopping where the face does.
        //
        // A straight-ended outline reaches full width right up to its last
        // station and nothing past it — a shield's flat top runs the whole
        // length of the head — so an outline that simply stops steps the band
        // from full width onto the swell in one sample. Measured on a shield:
        // 1.76 of full width per degree, which is a wall. Faired onto the swell
        // it is 0.002, and what it costs is a chamfer on the head's last sixth
        // in plan, which is the edge break a corner wanted anyway.
        // It closes *at* the face's end and not across it, so that whatever the
        // outline does at its last station is multiplied by nothing. The
        // silhouette's slope is discontinuous there for any shape with a
        // straight end — the station stops advancing while the outline is still
        // plunging — and a fade still half open at that point carries the kink
        // straight through: a rectangle steps 0.19 of full width per degree
        // against 0.01 when the fade has already closed.
        let e = HEAD_EDGE_BREAK.clamp(1e-3, 0.9);
        let fade = 1.0 - crate::field::smootherstep(face_edge * (1.0 - e), face_edge, d);
        let faired = blend_span(swell, body, fade);

        // Filleted where the outline crosses the swell — a heart's lobes, a
        // shield's shoulders — because a bare crossing is a corner in the
        // silhouette.
        //
        // The radius closes to nothing wherever the two are equal by
        // construction: at the head, and everywhere past the outline's end. A
        // smooth maximum rounds a tie *upward*, so leaving it open there would
        // push the head past full width and fatten the whole shank.
        let r = HEAD_SHANK_FILLET
            * (1.0 - shank)
            * fade
            * crate::field::smoothstep(0.0, HEAD_FILLET_ON, d.to_degrees() / arc);
        let reach = (smin(faired.0, swell.0, r), smax(faired.1, swell.1, r));

        // --- Table: the sharp outline, read a little inside the face's end. ---
        // At the end itself the outline is a *point*, so a table read there
        // would run to nothing and wedge the section to a fin. The hold is
        // approached smoothly: a hard `min` freezes the read in one step, and
        // that slope step swept a crease down the flank at the take-off
        // locus. One-sided, so the approach stays monotone and the hold is
        // exact past `take`.
        let take = (face_edge.tan() * (1.0 - HEAD_TAKEOFF)).atan();
        let tw = take * 0.15;
        let d_take = d + (take - d) * crate::field::smootherstep(take - tw, take, d);
        let xf = (plane_r * d_take.tan() / half_l).clamp(0.0, 1.0);
        let face = face_at(end * xf);

        // --- Crest: on the table plane over the face, then the shoulder. ---
        // A parabolic cap rides on the plane solve: full at the face's centre,
        // gone at its edge, so the edge break and shoulder are untouched.
        let xr = (plane_r * d.min(face_edge).tan() / half_l).clamp(0.0, 1.0);
        let cap = self.head.table_dome_mm.clamp(0.0, 3.0) * (1.0 - xr * xr);
        let span = self.head.shoulder_deg.clamp(1.0, 150.0).to_radians();
        let s_raw = (d - face_edge) / span;
        let s = s_raw.clamp(0.0, 1.0);
        let h00 = (1.0 - s).powf(HEAD_SHOULDER_POW);
        // The crest's *span* follows its height curve: the reference's head
        // falls off its plate corner as one funnel, and a span held wider
        // than the falling crest reads as a shelf beside the plate. The
        // crease this hold once patched is gone at the source — the wall is
        // one C² curve and the closure hands over in station space.
        let h_span = h00;

        // The table cannot reach past the body it is cut into. `body_extent`
        // already contains the face, but the fillet against the swell rounds a
        // crossing *down* by up to its own radius, so the last word belongs
        // here — a crest outside the bore is an undercut by construction.
        let crest = blend_span(draft_span(reach), draft_span(face), h_span);
        let crest = (crest.0.max(reach.0), crest.1.min(reach.1));

        // The crest line's corner at the plate's theta-end carries the same
        // rim rounding the section gives the outline: the plane solve climbs
        // at +0.19 mm/deg into the shoulder's -0.21 mm/deg dive, and the
        // unrounded peak was an 80 degree fold between the two slices that
        // straddled it. The reference's plate edge is rounded all the way
        // around, ends included. Tie-exact, so away from the corner both
        // curves are followed exactly.
        let rim = self.head.rim_round_mm.clamp(0.0, 2.0);
        let climb = plane_r
            / d.min(HEAD_MAX_HALF_DEG.to_radians()).cos().max(1e-6)
            + cap;
        let dive_h = (1.0 - s_raw.max(-0.25)).max(0.0).powf(HEAD_SHOULDER_POW);
        let dive = r_shank + (plane_r / face_edge.cos().max(1e-6) - r_shank) * dive_h;
        let outer_r = smin(climb, dive, rim);

        HeadAt { x: end * x, reach, face: crest, on_head: h00, outer_r }
    }
}

/// The span a head's table reaches, given what its body reaches at the bore:
/// the same shape, drawn in by [`HEAD_FACE_DRAFT`] about its own centre.
///
/// Proportional, so it cannot wedge. Insetting by a distance rather than a
/// share drafts a narrow station to nothing and leaves a fin standing off the
/// end of the head, which is what a heart does first.
fn draft_span(body: (f64, f64)) -> (f64, f64) {
    let mid = 0.5 * (body.0 + body.1);
    let k = 1.0 - HEAD_FACE_DRAFT;
    (mid + (body.0 - mid) * k, mid + (body.1 - mid) * k)
}

/// Blend one span toward another. Spans, not widths: an upright face is
/// off-centre, and averaging its width would put its crest in the wrong place.
fn blend_span(from: (f64, f64), to: (f64, f64), t: f64) -> (f64, f64) {
    (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
}

/// Per-angle modulation of the cross-section.
#[derive(Clone, Copy, Debug)]
pub struct ShankMod {
    pub width_scale: f64,
    pub thickness_scale: f64,
    /// Scale on the crown, so a section can round off independently of the
    /// style. The crown is clamped to the section, so a large value simply
    /// means "fully domed here".
    pub crown_scale: f64,
    /// Crest radius for this section, mm, replacing `thickness_scale` outright.
    /// The signet head needs it: its table is a plane, so the section's depth
    /// is set by where that plane sits, not by a fraction of the band's own.
    pub outer_r: Option<f64>,
    /// Where this section sits along the finger axis, as a fraction of the
    /// unmodulated half-width. A swept band is centred on its own mid-plane;
    /// an upright signet face is not, because it reaches further to its flat
    /// top than to its point.
    pub z_center_frac: f64,
    /// The span the section reaches at its **crest**, as `(low, high)`
    /// fractions of the unmodulated half-width — before the profile's own side
    /// draft, which is applied on top.
    ///
    /// `None` hands it the bore's span, which is the ordinary band: one shape,
    /// drafted by one angle. A signet hands it the face, so the flat facet and
    /// the body it is cut into can be different shapes, and the draft between
    /// them varies per angle. It is a span and not a width because an upright
    /// face is off-centre by a different amount at its crest than at its bore.
    pub crest_span: Option<(f64, f64)>,
    /// Hard radial cap, used by the Euro flat chord.
    pub outer_max_r: Option<f64>,
    /// How far toward [`BandProfile::morph`]'s target this section sits, 0..1.
    /// Filled by [`crate::RingDesign::modulation_at`], not by the shank kind,
    /// so morphing composes with every kind.
    pub drop_blend: f64,
    /// Skews the crown's two flanks against each other, -1..1: positive
    /// steepens the low-`z` flank and eases the high one. Applied as a power
    /// on the normalized distance, so each flank stays a monotone drop and
    /// the castability guarantee survives.
    pub flank_bias: f64,
    /// How much of a head this section is, 0..1. It reshapes the section's
    /// walls from the band's straight drafted chord into the head's convex
    /// wall, and swaps the edge fillet for [`SignetHead::rim_round_mm`]. C²
    /// along the sweep — a weight with a slope step sweeps a crease line down
    /// the whole wall.
    pub head: f64,
    /// Rounding between table and wall when `head` is 1, mm.
    pub head_rim_mm: f64,
    /// How softly the crest span's parting-plane floor engages here, 0..1.
    /// Near a face's along-ring ends the outline's edge plunges through the
    /// floor at millimetres per degree and the crossfade needs a wide radius
    /// to span whole degrees; over the plate's middle the same radius drags
    /// boundaries that sit legitimately close to the floor — it pulled a
    /// heart's cleft half shut — so the width follows the station.
    pub straddle_soft: f64,
}

impl ShankMod {
    pub fn identity() -> Self {
        Self {
            width_scale: 1.0,
            thickness_scale: 1.0,
            crown_scale: 1.0,
            outer_r: None,
            z_center_frac: 0.0,
            crest_span: None,
            outer_max_r: None,
            drop_blend: 0.0,
            flank_bias: 0.0,
            head: 0.0,
            head_rim_mm: 0.0,
            straddle_soft: 0.0,
        }
    }
}

impl ShankStyle {
    /// Modulation at a ring angle. `base_outer_r` is the unmodulated crest
    /// radius, used to position the Euro chord and the signet's table plane.
    pub fn modulation(&self, theta_deg: f64, inner_r: f64, base_outer_r: f64) -> ShankMod {
        let k = self.amount.clamp(0.0, 1.0);
        // 0 at the top of the ring, 1 at the bottom of the shank.
        let d = ((theta_deg - TOP_DEG).to_radians().cos() * -0.5 + 0.5).clamp(0.0, 1.0);
        match self.kind {
            ShankKind::Uniform => ShankMod::identity(),
            ShankKind::Pinched => {
                // Waist concentrated just off the top, on both shoulders.
                let p = (((theta_deg - TOP_DEG).to_radians().cos() * 0.5 + 0.5) as f64).powi(3);
                ShankMod {
                    width_scale: 1.0 - 0.35 * k * p,
                    thickness_scale: 1.0 + 0.15 * k * p,
                    ..ShankMod::identity()
                }
            }
            ShankKind::Bombe => {
                let s = (1.0 - d).powf(1.5);
                ShankMod {
                    width_scale: (1.0 + 0.50 * k * s) * (1.0 - 0.25 * k * d),
                    thickness_scale: (1.0 + 0.35 * k * s) * (1.0 - 0.20 * k * d),
                    crown_scale: 1.0 + 0.6 * k * s,
                    ..ShankMod::identity()
                }
            }
            ShankKind::Saddle => {
                let p = (1.0 - d).powf(2.0);
                ShankMod {
                    width_scale: 1.0 + 0.45 * k * p,
                    thickness_scale: 1.0 - 0.35 * k * p,
                    ..ShankMod::identity()
                }
            }
            ShankKind::Wave => {
                // The section slides along the finger; the crest span is
                // widened to the parting plane by construction, so the crest
                // circle stays level while the edges wave. The swing is capped
                // at 0.6 of the half-width: measured undercut converges to
                // 0.008% there and to 0.05% at -7 degrees by 0.85, where the
                // edge fillet starts leaning over the mould half beneath it.
                let waves = self.waves.clamp(1, 8) as f64;
                let phase = (theta_deg - TOP_DEG).to_radians() * waves;
                ShankMod {
                    z_center_frac: 0.6 * k * phase.sin(),
                    ..ShankMod::identity()
                }
            }
            ShankKind::Twist => {
                // The wave's slide plus a phase-locked flank skew: the steep
                // flank alternates sides as the edge crosses its mid-line, so
                // the light-line spirals. Both flanks stay monotone drops.
                // Capped where the measured undercut converges to phantom
                // scale: 0.011% at half these strengths, 0.089% at -5.6
                // degrees when the slide reaches 0.45 and the bias 0.8.
                let waves = self.waves.clamp(1, 8) as f64;
                let phase = (theta_deg - TOP_DEG).to_radians() * waves;
                ShankMod {
                    z_center_frac: 0.28 * k * phase.sin(),
                    flank_bias: 0.45 * k * phase.cos(),
                    ..ShankMod::identity()
                }
            }
            ShankKind::FlatTop => {
                let c = (theta_deg - TOP_DEG).to_radians().cos();
                let flat_depth = 0.35 * k * base_outer_r.min(12.0) * 0.25;
                let cap = if c > 0.05 {
                    Some((base_outer_r - flat_depth) / c)
                } else {
                    None
                };
                ShankMod { outer_max_r: cap, ..ShankMod::identity() }
            }
            ShankKind::Tapered => ShankMod {
                width_scale: 1.0 - 0.45 * k * d,
                thickness_scale: 1.0 - 0.30 * k * d,
                ..ShankMod::identity()
            },
            ShankKind::ReverseTaper => ShankMod {
                width_scale: 1.0 - 0.45 * k * (1.0 - d),
                thickness_scale: 1.0 - 0.30 * k * (1.0 - d),
                ..ShankMod::identity()
            },
            ShankKind::Cathedral => {
                // Swell concentrated on the shoulders either side of the top.
                let s = (1.0 - d).powf(2.2);
                ShankMod {
                    width_scale: 1.0 + 0.55 * k * s,
                    thickness_scale: 1.0 + 0.25 * k * s,
                    ..ShankMod::identity()
                }
            }
            ShankKind::EuroFlat => {
                let bottom = TOP_DEG + 180.0;
                let c = (theta_deg - bottom).to_radians().cos();
                let flat_depth = 0.35 * k * base_outer_r.min(12.0) * 0.25;
                // Chord perpendicular to the bottom direction.
                let cap = if c > 0.05 {
                    Some((base_outer_r - flat_depth) / c)
                } else {
                    None
                };
                ShankMod { outer_max_r: cap, ..ShankMod::identity() }
            }
            ShankKind::Signet => {
                let a = self.head_at(theta_deg, inner_r, base_outer_r);
                let band = self.signet_span(theta_deg, inner_r, base_outer_r);
                let (w, centre) = ((band.1 - band.0) * 0.5, (band.1 + band.0) * 0.5);
                // The table is the face's own outline, not the body's: the two
                // are different extents, and that difference is the head's
                // drafted flank.
                let crest = a.face;
                // The shank rounds off toward a wire as it narrows, so a flat
                // head sits on a round shank. The crown clamp caps it at a full
                // dome, so a large value only ever means "more domed here".
                let shank_crown = 1.0 + SIGNET_SHANK_ROUNDING * k * (1.0 - w);
                let table_crown = 1.0 - self.head.table_flat.clamp(0.0, 1.0);
                ShankMod {
                    width_scale: w,
                    crest_span: Some(crest),
                    // Unused: `outer_r` sets the section's depth outright, so
                    // the crown stays a fraction of the profile's own.
                    thickness_scale: 1.0,
                    crown_scale: shank_crown + (table_crown - shank_crown) * a.on_head,
                    outer_r: Some(a.outer_r),
                    z_center_frac: centre,
                    outer_max_r: None,
                    drop_blend: 0.0,
                    flank_bias: 0.0,
                    // `on_head` kinks where the shoulder starts; smoothing it
                    // here keeps the wall-shape weight C² along the sweep.
                    head: crate::field::smootherstep(0.0, 1.0, a.on_head),
                    head_rim_mm: self.head.rim_round_mm.clamp(0.0, 2.0),
                    straddle_soft: crate::field::smootherstep(0.55, 0.92, a.x.abs()),
                }
            }
        }
    }
}

// --- Polyline helpers ------------------------------------------------------

/// The superellipse drop law, monotone for any positive exponents.
fn superellipse_drop(x: f64, a: f64, b: f64) -> f64 {
    let a = a.max(0.05);
    let b = b.max(0.05);
    (1.0 - (1.0 - x.clamp(0.0, 1.0).powf(a)).max(0.0).powf(1.0 / b)).clamp(0.0, 1.0)
}

fn polyline_len(p: &[[f64; 2]]) -> f64 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

fn dedup(p: &mut Vec<[f64; 2]>) {
    p.dedup_by(|a, b| dist(*a, *b) < 1e-9);
}

/// Trim the arc length each end fillet takes over off the outer curve.
fn trim_outer(outer: &[[f64; 2]], er_lo: f64, er_hi: f64) -> &[[f64; 2]] {
    if outer.len() < 4 || (er_lo <= 1e-9 && er_hi <= 1e-9) {
        return outer;
    }
    let total = polyline_len(outer);
    let lo = if er_lo > 1e-9 {
        advance_index(outer, er_lo.min(total * 0.4), false)
    } else {
        0
    };
    let hi = if er_hi > 1e-9 {
        advance_index(outer, er_hi.min(total * 0.4), true)
    } else {
        outer.len() - 1
    };
    if lo >= hi { outer } else { &outer[lo..=hi] }
}

/// Index reached by walking `d` of arc length in from one end.
fn advance_index(p: &[[f64; 2]], d: f64, from_end: bool) -> usize {
    let mut acc = 0.0;
    if from_end {
        for i in (1..p.len()).rev() {
            acc += dist(p[i], p[i - 1]);
            if acc >= d {
                return i - 1;
            }
        }
        0
    } else {
        for i in 1..p.len() {
            acc += dist(p[i - 1], p[i]);
            if acc >= d {
                return i;
            }
        }
        p.len() - 1
    }
}

/// The flange's axial band and rim radius, after clamping.
struct FlangeBand {
    z_lo: f64,
    z_hi: f64,
    rim_r: f64,
}

/// Points per quadratic Bezier corner.
const ARC_STEPS: usize = 8;

/// Vertices along each curved side wall.
const FLANK_STEPS: usize = 18;

/// Append a quadratic Bezier from `p0` to `p2` with `corner` as the control
/// point, excluding `p0` itself.
fn push_arc(out: &mut Vec<[f64; 2]>, p0: [f64; 2], corner: [f64; 2], p2: [f64; 2]) {
    for i in 1..=ARC_STEPS {
        let t = i as f64 / ARC_STEPS as f64;
        let m = 1.0 - t;
        out.push([
            m * m * p0[0] + 2.0 * m * t * corner[0] + t * t * p2[0],
            m * m * p0[1] + 2.0 * m * t * corner[1] + t * t * p2[1],
        ]);
    }
}

/// Append a quadratic Bezier blending the side face into the outer surface.
/// The control point sits at the sharp corner, so the join is tangent
/// continuous at both ends and stays inside the corner's convex hull.
/// Returns the arc's two tangency points, or the sharp corner when there is
/// no arc — the slope discontinuities a sample row should land on.
fn push_fillet(
    out: &mut Vec<[f64; 2]>,
    side_end: [f64; 2],
    corner: [f64; 2],
    outer: &[[f64; 2]],
    top: bool,
    er: f64,
) -> ([f64; 2], [f64; 2]) {
    if er <= 1e-9 || outer.len() < 4 {
        if !top {
            out.push(corner);
        }
        return (corner, corner);
    }
    let total = polyline_len(outer);
    let er_o = er.min(total * 0.4);
    let idx = advance_index(outer, er_o, top);
    let on_outer = outer[idx];

    // Back off along the side face by the same amount.
    let side_len = dist(side_end, corner);
    let t = (er.min(side_len * 0.9) / side_len.max(1e-9)).clamp(0.0, 1.0);
    let on_side = [
        corner[0] + (side_end[0] - corner[0]) * t,
        corner[1] + (side_end[1] - corner[1]) * t,
    ];

    let steps = 10;
    let (p0, p2) = if top { (on_outer, on_side) } else { (on_side, on_outer) };
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let m = 1.0 - t;
        out.push([
            m * m * p0[0] + 2.0 * m * t * corner[0] + t * t * p2[0],
            m * m * p0[1] + 2.0 * m * t * corner[1] + t * t * p2[1],
        ]);
    }
    (p0, p2)
}

/// Normalized arc position along `p` of the vertex nearest each feature
/// point. Features further than 0.5 mm from the polyline are not on this
/// span and are dropped.
fn project_fractions(p: &[[f64; 2]], feats: &[[f64; 2]]) -> Vec<f64> {
    if p.len() < 2 || feats.is_empty() {
        return Vec::new();
    }
    let mut cum = Vec::with_capacity(p.len());
    let mut acc = 0.0;
    cum.push(0.0);
    for w in p.windows(2) {
        acc += dist(w[0], w[1]);
        cum.push(acc);
    }
    let total = acc.max(1e-12);

    let mut out: Vec<f64> = feats
        .iter()
        .filter_map(|f| {
            let (mut best, mut best_d) = (0usize, f64::MAX);
            for (i, q) in p.iter().enumerate() {
                let d = dist(*q, *f);
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            (best_d < 0.5).then_some(cum[best] / total)
        })
        .collect();
    out.sort_by(|a, b| a.total_cmp(b));
    out.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    out
}

/// Even arc positions with the nearest one moved onto each feature fraction.
/// A feature moves its sample by at most half a step, so the sequence stays
/// strictly monotone and the span's endpoints stay put.
fn snap_positions(count: usize, feats: &[f64]) -> Vec<f64> {
    let mut pos: Vec<f64> = (0..count).map(|i| i as f64 / count as f64).collect();
    if count < 4 {
        return pos;
    }
    for &f in feats {
        let i = (f * count as f64).round() as usize;
        if i == 0 || i + 1 >= count {
            continue;
        }
        pos[i] = f;
    }
    for i in 1..count {
        if pos[i] <= pos[i - 1] {
            pos[i] = pos[i - 1] + 1e-9;
        }
    }
    pos
}

/// Resample a polyline at normalized arc-length positions, which must be
/// monotone. The end point is never emitted: spans are joined head-to-tail
/// around the closed loop.
fn resample_at(p: &[[f64; 2]], targets: &[f64]) -> Vec<[f64; 2]> {
    if p.len() < 2 || targets.is_empty() {
        return p.iter().take(targets.len()).copied().collect();
    }
    let mut cum = Vec::with_capacity(p.len());
    let mut acc = 0.0;
    cum.push(0.0);
    for w in p.windows(2) {
        acc += dist(w[0], w[1]);
        cum.push(acc);
    }
    let total = acc.max(1e-12);

    let mut out = Vec::with_capacity(targets.len());
    let mut seg = 0usize;
    for &at in targets {
        let target = total * at.clamp(0.0, 1.0);
        while seg + 2 < p.len() && cum[seg + 1] < target {
            seg += 1;
        }
        let span = (cum[seg + 1] - cum[seg]).max(1e-12);
        let t = ((target - cum[seg]) / span).clamp(0.0, 1.0);
        out.push([
            p[seg][0] + (p[seg + 1][0] - p[seg][0]) * t,
            p[seg][1] + (p[seg + 1][1] - p[seg][1]) * t,
        ]);
    }
    out
}

/// Compute normals, `v` coordinates, and displacement weights for a finished
/// closed loop.
fn finish_loop(mut pts: Vec<ProfileSample>, feature_v: Vec<f64>) -> ProfileLoop {
    let n = pts.len();
    if n < 3 {
        return ProfileLoop::default();
    }

    // Central-difference tangents around the closed loop; outward normal of a
    // CCW loop with tangent (dr, dz) is (dz, -dr).
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let dr = next.r - prev.r;
        let dz = next.z - prev.z;
        let len = (dr * dr + dz * dz).sqrt().max(1e-12);
        pts[i].nr = dz / len;
        pts[i].nz = -dr / len;
    }

    let surface_start = pts.iter().position(|p| p.surface).unwrap_or(0);

    // Walk the surface span accumulating arc length.
    let mut v = 0.0;
    let mut crest_v = 0.0;
    let mut crest_r = f64::MIN;
    let mut count = 0usize;
    for k in 0..n {
        let i = (surface_start + k) % n;
        if !pts[i].surface {
            break;
        }
        if k > 0 {
            let prev = pts[(surface_start + k - 1) % n];
            v += ((pts[i].r - prev.r).powi(2) + (pts[i].z - prev.z).powi(2)).sqrt();
        }
        pts[i].v_mm = v;
        if pts[i].r > crest_r {
            crest_r = pts[i].r;
            crest_v = v;
        }
        count += 1;
    }
    let surface_len = v.max(1e-9);

    // Fade displacement to zero approaching the bore corners.
    for k in 0..count {
        let i = (surface_start + k) % n;
        let d = pts[i].v_mm.min(surface_len - pts[i].v_mm);
        pts[i].weight = (d / EDGE_FADE_MM).clamp(0.0, 1.0);
    }
    for p in pts.iter_mut().filter(|p| !p.surface) {
        p.weight = 0.0;
    }

    ProfileLoop {
        pts,
        surface_len_mm: surface_len,
        crest_v_mm: crest_v,
        crest_radius_mm: crest_r,
        surface_start,
        feature_v,
    }
}

#[cfg(test)]
mod tests {

    // --- Twist --------------------------------------------------------------

    /// The twist reads as a spiral because the steep flank alternates sides;
    /// it must actually alternate, and the band must still release.
    #[test]
    fn a_twist_band_alternates_its_steep_flank_and_releases() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(super::ProfileStyle::LowDome);
        d.shank = super::ShankStyle {
            kind: super::ShankKind::Twist,
            amount: 1.0,
            waves: 3,
            ..Default::default()
        };
        let inner_r = d.inner_radius_mm();
        let crest_r = d.reference_loop().crest_radius_mm;

        // Radial drop of each flank a fixed way out from the crest; the
        // steeper flank has dropped further.
        let flank_drop = |theta: f64| {
            let m = d.modulation_at(theta, inner_r, crest_r);
            let l = d.profile.sample_spaced(inner_r, 128, &m, None, None);
            let crest = l.crest_radius_mm;
            let crest_z = l
                .pts
                .iter()
                .filter(|p| p.surface)
                .max_by(|a, b| a.r.total_cmp(&b.r))
                .map(|p| p.z)
                .unwrap_or(0.0);
            let probe = |side: f64| {
                let z = crest_z + side * 1.4;
                l.pts
                    .iter()
                    .filter(|p| p.surface && (p.z - z).abs() < 0.25)
                    .map(|p| p.r)
                    .fold(0.0f64, f64::max)
            };
            (crest - probe(-1.0), crest - probe(1.0))
        };
        // Peak positive bias a quarter-wave apart from peak negative.
        let (lo_a, hi_a) = flank_drop(TOP_DEG);
        let (lo_b, hi_b) = flank_drop(TOP_DEG + 60.0);
        assert!(
            (lo_a - hi_a) * (lo_b - hi_b) < 0.0,
            "the steep flank should swap sides: {lo_a:.3}/{hi_a:.3} vs {lo_b:.3}/{hi_b:.3}"
        );

        let out = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams { theta_steps: 384, profile_steps: 128, ..Default::default() },
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, inner_r);
        assert!(
            cast.undercut_fraction() < 0.002 && cast.worst_draft_deg > -5.0,
            "twist locks: {:.4}% at {:.1}",
            cast.undercut_fraction() * 100.0,
            cast.worst_draft_deg
        );
    }

    // --- Cab dome -----------------------------------------------------------

    /// A cabochon table stands proud at its centre, stays inside the plane at
    /// the face's edge, and still releases.
    #[test]
    fn a_cab_dome_raises_the_table_centre_and_releases() {
        let lib = crate::AlphaLibrary::builtin();
        let mut flat = crate::RingDesign::default();
        flat.profile.width_mm = 8.0;
        flat.shank.apply_signet(8.0);
        let mut cab = flat.clone();
        cab.shank.head.table_dome_mm = 1.0;

        let r_at_top = |d: &crate::RingDesign| {
            let inner_r = d.inner_radius_mm();
            let crest_r = d.reference_loop().crest_radius_mm;
            d.shank.head_at(TOP_DEG, inner_r, crest_r).outer_r
        };
        // head_at on ShankStyle:
        let flat_r = flat.shank.head_at(TOP_DEG, flat.inner_radius_mm(), flat.reference_loop().crest_radius_mm).outer_r;
        let cab_r = r_at_top(&cab);
        assert!(
            (cab_r - flat_r - 1.0).abs() < 0.05,
            "the dome should stand ~1 mm proud at centre: {flat_r:.3} -> {cab_r:.3}"
        );

        let out = crate::mesh::build(
            &cab,
            &lib,
            crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() },
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let cast = crate::castability::analyze(&out.mesh, &cab.draft, cab.inner_radius_mm());
        assert!(
            cast.undercut_fraction() < 0.001,
            "cab head locks: {:.4}% at {:.1}",
            cast.undercut_fraction() * 100.0,
            cast.worst_draft_deg
        );
    }

    // --- Profile morph ------------------------------------------------------

    /// D-shape at the palm easing to a flat crown at the top: the section
    /// really changes, the mesh still closes, and the blend of two monotone
    /// drops keeps every angle castable.
    #[test]
    fn a_profile_morph_changes_the_top_and_stays_castable() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(super::ProfileStyle::DShape);
        d.profile.morph =
            Some(super::ProfileMorph::from_style(super::ProfileStyle::Flat, &d.profile));

        let inner_r = d.inner_radius_mm();
        let reference = d.reference_loop();
        let crest_r = reference.crest_radius_mm;

        // How far the dome has dropped 80% of the way to the band edge. At a
        // given z the section has a dome point and a side-face point; the dome
        // is the outer of the two.
        let hw = d.profile.width_mm * 0.5;
        let section_crown = |theta: f64| {
            let m = d.modulation_at(theta, inner_r, crest_r);
            let l = d.profile.sample_spaced(inner_r, 96, &m, None, None);
            let crest = l.crest_radius_mm;
            let dome = l
                .pts
                .iter()
                .filter(|p| p.surface && (p.z.abs() - 0.8 * hw).abs() < 0.15)
                .map(|p| p.r)
                .fold(0.0f64, f64::max);
            crest - dome
        };
        let top = section_crown(TOP_DEG);
        let bottom = section_crown(TOP_DEG + 180.0);
        assert!(
            bottom - top > 0.2,
            "the top should flatten: crown drop {top:.3} vs palm {bottom:.3}"
        );

        let out = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() },
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, inner_r);
        assert!(
            cast.undercut_fraction() < 0.001,
            "morphed band locks: {:.4}% at {:.1} deg",
            cast.undercut_fraction() * 100.0,
            cast.worst_draft_deg
        );
    }

    // --- Wave shank ---------------------------------------------------------

    /// The whole trick of the wave band: the bore span slides along the finger
    /// while the crest span is widened to contain the parting plane, so the
    /// crest circle stays level and every flank keeps its draft.
    #[test]
    fn a_wave_band_keeps_its_crest_level_and_releases() {
        let lib = crate::AlphaLibrary::builtin();
        for waves in [1u32, 2, 3] {
            let mut d = crate::RingDesign::default();
            d.profile.apply_style(super::ProfileStyle::LowDome);
            d.shank = super::ShankStyle {
                kind: super::ShankKind::Wave,
                amount: 1.0,
                waves,
                ..Default::default()
            };
            let out = crate::mesh::build(
                &d,
                &lib,
                crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() },
            );
            assert!(out.report.validation.watertight, "{waves} waves: {:?}", out.report.validation);

            // The crest of every slice sits on the parting plane.
            let inner_r = d.inner_radius_mm();
            let reference = d.reference_loop();
            for i in 0..32 {
                let theta = i as f64 / 32.0 * 360.0;
                let m = d.shank.modulation(theta, inner_r, reference.crest_radius_mm);
                let l = d.profile.sample_spaced(inner_r, 96, &m, None, None);
                let crest_z = l
                    .pts
                    .iter()
                    .filter(|p| p.surface)
                    .max_by(|a, b| a.r.total_cmp(&b.r))
                    .map(|p| p.z)
                    .unwrap_or(f64::NAN);
                assert!(
                    crest_z.abs() < 0.12,
                    "{waves} waves: crest rode to z {crest_z:.3} at theta {theta:.0}"
                );
            }

            let cast = crate::castability::analyze(&out.mesh, &d.draft, inner_r);
            // What is left at this sweep is the crest-line phantom: measured
            // 0.040% at 384x144 falling to 0.006% at 768x256.
            assert!(
                cast.undercut_fraction() < 0.002 && cast.worst_draft_deg > -4.0,
                "{waves} waves lock in the sand: {:.4}% at {:.1} deg",
                cast.undercut_fraction() * 100.0,
                cast.worst_draft_deg
            );
        }
    }

    // --- Feature lines -----------------------------------------------------

    /// Before feature snapping, the crest sample sat wherever the even grid
    /// left it, and the reported crest radius was the chord's, not the
    /// surface's.
    #[test]
    fn a_sample_row_lands_exactly_on_the_crest() {
        for &style in super::ProfileStyle::ALL {
            if style == super::ProfileStyle::Custom {
                continue;
            }
            let mut p = super::BandProfile::default();
            p.apply_style(style);
            let inner_r = 8.57;
            // A coarse loop, where a missed crest costs the most.
            let l = p.sample(inner_r, 48);
            let expect = inner_r + p.thickness_mm;
            assert!(
                (l.crest_radius_mm - expect).abs() < 1e-6,
                "{style:?}: crest sampled at {:.6}, surface peaks at {expect:.6}",
                l.crest_radius_mm
            );
            assert!(!l.feature_v.is_empty(), "{style:?} records its crest");
            for &f in &l.feature_v {
                assert!((0.0..=1.0).contains(&f), "{style:?}: feature at {f}");
            }
        }
    }

    /// The flange rim is two sharp corners; a chord across either rounds the
    /// rim the whole point of a flange is to keep square.
    #[test]
    fn flange_rim_corners_each_get_a_sample() {
        let mut p = super::BandProfile::default();
        p.flange.enabled = true;
        p.flange.v_pos = 0.5;
        p.flange.extent_mm = 0.8;
        p.flange.thickness_mm = 0.6;
        p.flange.edge_round_mm = 0.0;
        let l = p.sample(8.57, 64);
        let rim_r = l.crest_radius_mm;
        let on_rim: Vec<f64> = l
            .pts
            .iter()
            .filter(|q| q.surface && (q.r - rim_r).abs() < 1e-6)
            .map(|q| q.z)
            .collect();
        let z_lo = on_rim.iter().cloned().fold(f64::MAX, f64::min);
        let z_hi = on_rim.iter().cloned().fold(f64::MIN, f64::max);
        assert!(
            z_hi - z_lo > 0.5,
            "both rim corners sampled at full radius: rim z {z_lo:.4}..{z_hi:.4}"
        );
    }

    // --- Drop curve --------------------------------------------------------

    /// The whole point: a curve the editor accepts cannot fall back, so the
    /// crown it shapes cannot lean under itself however it is drawn.
    #[test]
    fn a_monotone_drop_curve_never_reverses() {
        let mut c = DropCurve::default();
        // Deliberately out of order, out of range, and falling.
        for (x, d) in [(0.5, 0.9), (0.2, 0.3), (0.8, 0.1), (1.4, 2.0), (-0.3, -1.0), (0.35, 0.6)] {
            c.insert(x, d);
        }
        let pts = c.points().to_vec();
        for w in pts.windows(2) {
            assert!(w[1][0] >= w[0][0], "points came back unsorted: {pts:?}");
            assert!(w[1][1] >= w[0][1], "drop falls back at {:?}", w[1]);
        }
        assert!(
            c.worst_reversal() < 1e-9,
            "interpolation reversed by {:.6} between control points",
            c.worst_reversal()
        );
    }

    /// A plain cubic would overshoot through a step and dip on the way out,
    /// which is an undercut the control points never asked for.
    #[test]
    fn interpolation_does_not_overshoot_a_step() {
        let mut c = DropCurve::default();
        for (x, d) in [(0.0, 0.0), (0.45, 0.02), (0.5, 0.95), (1.0, 1.0)] {
            c.insert(x, d);
        }
        let mut prev = c.eval(0.0);
        for i in 1..=512 {
            let y = c.eval(i as f64 / 512.0);
            assert!(y >= prev - 1e-9, "dipped from {prev:.6} to {y:.6}");
            assert!((0.0..=1.0).contains(&y), "left the unit range: {y}");
            prev = y;
        }
    }

    #[test]
    fn an_unlocked_curve_can_be_drawn_backwards() {
        let mut c = DropCurve { monotone: false, ..DropCurve::default() };
        for (x, d) in [(0.0, 0.0), (0.5, 0.8), (1.0, 0.2)] {
            c.insert(x, d);
        }
        assert!(
            c.worst_reversal() > 0.4,
            "unlocking should allow a real reversal, got {:.3}",
            c.worst_reversal()
        );
    }

    #[test]
    fn a_curve_adopted_from_a_superellipse_matches_it() {
        for (a, b) in [(2.0, 2.0), (8.0, 1.0), (2.5, 1.6)] {
            let mut p = BandProfile::default();
            p.shape_a = a;
            p.shape_b = b;
            let plain: Vec<f64> = (0..=20).map(|i| p.drop(i as f64 / 20.0)).collect();
            p.adopt_drop_curve(MAX_DROP_POINTS);
            for (k, want) in plain.iter().enumerate() {
                let got = p.drop(k as f64 / 20.0);
                assert!(
                    (got - want).abs() < 0.03,
                    "a={a} b={b} at x={:.2}: {got:.4} vs {want:.4}",
                    k as f64 / 20.0
                );
            }
        }
    }

    #[test]
    fn the_curve_cannot_outgrow_its_cap() {
        let mut c = DropCurve::default();
        for i in 0..200 {
            c.insert(i as f64 / 200.0, i as f64 / 200.0);
        }
        assert!(c.len() <= MAX_DROP_POINTS, "{} points", c.len());
        assert!(c.points().windows(2).all(|w| w[1][0] > w[0][0]), "duplicate x survived");
    }

    #[test]
    fn the_ends_of_the_curve_stay_pinned() {
        let mut c = DropCurve::from_superellipse(2.0, 2.0, 6);
        let n = c.len();
        c.set(0, 0.7, 0.5);
        c.set(n - 1, 0.2, 0.5);
        assert_eq!(c.points()[0][0], 0.0, "the crest end moved");
        assert_eq!(c.points()[c.len() - 1][0], 1.0, "the edge end moved");
        c.remove(0);
        c.remove(c.len() - 1);
        assert_eq!(c.len(), n, "an end point was removed");
    }

    /// A drawn crown goes through the same sweep as any other, so it has to
    /// come out watertight and undercut-free like the presets do.
    #[test]
    fn a_drawn_crown_builds_watertight_and_releases() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.build = crate::BuildParams {
            theta_steps: 192,
            profile_steps: 128,
            min_wall_mm: crate::mesh::MIN_WALL_MM,
            adaptive: false,
            refine: None,
            soften_mm: 0.0,
        };
        d.profile.adopt_drop_curve(6);
        // A deliberately lumpy crown: a shelf, then a fast fall.
        d.profile.drop_curve.set(1, 0.18, 0.04);
        d.profile.drop_curve.set(2, 0.30, 0.08);
        d.profile.drop_curve.set(3, 0.55, 0.72);
        d.profile.drop_curve.set(4, 0.78, 0.86);

        let out = crate::mesh::build(&d, &lib, d.build);
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let rep = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        println!(
            "drawn crown: {} undercut faces, {:.4}% of area, worst {:.2} deg",
            rep.undercut,
            rep.undercut_fraction() * 100.0,
            rep.worst_draft_deg
        );
        assert_eq!(rep.undercut, 0, "a monotone drawn crown must not undercut");
    }
    use super::*;

    fn loop_for(style: ProfileStyle) -> ProfileLoop {
        let mut p = BandProfile::default();
        p.apply_style(style);
        p.sample(8.65, 192)
    }

    #[test]
    fn drop_is_monotonic_for_every_style() {
        for &style in ProfileStyle::ALL {
            let mut p = BandProfile::default();
            p.apply_style(style);
            let mut prev = -1.0;
            for i in 0..=200 {
                let d = p.drop(i as f64 / 200.0);
                assert!(d >= prev - 1e-9, "{:?} drop not monotonic at {i}", style);
                prev = d;
            }
        }
    }

    #[test]
    fn loop_has_requested_vertex_count() {
        for &style in ProfileStyle::ALL {
            assert_eq!(loop_for(style).len(), 192, "{:?}", style);
        }
    }

    #[test]
    fn surface_span_is_contiguous_and_bore_is_excluded() {
        for &style in ProfileStyle::ALL {
            let l = loop_for(style);
            let n = l.len();
            let flags: Vec<bool> = (0..n).map(|k| l.pts[(l.surface_start + k) % n].surface).collect();
            let first_false = flags.iter().position(|f| !f).unwrap();
            assert!(
                flags[first_false..].iter().all(|f| !f),
                "{:?} surface span is not contiguous",
                style
            );
            assert!(l.surface_len_mm > 1.0, "{:?}", style);
        }
    }

    #[test]
    fn normals_point_away_from_the_ring_axis_on_the_surface() {
        for &style in ProfileStyle::ALL {
            let l = loop_for(style);
            let crest = l
                .pts
                .iter()
                .filter(|p| p.surface)
                .max_by(|a, b| a.r.total_cmp(&b.r))
                .unwrap();
            assert!(crest.nr > 0.5, "{:?} crest normal points inward: {:?}", style, crest);
            let bore = l.pts.iter().find(|p| !p.surface).unwrap();
            assert!(bore.nr < 0.0, "{:?} bore normal points outward", style);
        }
    }

    #[test]
    fn edges_keep_minimum_castable_thickness() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::KnifeEdge);
        assert!(p.edge_thickness_mm() >= MIN_EDGE_MM - 1e-9);
    }

    /// A size-7 band: bore radius and unmodulated crest radius.
    const BORE_R: f64 = 8.65;
    const CREST_R: f64 = 10.65;

    fn signet_shank() -> ShankStyle {
        ShankStyle { kind: ShankKind::Signet, amount: 0.85, ..Default::default() }
    }

    #[test]
    fn a_signet_shank_is_widest_at_the_top_and_narrowest_at_the_bottom() {
        let sh = signet_shank();
        let w = |t: f64| sh.signet_width_frac(t, BORE_R, CREST_R);
        let top = w(TOP_DEG);
        let bottom = w(TOP_DEG + 180.0);
        assert!((top - 1.0).abs() < 1e-12, "the head is not full width: {top}");
        assert!(bottom < 0.30, "the shank is not narrow enough: {bottom}");

        // Monotone from head to shank, and symmetric either side.
        let mut last = top;
        for step in 1..=36 {
            let d = step as f64 * 5.0;
            let at = w(TOP_DEG + d);
            assert!(at <= last + 1e-12, "width grew again at {d} deg: {at} after {last}");
            let mirror = w(TOP_DEG - d);
            assert!((at - mirror).abs() < 1e-12, "lopsided at {d} deg: {at} vs {mirror}");
            last = at;
        }
    }

    /// A fuller outline holds the **table** wider further round than a pointed
    /// one. The table, and not the band: the body under it is faired, and a
    /// pointed outline has more to fair out.
    ///
    /// Both of those are right. A real marquise signet is a pointed plate on a
    /// rounded body — the shape lives in the table, which is the outline as
    /// drawn, while the band beneath carries the swell and whatever the fairing
    /// leaves of the outline.
    #[test]
    fn a_fuller_outline_holds_the_width_further_round() {
        let head = |o: SignetOutline| ShankStyle {
            head: SignetHead { outline: o, ..SignetHead::default() },
            ..signet_shank()
        };
        let styles = [
            SignetOutline::Marquise,
            SignetOutline::Oval,
            SignetOutline::Cushion,
            SignetOutline::Rectangle,
        ];
        let at = TOP_DEG + 20.0;
        let table = |o: SignetOutline| {
            let a = head(o).head_at(at, BORE_R, CREST_R);
            a.face.1 - a.face.0
        };
        let got: Vec<f64> = styles.iter().map(|&o| table(o)).collect();
        for pair in got.windows(2) {
            assert!(pair[0] <= pair[1] + 1e-12, "fullness inverted at {at} deg: {got:?}");
        }
        assert!(
            got[3] > got[0] + 0.05,
            "a rectangle table is no fuller than a marquise at {at} deg: {got:?}"
        );
        // Every one still reaches full width at the top and shank width behind.
        for &o in &styles {
            let sh = head(o);
            let top = sh.signet_width_frac(TOP_DEG, BORE_R, CREST_R);
            assert!((top - 1.0).abs() < 1e-9, "{o:?} is not full width at the top: {top}");
            assert!(sh.signet_width_frac(TOP_DEG + 180.0, BORE_R, CREST_R) < 0.30, "{o:?}");
        }
    }

    /// A crease is a *step* in slope, not slope itself. The outline is followed
    /// as drawn, so the width really does dive at the end of an oval — what
    /// must not happen is a corner, and a corner is what refusing to shrink
    /// with the step size looks like.
    #[test]
    fn the_head_taper_joins_the_shank_without_a_crease() {
        let sh = signet_shank();
        let w = |t: f64| sh.signet_width_frac(TOP_DEG + t, BORE_R, CREST_R);
        let half = (sh.head.length_mm * 0.5 / (CREST_R + sh.head.rise_mm)).atan().to_degrees();

        // Flat at the top of the head: the face does not come to a peak there.
        let peak = (w(0.5) - w(0.0)) / 0.5;
        assert!(peak.abs() < 1e-3, "the head is peaked, not flat: {peak}");

        // Worst change in slope per sample, over the head, the shoulder and a
        // little of the plain shank beyond it.
        let jump = |step: f64| {
            let slope = |t: f64| (w(t + step) - w(t)) / step;
            let mut worst: f64 = 0.0;
            let mut prev = slope(0.0);
            let mut t = step;
            while t < half + sh.head.shoulder_deg + 8.0 {
                let s = slope(t);
                worst = worst.max((s - prev).abs());
                prev = s;
                t += step;
            }
            worst
        };
        // The worst of it sits just inside the head's tip, where the outline's
        // own curvature is running away — an oval really does turn hard there.
        // A corner would hold its jump as the sample shrinks; a curve, however
        // tight, gives it up.
        let (coarse, fine) = (jump(0.25), jump(0.0625));
        println!("worst slope step: {coarse:.5} per degree at 0.25 deg, {fine:.5} at 0.0625 deg");
        assert!(
            fine < coarse * 0.6,
            "slope steps by {fine:.5} at a quarter of the sample, against {coarse:.5} — it is \
             not shrinking with the step, which is a corner"
        );
    }

    #[test]
    fn a_signet_head_is_flat_topped_and_its_shank_rounds_off() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::HalfRound);
        p.width_mm = 12.0;
        p.thickness_mm = 2.0;
        let sh = signet_shank();
        let head = sh.modulation(TOP_DEG, BORE_R, CREST_R);
        let back = sh.modulation(TOP_DEG + 180.0, BORE_R, CREST_R);
        // The head flattens whatever the profile is: a signet's table does not
        // inherit the shank's dome.
        assert!(head.crown_scale.abs() < 1e-12, "the head kept a crown: {}", head.crown_scale);
        assert!(back.crown_scale > 3.0, "the shank did not round off: {}", back.crown_scale);

        // The crown clamp keeps the rounding from eating the section.
        let loop_ = p.sample_mod(BORE_R, 128, &back);
        let depth = loop_.crest_radius_mm - BORE_R;
        assert!(depth <= p.thickness_mm + 1e-6, "section grew: {depth} vs {}", p.thickness_mm);
    }

    /// The head is the band, not a pad on it: the section at the top of the
    /// ring has to be deeper than the shank's by the rise, and wider by the
    /// taper.
    #[test]
    fn the_head_is_the_band_swelling_not_a_pad() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::HalfRound);
        p.width_mm = 12.0;
        p.thickness_mm = 2.0;
        let sh = signet_shank();
        let at = |t: f64| {
            let m = sh.modulation(t, BORE_R, CREST_R);
            let l = p.sample_mod(BORE_R, 256, &m);
            let (lo, hi) = l.z_range();
            (l.crest_radius_mm - BORE_R, hi - lo)
        };
        let (head_t, head_w) = at(TOP_DEG);
        let (shank_t, shank_w) = at(TOP_DEG + 180.0);
        assert!(
            head_t > shank_t + sh.head.rise_mm * 0.9,
            "the head is only {head_t:.3} mm deep against a {shank_t:.3} mm shank"
        );
        assert!(head_w > shank_w * 3.0, "the head is {head_w:.2} mm wide, shank {shank_w:.2} mm");
    }

    /// The table is a plane, not a slice of cylinder. Swept into world space,
    /// every crest point over the face has to land on one flat.
    #[test]
    fn the_signet_table_is_a_true_plane() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::Flat);
        p.width_mm = 12.0;
        p.thickness_mm = 2.6;
        let sh = signet_shank();
        // The table faces +Y at the top of the ring, so a plane is a constant y.
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..=720 {
            let theta = TOP_DEG - 40.0 + 80.0 * i as f64 / 720.0;
            let a = sh.head_at(theta, BORE_R, CREST_R);
            // The middle of the plate: the last stretch of the face carries
            // the rim's own rounding, like the reference's plate edge.
            if a.on_head < 1.0 || a.x.abs() > 0.85 {
                continue;
            }
            let m = sh.modulation(theta, BORE_R, CREST_R);
            let l = p.sample_mod(BORE_R, 256, &m);
            let y = l.crest_radius_mm * theta.to_radians().sin();
            lo = lo.min(y);
            hi = hi.max(y);
        }
        assert!(hi > lo, "no samples landed on the table");
        assert!(hi - lo < 0.02, "the table is {:.4} mm out of flat; a graver needs a true \
             surface", hi - lo);
    }

    /// The crest leaves the table's rim **already falling**, and the band's
    /// width never stops on its way to the shank.
    ///
    /// A rim is an edge, and the flank under it is a fillet running out flat —
    /// so the fall is steep first and eases in, which is the opposite of a
    /// Hermite. Measured off `BlankSignet.obj` against the Hermite this
    /// replaced:
    ///
    /// | past the rim | 3 deg | 13 deg | 23 deg |
    /// | --- | --- | --- | --- |
    /// | reference | 0.85 | 0.39 | 0.14 |
    /// | Hermite | 0.97 | 0.66 | 0.23 |
    ///
    /// Starting flat put a shelf at exactly the place the eye goes: with a
    /// plain ramp off the end of the face the band's edge went from diving at
    /// 0.50 mm per degree to 0.003 in one step, a lip standing 2.9 mm proud.
    #[test]
    fn the_shoulder_leaves_the_rim_already_falling() {
        let hd = SignetHead::default();
        let edge = (hd.length_mm * 0.5 / (CREST_R + hd.rise_mm)).atan().to_degrees();

        let sh = signet_shank();
        let h = |past: f64| sh.head_at(TOP_DEG + edge + past, BORE_R, CREST_R).on_head;
        let tenth = h(hd.shoulder_deg * 0.1);
        println!("{:.3} of the crest left a tenth of the way down the shoulder", tenth);
        assert!(
            tenth < 0.85,
            "the crest is still {tenth:.3} up a tenth of the way past the rim — it leaves flat, \
             which is a shelf"
        );
        assert!(h(hd.shoulder_deg * 0.95) < 0.02, "the crest does not ease into the shank");

        // And past the face the width goes on narrowing without a flat spot.
        // Only past it: a rectangle's outline really is straight-sided over the
        // face, and holding the band at full width there is the shape, not a
        // shelf.
        for &o in SignetOutline::ALL {
            let sh = ShankStyle {
                head: SignetHead { outline: o, ..SignetHead::default() },
                ..signet_shank()
            };
            let at = |d: f64| sh.signet_width_frac(TOP_DEG + d, BORE_R, CREST_R);
            let mut d = edge;
            let arc = hd.swell_arc_deg(edge);
            while d < arc - 4.0 {
                let fall = at(d) - at(d + 1.0);
                assert!(
                    fall > 1e-4,
                    "{o:?} stops narrowing at {d:.1} deg, which is a shelf: {fall:.5} per degree"
                );
                d += 0.5;
            }
        }

        // Past the swell the band is the shank strip alone, exactly — the
        // fillet has to close there or the whole back of the ring gains width
        // it was never asked for.
        let sh = signet_shank();
        let past = hd.swell_arc_deg(edge) + 6.0;
        let a = sh.head_at(TOP_DEG + past, BORE_R, CREST_R);
        let strip = sh.signet_shank_frac(TOP_DEG + past);
        assert_eq!(a.on_head, 0.0, "the crest never lands");
        assert_eq!(a.reach, (-strip, strip), "the swell never lands on the shank");
        assert!(
            (sh.signet_width_frac(TOP_DEG + past, BORE_R, CREST_R) - strip).abs() < 1e-9,
            "the band is not the bare shank past the swell"
        );
    }

    /// Seen from the side, a signet reads as tapering because the **swell** in
    /// front of the shank is long, not because the shank itself tapers.
    ///
    /// Measured on `BlankSignet.obj`: its shank varies by 1% over the 215
    /// degrees behind the head, while the swell takes 75 degrees to come down.
    /// Tapering the strip as well was a wrong turn — this pins it flat.
    #[test]
    fn the_shank_is_flat() {
        let sh = signet_shank();
        let at = |d: f64| sh.signet_width_frac(TOP_DEG + d, BORE_R, CREST_R);
        let shank = at(180.0);
        for d in [90.0, 120.0, 150.0, 180.0, 210.0, 270.0] {
            assert!(
                (at(d) - shank).abs() < 1e-9,
                "the shank is not flat: {:.4} at {d} deg against {shank:.4}",
                at(d)
            );
        }
    }

    /// The silhouette against a real signet, measured off `BlankSignet.obj` —
    /// a 14.7 mm round face on a 20 mm bore, 7 mm shank, 1.75 mm thick, its
    /// table 0.265 mm proud and its body 16.0 mm across.
    ///
    /// Both columns, because they are two different curves and getting one
    /// right is not getting the head right. The **width** comes down over 75
    /// degrees; the **crest** follows the table plane out to the face's edge at
    /// 31.6, peaks at the table's corner, and is on the shank by 75. Those are
    /// [`HEAD_SWELL_DEG`] and [`HEAD_SHOULDER_DEG`], and this is where they
    /// come from.
    ///
    /// It closed a real gap. When the band's silhouette *was* the face outline,
    /// the swell was half gone by 22 degrees and over by 28 against the
    /// reference's 37 and 75 — worst error 0.63 of the drop. It is 0.04 now.
    #[test]
    fn the_swell_matches_a_real_signet() {
        // Degrees off the top, width as a fraction of the head, crest radius mm.
        const REF: [(f64, f64, f64); 19] = [
            (0., 1.000, 11.93), (5., 0.987, 12.01), (10., 0.965, 12.13),
            (15., 0.903, 12.47), (20., 0.846, 12.76), (25., 0.742, 13.36),
            (30., 0.654, 13.88), (35., 0.522, 13.54), (40., 0.441, 13.08),
            (45., 0.309, 12.51), (50., 0.234, 12.26), (55., 0.149, 11.97),
            (60., 0.105, 11.84), (65., 0.061, 11.73), (70., 0.042, 11.68),
            (75., 0.021, 11.65), (80., 0.011, 11.65), (85., 0.003, 11.67),
            (90., 0.002, 11.67),
        ];
        let inner_r = 9.905;
        let crest_r = inner_r + 1.75;
        let sh = ShankStyle {
            kind: ShankKind::Signet,
            amount: (1.0 - 7.0 / 16.0) / (1.0 - SIGNET_MIN_SHANK_FRAC),
            waves: 1,
            head: SignetHead {
                outline: SignetOutline::Round,
                length_mm: 14.7,
                rise_mm: 0.265,
                ..SignetHead::default()
            },
        };
        let raw = |d: f64| sh.signet_width_frac(TOP_DEG + d, inner_r, crest_r);
        let (head, shank) = (raw(0.0), raw(180.0));
        // Both normalized to their own head and shank, so what is compared is
        // the shape of the curve and not two rings' proportions.
        let w = |d: f64| (raw(d) - shank) / (head - shank);
        let r = |d: f64| sh.head_at(TOP_DEG + d, inner_r, crest_r).outer_r;
        let (peak, base) = (13.88, 11.65);
        let h = |d: f64| (r(d) - crest_r) / (peak - crest_r);

        let (mut worst_w, mut worst_h) = (0.0f64, 0.0f64);
        println!(" deg   width          crest");
        for (d, want_w, want_r) in REF {
            let want_h = (want_r - base) / (peak - base);
            println!(
                "{d:4.0}  {want_w:.3} {:.3} {:+.3}   {want_h:.3} {:.3} {:+.3}",
                w(d),
                w(d) - want_w,
                h(d),
                h(d) - want_h
            );
            worst_w = worst_w.max((w(d) - want_w).abs());
            worst_h = worst_h.max((h(d) - want_h).abs());
        }
        println!("worst: width {worst_w:.4}, crest {worst_h:.4}");
        assert!(worst_w < 0.06, "the swell is {worst_w:.4} off the reference");
        assert!(worst_h < 0.12, "the shoulder is {worst_h:.4} off the reference");

        let half_way = (1..=180).map(|i| i as f64).find(|&d| w(d) <= 0.5).unwrap();
        let landed = (1..=180).map(|i| i as f64).find(|&d| w(d) <= 1e-3).unwrap();
        println!("half gone by {half_way} deg (ref 37), on the shank by {landed} deg (ref 75)");
        assert!((half_way - 37.0).abs() < 5.0, "half gone by {half_way} deg, not 37");
        assert!((landed - 75.0).abs() < 6.0, "on the shank by {landed} deg, not 75");

        // Monotone all the way down, with no step.
        let mut last = f64::MAX;
        for i in 0..=180 {
            let at = raw(i as f64);
            assert!(at <= last + 1e-12, "the swell grows again at {i} deg");
            last = at;
        }
    }

    /// A head saved before the swell existed loads with one, rather than with a
    /// zero that would collapse the band to its shank the moment the face ends.
    #[test]
    fn an_older_head_gains_the_swell_it_was_saved_without() {
        let json = r#"{"outline":"Heart","theta_deg":90.0,"length_mm":12.0,
            "rise_mm":0.8,"shoulder_deg":34.0,"table_flat":1.0}"#;
        let head: SignetHead = serde_json::from_str(json).unwrap();
        assert_eq!(head.swell_deg, None);
        assert_eq!(head.body_fair, HEAD_BODY_FAIR);
        assert_eq!(head.outline, SignetOutline::Heart);
    }

    /// The band's silhouette runs from head to shank without a crease.
    ///
    /// A crease is a step in **curvature**, not in slope — a surface can be C¹
    /// and still catch the light in a line. Worst curvature of the bore's reach
    /// per degree squared, with the body extruded from the face against faired
    /// off it:
    ///
    /// | | heart | hexagon | rectangle | oval |
    /// | --- | --- | --- | --- | --- |
    /// | extruded | 2.67 | 2.44 | 0.0071 | 0.0043 |
    /// | faired | 0.0091 | 0.0154 | 0.0070 | 0.0010 |
    ///
    /// The shapes with a concavity or a corner in plan are the ones that had a
    /// crease, and they are the ones the fairing is for. What is left on a heart
    /// is its own curvature, not a join.
    #[test]
    fn the_head_runs_to_the_shank_without_a_crease() {
        // Against the extruded body rather than a remembered number, so the
        // comparison stays honest if the head's proportions ever move.
        let worst_of = |o: SignetOutline, fair: f64| {
            let sh = ShankStyle {
                head: SignetHead { outline: o, body_fair: fair, ..SignetHead::default() },
                ..signet_shank()
            };
            let reach = |t: f64| sh.head_at(TOP_DEG + t, BORE_R, CREST_R).reach;
            const H: f64 = 0.02;
            let (mut worst, mut at) = (0.0f64, 0.0);
            let mut t = H;
            while t < 90.0 {
                let (a, b, c) = (reach(t - H), reach(t), reach(t + H));
                for pick in [|s: (f64, f64)| s.0, |s: (f64, f64)| s.1] {
                    let k = (pick(c) - 2.0 * pick(b) + pick(a)) / (H * H);
                    if k.abs() > worst {
                        (worst, at) = (k.abs(), t);
                    }
                }
                t += H;
            }
            (worst, at)
        };

        println!("worst curvature of the bore's reach, per degree squared:");
        for &o in SignetOutline::ALL {
            let (extruded, _) = worst_of(o, 0.0);
            let (faired, at) = worst_of(o, 1.0);
            println!(
                "  {:<10} extruded {extruded:.4}   faired {faired:.4} at {at:.2} deg",
                o.label()
            );
            // Where the outline has something to fair — a hexagon's plan corner,
            // a heart's dimple — the body has to take most of it out. Below
            // about 0.005 the two are the same head and the difference is the
            // tip rounding, so there is nothing to compare.
            if extruded > 0.02 {
                assert!(
                    faired < extruded * 0.5,
                    "{o:?}: fairing the body left {faired:.4} of an extruded {extruded:.4}"
                );
            }
            // What is left below this is the outline's own plan curvature: an
            // oval reads 0.001 and a hexagon 0.015, because a hexagonal head is
            // meant to show the corner where its flat side meets its slanted
            // one. The body fillets that corner rather than passing it through.
            assert!(
                faired < 0.02,
                "{o:?} kinks at {faired:.4} per degree squared, {at:.2} deg off the top"
            );
        }
    }

    /// The table never reaches past the body it is cut into.
    ///
    /// Which is the head's flank being drafted rather than leaning back over the
    /// mould half it sits in — the same undercut `castability` would find, said
    /// where it can be said for certain instead of sampled off a mesh.
    #[test]
    fn the_table_stays_inside_the_body() {
        for &o in SignetOutline::ALL {
            let sh = ShankStyle {
                head: SignetHead { outline: o, ..SignetHead::default() },
                ..signet_shank()
            };
            for i in 0..=3600 {
                let t = i as f64 * 0.1;
                let a = sh.head_at(TOP_DEG + t, BORE_R, CREST_R);
                assert!(
                    a.face.0 >= a.reach.0 - 1e-12 && a.face.1 <= a.reach.1 + 1e-12,
                    "{o:?} at {t:.1} deg: table {:?} reaches past body {:?}",
                    a.face,
                    a.reach
                );
            }
        }
    }

    /// An upright face reaches further to its top than to its point, so the
    /// band has to move along the finger to carry it. A swept section is
    /// centred on its own mid-plane unless something moves it.
    #[test]
    fn an_upright_face_moves_the_band_off_its_mid_plane() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::Flat);
        p.width_mm = 12.0;
        let sh = ShankStyle {
            head: SignetHead { outline: SignetOutline::Shield, ..SignetHead::default() },
            ..signet_shank()
        };
        let centre = |t: f64| {
            let m = sh.modulation(t, BORE_R, CREST_R);
            let l = p.sample_mod(BORE_R, 256, &m);
            let (lo, hi) = l.z_range();
            (lo + hi) * 0.5
        };
        // Read over the whole head rather than at one angle. The swell is
        // centred and the face is not, so how far off the band sits depends on
        // which of the two is carrying it there.
        let pick = |f: &dyn Fn(f64) -> f64| {
            (0..=200).map(|i| f(TOP_DEG + i as f64 * 0.2)).fold(0.0f64, |a, c| {
                if c.abs() > a.abs() { c } else { a }
            })
        };
        let back = centre(TOP_DEG + 180.0);
        assert!(back.abs() < 1e-9, "the plain shank is off centre by {back:.4} mm");
        let worst = pick(&centre);
        assert!(
            worst.abs() > 0.25,
            "a shield sits centred to within {worst:.4} mm all the way round, so it has no \
             point and no top"
        );

        // The **table** carries more of it than the band does, and has to: the
        // body under it is faired, so it is nearer symmetric than the face it
        // holds. Measured on a shield, 0.29 mm at the bore against 0.47 at the
        // crest.
        let table = |t: f64| {
            let a = sh.head_at(t, BORE_R, CREST_R);
            (a.face.0 + a.face.1) * 0.5 * p.width_mm * 0.5
        };
        let crest = pick(&table);
        assert!(
            crest.abs() > worst.abs() * 1.3,
            "the table is off centre by {crest:.4} mm against the band's {worst:.4} — the body \
             is carrying the face's own shape rather than a faired one"
        );
        assert!(
            (centre(TOP_DEG + 20.0) - centre(TOP_DEG - 20.0)).abs() < 1e-9,
            "the shield is lopsided round the ring"
        );

        // A symmetric face does not move it, and neither does any other style.
        for o in [SignetOutline::Oval, SignetOutline::Cushion, SignetOutline::Hexagon] {
            let sym = ShankStyle {
                head: SignetHead { outline: o, ..SignetHead::default() },
                ..signet_shank()
            };
            let m = sym.modulation(TOP_DEG, BORE_R, CREST_R);
            assert!(m.z_center_frac.abs() < 1e-9, "{o:?} moved the band: {}", m.z_center_frac);
        }
        // Signet and Wave are the two kinds whose whole point is moving the
        // section along the finger; everything else must stay centred.
        for &kind in ShankKind::ALL {
            let s = ShankStyle { kind, amount: 1.0, ..Default::default() };
            let m = s.modulation(TOP_DEG + 40.0, BORE_R, CREST_R);
            assert!(
                matches!(kind, ShankKind::Signet | ShankKind::Wave | ShankKind::Twist)
                    || m.z_center_frac == 0.0,
                "{kind:?} moved the band off centre"
            );
        }
    }

    /// A head has to be able to sit anywhere round the ring, including across
    /// the 0/360 joint, without the silhouette tearing.
    #[test]
    fn a_head_at_the_joint_stays_whole() {
        let sh = ShankStyle {
            head: SignetHead { theta_deg: 0.0, ..SignetHead::default() },
            ..signet_shank()
        };
        let w = |t: f64| sh.signet_width_frac(t, BORE_R, CREST_R);
        assert!((w(0.0) - 1.0).abs() < 1e-9, "the head is not full width at its centre");
        for d in [2.0, 6.0, 12.0, 20.0] {
            let (a, b) = (w(d), w(360.0 - d));
            assert!((a - b).abs() < 1e-9, "torn at the joint {d} deg out: {a} vs {b}");
        }
        assert!(w(180.0) < 0.30, "the shank did not narrow opposite the head");
    }

    #[test]
    fn shank_thickness_scale_keeps_the_crown_proportional() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::HalfRound);
        p.edge_round_mm = 0.0;
        p.side_draft_deg = 0.0;
        p.comfort_fit_mm = 0.0;
        let inner_r = 8.65;
        let m = ShankMod { thickness_scale: 0.5, ..ShankMod::identity() };
        let l = p.sample_mod(inner_r, 256, &m);

        let hwo = p.width_mm * 0.5;
        let probe_z = hwo * 0.95;
        let near = l
            .pts
            .iter()
            .filter(|q| q.surface)
            .min_by(|a, b| (a.z - probe_z).abs().total_cmp(&(b.z - probe_z).abs()))
            .unwrap();

        // thickness 1.0 and crown 0.8 give this drop; a crown scaled twice
        // would leave the band roughly 0.27 mm fatter here.
        let expected = inner_r + 1.0 - 0.8 * p.drop(0.95);
        assert!(
            (near.r - expected).abs() < 0.06,
            "edge radius {} not {expected}: crown was not scaled once",
            near.r
        );
    }

    #[test]
    fn comfort_fit_bore_is_narrowest_at_the_centre() {
        let mut p = BandProfile::default();
        p.comfort_fit_mm = 0.4;
        let l = p.sample(8.65, 192);
        let bore: Vec<&ProfileSample> = l.pts.iter().filter(|p| !p.surface).collect();
        let centre = bore.iter().min_by(|a, b| a.z.abs().total_cmp(&b.z.abs())).unwrap();
        let edge = bore.iter().max_by(|a, b| a.z.abs().total_cmp(&b.z.abs())).unwrap();
        assert!(centre.r < edge.r, "comfort fit bore is inverted");
        assert!((centre.r - 8.65).abs() < 0.02, "size is not measured at the contact band");
    }

    #[test]
    fn sample_is_clamped_at_both_ends_so_a_design_file_cannot_explode_it() {
        let p = BandProfile::default();
        // A design JSON can carry any usize here; mesh::build clamps to the
        // same range, so the swept grid stays aligned with what comes back.
        assert_eq!(p.sample(8.65, 4_000_000).len(), MAX_PROFILE_STEPS);
        assert_eq!(p.sample(8.65, 0).len(), MIN_PROFILE_STEPS);
        assert_eq!(p.sample(8.65, 192).len(), 192);
    }

    // --- Flange ------------------------------------------------------------

    const BORE: f64 = 8.65;

    /// Twice the area enclosed by the loop; positive when it is wound CCW.
    fn shoelace(l: &ProfileLoop) -> f64 {
        let n = l.len();
        (0..n)
            .map(|i| {
                let (a, b) = (l.pts[i], l.pts[(i + 1) % n]);
                a.r * b.z - b.r * a.z
            })
            .sum()
    }

    /// Shortest step between neighbouring vertices, mm.
    fn min_step(l: &ProfileLoop) -> f64 {
        let n = l.len();
        (0..n)
            .map(|i| {
                let (a, b) = (l.pts[i], l.pts[(i + 1) % n]);
                (a.r - b.r).hypot(a.z - b.z)
            })
            .fold(f64::MAX, f64::min)
    }

    /// The axial band a flange occupies on a profile with no side draft.
    fn flange_band(p: &BandProfile) -> (f64, f64) {
        let hw = p.width_mm * 0.5;
        let t = p.flange.thickness_mm;
        let z_lo = (-hw + 2.0 * hw * p.flange.v_pos - 0.5 * t).clamp(-hw, hw - t);
        (z_lo, z_lo + t)
    }

    fn flanged(style: ProfileStyle, v_pos: f64) -> BandProfile {
        let mut p = BandProfile::default();
        p.apply_style(style);
        p.flange = Flange { enabled: true, v_pos, ..Flange::default() };
        p
    }

    #[test]
    fn every_style_takes_a_flange_at_any_position() {
        for &style in ProfileStyle::ALL {
            // crest_bias -0.5 puts the crest at v 0.25.
            for v in [0.0, 0.25, 0.5, 1.0] {
                let mut p = flanged(style, v);
                p.crest_bias = -0.5;
                let l = p.sample(BORE, 192);
                let tag = format!("{style:?} with a flange at v {v}");
                assert_eq!(l.len(), 192, "{tag}");
                assert!(
                    l.pts.iter().all(|q| q.r.is_finite() && q.z.is_finite() && q.nr.is_finite()),
                    "{tag}: loop is not finite"
                );
                assert!(shoelace(&l) > 0.0, "{tag}: loop is wound clockwise");
                assert!(min_step(&l) > 1e-9, "{tag}: loop doubles back on a vertex");

                let n = l.len();
                let flags: Vec<bool> =
                    (0..n).map(|k| l.pts[(l.surface_start + k) % n].surface).collect();
                let first_bore = flags.iter().position(|f| !f).unwrap();
                assert!(flags[first_bore..].iter().all(|f| !f), "{tag}: surface span is split");
            }
        }
    }

    #[test]
    fn the_crest_sits_on_the_flange() {
        for &style in ProfileStyle::ALL {
            for v in [0.0, 0.3, 0.5, 1.0] {
                let mut p = flanged(style, v);
                p.side_draft_deg = 0.0;
                let mut plain = p;
                plain.flange.enabled = false;
                let tag = format!("{style:?} with a flange at v {v}");

                let l = p.sample(BORE, 256);
                assert!(
                    l.crest_radius_mm > plain.sample(BORE, 256).crest_radius_mm + 0.1,
                    "{tag}: the rim is not the widest silhouette"
                );

                let (z_lo, z_hi) = flange_band(&p);
                for q in l.pts.iter().filter(|q| q.r >= l.crest_radius_mm - 1e-9) {
                    assert!(
                        q.z >= z_lo - 1e-6 && q.z <= z_hi + 1e-6,
                        "{tag}: crest at z {} is outside the flange band {z_lo}..{z_hi}",
                        q.z
                    );
                }
                let at_crest_v = l
                    .pts
                    .iter()
                    .find(|q| q.surface && (q.v_mm - l.crest_v_mm).abs() < 1e-9)
                    .unwrap();
                assert!(
                    (at_crest_v.r - l.crest_radius_mm).abs() < 1e-9,
                    "{tag}: crest_v_mm addresses r {} not the crest",
                    at_crest_v.r
                );
            }
        }
    }

    #[test]
    fn an_edge_flange_widens_the_side_face_into_one_flat_run() {
        let mut p = BandProfile::default();
        p.side_draft_deg = 0.0;
        p.comfort_fit_mm = 0.0;
        p.flange = Flange { enabled: true, v_pos: 0.0, ..Flange::default() };
        let l = p.sample(BORE, 256);

        let z_edge = -p.width_mm * 0.5;
        let run: Vec<&ProfileSample> = l
            .pts
            .iter()
            .filter(|q| q.surface && (q.z - z_edge).abs() < 1e-6)
            .collect();
        assert!(run.len() > 8, "the edge face carries only {} vertices", run.len());

        let lo = run.iter().map(|q| q.r).fold(f64::MAX, f64::min);
        let hi = run.iter().map(|q| q.r).fold(f64::MIN, f64::max);
        assert!(
            (lo - BORE).abs() < 1e-6,
            "the flat run starts at r {lo}, clear of the bore corner"
        );
        assert!(
            hi - lo >= p.edge_thickness_mm() + p.flange.extent_mm - 1e-6,
            "the flat run spans only {:.2} mm",
            hi - lo
        );
        let axial = run.iter().filter(|q| q.nz < -0.99).count();
        assert!(axial >= run.len() - 2, "only {axial} of {} vertices face -Z", run.len());
    }

    #[test]
    fn absurd_flange_dimensions_cannot_break_the_loop() {
        let mut p = BandProfile::default();
        for f in [
            Flange { enabled: true, v_pos: 0.5, extent_mm: 1e6, thickness_mm: 1e6, edge_round_mm: 1e6 },
            Flange { enabled: true, v_pos: -9.0, extent_mm: -9.0, thickness_mm: -9.0, edge_round_mm: -9.0 },
            Flange { enabled: true, v_pos: 0.35, extent_mm: 0.0, thickness_mm: 0.0, edge_round_mm: 0.0 },
        ] {
            p.flange = f;
            let l = p.sample(BORE, 192);
            assert_eq!(l.len(), 192, "{f:?}");
            assert!(shoelace(&l) > 0.0, "{f:?}: loop is wound clockwise");
            let (z0, z1) = l.z_range();
            let hw = p.width_mm * 0.5;
            assert!(z0 >= -hw - 1e-9 && z1 <= hw + 1e-9, "{f:?}: flange grew the band to {z0}..{z1}");
            assert!(
                l.crest_radius_mm > BORE && l.crest_radius_mm <= BORE + p.thickness_mm + p.width_mm + 1e-9,
                "{f:?}: rim reached r {}",
                l.crest_radius_mm
            );
        }
    }

    /// Mesh a default band carrying a flange and analyse it for draft.
    fn flanged_report(v_pos: f64) -> (crate::CastReport, bool) {
        let mut d = crate::RingDesign::default();
        d.profile.flange = Flange { enabled: true, v_pos, ..Flange::default() };
        let out = crate::mesh::build(
            &d,
            &crate::AlphaLibrary::builtin(),
            crate::BuildParams { theta_steps: 128, profile_steps: 96, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true, refine: None, soften_mm: 0.0 },
        );
        let report = crate::castability::analyze(
            &out.mesh,
            &crate::DraftSettings::default(),
            d.inner_radius_mm(),
        );
        (report, out.report.validation.watertight)
    }

    /// Deepest re-entrancy of the swept outer surface under a ±Z pull, mm.
    ///
    /// For a body of revolution the cope lifts cleanly only where `r` never
    /// grows with `z` above the parting plane, and symmetrically below it. The
    /// worst such growth, minimized over parting heights, is the undercut depth.
    fn analytic_undercut_depth(p: &BandProfile, inner_r: f64) -> (f64, f64) {
        let l = p.sample(inner_r, 1024);
        let mut outer: Vec<(f64, f64)> =
            l.pts.iter().filter(|s| s.surface).map(|s| (s.z, s.r)).collect();
        outer.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (z_lo, z_hi) = (outer[0].0, outer[outer.len() - 1].0);
        let mut best = f64::MAX;
        let mut best_z = 0.0;
        for i in 0..=2048 {
            let zp = z_lo + (z_hi - z_lo) * i as f64 / 2048.0;
            let mut worst = 0.0f64;
            // Above the plane: r must never climb as z rises.
            let mut lowest = f64::MAX;
            for &(_, r) in outer.iter().filter(|(z, _)| *z >= zp) {
                lowest = lowest.min(r);
                worst = worst.max(r - lowest);
            }
            // Below it: r must never climb as z falls.
            let mut lowest = f64::MAX;
            for &(_, r) in outer.iter().rev().filter(|(z, _)| *z < zp) {
                lowest = lowest.min(r);
                worst = worst.max(r - lowest);
            }
            if worst < best {
                best = worst;
                best_z = zp;
            }
        }
        (best, best_z)
    }

    #[test]
    fn scratch_flange_analytic_depth() {
        let p0 = crate::RingDesign::default();
        let inner_r = p0.inner_radius_mm();
        let (base, base_z) = analytic_undercut_depth(&p0.profile, inner_r);
        println!(
            "control, no flange: analytic depth {base:.6} mm ({:.1} um) at parting z {base_z:+.3}",
            base * 1000.0
        );
        println!("== flange v_pos: analytic undercut depth of the swept section ==");
        for k in 0..=20 {
            let v = k as f64 / 20.0;
            let mut p = p0.profile;
            p.flange = Flange { enabled: true, v_pos: v, ..Flange::default() };
            let (depth, zp) = analytic_undercut_depth(&p, inner_r);
            let claimed = p.flange.is_castable_at(0.5);
            println!(
                "v={v:.2} claims {:9} | analytic undercut depth {depth:9.6} mm ({:8.1} um) at parting z {zp:+.3}",
                if claimed { "castable" } else { "undercut" },
                depth * 1000.0
            );
        }
        // A high-resolution analysis of the two positions the sweep disagreed on.
        for v in [0.40, 0.45, 0.5] {
            let mut d = crate::RingDesign::default();
            d.profile.flange = Flange { enabled: true, v_pos: v, ..Flange::default() };
            for steps in [96usize, 384, 1024] {
                let out = crate::mesh::build(
                    &d,
                    &crate::AlphaLibrary::builtin(),
                    crate::BuildParams { theta_steps: 128, profile_steps: steps, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true, refine: None, soften_mm: 0.0 },
                );
                let rep = crate::castability::analyze(
                    &out.mesh,
                    &crate::DraftSettings::default(),
                    d.inner_radius_mm(),
                );
                println!(
                    "v={v:.2} profile_steps={steps:4}: undercut {:5} faces {:7.4}% worst {:7.3} deg",
                    rep.undercut,
                    rep.undercut_fraction() * 100.0,
                    rep.worst_draft_deg
                );
            }
        }
    }

    #[test]
    fn scratch_flange_proof() {
        let base_crest_t = 0.5;
        println!("== flange v_pos: is_castable_at vs measured undercut ==");
        let mut optimistic: Vec<String> = Vec::new();
        let mut conservative: Vec<String> = Vec::new();
        for k in 0..=20 {
            let v = k as f64 / 20.0;
            let f = Flange { enabled: true, v_pos: v, ..Flange::default() };
            let claimed = f.is_castable_at(base_crest_t);
            let (rep, watertight) = flanged_report(v);
            let pct = rep.undercut_fraction() * 100.0;
            let measured = rep.undercut == 0;
            println!(
                "v={v:.2} claims {:9} | undercut {:5} faces {pct:6.3}% ({:.2} mm2) worst {:7.2} deg | {:?} | watertight {watertight}",
                if claimed { "castable" } else { "undercut" },
                rep.undercut,
                rep.undercut_area_mm2,
                rep.worst_draft_deg,
                rep.verdict
            );
            assert!(watertight, "flange at v {v} broke the mesh");
            match (claimed, measured) {
                (true, false) => optimistic.push(format!("v={v:.2} ({pct:.3}%)")),
                (false, true) => conservative.push(format!("v={v:.2}")),
                _ => {}
            }
        }
        println!("optimistic (claimed safe, undercuts): {optimistic:?}");
        println!("conservative (claimed unsafe, releases): {conservative:?}");
        assert!(optimistic.is_empty(), "is_castable_at called an undercutting position safe");
    }

    #[test]
    fn scratch_flange_watertight_every_style() {
        println!("== every profile style with a flange enabled ==");
        for &style in ProfileStyle::ALL {
            for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let mut d = crate::RingDesign::default();
                d.profile.apply_style(style);
                d.profile.flange = Flange { enabled: true, v_pos: v, ..Flange::default() };
                let out = crate::mesh::build(
                    &d,
                    &crate::AlphaLibrary::builtin(),
                    crate::BuildParams { theta_steps: 128, profile_steps: 96, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true, refine: None, soften_mm: 0.0 },
                );
                assert!(
                    out.report.validation.watertight,
                    "{style:?} with a flange at v {v}: {:?}",
                    out.report.validation
                );
            }
            println!("{style:?}: watertight at v 0, 0.25, 0.5, 0.75, 1");
        }
    }

    #[test]
    fn a_mid_band_flange_off_the_crest_undercuts_as_the_doc_says() {
        let f = Flange { enabled: true, v_pos: 0.25, ..Flange::default() };
        assert!(!f.is_castable_at(0.5));
        let (rep, watertight) = flanged_report(0.25);
        assert!(watertight, "mid-band flange broke the mesh");
        assert!(rep.undercut > 0, "the dome under the rim released: {:?}", rep.notes);
        assert_ne!(rep.verdict, crate::castability::Verdict::Castable);
    }

    #[test]
    fn an_edge_flange_releases_as_the_doc_says() {
        for v in [0.0, 1.0] {
            let f = Flange { enabled: true, v_pos: v, ..Flange::default() };
            assert!(f.is_castable_at(0.5));
            let (rep, watertight) = flanged_report(v);
            assert!(watertight, "edge flange at v {v} broke the mesh");
            assert_eq!(rep.undercut, 0, "edge flange at v {v}: {:?}", rep.notes);
            assert_eq!(rep.verdict, crate::castability::Verdict::Castable, "at v {v}");
        }
    }

    #[test]
    fn a_flange_level_with_the_crest_releases() {
        let f = Flange { enabled: true, v_pos: 0.5, ..Flange::default() };
        assert!(f.is_castable_at(0.5));
        let (rep, watertight) = flanged_report(0.5);
        assert!(watertight);
        assert_eq!(rep.undercut, 0, "{:?}", rep.notes);
    }
}
