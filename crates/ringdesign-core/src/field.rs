//! The height field: everything decorative is a layer of `h(u, v)` in mm,
//! displacing the swept band surface along its outward normal.
//!
//! `u` wraps at the circumference, so any layer that positions itself by an
//! integer count around the ring is seamless at the joint by construction.

use serde::{Deserialize, Serialize};

use crate::alpha::AlphaLibrary;
use crate::tiling::TilingLayer;

/// A point on the unrolled band surface, in mm.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Uv {
    /// Arc distance around the ring at the crest radius. Wraps at the
    /// circumference.
    pub u: f64,
    /// Arc distance across the cross-section, from one bore edge to the other.
    pub v: f64,
}

/// Base cross-section radius and outward normal sampled across `v`.
///
/// A layer that must produce a true plane needs the local radius and normal to
/// solve for it: a constant offset along a curved surface's normal stays curved.
#[derive(Clone, Debug, Default)]
pub struct SurfaceProfile {
    /// `(radius mm, outward normal radial component)`, uniform over the `v` span.
    pub samples: Vec<(f32, f32)>,
}

impl SurfaceProfile {
    pub fn is_empty(&self) -> bool {
        self.samples.len() < 2
    }

    /// Sample the displaceable span of a cross-section into `n` uniform steps.
    pub fn from_loop(loop_: &crate::profile::ProfileLoop, n: usize) -> Self {
        let n = n.max(2);
        let span = loop_.surface_len_mm.max(1e-9);
        let pts: Vec<&crate::profile::ProfileSample> =
            loop_.pts.iter().filter(|p| p.surface).collect();
        if pts.len() < 2 {
            return Self::default();
        }
        let samples = (0..n)
            .map(|i| {
                let target = span * i as f64 / (n - 1) as f64;
                let k = pts
                    .binary_search_by(|p| p.v_mm.total_cmp(&target))
                    .unwrap_or_else(|k| k)
                    .min(pts.len() - 1);
                (pts[k].r as f32, pts[k].nr as f32)
            })
            .collect();
        Self { samples }
    }

    /// Base draft at a `v`, degrees: 90 on a face square to the mould pull, 0 on
    /// a wall parallel to it. The normal is a unit vector in `(r, z)`, so the
    /// axial component is what the radial one leaves.
    pub fn draft_deg(&self, v: f64, band_v_len: f64) -> Option<f64> {
        let (_, nr) = self.at(v, band_v_len)?;
        Some(nr.abs().clamp(0.0, 1.0).acos().to_degrees())
    }

    /// Interpolated `(radius, outward normal radial component)` at a `v`.
    pub fn at(&self, v: f64, band_v_len: f64) -> Option<(f64, f64)> {
        if self.is_empty() || band_v_len <= 1e-9 || !v.is_finite() {
            return None;
        }
        let last = self.samples.len() - 1;
        let t = (v / band_v_len).clamp(0.0, 1.0) * last as f64;
        let i = (t.floor() as usize).min(last - 1);
        let f = t - i as f64;
        let (r0, n0) = self.samples[i];
        let (r1, n1) = self.samples[i + 1];
        Some((
            r0 as f64 + (r1 as f64 - r0 as f64) * f,
            n0 as f64 + (n1 as f64 - n0 as f64) * f,
        ))
    }
}

/// Dimensions of the unrolled band surface.
#[derive(Clone, Debug, Default)]
pub struct FieldContext {
    pub circumference_mm: f64,
    /// Total `v` span of the displaceable surface.
    pub band_v_len_mm: f64,
    /// `v` of the crest — the outermost line of the band.
    pub crest_v_mm: f64,
    pub crest_radius_mm: f64,
    /// Base geometry across the cross-section. Empty when unknown, in which
    /// case a layer needing it falls back to riding the surface.
    pub surface: SurfaceProfile,
}

/// Base draft a surface must clear to count as a side face, degrees.
pub const SIDE_FACE_MIN_DRAFT_DEG: f64 = 80.0;

/// Narrowest side face worth putting ornament on, mm.
pub const MIN_SIDE_FACE_MM: f64 = 0.25;

/// The runs of `v` square enough to the mould pull to hold relief.
///
/// Each edge is independent: a one-sided edge flange gives a wide face on one
/// edge and none on the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SideFaces {
    /// `(start, end)` of the face at the low `v` edge, mm.
    pub low: Option<(f64, f64)>,
    /// `(start, end)` of the face at the high `v` edge, mm.
    pub high: Option<(f64, f64)>,
}

impl SideFaces {
    pub fn low_width(&self) -> f64 {
        self.low.map_or(0.0, |(a, b)| (b - a).max(0.0))
    }

    pub fn high_width(&self) -> f64 {
        self.high.map_or(0.0, |(a, b)| (b - a).max(0.0))
    }

    /// Whether both faces exist and can carry the same band, so ornament can be
    /// mirrored onto them instead of sitting on one edge.
    pub fn is_even(&self) -> bool {
        let (a, b) = (self.low_width(), self.high_width());
        self.low.is_some() && self.high.is_some() && (a - b).abs() <= a.max(b) * 0.25
    }

    /// The wider of the two, as `(start, end)`.
    pub fn wider(&self) -> Option<(f64, f64)> {
        if self.low_width() >= self.high_width() { self.low.or(self.high) } else { self.high }
    }
}

impl FieldContext {
    /// The run of `v` inward from each band edge whose base draft clears
    /// `min_deg` — the faces that pull straight out of the sand, and so the only
    /// ones that hold deep relief.
    ///
    /// Each run starts where the fillet rolling out of the bore edge finishes,
    /// not at the edge itself. `None` when no face on the profile is square
    /// enough, which is the honest answer for a dome.
    pub fn side_faces(&self, min_deg: f64) -> Option<SideFaces> {
        if self.surface.is_empty() || self.band_v_len_mm <= 1e-9 {
            return None;
        }
        let n = self.surface.samples.len();
        let step = self.band_v_len_mm / (n - 1) as f64;
        let ok = |i: usize| {
            self.surface
                .draft_deg(i as f64 * step, self.band_v_len_mm)
                .is_some_and(|d| d >= min_deg)
        };
        // The outermost sample sits on the fillet rolling into the bore edge, so
        // the run is allowed to start a fillet's width in rather than at v = 0.
        let skip = ((MIN_SIDE_FACE_MM / step).ceil() as usize).min(n / 4);
        let at = |i: usize| i as f64 * step;

        let low = (0..=skip).find(|&i| ok(i)).map(|low0| {
            let mut low1 = low0;
            while low1 + 1 < n && ok(low1 + 1) {
                low1 += 1;
            }
            (at(low0), at(low1))
        });
        let high = (0..=skip).map(|i| n - 1 - i).find(|&i| ok(i)).map(|high1| {
            let mut high0 = high1;
            while high0 > 0 && ok(high0 - 1) {
                high0 -= 1;
            }
            (at(high0), at(high1))
        });

        // A run that swallowed the whole section is not a side face, and one
        // narrower than a bead is no use to put ornament on.
        let mut faces = SideFaces { low, high };
        if faces.low_width() < MIN_SIDE_FACE_MM {
            faces.low = None;
        }
        if faces.high_width() < MIN_SIDE_FACE_MM {
            faces.high = None;
        }
        if let (Some(l), Some(h)) = (faces.low, faces.high)
            && l.1 >= h.0
        {
            return None;
        }
        (faces.low.is_some() || faces.high.is_some()).then_some(faces)
    }

    /// Ring angle in degrees for a `u` coordinate.
    pub fn theta_of_u(&self, u: f64) -> f64 {
        if self.circumference_mm <= 1e-9 {
            0.0
        } else {
            u / self.circumference_mm * 360.0
        }
    }

    /// `u` coordinate for a ring angle in degrees.
    pub fn u_of_theta(&self, theta_deg: f64) -> f64 {
        theta_deg / 360.0 * self.circumference_mm
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    Add,
    Max,
    Min,
    Subtract,
    Replace,
}

impl Blend {
    pub const ALL: &'static [Blend] =
        &[Blend::Add, Blend::Max, Blend::Min, Blend::Subtract, Blend::Replace];

    pub fn label(self) -> &'static str {
        match self {
            Blend::Add => "Add",
            Blend::Max => "Max",
            Blend::Min => "Min",
            Blend::Subtract => "Carve",
            Blend::Replace => "Replace",
        }
    }

    pub fn apply(self, acc: f64, x: f64) -> f64 {
        match self {
            Blend::Add => acc + x,
            Blend::Max => acc.max(x),
            Blend::Min => acc.min(x),
            Blend::Subtract => acc - x,
            Blend::Replace => x,
        }
    }
}

/// Angular gate restricting a layer to part of the ring.
///
/// Positional rather than periodic, so it needs no integer count: `wrap_delta`
/// makes it continuous across the 0 degree joint.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub enabled: bool,
    /// Centre of the span, degrees. 90 is the top of the ring.
    pub theta_deg: f64,
    /// Extent held at full strength, degrees.
    pub span_deg: f64,
    /// Falloff at each end of the span, degrees.
    pub fade_deg: f64,
    /// Keep the layer outside the span instead of inside.
    pub invert: bool,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            enabled: false,
            theta_deg: crate::profile::TOP_DEG,
            span_deg: 90.0,
            fade_deg: 12.0,
            invert: false,
        }
    }
}

impl Window {
    /// A span centred on a ring angle, with a fade sized to a fifth of it.
    pub fn around(theta_deg: f64, span_deg: f64) -> Self {
        Self {
            enabled: true,
            theta_deg,
            span_deg,
            fade_deg: (span_deg * 0.2).clamp(2.0, 40.0),
            invert: false,
        }
    }

    /// Everywhere but a span, for keeping ornament clear of a signet head.
    pub fn except(theta_deg: f64, span_deg: f64) -> Self {
        Self { invert: true, ..Self::around(theta_deg, span_deg) }
    }

    /// Angular half-extent including the fade, degrees.
    pub fn outer_half_deg(&self) -> f64 {
        (self.span_deg.max(0.0) * 0.5 + self.fade_deg.max(0.0)).min(180.0)
    }

    /// Strength at a surface point, 0..1. Always 1 when disabled.
    pub fn mask(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        if !self.enabled {
            return 1.0;
        }
        let theta = ctx.theta_of_u(uv.u);
        if !theta.is_finite() {
            return 1.0;
        }
        let d = wrap_delta(theta - self.theta_deg, 360.0).abs();
        let half = (self.span_deg.max(0.0) * 0.5).min(180.0);
        let outer = self.outer_half_deg();
        let m = if outer <= half {
            if d <= half { 1.0 } else { 0.0 }
        } else {
            1.0 - smoothstep(half, outer, d)
        };
        if self.invert { 1.0 - m } else { m }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Layer {
    Tiling(TilingLayer),
    Signet(SignetLayer),
    Border(BorderLayer),
    SeatPad(SeatPadLayer),
    Milgrain(MilgrainLayer),
}

impl Layer {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Layer::Tiling(_) => "Tiling",
            Layer::Signet(_) => "Signet",
            Layer::Border(_) => "Border",
            Layer::SeatPad(_) => "Gem Seat Pad",
            Layer::Milgrain(_) => "Milgrain",
        }
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        match self {
            Layer::Tiling(l) => l.height(uv, ctx, lib),
            Layer::Signet(l) => l.height(uv, ctx),
            Layer::Border(l) => l.height(uv, ctx),
            Layer::SeatPad(l) => l.height(uv, ctx),
            Layer::Milgrain(l) => l.height(uv, ctx),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerEntry {
    pub name: String,
    pub enabled: bool,
    pub blend: Blend,
    /// Overall scale on this layer's output, 0..1+.
    pub opacity: f64,
    /// Angular gate. Disabled by default, so the layer runs the whole way round.
    #[serde(default)]
    pub window: Window,
    pub layer: Layer,
}

impl LayerEntry {
    pub fn new(name: impl Into<String>, layer: Layer) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            blend: Blend::Max,
            opacity: 1.0,
            window: Window::default(),
            layer,
        }
    }

    pub fn with_window(mut self, window: Window) -> Self {
        self.window = window;
        self
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LayerStack {
    pub layers: Vec<LayerEntry>,
}

impl LayerStack {
    /// Composite every enabled layer at a surface point. Returns mm of
    /// displacement along the outward normal.
    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        let mut acc = 0.0;
        for e in &self.layers {
            if !e.enabled {
                continue;
            }
            // A gated-out layer takes no part in the blend at all, so Replace
            // outside its window cannot wipe the layers under it.
            let w = e.window.mask(uv, ctx);
            if w <= 0.0 {
                continue;
            }
            let h = e.layer.height(uv, ctx, lib) * e.opacity * w;
            acc = e.blend.apply(acc, h);
        }
        acc
    }

    pub fn is_empty(&self) -> bool {
        self.layers.iter().all(|l| !l.enabled)
    }
}

// --- Border ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BorderProfile {
    Round,
    Flat,
    Knife,
    Step,
    Rope,
}

impl BorderProfile {
    pub const ALL: &'static [BorderProfile] = &[
        BorderProfile::Round,
        BorderProfile::Flat,
        BorderProfile::Knife,
        BorderProfile::Step,
        BorderProfile::Rope,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BorderProfile::Round => "Round wire",
            BorderProfile::Flat => "Flat rail",
            BorderProfile::Knife => "Knife rail",
            BorderProfile::Step => "Stepped",
            BorderProfile::Rope => "Rope twist",
        }
    }
}

/// A rail running the full way around the band at a fixed `v`.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BorderLayer {
    /// Centre of the rail across the band, mm.
    pub v_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub profile: BorderProfile,
    /// Also place a copy mirrored about the middle of the band.
    pub mirror: bool,
    /// Twists per revolution, for the rope profile. Integer keeps it seamless.
    pub rope_twists: u32,
}

impl Default for BorderLayer {
    fn default() -> Self {
        Self {
            v_mm: 1.0,
            width_mm: 0.7,
            height_mm: 0.35,
            profile: BorderProfile::Round,
            mirror: true,
            rope_twists: 48,
        }
    }
}

impl BorderLayer {
    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let mut h = self.rail_at(uv, ctx, self.v_mm);
        if self.mirror {
            let mirrored = ctx.band_v_len_mm - self.v_mm;
            h = h.max(self.rail_at(uv, ctx, mirrored));
        }
        h
    }

    fn rail_at(&self, uv: Uv, ctx: &FieldContext, centre_v: f64) -> f64 {
        let half = (self.width_mm * 0.5).max(1e-6);
        let x = (uv.v - centre_v) / half;
        if x.abs() >= 1.0 {
            return 0.0;
        }
        let s = match self.profile {
            BorderProfile::Round => (1.0 - x * x).max(0.0).sqrt(),
            BorderProfile::Flat => 1.0 - smoothstep(0.7, 1.0, x.abs()),
            BorderProfile::Knife => 1.0 - x.abs(),
            BorderProfile::Step => {
                if x.abs() < 0.55 { 1.0 } else { 1.0 - smoothstep(0.55, 1.0, x.abs()) }
            }
            BorderProfile::Rope => {
                // A round rail whose crest spirals: the bead migrates across the
                // rail as u advances, giving a twisted-wire read.
                let twists = self.rope_twists.max(1) as f64;
                let phase = uv.u / ctx.circumference_mm.max(1e-9) * twists * std::f64::consts::TAU;
                let offset = 0.45 * phase.sin();
                let xr = ((uv.v - centre_v) / half - offset).clamp(-1.0, 1.0);
                (1.0 - xr * xr).max(0.0).sqrt()
            }
        };
        self.height_mm * s.clamp(0.0, 1.0)
    }
}

// --- Gem seat pad ----------------------------------------------------------

/// A raised circular boss the bench jeweller cuts a seat into by hand. Domed
/// so it releases from a ±Z pull; the skirt fairs it into the band.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SeatPadLayer {
    /// Position around the ring, degrees.
    pub theta_deg: f64,
    /// Position across the band, mm.
    pub v_mm: f64,
    pub diameter_mm: f64,
    pub height_mm: f64,
    /// 0 = flat-topped boss, 1 = full dome.
    pub crown: f64,
    /// Skirt width fairing the pad into the band, mm.
    pub blend_mm: f64,
}

impl Default for SeatPadLayer {
    fn default() -> Self {
        Self {
            theta_deg: crate::profile::TOP_DEG,
            v_mm: 0.0,
            diameter_mm: 5.0,
            height_mm: 1.2,
            crown: 0.65,
            blend_mm: 0.8,
        }
    }
}

impl SeatPadLayer {
    /// Diameter of the largest stone this pad can reasonably seat, mm.
    pub fn suggested_stone_mm(&self) -> f64 {
        (self.diameter_mm - 1.2).max(0.5)
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let r = (self.diameter_mm * 0.5).max(1e-6);
        let blend = self.blend_mm.max(0.0);
        let u0 = ctx.u_of_theta(self.theta_deg);
        let du = wrap_delta(uv.u - u0, ctx.circumference_mm);
        let dv = uv.v - self.v_mm;
        let d = (du * du + dv * dv).sqrt();

        let crown = self.crown.clamp(0.0, 1.0);
        if d <= r {
            let t = d / r;
            let dome = (1.0 - t * t).max(0.0).sqrt();
            let flat = 1.0 - smoothstep(0.82, 1.0, t);
            self.height_mm * ((1.0 - crown) * flat + crown * dome)
        } else if blend > 1e-9 && d <= r + blend {
            // Only the flat-topped share has material left at the rim to fair.
            let rim = self.height_mm * (1.0 - crown);
            rim * (1.0 - smoothstep(0.0, 1.0, (d - r) / blend))
        } else {
            0.0
        }
    }
}

// --- Milgrain --------------------------------------------------------------

/// A ring of beads. The count is an integer so the pattern closes on itself.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MilgrainLayer {
    pub v_mm: f64,
    pub bead_diameter_mm: f64,
    pub beads_around: u32,
    pub height_mm: f64,
    pub mirror: bool,
}

impl Default for MilgrainLayer {
    fn default() -> Self {
        Self {
            v_mm: 0.8,
            bead_diameter_mm: 0.45,
            beads_around: 120,
            height_mm: 0.22,
            mirror: true,
        }
    }
}

impl MilgrainLayer {
    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let mut h = self.beads_at(uv, ctx, self.v_mm);
        if self.mirror {
            h = h.max(self.beads_at(uv, ctx, ctx.band_v_len_mm - self.v_mm));
        }
        h
    }

    fn beads_at(&self, uv: Uv, ctx: &FieldContext, centre_v: f64) -> f64 {
        let r = (self.bead_diameter_mm * 0.5).max(1e-6);
        let dv = uv.v - centre_v;
        if dv.abs() >= r {
            return 0.0;
        }
        let n = self.beads_around.max(1) as f64;
        let pitch = ctx.circumference_mm / n;
        // Distance to the nearest bead centre along u.
        let du = uv.u - (uv.u / pitch).round() * pitch;
        let d = (du * du + dv * dv).sqrt();
        if d >= r {
            return 0.0;
        }
        let t = d / r;
        self.height_mm * (1.0 - t * t).max(0.0).sqrt()
    }
}


// --- Signet ----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignetOutline {
    Oval,
    Round,
    Cushion,
    Rectangle,
    Hexagon,
}

impl SignetOutline {
    pub const ALL: &'static [SignetOutline] = &[
        SignetOutline::Oval,
        SignetOutline::Round,
        SignetOutline::Cushion,
        SignetOutline::Rectangle,
        SignetOutline::Hexagon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SignetOutline::Oval => "Oval",
            SignetOutline::Round => "Round",
            SignetOutline::Cushion => "Cushion",
            SignetOutline::Rectangle => "Rectangle",
            SignetOutline::Hexagon => "Hexagon",
        }
    }

    /// Superellipse exponent for the outline. Hexagon is handled separately.
    pub fn exponent(self) -> f64 {
        match self {
            SignetOutline::Oval | SignetOutline::Round => 2.0,
            SignetOutline::Cushion => 4.0,
            SignetOutline::Rectangle => 8.0,
            SignetOutline::Hexagon => 2.0,
        }
    }
}

/// A raised signet table to hand-engrave, faired into the shank.
///
/// Displacement is constant across the flat, so the table is a uniform offset of
/// the band and keeps the band's own curvature rather than becoming a plane: on a
/// size 7 half-round, a 12 x 9 mm table stands 2.15 mm out of flat. The shoulder
/// takes the sides back down to the band instead of leaving a wall.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SignetLayer {
    /// Position around the ring, degrees. 90 is the top.
    pub theta_deg: f64,
    /// Position across the band, mm. Usually the crest.
    pub v_mm: f64,
    pub outline: SignetOutline,
    /// Extent around the ring, mm.
    pub length_mm: f64,
    /// Extent across the band, mm.
    pub width_mm: f64,
    /// Height of the table above the band, mm.
    pub height_mm: f64,
    /// Fraction of the face held at full height, 0..1. The rest rolls off.
    pub top_flat: f64,
    /// Width of the shoulder fairing the table into the band, mm.
    pub shoulder_mm: f64,
    /// Rotation of the outline within the band, degrees.
    pub rotation_deg: f64,
}

impl Default for SignetLayer {
    fn default() -> Self {
        Self {
            theta_deg: crate::profile::TOP_DEG,
            v_mm: 0.0,
            outline: SignetOutline::Oval,
            length_mm: 8.0,
            width_mm: 5.0,
            height_mm: 1.6,
            top_flat: 0.72,
            shoulder_mm: 1.4,
            rotation_deg: 0.0,
        }
    }
}

impl SignetLayer {
    /// A table sized to stay flat on the given band, centred on the crest.
    pub fn fitted_to(ctx: &FieldContext) -> Self {
        let width = (Self::room_across(ctx) * 0.55).clamp(2.0, 14.0);
        Self {
            v_mm: ctx.crest_v_mm,
            width_mm: width,
            length_mm: width * 1.6,
            ..Default::default()
        }
    }

    /// Surface across the band a table can stand on, mm.
    ///
    /// Measured from the crest out, because a crest sitting off centre — a
    /// flange takes it to one side — leaves less room than the band width says.
    /// Side faces are excluded: a shoulder rolling off onto the fillet between
    /// crest and side face leaves a wall rather than a fairing.
    pub fn room_across(ctx: &FieldContext) -> f64 {
        let (lo, hi) = match ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG) {
            Some(f) => (
                f.low.map_or(0.0, |(_, end)| end),
                f.high.map_or(ctx.band_v_len_mm, |(start, _)| start),
            ),
            None => (0.0, ctx.band_v_len_mm),
        };
        let half = (ctx.crest_v_mm - lo).min(hi - ctx.crest_v_mm).max(0.0);
        (half * 2.0).min(ctx.band_v_len_mm)
    }

    /// Whether the table reaches past the surface that can support it, which is
    /// what makes it bow away from a true plane and wall up at its shoulders.
    pub fn overhangs(&self, ctx: &FieldContext) -> bool {
        self.width_mm > Self::room_across(ctx) * 0.75
    }

    /// Usable engraving area, mm2, over the flat part of the table.
    pub fn engraving_area_mm2(&self) -> f64 {
        let f = self.top_flat.clamp(0.0, 1.0);
        std::f64::consts::PI * (self.length_mm * 0.5 * f) * (self.width_mm * 0.5 * f)
    }

    /// Normalized outline distance at a point local to the table centre, in mm.
    /// Returns 0 at the centre and 1 on the outline.
    ///
    /// `Round` is a true circle on the smaller extent; every other outline fills
    /// `length_mm` by `width_mm`.
    pub fn outline_distance(&self, du: f64, dv: f64) -> f64 {
        let (sin_a, cos_a) = (-self.rotation_deg.to_radians()).sin_cos();
        let x_mm = du * cos_a - dv * sin_a;
        let y_mm = du * sin_a + dv * cos_a;

        let (half_u, half_v) = match self.outline {
            SignetOutline::Round => {
                let r = (self.length_mm.min(self.width_mm) * 0.5).max(1e-6);
                (r, r)
            }
            _ => ((self.length_mm * 0.5).max(1e-6), (self.width_mm * 0.5).max(1e-6)),
        };
        let x = x_mm / half_u;
        let y = y_mm / half_v;

        match self.outline {
            // Three slabs 60 degrees apart, scaled to the extents: flat sides,
            // points at the length ends.
            SignetOutline::Hexagon => y.abs().max(x.abs() + 0.5 * y.abs()),
            _ => {
                let n = self.outline.exponent().max(1e-3);
                (x.abs().powf(n) + y.abs().powf(n)).powf(1.0 / n)
            }
        }
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let u0 = ctx.u_of_theta(self.theta_deg);
        let du = wrap_delta(uv.u - u0, ctx.circumference_mm);
        let dv = uv.v - self.v_mm;
        let d = self.outline_distance(du, dv);

        let flat = self.top_flat.clamp(0.0, 1.0);
        // The metric is homogeneous along a ray, so mm convert by that ray's own scale.
        let r_mm = (du * du + dv * dv).sqrt();
        let per_mm = if r_mm > 1e-9 { d / r_mm } else { 0.0 };
        let outer = 1.0 + self.shoulder_mm.max(0.0) * per_mm;
        if d >= outer {
            return 0.0;
        }
        let table = self.table_height(uv, ctx, du);
        if d <= flat {
            return table;
        }
        table * (1.0 - smoothstep(flat, outer, d))
    }

    /// Displacement landing this point on the table's plane.
    ///
    /// The plane stands `height_mm` proud of the crest, perpendicular to the
    /// radius at `theta_deg`. Reaching it needs a radius of `plane / cos` of the
    /// angle off centre, so the displacement is solved per point rather than
    /// held constant — a constant offset from a curved band stays curved.
    fn table_height(&self, uv: Uv, ctx: &FieldContext, du: f64) -> f64 {
        let Some((r, nr)) = ctx.surface.at(uv.v, ctx.band_v_len_mm) else {
            return self.height_mm;
        };
        if ctx.crest_radius_mm <= 1e-9 {
            return self.height_mm;
        }
        let plane_r = ctx.crest_radius_mm + self.height_mm;
        let cos_t = (du / ctx.crest_radius_mm).cos();
        if cos_t <= 1e-3 {
            return self.height_mm;
        }
        // Clamped so a point far down the flank cannot demand a runaway offset.
        ((plane_r / cos_t - r) / nr.max(0.25)).clamp(0.0, self.height_mm * 6.0)
    }
}

// --- Helpers ---------------------------------------------------------------

/// Shortest signed difference on a circle of the given period.
pub fn wrap_delta(d: f64, period: f64) -> f64 {
    if period <= 1e-9 {
        return d;
    }
    let mut x = d % period;
    if x > period * 0.5 {
        x -= period;
    } else if x < -period * 0.5 {
        x += period;
    }
    x
}

/// Hermite smoothstep, clamped.
pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-12)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> FieldContext {
        FieldContext {
            circumference_mm: 60.0,
            band_v_len_mm: 8.0,
            crest_v_mm: 4.0,
            crest_radius_mm: 9.5,
            surface: Default::default(),
        }
    }

    #[test]
    fn seat_pad_peaks_at_its_centre_and_vanishes_outside() {
        let c = ctx();
        let pad = SeatPadLayer { theta_deg: 90.0, v_mm: 4.0, ..Default::default() };
        let u0 = c.u_of_theta(90.0);
        let peak = pad.height(Uv { u: u0, v: 4.0 }, &c);
        assert!((peak - pad.height_mm).abs() < 1e-9);
        let far = pad.height(Uv { u: u0 + 12.0, v: 4.0 }, &c);
        assert_eq!(far, 0.0);
    }

    #[test]
    fn seat_pad_wraps_across_the_seam() {
        let c = ctx();
        let pad = SeatPadLayer { theta_deg: 0.0, v_mm: 4.0, diameter_mm: 6.0, ..Default::default() };
        // Just before the seam and just after must both be on the pad.
        let a = pad.height(Uv { u: 0.5, v: 4.0 }, &c);
        let b = pad.height(Uv { u: c.circumference_mm - 0.5, v: 4.0 }, &c);
        assert!(a > 0.0 && b > 0.0);
        assert!((a - b).abs() < 1e-9, "pad is not symmetric across the seam");
    }

    #[test]
    fn milgrain_closes_on_itself() {
        let c = ctx();
        let m = MilgrainLayer { beads_around: 120, v_mm: 1.0, mirror: false, ..Default::default() };
        let a = m.height(Uv { u: 0.0, v: 1.0 }, &c);
        let b = m.height(Uv { u: c.circumference_mm, v: 1.0 }, &c);
        assert!((a - b).abs() < 1e-9, "milgrain does not close: {a} vs {b}");
        assert!(a > 0.0);
    }

    #[test]
    fn border_mirror_is_symmetric_about_the_band_centre() {
        let c = ctx();
        let b = BorderLayer { v_mm: 1.0, mirror: true, ..Default::default() };
        let lo = b.height(Uv { u: 0.0, v: 1.0 }, &c);
        let hi = b.height(Uv { u: 0.0, v: c.band_v_len_mm - 1.0 }, &c);
        assert!((lo - hi).abs() < 1e-9);
        assert!(lo > 0.0);
    }

    /// A table sized to the whole band on a squared-sided profile rolls its
    /// shoulder off onto the fillet between crest and side face, which walls up
    /// instead of fairing. The fit has to stop at the side faces.
    #[test]
    fn a_fitted_table_keeps_clear_of_the_side_faces() {
        let mut d = crate::RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 3.4;
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let ctx = d.field_context();

        let faces = ctx.side_faces(SIDE_FACE_MIN_DRAFT_DEG).expect("squared sides have faces");
        let room = SignetLayer::room_across(&ctx);
        assert!(
            room < ctx.band_v_len_mm - faces.low_width(),
            "room {room:.2} mm still covers a side face on a {:.2} mm band",
            ctx.band_v_len_mm
        );

        let s = SignetLayer::fitted_to(&ctx);
        assert!(!s.overhangs(&ctx));
        let (lo, hi) = (s.v_mm - s.width_mm * 0.5, s.v_mm + s.width_mm * 0.5);
        let low_end = faces.low.map_or(0.0, |(_, e)| e);
        let high_start = faces.high.map_or(ctx.band_v_len_mm, |(s, _)| s);
        assert!(lo > low_end, "table starts at {lo:.2} mm, inside the side face ending {low_end:.2}");
        assert!(hi < high_start, "table ends at {hi:.2} mm, inside the side face from {high_start:.2}");
    }

    #[test]
    fn window_is_full_strength_inside_and_zero_outside() {
        let c = ctx();
        let w = Window::around(90.0, 60.0);
        let at = |deg: f64| w.mask(Uv { u: c.u_of_theta(deg), v: 0.0 }, &c);
        assert!((at(90.0) - 1.0).abs() < 1e-12);
        assert!((at(115.0) - 1.0).abs() < 1e-12, "30 deg out is still inside the span");
        assert_eq!(at(180.0), 0.0);
        assert_eq!(at(0.0), 0.0);
        // Monotone across the fade.
        let a = at(122.0);
        let b = at(128.0);
        assert!(a > b && b > 0.0 && a < 1.0, "fade is not monotone: {a} then {b}");
    }

    #[test]
    fn window_is_continuous_across_the_seam() {
        let c = ctx();
        let w = Window::around(0.0, 60.0);
        let just_before = w.mask(Uv { u: c.circumference_mm - 1e-6, v: 0.0 }, &c);
        let just_after = w.mask(Uv { u: 0.0, v: 0.0 }, &c);
        assert!((just_before - just_after).abs() < 1e-9);
        assert!((just_after - 1.0).abs() < 1e-12);
        // Symmetric either side of a window centred on the joint.
        let lo = w.mask(Uv { u: c.u_of_theta(340.0), v: 0.0 }, &c);
        let hi = w.mask(Uv { u: c.u_of_theta(20.0), v: 0.0 }, &c);
        assert!((lo - hi).abs() < 1e-9, "seam window is lopsided: {lo} vs {hi}");
    }

    #[test]
    fn inverted_window_is_the_complement() {
        let c = ctx();
        let inside = Window::around(90.0, 70.0);
        let outside = Window::except(90.0, 70.0);
        for deg in [0.0, 45.0, 90.0, 121.0, 135.0, 200.0, 359.0] {
            let uv = Uv { u: c.u_of_theta(deg), v: 0.0 };
            let sum = inside.mask(uv, &c) + outside.mask(uv, &c);
            assert!((sum - 1.0).abs() < 1e-12, "at {deg} deg the masks sum to {sum}");
        }
    }

    #[test]
    fn disabled_window_passes_everything() {
        let c = ctx();
        let w = Window::default();
        assert!(!w.enabled);
        for deg in [0.0, 90.0, 270.0] {
            assert_eq!(w.mask(Uv { u: c.u_of_theta(deg), v: 0.0 }, &c), 1.0);
        }
    }

    #[test]
    fn a_gated_out_replace_layer_leaves_the_stack_alone() {
        let c = ctx();
        let lib = AlphaLibrary::builtin();
        let mut stack = LayerStack::default();
        stack.layers.push(LayerEntry::new(
            "rail",
            Layer::Border(BorderLayer { v_mm: 1.0, mirror: false, ..Default::default() }),
        ));
        let mut wiper = LayerEntry::new(
            "wiper",
            Layer::SeatPad(SeatPadLayer { theta_deg: 90.0, v_mm: 4.0, ..Default::default() }),
        );
        wiper.blend = Blend::Replace;
        wiper.window = Window::around(90.0, 20.0);
        stack.layers.push(wiper);

        // Far from the window the rail must survive untouched.
        let uv = Uv { u: c.u_of_theta(270.0), v: 1.0 };
        let with = stack.height(uv, &c, &lib);
        stack.layers.pop();
        let without = stack.height(uv, &c, &lib);
        assert!(without > 0.0);
        assert!((with - without).abs() < 1e-12, "gated Replace still wiped the stack");
    }

    #[test]
    fn window_scales_a_layer_rather_than_moving_it() {
        let c = ctx();
        let lib = AlphaLibrary::builtin();
        let rail = Layer::Border(BorderLayer { v_mm: 1.0, mirror: false, ..Default::default() });
        let uv = Uv { u: c.u_of_theta(126.0), v: 1.0 };
        let full = rail.height(uv, &c, &lib);
        let mut e = LayerEntry::new("rail", rail);
        e.window = Window::around(90.0, 60.0);
        let mut stack = LayerStack::default();
        let m = e.window.mask(uv, &c);
        stack.layers.push(e);
        assert!(m > 0.0 && m < 1.0, "test point is not in the fade");
        assert!((stack.height(uv, &c, &lib) - full * m).abs() < 1e-12);
    }

    #[test]
    fn blend_modes_composite_in_order() {
        assert_eq!(Blend::Add.apply(1.0, 2.0), 3.0);
        assert_eq!(Blend::Max.apply(1.0, 2.0), 2.0);
        assert_eq!(Blend::Subtract.apply(1.0, 2.0), -1.0);
        assert_eq!(Blend::Replace.apply(1.0, 2.0), 2.0);
    }

    fn signet() -> SignetLayer {
        SignetLayer { theta_deg: 90.0, v_mm: 4.0, ..Default::default() }
    }

    /// Area enclosed by the outline, by sampling.

    #[test]
    fn signet_centre_is_the_full_table_height() {
        let c = ctx();
        let s = signet();
        let u0 = c.u_of_theta(s.theta_deg);
        assert_eq!(s.outline_distance(0.0, 0.0), 0.0);
        assert_eq!(s.height(Uv { u: u0, v: s.v_mm }, &c), s.height_mm);
    }

    #[test]
    fn signet_field_is_constant_inside_top_flat() {
        let c = ctx();
        let s = signet();
        let u0 = c.u_of_theta(s.theta_deg);
        let mut sampled = 0;
        for i in 0..=40 {
            let du = -s.length_mm * 0.5 + s.length_mm * i as f64 / 40.0;
            for j in 0..=40 {
                let dv = -s.width_mm * 0.5 + s.width_mm * j as f64 / 40.0;
                if s.outline_distance(du, dv) > s.top_flat {
                    continue;
                }
                let h = s.height(Uv { u: u0 + du, v: s.v_mm + dv }, &c);
                assert_eq!(h, s.height_mm, "engraving table is not constant at ({du}, {dv})");
                sampled += 1;
            }
        }
        assert!(sampled > 100, "sampled too little of the flat: {sampled}");
    }

    #[test]
    fn signet_fairs_to_zero_over_the_shoulder() {
        let c = ctx();
        let s = signet();
        let u0 = c.u_of_theta(s.theta_deg);
        let at = |du: f64| s.height(Uv { u: u0 + du, v: s.v_mm }, &c);
        // On the outline the table still has material, so the side is not a wall.
        let rim = at(s.length_mm * 0.5);
        assert!(rim > 0.0 && rim < s.height_mm, "rim {rim}");
        assert!(at(s.length_mm * 0.5 + s.shoulder_mm * 0.5) > 0.0);
        assert_eq!(at(s.length_mm * 0.5 + s.shoulder_mm + 0.2), 0.0);
        assert_eq!(at(s.length_mm * 0.5 + 4.0), 0.0);
        // Monotone descent from the flat out to the band.
        let mut prev = s.height_mm;
        for k in 0..=80 {
            let h = at(s.length_mm * 0.5 * s.top_flat + 0.1 * k as f64);
            assert!(h <= prev + 1e-12, "height rises again at step {k}");
            prev = h;
        }
    }

    #[test]
    fn signet_table_wraps_across_the_seam() {
        let c = ctx();
        let s = SignetLayer { theta_deg: 0.0, v_mm: 4.0, ..signet() };
        assert_eq!(s.height(Uv { u: 0.0, v: s.v_mm }, &c), s.height_mm);
        let before = s.height(Uv { u: c.circumference_mm - 5.0, v: s.v_mm }, &c);
        let after = s.height(Uv { u: 5.0, v: s.v_mm }, &c);
        assert!(before > 0.0 && before < s.height_mm, "seam sample is off the roll-off");
        assert!((before - after).abs() < 1e-12, "table is not symmetric across the seam");
    }

    #[test]
    fn signet_outlines_have_distinct_footprints() {
        // Compared by shape rather than enclosed area: an ellipse and a hexagon
        // with the same extents enclose nearly the same area.
        let base = SignetLayer { length_mm: 8.0, width_mm: 5.0, ..Default::default() };
        let angles: Vec<f64> = (0..24).map(|i| i as f64 / 24.0 * std::f64::consts::TAU).collect();

        let signature = |o: SignetOutline| -> Vec<f64> {
            let s = SignetLayer { outline: o, ..base };
            angles
                .iter()
                .map(|a| s.outline_distance(4.0 * a.cos(), 2.5 * a.sin()))
                .collect()
        };

        for (i, &a) in SignetOutline::ALL.iter().enumerate() {
            for &b in &SignetOutline::ALL[i + 1..] {
                let (sa, sb) = (signature(a), signature(b));
                let diff: f64 = sa
                    .iter()
                    .zip(&sb)
                    .map(|(x, y)| (x - y).abs())
                    .fold(0.0, f64::max);
                assert!(diff > 1e-3, "{a:?} and {b:?} trace the same outline");
            }
        }
    }

    #[test]
    fn signet_rotation_turns_the_outline() {
        let s = SignetLayer { outline: SignetOutline::Rectangle, rotation_deg: 90.0, ..signet() };
        // The long axis now runs across the band.
        assert!(s.outline_distance(0.0, s.length_mm * 0.45) < 1.0);
        assert!(s.outline_distance(s.length_mm * 0.45, 0.0) > 1.0);
    }

    #[test]
    fn signet_engraving_area_scales_with_the_flat() {
        let small = SignetLayer { top_flat: 0.5, ..signet() };
        let big = SignetLayer { top_flat: 0.9, ..signet() };
        assert!(small.engraving_area_mm2() > 0.0);
        assert!(big.engraving_area_mm2() > small.engraving_area_mm2());
        let ratio = big.engraving_area_mm2() / small.engraving_area_mm2();
        assert!((ratio - (0.9f64 / 0.5).powi(2)).abs() < 1e-9, "ratio {ratio}");
        // Never larger than the table it sits on.
        let full = SignetLayer { top_flat: 1.0, ..signet() };
        assert!(full.engraving_area_mm2() <= full.length_mm * full.width_mm);
    }

    #[test]
    fn wrap_delta_takes_the_short_way_round() {
        assert!((wrap_delta(59.0, 60.0) - -1.0).abs() < 1e-9);
        assert!((wrap_delta(-59.0, 60.0) - 1.0).abs() < 1e-9);
        assert!((wrap_delta(1.0, 60.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_signet_table_that_fits_the_band_is_flat_on_the_mesh() {
        use crate::mesh::{self, BuildParams};
        use crate::alpha::AlphaLibrary;

        let lib = AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        let ctx = d.field_context();
        let s = SignetLayer::fitted_to(&ctx);
        assert!(!s.overhangs(&ctx), "fitted_to produced an overhanging table");
        let probe = s;
        d.layers.layers.push(LayerEntry::new("signet", Layer::Signet(s)));

        let built = mesh::build(
            &d,
            &lib,
            BuildParams { theta_steps: 512, profile_steps: 256, ..Default::default() },
        );

        // The table faces +Y; anything at or below the crest is band, not table.
        let floor_r = ctx.crest_radius_mm + 0.05;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for v in &built.mesh.vertices {
            let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
            if y <= 0.0 || (x * x + y * y).sqrt() < floor_r {
                continue;
            }
            let du = x.atan2(y) * ctx.crest_radius_mm;
            if probe.outline_distance(du, z) > probe.top_flat * 0.9 {
                continue;
            }
            lo = lo.min(y);
            hi = hi.max(y);
        }
        assert!(lo < hi, "no table vertices found");
        assert!(
            hi - lo < 0.05,
            "table is {:.4} mm out of flat; a graver needs a true surface",
            hi - lo
        );
    }
}
