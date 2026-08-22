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
    /// Radius of the finger hole, mm. 0 when unknown, which disables the
    /// layers that carve relative to it.
    pub bore_radius_mm: f64,
    /// Side-face runs at the standard threshold, found on first use. A gate
    /// reads this per sample, and the walk behind it is far too slow for that.
    pub side_faces_cache: std::sync::OnceLock<Option<SideFaces>>,
}

impl FieldContext {
    /// Real arc per unit of `u` at this `v` — `r(v) / r_crest`, in `(0, 1]`.
    ///
    /// `u` is arc distance **at the crest radius**, so it overstates the
    /// metal anywhere the surface sits further in. That is not a rounding
    /// error: a squared band's side face runs at 0.80–0.83 of the crest
    /// radius, so a bridge the chart calls 0.55 mm is 0.45 mm of metal —
    /// optimistic, on exactly the surfaces the doctrine sends all ornament
    /// to. On the crest it is exactly 1, so nothing on the parting plane
    /// moves.
    ///
    /// This corrects what is *reported* and the integer counts generators
    /// solve, never `h(u, v)` itself: the chart stays one clean
    /// reparameterization of theta, and no saved design changes shape.
    pub fn arc_scale(&self, v_mm: f64) -> f64 {
        if !(self.crest_radius_mm > 1e-9) {
            return 1.0;
        }
        match self.surface.at(v_mm, self.band_v_len_mm) {
            Some((r, _)) if r > 1e-9 => (r / self.crest_radius_mm).clamp(1e-6, 1.0),
            _ => 1.0,
        }
    }

    /// The least [`arc_scale`](Self::arc_scale) over a `v` range — the
    /// conservative read for a feature whose footprint spans the section,
    /// where no single number is the metric.
    pub fn arc_scale_min(&self, v_lo: f64, v_hi: f64) -> f64 {
        let (lo, hi) = if v_lo <= v_hi { (v_lo, v_hi) } else { (v_hi, v_lo) };
        let steps = 8;
        (0..=steps)
            .map(|i| self.arc_scale(lo + (hi - lo) * i as f64 / steps as f64))
            .fold(1.0f64, f64::min)
    }
}

/// Base draft a surface must clear to count as a side face, degrees.
pub const SIDE_FACE_MIN_DRAFT_DEG: f64 = 80.0;

/// Narrowest side face worth putting ornament on, mm. A dome leaves a sliver
/// of near-square metal at the bore edge where the crown clamp bottoms out on
/// MIN_EDGE_MM; that is geometry, not a surface to decorate.
pub const MIN_SIDE_FACE_MM: f64 = 0.45;

/// Share of the room across a head a signet table fills. Measured: clean here,
/// bowing by 0.82 and walled up by 0.92.
pub const SIGNET_TABLE_FRAC: f64 = 0.70;

/// Steepest base draft a signet table will stand on. Past this the surface has
/// dropped too far below the table plane for a shoulder to fair it back.
pub const TABLE_MAX_DRAFT_DEG: f64 = 20.0;

/// `v` of a sample index.
#[inline]
fn at_step(i: usize, step: f64) -> f64 {
    i as f64 * step
}

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
    /// [`side_faces`](Self::side_faces) at [`SIDE_FACE_MIN_DRAFT_DEG`], cached.
    pub fn side_faces_std(&self) -> Option<SideFaces> {
        *self.side_faces_cache.get_or_init(|| self.side_faces(SIDE_FACE_MIN_DRAFT_DEG))
    }

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
    /// Max with the crossing filleted over the entry's `soft_mm`.
    SmoothMax,
    /// Min with the same filleted crossing.
    SmoothMin,
}

impl Blend {
    pub const ALL: &'static [Blend] = &[
        Blend::Add,
        Blend::Max,
        Blend::SmoothMax,
        Blend::Min,
        Blend::SmoothMin,
        Blend::Subtract,
        Blend::Replace,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Blend::Add => "Add",
            Blend::Max => "Max",
            Blend::Min => "Min",
            Blend::Subtract => "Carve",
            Blend::Replace => "Replace",
            Blend::SmoothMax => "Smooth max",
            Blend::SmoothMin => "Smooth min",
        }
    }

    pub fn is_smooth(self) -> bool {
        matches!(self, Blend::SmoothMax | Blend::SmoothMin)
    }

    /// `soft_mm` fillets the smooth modes' crossings; the rest ignore it.
    pub fn apply(self, acc: f64, x: f64, soft_mm: f64) -> f64 {
        match self {
            Blend::Add => acc + x,
            Blend::Max => acc.max(x),
            Blend::Min => acc.min(x),
            Blend::Subtract => acc - x,
            Blend::Replace => x,
            Blend::SmoothMax => smax(acc, x, soft_mm.max(0.0)),
            Blend::SmoothMin => -smax(-acc, -x, soft_mm.max(0.0)),
        }
    }
}

/// The tie-exact smooth maximum [`profile`] proved out for the signet union:
/// a crossfade over the band rather than an addition to the maximum, so it is
/// exact when either side dominates, exact on a tie, and C¹ at both ends. The
/// usual `max + r·h²/4` rounds a tie *outward*, which fattens whatever it
/// touches.
pub(crate) fn smax(a: f64, b: f64, r: f64) -> f64 {
    if r <= 1e-9 {
        return a.max(b);
    }
    let w = smoothstep(-1.0, 1.0, ((a - b) / r).clamp(-1.0, 1.0));
    a * w + b * (1.0 - w)
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
    /// Cross-band gate, multiplied into the angular mask. Works with the
    /// angular window disabled, so a layer can be gated across the band alone.
    #[serde(default)]
    pub v_gate: VGate,
}

/// Falloff a side-face gate applies inward from each run boundary, mm.
pub const SIDE_GATE_FADE_MM: f64 = 0.25;

/// Cross-band gate: which strip of the section a layer covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum VGate {
    /// The whole section.
    #[default]
    Off,
    /// A band holding `span_mm` at full strength around `center_mm`, with
    /// `fade_mm` shoulders beyond it.
    Band { center_mm: f64, span_mm: f64, fade_mm: f64 },
    /// The side-face runs the base profile guarantees castable, resolved at
    /// evaluation time so the gate tracks profile edits.
    SideFaces(SideFacePick),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideFacePick {
    Low,
    High,
    #[default]
    Wider,
    Both,
}

impl VGate {
    /// Strength at a `v` position, 0..1.
    pub fn mask(&self, v: f64, ctx: &FieldContext) -> f64 {
        match *self {
            VGate::Off => 1.0,
            VGate::Band { center_mm, span_mm, fade_mm } => {
                let half = (span_mm * 0.5).max(0.0);
                let fade = fade_mm.max(0.0);
                let d = (v - center_mm).abs();
                if fade <= 1e-9 {
                    if d <= half { 1.0 } else { 0.0 }
                } else {
                    1.0 - smoothstep(half, half + fade, d)
                }
            }
            VGate::SideFaces(pick) => {
                let Some(sf) = ctx.side_faces_std() else { return 0.0 };
                // Fades run inward, so the relief dies before leaving the run.
                let run = |r: Option<(f64, f64)>| {
                    let Some((lo, hi)) = r else { return 0.0 };
                    let fade = SIDE_GATE_FADE_MM.min((hi - lo) * 0.25).max(1e-9);
                    smoothstep(lo, lo + fade, v) * (1.0 - smoothstep(hi - fade, hi, v))
                };
                match pick {
                    SideFacePick::Low => run(sf.low),
                    SideFacePick::High => run(sf.high),
                    SideFacePick::Wider => run(sf.wider()),
                    SideFacePick::Both => run(sf.low).max(run(sf.high)),
                }
            }
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, VGate::Off)
    }
}

impl Default for Window {
    fn default() -> Self {
        Self {
            enabled: false,
            theta_deg: crate::profile::TOP_DEG,
            span_deg: 90.0,
            fade_deg: 12.0,
            invert: false,
            v_gate: VGate::Off,
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
            v_gate: VGate::Off,
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
        let am = if !self.enabled {
            1.0
        } else {
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
        };
        if self.v_gate.is_off() { am } else { am * self.v_gate.mask(uv.v, ctx) }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Layer {
    Tiling(TilingLayer),
    Signet(SignetLayer),
    Border(BorderLayer),
    SeatPad(SeatPadLayer),
    Milgrain(MilgrainLayer),
    /// A nested stack composited to one height, then blended, windowed,
    /// masked and opacity-scaled as a unit. `Replace` inside a group cannot
    /// leak past it.
    Group(GroupLayer),
    /// A wire swept along a drawn path — scrolls, vines, wavy rails.
    Curve(crate::curve::CurveLayer),
    /// Parallel reeds or grooves with exact wall geometry.
    Flutes(FlutesLayer),
    /// Drafted excavation toward a floor over the bore — the pierced look.
    Openwork(OpenworkLayer),
    /// Free-placed motif stamps.
    Decals(DecalLayer),
    /// A row of identical seats — eternity stock.
    SeatRun(SeatRunLayer),
}

/// Nesting deeper than this contributes nothing; the recursion is per sample
/// and a hostile file must not be able to overflow the stack with it.
pub const MAX_GROUP_DEPTH: usize = 8;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GroupLayer {
    pub stack: LayerStack,
    /// The generator recipe this group was made by, when it is *live*: the
    /// editors re-run the generator against the current band whenever the
    /// recipe or the profile changes, and the stack below is its output.
    /// `None` is a plain (or baked) group — the stack is hand-owned. Builds
    /// and analysis never read this: the stored stack is what a file means.
    #[serde(default)]
    pub recipe: Option<crate::pave::GenRecipe>,
}

impl Layer {
    pub fn kind_label(&self) -> &'static str {
        match self {
            Layer::Tiling(_) => "Tiling",
            Layer::Signet(_) => "Signet",
            Layer::Border(_) => "Border",
            Layer::SeatPad(_) => "Gem Seat Pad",
            Layer::Milgrain(_) => "Milgrain",
            Layer::Group(_) => "Group",
            Layer::Curve(_) => "Curve",
            Layer::Flutes(_) => "Flutes",
            Layer::Decals(_) => "Decals",
            Layer::SeatRun(_) => "Seat Run",
            Layer::Openwork(_) => "Openwork",
        }
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        self.height_d(uv, ctx, lib, 0)
    }

    fn height_d(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary, depth: usize) -> f64 {
        match self {
            Layer::Tiling(l) => l.height(uv, ctx, lib),
            Layer::Signet(l) => l.height(uv, ctx),
            Layer::Border(l) => l.height(uv, ctx),
            Layer::SeatPad(l) => l.height(uv, ctx),
            Layer::Milgrain(l) => l.height(uv, ctx),
            Layer::Group(g) => {
                if depth >= MAX_GROUP_DEPTH {
                    return 0.0;
                }
                g.stack.height_d(uv, ctx, lib, depth + 1)
            }
            Layer::Curve(l) => l.height(uv, ctx),
            Layer::Flutes(l) => l.height(uv, ctx),
            Layer::Decals(l) => l.height(uv, ctx, lib),
            Layer::SeatRun(l) => l.height(uv, ctx),
            Layer::Openwork(l) => l.height(uv, ctx, lib),
        }
    }

    /// The layer's finest feature scale and where on the band it lives.
    /// Refinement pre-splits these regions to half the feature scale, so an
    /// error probe cannot step over the feature entirely.
    pub fn feature_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let band = ctx.band_v_len_mm;
        let mirrored = |v: (f64, f64)| (band - v.1, band - v.0);
        match self {
            Layer::Tiling(l) => {
                let (cw, ch) = l.cell_size(ctx);
                vec![FeatureFootprint {
                    feature_u_mm: cw.max(0.1),
                    feature_v_mm: ch.max(0.1),
                    u_mm: None,
                    v_mm: l.v_bounds(),
                }]
            }
            Layer::Border(l) => {
                let v = (l.v_mm - l.width_mm * 0.5, l.v_mm + l.width_mm * 0.5);
                let f = |v| FeatureFootprint::across(l.width_mm.max(0.1), None, v);
                if l.mirror { vec![f(v), f(mirrored(v))] } else { vec![f(v)] }
            }
            Layer::Milgrain(l) => {
                let half = l.bead_diameter_mm * 0.5;
                let v = (l.v_mm - half, l.v_mm + half);
                let f = |v| FeatureFootprint::round(l.bead_diameter_mm.max(0.1), None, v);
                if l.mirror { vec![f(v), f(mirrored(v))] } else { vec![f(v)] }
            }
            Layer::SeatPad(l) => {
                // A marker pad — the sand halo's melee — carries a stone for
                // the report and the preview but raises no metal, so it has
                // no feature for the detail floor or the refiner to find.
                if l.height_mm.abs() <= 1e-9 {
                    return Vec::new();
                }
                let (hu, hv) = l.half_extents_mm();
                let skirt = l.blend_mm.max(0.0);
                let u0 = ctx.u_of_theta(l.theta_deg);
                vec![FeatureFootprint::round(
                    l.blend_mm.clamp(0.15, l.diameter_mm.max(0.15)),
                    Some((u0 - hu - skirt, u0 + hu + skirt)),
                    (l.v_mm - hv - skirt, l.v_mm + hv + skirt),
                )]
            }
            Layer::Signet(l) => {
                let reach = l.reach_mm();
                let u0 = ctx.u_of_theta(l.theta_deg);
                let half_w = l.width_mm * 0.5 + l.shoulder_mm;
                vec![FeatureFootprint::round(
                    l.shoulder_mm.max(0.15),
                    Some((u0 - reach, u0 + reach)),
                    (l.v_mm - half_w, l.v_mm + half_w),
                )]
            }
            Layer::Group(g) => g.stack.feature_footprints(ctx),
            Layer::Curve(l) => l.feature_footprints(ctx),
            Layer::Flutes(l) => vec![FeatureFootprint::across(
                l.width_mm.max(0.1),
                None,
                (0.0, ctx.band_v_len_mm),
            )],
            Layer::Decals(l) => l.feature_footprints(ctx),
            Layer::SeatRun(l) => l.feature_footprints(ctx),
            Layer::Openwork(l) => l.tiling.feature_footprints_as_tiling(ctx),
        }
    }
}

/// Pierced-look carving: the mask's ink is excavated toward a floor that
/// keeps `keep_mm` of metal standing over the finger hole. Walls ramp over
/// the inner tiling's `edge_mm` distance field so they carry the same
/// drafted width at any tile size, and the floor approach is smoothstepped
/// so neither end of the wall creases. The carve depth follows the local
/// base thickness — deep under a crest, shallow at a thin edge — which is
/// what makes the result read as piercing without ever piercing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenworkLayer {
    /// Placement lattice and mask. Its `height_mm` is a 0..1 coverage scale
    /// (leave it 1), and its `edge_mm` is the wall ramp; ink is depth.
    pub tiling: TilingLayer,
    /// Deepest carve along the normal, mm.
    pub depth_mm: f64,
    /// Metal left standing over the bore, mm.
    pub keep_mm: f64,
}

impl OpenworkLayer {
    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        if ctx.bore_radius_mm <= 1e-9 || ctx.surface.is_empty() {
            return 0.0;
        }
        let scale = self.tiling.height_mm;
        if !(scale > 1e-9) {
            return 0.0;
        }
        let cov = (self.tiling.height(uv, ctx, lib) / scale).clamp(0.0, 1.0);
        if cov <= 0.0 {
            return 0.0;
        }
        let Some((r, nr)) = ctx.surface.at(uv.v, ctx.band_v_len_mm) else {
            return 0.0;
        };
        // A carve of depth D along the normal eats D·nr of radial metal, so
        // the floor over the finger hole caps D at radial/nr. On a side face
        // nr is ~0 and the cap opens up — the side-face doctrine's home for
        // deep carves — leaving `depth_mm` as the only limit there.
        let radial = (r - ctx.bore_radius_mm - self.keep_mm.max(0.2)).max(0.0);
        let depth = self.depth_mm.clamp(0.0, 6.0).min(radial / nr.clamp(0.05, 1.0));
        let s = cov * cov * (3.0 - 2.0 * cov);
        -(s * depth)
    }
}

/// One region of the band carrying detail at a known scale, in unrolled mm.
///
/// The two axes are kept apart because they are not the same measure. `v` is
/// arc length on the section itself, so a width across the band is true as it
/// stands; `u` is arc length **at the crest radius**, so a feature running
/// around the ring is shorter in metal wherever the surface sits inside that
/// — 0.80–0.83 of it on a squared band's side faces. Refinement wants the
/// chart figure, because the mesh grid lives in the chart; the sand's detail
/// floor wants the metal one.
#[derive(Clone, Copy, Debug)]
pub struct FeatureFootprint {
    /// Smallest feature measured around the ring, mm of chart `u`. Infinite
    /// when nothing about the layer is fine along `u` — a rail, a bead line.
    pub feature_u_mm: f64,
    /// Smallest feature measured across the section, mm. Infinite when
    /// nothing about the layer is fine across `v`.
    pub feature_v_mm: f64,
    /// Arc extent around the ring, mm at the crest radius; may extend past the
    /// wrap. `None` covers the whole ring.
    pub u_mm: Option<(f64, f64)>,
    /// Extent across the band surface, mm.
    pub v_mm: (f64, f64),
}

impl FeatureFootprint {
    /// A feature the same size both ways — a bead, a stamp, a pad's skirt.
    pub fn round(mm: f64, u_mm: Option<(f64, f64)>, v_mm: (f64, f64)) -> Self {
        Self { feature_u_mm: mm, feature_v_mm: mm, u_mm, v_mm }
    }

    /// A feature measured only across the section — a rail, a flute, a
    /// milgrain line: it runs the whole way round, so `u` does not limit it.
    pub fn across(mm: f64, u_mm: Option<(f64, f64)>, v_mm: (f64, f64)) -> Self {
        Self { feature_u_mm: f64::INFINITY, feature_v_mm: mm, u_mm, v_mm }
    }

    /// The finest feature in the chart, mm — what refinement seeds on.
    pub fn min_feature_mm(&self) -> f64 {
        self.feature_u_mm.min(self.feature_v_mm)
    }

    /// The finest feature in **metal**, mm — what the sand's detail floor
    /// judges, with the `u` side taken at the footprint's own arc scale.
    pub fn metal_feature_mm(&self, ctx: &FieldContext) -> f64 {
        let k = ctx.arc_scale_min(self.v_mm.0, self.v_mm.1);
        (self.feature_u_mm * k).min(self.feature_v_mm)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayerEntry {
    pub name: String,
    pub enabled: bool,
    pub blend: Blend,
    /// Overall scale on this layer's output, 0..1+.
    pub opacity: f64,
    /// Fillet radius for the smooth blend modes, mm.
    #[serde(default = "default_soft_mm")]
    pub soft_mm: f64,
    /// Angular gate. Disabled by default, so the layer runs the whole way round.
    #[serde(default)]
    pub window: Window,
    /// Alpha multiplied into the mask, sampled over the whole unrolled band —
    /// the freeform counterpart to the window. Painted or imported like any
    /// other alpha; a missing name passes 1.0 rather than silencing the layer.
    #[serde(default)]
    pub mask: Option<String>,
    /// Relief reshaping applied to the layer's output before opacity.
    #[serde(default)]
    pub remap: Remap,
    pub layer: Layer,
}

fn default_soft_mm() -> f64 {
    0.3
}

impl LayerEntry {
    pub fn new(name: impl Into<String>, layer: Layer) -> Self {
        Self {
            name: name.into(),
            enabled: true,
            blend: Blend::Max,
            opacity: 1.0,
            soft_mm: default_soft_mm(),
            window: Window::default(),
            mask: None,
            remap: Remap::Off,
            layer,
        }
    }

    /// Mask strength at a point: window times painted mask.
    pub fn mask_at(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        let mut w = self.window.mask(uv, ctx);
        if w <= 0.0 {
            return w;
        }
        if let Some(name) = &self.mask
            && let Some(a) = lib.get(name)
        {
            let x = uv.u / ctx.circumference_mm.max(1e-9);
            let y = uv.v / ctx.band_v_len_mm.max(1e-9);
            w *= a.sample_wrapped(x, y.clamp(0.0, 1.0)) as f64;
        }
        w
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
        self.height_d(uv, ctx, lib, 0)
    }

    fn height_d(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary, depth: usize) -> f64 {
        let mut acc = 0.0;
        for e in &self.layers {
            if !e.enabled {
                continue;
            }
            // A gated-out layer takes no part in the blend at all, so Replace
            // outside its window cannot wipe the layers under it.
            let w = e.mask_at(uv, ctx, lib);
            if w <= 0.0 {
                continue;
            }
            let h = e.remap.apply(e.layer.height_d(uv, ctx, lib, depth)) * e.opacity * w;
            acc = e.blend.apply(acc, h, e.soft_mm);
        }
        acc
    }

    pub fn is_empty(&self) -> bool {
        self.layers.iter().all(|l| !l.enabled)
    }

    /// Where each enabled layer's finest detail lives, for refinement seeding.
    /// Windows widen the arc by their fade; an inverted window covers the ring.
    pub fn feature_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let mut out = Vec::new();
        for e in &self.layers {
            if !e.enabled || e.opacity <= 0.0 {
                continue;
            }
            let u_mm = match (&e.window.enabled, e.window.invert) {
                (true, false) => {
                    let half = e.window.span_deg * 0.5 + e.window.fade_deg;
                    let to_u = |deg: f64| deg / 360.0 * ctx.circumference_mm;
                    Some((to_u(e.window.theta_deg - half), to_u(e.window.theta_deg + half)))
                }
                _ => None,
            };
            for mut f in e.layer.feature_footprints(ctx) {
                f.u_mm = match (f.u_mm, u_mm) {
                    (Some(a), _) => Some(a),
                    (None, w) => w,
                };
                out.push(f);
            }
        }
        out
    }

    /// Names of every alpha the stack samples, groups included, deduplicated,
    /// order preserved.
    pub fn referenced_alphas(&self) -> Vec<&str> {
        fn collect<'s>(stack: &'s LayerStack, out: &mut Vec<&'s str>) {
            for e in &stack.layers {
                let tile = match &e.layer {
                    Layer::Tiling(t) => Some(t.alpha.as_str()),
                    Layer::Decals(d) => Some(d.alpha.as_str()),
                    _ => None,
                };
                for name in tile.into_iter().chain(e.mask.as_deref()) {
                    if !out.contains(&name) {
                        out.push(name);
                    }
                }
                if let Layer::Group(g) = &e.layer {
                    collect(&g.stack, out);
                }
            }
        }
        let mut out = Vec::new();
        collect(self, &mut out);
        out
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

/// What the cast stock for a stone looks like before the bench work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeatStyle {
    /// A raised boss the jeweller drills and cuts a seat into.
    #[default]
    Boss,
    /// An annular collar around a recessed pocket, burnished over the girdle
    /// at the bench. The pocket is a cup, so it belongs on a side face; on
    /// the crest its walls turn to ceilings.
    Bezel,
    /// A tall rounded mound for a flush ("gypsy") setting.
    GypsyMound,
}

impl SeatStyle {
    pub const ALL: &'static [SeatStyle] =
        &[SeatStyle::Boss, SeatStyle::Bezel, SeatStyle::GypsyMound];

    pub fn label(self) -> &'static str {
        match self {
            SeatStyle::Boss => "Boss",
            SeatStyle::Bezel => "Bezel collar",
            SeatStyle::GypsyMound => "Gypsy mound",
        }
    }
}

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
    #[serde(default)]
    pub style: SeatStyle,
    /// Bezel only: collar wall thickness, mm.
    #[serde(default = "default_bezel_wall")]
    pub bezel_wall_mm: f64,
    /// Bezel only: pocket depth below the rim, mm.
    #[serde(default = "default_recess")]
    pub recess_mm: f64,
    /// Drafted cone bumps on the seat circle — cast oversize, notched and
    /// shaped at the bench, which is how prongs survive sand. 0 is none.
    #[serde(default)]
    pub prongs: u32,
    /// Height a prong stands above the pad top, mm.
    #[serde(default = "default_prong")]
    pub prong_mm: f64,
    /// The stone the pad was sized for, carried for the report and preview.
    #[serde(default)]
    pub gem: Option<crate::gem::Gem>,
    /// Centre dimple diameter, mm — a cast guide for the setting bur, so
    /// the drill starts where the seat means it to. 0 is none.
    #[serde(default)]
    pub dimple_mm: f64,
    /// Long axis over short, 1 = round. `diameter_mm` stays the short axis,
    /// so an oval, marquise, emerald or baguette seat carries the stock its
    /// own girdle needs instead of a circle drawn round its length.
    #[serde(default = "default_elong")]
    pub elong: f64,
    /// Rotation of the long axis about the seat normal, degrees. 0 lays it
    /// along the ring; 90 lays it across the band.
    #[serde(default)]
    pub rot_deg: f64,
    /// Superellipse exponent of the rim in plan: 2 is an ellipse, 6 a
    /// rounded rectangle for the step cuts, 1.5 the pointed ends of a
    /// marquise. Read from the stone by [`fit_stone`](Self::fit_stone).
    /// Floored at 1, which keeps the plan convex.
    #[serde(default = "default_plan_pow")]
    pub plan_pow: f64,
    /// How far the girdle sits below the pad's top, mm — how deep the stone
    /// is set. `None` takes the style's own: a bezel's pocket floor, a
    /// whisker into a drilled pad, flush on a cabochon's bed.
    ///
    /// One number for a thing the model used to hold in two places that
    /// disagreed: the preview sank a stone by a fraction of its depth while
    /// the report credited the whole pad height as metal under it.
    #[serde(default)]
    pub set_depth_mm: Option<f64>,
}

fn default_elong() -> f64 {
    1.0
}

fn default_plan_pow() -> f64 {
    2.0
}

fn default_bezel_wall() -> f64 {
    0.5
}

fn default_recess() -> f64 {
    // Calibrated against the 16 CrossGems factory bezel presets, whose
    // "Stone Seat Depth" — the same quantity, where the girdle ledge sits
    // below the rim — runs 0.525-0.6125 mm. 0.5 is their low end; a cast
    // pocket the bench opens wants no more.
    0.5
}

fn default_prong() -> f64 {
    0.9
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
            style: SeatStyle::Boss,
            bezel_wall_mm: default_bezel_wall(),
            recess_mm: default_recess(),
            prongs: 0,
            prong_mm: default_prong(),
            gem: None,
            dimple_mm: 0.0,
            elong: default_elong(),
            rot_deg: 0.0,
            plan_pow: default_plan_pow(),
            set_depth_mm: None,
        }
    }
}

/// `2 atan(c tan(x/2))`, in the branch-safe form — exactly `±π` at `x = ±π`
/// with no clamp and no case split.
///
/// The eccentric-anomaly substitution: a monotone, odd reparameterization of
/// the circle onto itself, the identity at `c = 1`, and the closed-form
/// integral of `dΔ / (A + B cos Δ)`.
fn eccentric_warp(x: f64, c: f64) -> f64 {
    let (sin, cos) = x.sin_cos();
    let (a, b) = (2.0 * c * sin, (1.0 + cos) - c * c * (1.0 - cos));
    a.atan2(b)
}

/// The radius of a superellipse with semi-axes `ra`, `rb` and exponent `n`,
/// in the direction of `(a, b)` — the same plan a girdle and the stock cut
/// for it both read, so a seat and the stone in it are never two shapes.
///
/// `n` is floored at 1, which keeps the outline convex: convex means
/// star-shaped about the centre, which is what makes a mound built on it a
/// monotone drop in every direction and so castable wherever a round one is.
pub fn superellipse_radius_mm(a: f64, b: f64, ra: f64, rb: f64, n: f64) -> f64 {
    let (ra, rb) = (ra.max(1e-9), rb.max(1e-9));
    let n = n.max(1.0);
    if ra <= rb * (1.0 + 1e-12) && (n - 2.0).abs() < 1e-12 {
        return rb;
    }
    let d = (a * a + b * b).sqrt();
    if d <= 1e-12 {
        return rb;
    }
    let t = ((a / ra).abs().powf(n) + (b / rb).abs().powf(n)).powf(1.0 / n);
    d / t.max(1e-12)
}

/// Half-extents of a superellipse plan along `u` and across `v`, mm, after
/// turning it by `rot_deg` in the chart.
///
/// Exact at any bearing: a superellipse is the unit ball of a weighted
/// `p`-norm, so its support function is the dual `q`-norm with
/// `1/p + 1/q = 1`. At `p = 2` that is the familiar ellipse formula; as `p`
/// grows the dual runs to `q = 1` and the answer becomes the rotated
/// rectangle's, which is what a step cut wants.
pub fn plan_half_extents_mm(ra: f64, rb: f64, n: f64, rot_deg: f64) -> (f64, f64) {
    if rot_deg == 0.0 {
        return (ra, rb);
    }
    let (sin, cos) = rot_deg.to_radians().sin_cos();
    let (ca, sa) = (cos.abs(), sin.abs());
    let n = n.max(1.0);
    if n <= 1.0 + 1e-9 {
        // The dual of the diamond is the box: the extent is whichever
        // vertex reaches furthest.
        return ((ra * ca).max(rb * sa), (ra * sa).max(rb * ca));
    }
    let q = n / (n - 1.0);
    let dual = |x: f64, y: f64| (x.powf(q) + y.powf(q)).powf(1.0 / q);
    (dual(ra * ca, rb * sa), dual(ra * sa, rb * ca))
}

/// The stone's own half-extents in the chart, mm — its girdle plan turned
/// by the seat that holds it.
pub fn gem_half_extents_mm(gem: crate::gem::Gem, rot_deg: f64) -> (f64, f64) {
    plan_half_extents_mm(gem.l_mm * 0.5, gem.w_mm * 0.5, gem.cut.plan_pow(), rot_deg)
}

impl SeatPadLayer {
    /// Diameter of the largest stone this pad can reasonably seat, mm.
    pub fn suggested_stone_mm(&self) -> f64 {
        match self.style {
            SeatStyle::Bezel => (self.diameter_mm - 2.0 * self.bezel_wall_mm).max(0.5),
            _ => (self.diameter_mm - 1.2).max(0.5),
        }
    }

    /// Stock allowance around the girdle, mm — the metal the bench cuts away.
    fn stock_mm(&self) -> f64 {
        match self.style {
            SeatStyle::Bezel => 2.0 * self.bezel_wall_mm.max(0.2),
            SeatStyle::Boss => 1.2,
            SeatStyle::GypsyMound => 1.8,
        }
    }

    /// Size the pad for a chosen stone instead of inferring the stone from
    /// the pad — a bezel needs its walls around the girdle, a boss needs its
    /// drilling allowance around the seat.
    ///
    /// The allowance is a constant width all round, so an elongated stone's
    /// pad is *less* elongated than the stone: `(l + stock) / (w + stock)`.
    pub fn fit_stone(&mut self, gem: crate::gem::Gem) {
        let w = gem.w_mm.max(0.5);
        let l = gem.l_mm.max(w);
        let stock = self.stock_mm();
        self.diameter_mm = w + stock;
        self.elong = (l + stock) / (w + stock);
        self.plan_pow = gem.cut.plan_pow();
        if self.style == SeatStyle::GypsyMound {
            self.crown = 1.0;
        }
        self.gem = Some(gem);
    }

    /// Semi-axes of the pad's rim, mm: along its long axis, then across it.
    pub fn semi_axes_mm(&self) -> (f64, f64) {
        let rb = (self.diameter_mm * 0.5).max(1e-6);
        (rb * self.elong.max(1.0), rb)
    }

    /// A sample's offset in the pad's own frame: along the long axis, then
    /// across it.
    fn pad_frame(&self, du: f64, dv: f64) -> (f64, f64) {
        if self.rot_deg == 0.0 {
            return (du, dv);
        }
        let (sin, cos) = self.rot_deg.to_radians().sin_cos();
        (du * cos + dv * sin, -du * sin + dv * cos)
    }

    /// Rim radius in the direction of a pad-frame offset, mm — the plan
    /// outline's own radius along that ray, and a constant for a round pad.
    fn rim_mm(&self, a: f64, b: f64) -> f64 {
        let (ra, rb) = self.semi_axes_mm();
        superellipse_radius_mm(a, b, ra, rb, self.plan_pow)
    }

    /// The furthest the rim reaches from the seat centre, mm. A bound, and
    /// exact for the ellipse family: a plan squarer than an ellipse pushes
    /// its corners past the long semi-axis, out to the box diagonal.
    fn plan_reach_mm(&self) -> f64 {
        let (ra, rb) = self.semi_axes_mm();
        if self.plan_pow <= 2.0 + 1e-12 {
            ra
        } else {
            (ra * ra + rb * rb).sqrt()
        }
    }

    /// Half-extents of the rim along `u` and across `v`, mm — what the band
    /// edge, the bridge between neighbours and the refiner all measure
    /// against.
    pub fn half_extents_mm(&self) -> (f64, f64) {
        let (ra, rb) = self.semi_axes_mm();
        plan_half_extents_mm(ra, rb, self.plan_pow, self.rot_deg)
    }

    /// The stone's own half-reach across the band, mm — its width when it
    /// lies along the ring, its length when the seat turns it across.
    pub fn stone_half_v_mm(&self, gem: crate::gem::Gem) -> f64 {
        gem_half_extents_mm(gem, self.rot_deg).1
    }

    /// How far the girdle sits below the pad's top, mm. The one number the
    /// preview, the pavilion check and the spacing census all read, so the
    /// stone they each mean is the same stone.
    pub fn girdle_drop_mm(&self, gem: crate::gem::Gem) -> f64 {
        let d = self.set_depth_mm.unwrap_or(match (gem.form, self.style) {
            // A cabochon is flat-backed: it rests on the surface it is given.
            (crate::gem::GemForm::Cabochon, _) => 0.0,
            // A bezel's pocket floor is where the girdle lands, by
            // construction — the collar is then burnished over it.
            (_, SeatStyle::Bezel) => self.recess_mm.max(0.0),
            // A drilled pad takes the stone a whisker in, so the pavilion
            // disappears into metal and the crown stands proud.
            _ => 0.22 * gem.depth_mm(),
        });
        d.clamp(0.0, self.height_mm.max(0.0))
    }

    /// Height of the girdle over the bare band, mm — the pad's own stand-off
    /// less how deep the stone is set into it.
    pub fn stand_off_mm(&self, gem: crate::gem::Gem) -> f64 {
        (self.height_mm - self.girdle_drop_mm(gem)).max(0.0)
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let blend = self.blend_mm.max(0.0);
        let u0 = ctx.u_of_theta(self.theta_deg);
        let du = wrap_delta(uv.u - u0, ctx.circumference_mm);
        let dv = uv.v - self.v_mm;
        let d = (du * du + dv * dv).sqrt();
        let (_, rb) = self.semi_axes_mm();
        let skirt = if self.style == SeatStyle::GypsyMound { blend.max(0.3) } else { blend };
        let prong_pad =
            if self.prongs > 0 { (0.28 * rb).clamp(0.35, 0.8) * 0.4 } else { 0.0 };
        if d > self.plan_reach_mm() + skirt + prong_pad {
            return 0.0;
        }
        // The rim radius along this ray. Every law below is the same
        // one-dimensional drop it always was, read at `d / r`, so an
        // elongated pad is monotone from its centre in every direction
        // exactly as a round one is.
        let (a, b) = self.pad_frame(du, dv);
        let r = self.rim_mm(a, b);

        let body = match self.style {
            SeatStyle::Boss => {
                let crown = self.crown.clamp(0.0, 1.0);
                if d <= r {
                    let t = d / r;
                    let dome = (1.0 - t * t).max(0.0).sqrt();
                    let flat = 1.0 - smoothstep(0.82, 1.0, t);
                    self.height_mm * ((1.0 - crown) * flat + crown * dome)
                } else {
                    self.skirt(d, r, blend)
                }
            }
            SeatStyle::GypsyMound => {
                // A cosine mound over the whole reach: finite edge slope,
                // flush look.
                let reach = r + blend.max(0.3);
                let t = (d / reach).clamp(0.0, 1.0);
                self.height_mm * (0.5 + 0.5 * (std::f64::consts::PI * t).cos())
            }
            SeatStyle::Bezel => {
                let wall = self.bezel_wall_mm.clamp(0.2, r);
                let r_in = (r - wall).max(0.1);
                let floor = (self.height_mm - self.recess_mm.max(0.0)).max(0.1);
                // Pocket walls drafted over a quarter of the collar wall.
                let soft = (wall * 0.35).max(0.08);
                if d <= r {
                    let rim = self.height_mm;
                    rim + (floor - rim) * (1.0 - smoothstep(r_in - soft, r_in, d))
                } else {
                    self.skirt(d, r, blend)
                }
            }
        };

        // The bur guide: a shallow centre dimple so the drill starts true.
        // Carved from the body, capped so it cannot punch through the pad.
        let body = match self.dimple_mm.clamp(0.0, self.diameter_mm) {
            dim if dim > 0.05 => {
                let dr = dim * 0.5;
                let t = (d / dr).clamp(0.0, 1.0);
                let depth = (0.18f64).min(self.height_mm * 0.4);
                body - depth * (0.5 + 0.5 * (std::f64::consts::PI * t).cos())
            }
            _ => body,
        };

        let n = self.prongs.min(8);
        if n == 0 {
            return body;
        }
        // Drafted cone bumps standing just inside the rim, evenly spaced
        // round it — on the plan outline itself, so a turned or elongated
        // seat carries its claws where its girdle is.
        let prong_r = (0.28 * rb).clamp(0.35, 0.8);
        let step = std::f64::consts::TAU / n as f64;
        let mut prong: f64 = 0.0;
        for k in 0..n {
            let (sin, cos) = (k as f64 * step).sin_cos();
            let seat_r = (self.rim_mm(cos, sin) - prong_r * 0.6).max(0.2);
            let dp = ((a - seat_r * cos).powi(2) + (b - seat_r * sin).powi(2)).sqrt();
            let t = (dp / prong_r).clamp(0.0, 1.0);
            prong = prong.max(
                (self.height_mm + self.prong_mm) * (0.5 + 0.5 * (std::f64::consts::PI * t).cos()),
            );
        }
        body.max(prong)
    }

    /// The fairing outside the pad's rim shared by the flat-rimmed styles.
    fn skirt(&self, d: f64, r: f64, blend: f64) -> f64 {
        if blend <= 1e-9 || d > r + blend {
            return 0.0;
        }
        let rim = match self.style {
            SeatStyle::Boss => self.height_mm * (1.0 - self.crown.clamp(0.0, 1.0)),
            _ => self.height_mm,
        };
        rim * (1.0 - smoothstep(0.0, 1.0, (d - r) / blend))
    }
}

// --- Height remap ----------------------------------------------------------

/// Reshapes a layer's relief profile after evaluation — the pro-CAD "relief
/// curve": cushioned tops, chamfered take-offs, terraced steps. Applied before
/// opacity and the window fade, so gating still fades the finished shape.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Remap {
    Off,
    /// Monotone curve over the layer's output, normalized by `span_mm`.
    Curve { curve: crate::profile::DropCurve, span_mm: f64 },
    /// Flat treads of `span_mm / steps`, each riser spending `riser` of its
    /// tread rising. A riser near zero is a wall; the editor floors it.
    Terrace { steps: u32, span_mm: f64, riser: f64 },
}

impl Default for Remap {
    fn default() -> Self {
        Remap::Off
    }
}

impl Remap {
    pub fn apply(&self, h: f64) -> f64 {
        if h <= 0.0 {
            return h;
        }
        match *self {
            Remap::Off => h,
            Remap::Curve { curve, span_mm } => {
                let span = span_mm.max(1e-6);
                span * curve.eval((h / span).clamp(0.0, 1.0))
            }
            Remap::Terrace { steps, span_mm, riser } => {
                let span = span_mm.max(1e-6);
                let q = span / steps.clamp(1, 64) as f64;
                let t = (h / q).min(steps as f64);
                let cell = t.floor();
                let r = riser.clamp(0.05, 1.0);
                q * (cell + smoothstep(1.0 - r, 1.0, t - cell))
            }
        }
    }

    pub fn is_off(&self) -> bool {
        matches!(self, Remap::Off)
    }

    /// Rounded top: the relief rises fast and eases into its full height.
    pub fn cushion(span_mm: f64) -> Self {
        let curve = crate::profile::DropCurve::from_points(&[
            [0.0, 0.0],
            [0.35, 0.62],
            [0.72, 0.92],
            [1.0, 1.0],
        ]);
        Remap::Curve { curve, span_mm }
    }

    /// Chamfered take-off: low detail is suppressed, then a straight rise.
    pub fn chamfer(span_mm: f64) -> Self {
        let curve = crate::profile::DropCurve::from_points(&[
            [0.0, 0.0],
            [0.28, 0.05],
            [0.75, 0.7],
            [1.0, 1.0],
        ]);
        Remap::Curve { curve, span_mm }
    }
}

// --- Decals ----------------------------------------------------------------

/// Instances beyond this are ignored; each is evaluated per sample.
pub const MAX_DECALS: usize = 64;

/// One free-placed stamp of the layer's alpha.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Decal {
    /// Position around the ring, degrees.
    pub theta_deg: f64,
    /// Position across the band, mm.
    pub v_mm: f64,
    /// Footprint along the stamp's own width, mm; the height follows the
    /// alpha's aspect.
    pub size_mm: f64,
    pub rotation_deg: f64,
    pub height_mm: f64,
    /// Mirror the stamp left-to-right.
    pub flip: bool,
}

impl Default for Decal {
    fn default() -> Self {
        Self {
            theta_deg: crate::profile::TOP_DEG,
            v_mm: 0.0,
            size_mm: 4.0,
            rotation_deg: 0.0,
            height_mm: 0.35,
            flip: false,
        }
    }
}

/// Free-placed motif stamps — the compositions a lattice cannot express: one
/// scroll per shoulder at different sizes, a scatter of stars, an off-axis
/// crest. Instances overlap by max, and positions wrap with the ring.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecalLayer {
    /// Alpha sampled by every stamp.
    pub alpha: String,
    pub decals: Vec<Decal>,
    /// Edge fade inside each stamp's border, mm, so no stamp ends in a wall.
    pub feather_mm: f64,
    pub invert: bool,
}

impl Default for DecalLayer {
    fn default() -> Self {
        Self {
            alpha: String::new(),
            decals: vec![Decal::default()],
            feather_mm: 0.3,
            invert: false,
        }
    }
}

impl DecalLayer {
    pub fn height(&self, uv: Uv, ctx: &FieldContext, lib: &AlphaLibrary) -> f64 {
        let Some(a) = lib.get(&self.alpha) else { return 0.0 };
        if a.is_empty() || !uv.u.is_finite() || !uv.v.is_finite() {
            return 0.0;
        }
        let aspect = a.height as f64 / a.width.max(1) as f64;
        let mut h: f64 = 0.0;
        for d in self.decals.iter().take(MAX_DECALS) {
            let w = d.size_mm.max(1e-6);
            let hh = w * aspect;
            let du = wrap_delta(uv.u - ctx.u_of_theta(d.theta_deg), ctx.circumference_mm);
            let dv = uv.v - d.v_mm;
            // Quick reject on the rotation-proof bounding circle.
            let reach = 0.5 * (w * w + hh * hh).sqrt();
            if du * du + dv * dv > reach * reach {
                continue;
            }
            let (sin, cos) = d.rotation_deg.to_radians().sin_cos();
            let mut lx = du * cos + dv * sin;
            let ly = -du * sin + dv * cos;
            if d.flip {
                lx = -lx;
            }
            let x = lx / w + 0.5;
            let y = 0.5 - ly / hh;
            if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
                continue;
            }
            let mut s = a.sample(x, y) as f64;
            if self.invert {
                s = 1.0 - s;
            }
            // Fade to nothing at the stamp's border.
            let edge = (lx.abs() - 0.5 * w).abs().min((ly.abs() - 0.5 * hh).abs());
            let feather = self.feather_mm.max(1e-6);
            s *= (edge / feather).clamp(0.0, 1.0);
            h = h.max(d.height_mm * s.clamp(0.0, 1.0));
        }
        h
    }

    pub fn feature_footprints(&self, ctx: &FieldContext) -> Vec<FeatureFootprint> {
        self.decals
            .iter()
            .take(MAX_DECALS)
            .map(|d| {
                let u0 = ctx.u_of_theta(d.theta_deg);
                let reach = d.size_mm;
                FeatureFootprint::round(
                    (d.size_mm * 0.15).max(0.15),
                    Some((u0 - reach, u0 + reach)),
                    (d.v_mm - reach, d.v_mm + reach),
                )
            })
            .collect()
    }
}

// --- Flutes ----------------------------------------------------------------

/// Cross-profile of one flute. Round is a cosine dome, so a flute wall's
/// angle is bounded by its width-to-depth ratio rather than going vertical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FluteProfile {
    Round,
    Vee,
    Square,
}

impl FluteProfile {
    pub const ALL: &'static [FluteProfile] =
        &[FluteProfile::Round, FluteProfile::Vee, FluteProfile::Square];

    pub fn label(self) -> &'static str {
        match self {
            FluteProfile::Round => "Round",
            FluteProfile::Vee => "V-cut",
            FluteProfile::Square => "Square",
        }
    }

    fn shape(self, x: f64) -> f64 {
        let x = x.abs().clamp(0.0, 1.0);
        match self {
            FluteProfile::Round => 0.5 + 0.5 * (std::f64::consts::PI * x).cos(),
            FluteProfile::Vee => 1.0 - x,
            FluteProfile::Square => 1.0 - smoothstep(0.55, 0.95, x),
        }
    }
}

/// Parallel flutes with exact wall geometry: reeding when blended `Max`,
/// coin-edge grooves when blended `Carve`. The count is an integer so the
/// pattern closes on itself.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FlutesLayer {
    pub count: u32,
    pub profile: FluteProfile,
    /// Width of one flute, mm; the rest of the cell stays bare band.
    pub width_mm: f64,
    pub height_mm: f64,
    /// Cells of sideways drift across the band's full width — diagonal
    /// reeding. Any value keeps the joint closed; only the count must be
    /// an integer.
    pub lean: f64,
    /// Run the flutes around the ring instead of across it (melon lobes).
    /// The count then spans the band and does not need to wrap.
    pub along: bool,
}

impl Default for FlutesLayer {
    fn default() -> Self {
        Self {
            count: 96,
            profile: FluteProfile::Round,
            width_mm: 0.5,
            height_mm: 0.18,
            lean: 0.0,
            along: false,
        }
    }
}

impl FlutesLayer {
    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let n = self.count.clamp(1, 1024) as f64;
        let (circ, band) = (ctx.circumference_mm, ctx.band_v_len_mm);
        if !(circ > 1e-9) || !(band > 1e-9) || !uv.u.is_finite() || !uv.v.is_finite() {
            return 0.0;
        }
        let (phase, cell_mm) = if self.along {
            (uv.v / band * n + self.lean * (uv.u / circ), band / n)
        } else {
            (uv.u / circ * n + self.lean * (uv.v / band), circ / n)
        };
        let t = phase - phase.floor();
        let half = (self.width_mm.min(cell_mm).max(1e-6)) * 0.5;
        let d = (t - 0.5) * cell_mm;
        if d.abs() >= half {
            return 0.0;
        }
        (self.height_mm * self.profile.shape(d / half)).max(0.0)
    }
}

// --- Seat run ---------------------------------------------------------------

/// A row of identical seats around the ring — eternity and half-eternity
/// stock. The count is an integer so the row closes on itself; a half row is
/// the same layer behind an angular window.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SeatRunLayer {
    /// The seat repeated at every station. Its own `theta_deg` is ignored.
    pub seat: SeatPadLayer,
    pub count: u32,
    /// The stone each seat holds, for spacing and the report.
    pub gem: crate::gem::Gem,
    /// Metal left between neighbouring stones, mm.
    pub bridge_mm: f64,
    /// Graduation: how far the stones shrink toward the far side of the
    /// ring, 0..0.85 of the full size. 0 keeps every station identical;
    /// 0.4 reads as the classic graduated eternity. Seats scale with their
    /// stones, and the report carries the graded sizes.
    #[serde(default)]
    pub taper: f64,
    /// Where the largest stone sits, degrees. 90 is the top of the ring.
    #[serde(default = "default_taper_theta")]
    pub taper_theta_deg: f64,
    /// Shared-prong posts: one pair at each boundary between neighbouring
    /// stones, straddling the stone column, cut for both stones at once.
    /// Height proud of the seat stock, mm; 0 = none. Posts follow the
    /// graduation like their stones. A full-ring run keeps every boundary;
    /// an arc window fades boundary posts with everything else.
    ///
    /// Lost-wax stock: proud posts flank the column off the parting plane
    /// and lean under a two-part pull — measured 2.8–3.0% at −62°,
    /// converging, on a low dome (`examples/prong_probe.rs`). In sand keep
    /// 0 and bead-set from the cast surface.
    #[serde(default)]
    pub shared_prong_mm: f64,
    /// Every stone turned in plan by this bearing, degrees, on top of the
    /// seat's own: 45 sets a square stone on the diagonal. A turned convex
    /// plan is still one monotone mound, so the row pulls wherever it did;
    /// it re-packs to the reach it has.
    #[serde(default)]
    pub tilt_deg: f64,
}

fn default_taper_theta() -> f64 {
    crate::profile::TOP_DEG
}

impl Default for SeatRunLayer {
    fn default() -> Self {
        // Gypsy mounds, not flat bosses: a row of flat-topped pads undercuts
        // at its rims where they reach onto the dome flank — measured 8.6% at
        // -51 degrees — while a cosine-mound row measures 0.000%. The bench
        // drills the seats into the mounds either way.
        let gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 2.5);
        let mut seat = SeatPadLayer {
            style: SeatStyle::GypsyMound,
            height_mm: 0.6,
            crown: 1.0,
            blend_mm: 0.5,
            ..Default::default()
        };
        seat.fit_stone(gem);
        Self {
            seat,
            count: 18,
            gem,
            bridge_mm: 0.4,
            taper: 0.0,
            taper_theta_deg: default_taper_theta(),
            shared_prong_mm: 0.0,
            tilt_deg: 0.0,
        }
    }
}

impl SeatRunLayer {
    /// A seat as the row sets it: turned by the tilt.
    pub fn turned(&self, mut seat: SeatPadLayer) -> SeatPadLayer {
        seat.rot_deg += self.tilt_deg;
        seat
    }

    /// Solve the count from the stone: fit the seat first, then take the most
    /// stations of that seat plus the bridge that fit the ring.
    pub fn solve_spacing(&mut self, ctx: &FieldContext) {
        self.seat.fit_stone(self.gem);
        // Solved in metal, not in chart arc: the seat's own span shrinks with
        // the row's radius exactly as the pitch does, so only the bridge is
        // an absolute. Keeps `bridge_at` at the asked-for figure, which is
        // the invariant the two have to share.
        //
        // Graded, the pitch is not one number — it runs from `span + bridge`
        // at the large pole to `span(1-t) + bridge` at the small one — and
        // the count the constant-bridge law asks for is the circumference
        // over their **geometric** mean.
        let k = ctx.arc_scale(self.seat.v_mm);
        let (near, far) = self.pitch_poles(k);
        self.count = ((ctx.circumference_mm * k / (near * far).sqrt()).floor() as u32)
            .clamp(3, 200);
    }

    /// The pitch the constant-bridge law wants at each pole, mm of metal:
    /// at the largest stone, then at the smallest.
    fn pitch_poles(&self, k: f64) -> (f64, f64) {
        let span = self.seat_span_mm().max(0.5) * k;
        let bridge = self.bridge_mm.max(0.0);
        let t = self.taper.clamp(0.0, 0.85);
        ((span + bridge).max(1e-6), (span * (1.0 - t) + bridge).max(1e-6))
    }

    /// The station-spacing warp's own constant: 1 for an ungraded row.
    fn spacing_c(&self, k: f64) -> f64 {
        let (near, far) = self.pitch_poles(k);
        (far / near).sqrt()
    }

    /// The ring angle station `k` stands at, degrees.
    ///
    /// Stations are evenly spaced in a warped angle, not in theta. A graded
    /// row's seats shrink toward the far pole, so a uniform angular pitch
    /// leaves the metal between them growing with every step — measured
    /// 0.44 mm at the large pole against 3.20 mm at the small one on a
    /// taper-0.85 row, a sevenfold spread down what is meant to read as one
    /// continuous line of stones.
    ///
    /// Holding the *bridge* constant instead makes `R dΔ = span·scale(Δ) +
    /// bridge`, and `scale_at` is exactly a raised cosine in Δ, so this is
    /// `R dΔ = A + B cos Δ` — which integrates in closed form by the
    /// eccentric-anomaly substitution. No solver, no iteration, and at
    /// `taper = 0` the warp is the identity, so every ungraded row is
    /// bit-identical.
    pub fn theta_of_station(&self, k: f64, ctx: &FieldContext) -> f64 {
        let n = self.count.clamp(1, 200) as f64;
        if self.taper <= 0.0 {
            return k * 360.0 / n;
        }
        let c = self.spacing_c(ctx.arc_scale(self.seat.v_mm));
        let phi = self.station_phase(c) + k * std::f64::consts::TAU / n;
        self.taper_theta_deg + eccentric_warp(phi, 1.0 / c.max(1e-9)).to_degrees()
    }

    /// The (fractional) station standing at a ring angle — the inverse of
    /// [`theta_of_station`](Self::theta_of_station).
    pub fn station_of_theta(&self, theta_deg: f64, ctx: &FieldContext) -> f64 {
        let n = self.count.clamp(1, 200) as f64;
        if self.taper <= 0.0 {
            return theta_deg / 360.0 * n;
        }
        let c = self.spacing_c(ctx.arc_scale(self.seat.v_mm));
        let d = wrap_delta(theta_deg - self.taper_theta_deg, 360.0).to_radians();
        (eccentric_warp(d, c) - self.station_phase(c)) / std::f64::consts::TAU * n
    }

    /// Where station 0 sits in the warped angle. Anchored so that ungrading
    /// a row puts its stations back at `k · 360/n` exactly, rather than
    /// sliding the whole lattice onto the taper's centre.
    fn station_phase(&self, c: f64) -> f64 {
        eccentric_warp(wrap_delta(-self.taper_theta_deg, 360.0).to_radians(), c)
    }

    /// The seat's own reach along the ring, mm — its full width for a round
    /// seat, its rotated ellipse's `u` extent for an elongated one. A row of
    /// baguettes laid along the band packs by their length, not their width.
    pub fn seat_span_mm(&self) -> f64 {
        self.turned(self.seat).half_extents_mm().0 * 2.0
    }

    /// Bridge actually left between neighbouring seats, mm of metal.
    ///
    /// The pitch and the seat's span both scale by the row's own
    /// [`arc_scale`](FieldContext::arc_scale), so the chart figure times `k`
    /// is the real one exactly — one multiply, not an approximation.
    ///
    /// Graded, the bridge is a constant the station warp holds, and the
    /// count the ring rounded to decides what constant: it is the positive
    /// root of `b² + b·span(2−t) + span²(1−t) = (C/n)²`, which is the same
    /// geometric-mean identity `solve_spacing` runs forward.
    pub fn bridge_at(&self, ctx: &FieldContext) -> f64 {
        let k = ctx.arc_scale(self.seat.v_mm);
        let pitch = ctx.circumference_mm * k / self.count.max(1) as f64;
        let span = self.seat_span_mm().max(0.5) * k;
        let t = self.taper.clamp(0.0, 0.85);
        if t <= 0.0 {
            return pitch - span;
        }
        let (b, c) = (span * (2.0 - t), span * span * (1.0 - t) - pitch * pitch);
        0.5 * (-b + (b * b - 4.0 * c).max(0.0).sqrt())
    }

    /// Size factor at a ring angle: 1 at [`taper_theta_deg`](Self::taper_theta_deg),
    /// falling smoothly to `1 - taper` at the far side. Cosine in the angular
    /// distance, so a full-ring run stays seamless and C1 at both poles.
    pub fn scale_at(&self, theta_deg: f64) -> f64 {
        let t = self.taper.clamp(0.0, 0.85);
        if t <= 0.0 {
            return 1.0;
        }
        let d = wrap_delta(theta_deg - self.taper_theta_deg, 360.0).abs() / 180.0;
        1.0 - t * 0.5 * (1.0 - (std::f64::consts::PI * d).cos())
    }

    /// The gem at one station, graded. What the report and preview read.
    pub fn gem_at(&self, theta_deg: f64) -> crate::gem::Gem {
        let k = self.scale_at(theta_deg);
        let mut g = self.gem;
        g.w_mm *= k;
        g.l_mm *= k;
        g
    }

    /// Shared-prong post radius at full scale, mm.
    pub fn prong_r_mm(&self) -> f64 {
        (self.gem.w_mm * 0.16).clamp(0.3, 0.9)
    }

    /// The posts' offset from the stone column, mm: post centres ride the
    /// girdle edge so the cut claw overhangs both stones.
    pub fn prong_off_mm(&self) -> f64 {
        self.turned(self.seat).stone_half_v_mm(self.gem) + self.prong_r_mm() * 0.35
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        if !(ctx.circumference_mm > 1e-9) || !uv.u.is_finite() {
            return 0.0;
        }
        let theta = uv.u / ctx.circumference_mm * 360.0;
        // Nearest station and its neighbours, so a generous skirt cannot
        // clip at the cell boundary. Stations are evenly spaced in the
        // warped angle, which is the identity unless the row is graded.
        let k = self.station_of_theta(theta, ctx).round();
        let mut h: f64 = 0.0;
        for dk in [-1.0, 0.0, 1.0] {
            let mut s = self.turned(self.seat);
            s.theta_deg = self.theta_of_station(k + dk, ctx);
            s.v_mm = self.seat.v_mm;
            // Graduation scales the whole seat with its stone — footprint,
            // stand-off and skirt together, so a graded run stays a row of
            // self-similar mounds and the castability story is unchanged.
            let scale = self.scale_at(s.theta_deg);
            if scale < 1.0 {
                s.diameter_mm *= scale;
                s.height_mm *= scale;
                s.blend_mm *= scale;
                s.dimple_mm *= scale;
            }
            h = h.max(s.height(uv, ctx));
        }
        if self.shared_prong_mm > 1e-9 {
            // One post pair per boundary between stations, at the midpoints.
            let kb = (self.station_of_theta(theta, ctx) - 0.5).round();
            for dk in [-1.0, 0.0, 1.0] {
                let theta_b = self.theta_of_station(kb + dk + 0.5, ctx);
                let scale = self.scale_at(theta_b);
                let r_post = self.prong_r_mm() * scale;
                let off = self.prong_off_mm() * scale;
                let amp = (self.seat.height_mm + self.shared_prong_mm) * scale;
                let du = wrap_delta(uv.u - ctx.u_of_theta(theta_b), ctx.circumference_mm);
                for side in [-1.0, 1.0] {
                    let dv = uv.v - (self.seat.v_mm + side * off);
                    let d = (du * du + dv * dv).sqrt();
                    let t = (d / r_post.max(1e-6)).clamp(0.0, 1.0);
                    h = h.max(amp * (0.5 + 0.5 * (std::f64::consts::PI * t).cos()));
                }
            }
        }
        h
    }

    pub fn feature_footprints(&self, _ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let reach = self.turned(self.seat).half_extents_mm().1 + self.seat.blend_mm;
        let mut out = vec![FeatureFootprint::round(
            (self.seat.diameter_mm * 0.2).max(0.15),
            None,
            (self.seat.v_mm - reach, self.seat.v_mm + reach),
        )];
        if self.shared_prong_mm > 1e-9 {
            let r = self.prong_r_mm();
            let off = self.prong_off_mm();
            out.push(FeatureFootprint::round(
                r,
                None,
                (self.seat.v_mm - off - r, self.seat.v_mm + off + r),
            ));
        }
        out
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignetOutline {
    #[default]
    Oval,
    Round,
    Cushion,
    Rectangle,
    Hexagon,
    Heart,
    Shield,
    Octagon,
    Marquise,
    /// A rhombus standing on one corner — four gently convex sides.
    Diamond,
    /// A plus of two rounded bars — the plaque a gem column sits on.
    Cross,
    /// An imported plan, indexing [`crate::profile::ShankStyle::custom_outlines`].
    ///
    /// The geometry path resolves it through
    /// [`ShankStyle::outline_extent`](crate::profile::ShankStyle::outline_extent),
    /// which owns the registry; the bare methods on the enum itself fall back
    /// to [`SignetOutline::Oval`] so nothing can panic without one. Not in
    /// [`SignetOutline::ALL`], so every picker and sweep over the builtins is
    /// unchanged.
    Custom(u8),
}

/// Steps in a polar boundary table. One per 0.5 degree.
const OUTLINE_STEPS: usize = 720;
/// Points an imported boundary is resampled to, near-uniform in arc length.
const OUTLINE_DENSIFY: usize = 4096;
/// Circular Gaussian over the imported table, in table steps: kills chord
/// noise and rounds a true square corner by under 0.004 of the radius.
const OUTLINE_SMOOTH_SIGMA: f64 = 1.5;

/// Split every chord of a closed polyline so it is near-uniform in arc length.
fn densify_closed(pts: &[[f64; 2]], target: usize) -> Vec<[f64; 2]> {
    let n = pts.len();
    let len = |a: [f64; 2], b: [f64; 2]| (b[0] - a[0]).hypot(b[1] - a[1]);
    let per: f64 = (0..n).map(|i| len(pts[i], pts[(i + 1) % n])).sum();
    let step = (per / target.max(1) as f64).max(1e-12);
    let mut out = Vec::with_capacity(target + n);
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let m = ((len(a, b) / step) as usize).clamp(1, target);
        for k in 0..m {
            let t = k as f64 / m as f64;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}

/// Gaussian along a polar table, wrapping.
fn smooth_circular(r: &mut [f64; OUTLINE_STEPS], sigma: f64) {
    let rad = ((3.0 * sigma) as usize).max(1);
    let w: Vec<f64> = (0..=2 * rad)
        .map(|k| (-0.5 * ((k as f64 - rad as f64) / sigma).powi(2)).exp())
        .collect();
    let tot: f64 = w.iter().sum();
    let src = *r;
    for i in 0..OUTLINE_STEPS {
        let mut acc = 0.0;
        for (j, wj) in w.iter().enumerate() {
            acc += wj * src[(i + OUTLINE_STEPS + j - rad) % OUTLINE_STEPS];
        }
        r[i] = acc / tot;
    }
}

/// Boundary radius per direction, normalized so the outline fits the extents.
///
/// A table rather than a formula, so an outline only has to be describable as
/// "how far out is the edge in this direction" — the awkward ones are built
/// once at first use instead of solved per vertex.
struct PolarOutline {
    r: [f64; OUTLINE_STEPS],
}

impl PolarOutline {
    /// Build from a boundary radius function, recentred and scaled so the shape
    /// fills -1..1 in both axes and every outline honours `length_mm` by
    /// `width_mm`.
    ///
    /// Neither step can be done in the radius alone, and skipping either leaves
    /// a table describing a shape nothing has:
    ///
    /// - Scaling the two axes by different factors **moves each boundary point
    ///   round the circle**, so its new radius belongs at a new angle.
    /// - Scaling about the origin only works if the shape is centred there. A
    ///   heart is four times as far to its point as to its lobes, so dividing
    ///   by the larger extent squashes the lobes to a sixth of their size and
    ///   the outline comes out a lens.
    ///
    /// So the boundary is built in Cartesian, fitted to its own bounding box,
    /// and the table read back off it by casting one ray per step.
    fn build(f: impl Fn(f64) -> f64) -> Self {
        let step = std::f64::consts::TAU / OUTLINE_STEPS as f64;
        let raw: Vec<[f64; 2]> = (0..OUTLINE_STEPS)
            .map(|i| {
                let a = i as f64 * step;
                let v = f(a).max(0.0);
                [v * a.cos(), v * a.sin()]
            })
            .collect();
        Self::from_boundary(&raw)
    }

    /// The same recentre-fit-raycast, from a closed boundary polyline given
    /// as points — the door an imported outline comes in through.
    fn from_boundary(raw: &[[f64; 2]]) -> Self {
        let step = std::f64::consts::TAU / OUTLINE_STEPS as f64;
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for p in raw {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let b: Vec<[f64; 2]> = raw
            .iter()
            .map(|p| {
                let mut q = [0.0; 2];
                for k in 0..2 {
                    q[k] = 2.0 * (p[k] - lo[k]) / (hi[k] - lo[k]).max(1e-9) - 1.0;
                }
                q
            })
            .collect();

        // Furthest crossing per direction, not the first: that is the
        // silhouette, which is what a band's width can follow even where the
        // shape is hollow behind it.
        let n = b.len();
        let mut r = [1e-6f64; OUTLINE_STEPS];
        for (i, slot) in r.iter_mut().enumerate() {
            let a = i as f64 * step;
            let (sin_a, cos_a) = a.sin_cos();
            for k in 0..n {
                let (p, q) = (b[k], b[(k + 1) % n]);
                let (ex, ey) = (q[0] - p[0], q[1] - p[1]);
                // Ray x segment: the ray's own cross product vanishes on it.
                let den = cos_a * ey - sin_a * ex;
                if den.abs() <= 1e-12 {
                    continue;
                }
                let t = (sin_a * p[0] - cos_a * p[1]) / den;
                // A ray through a vertex lands at t = 1 on one segment and
                // t = 0 on the next, and rounding can put it a hair outside
                // both: the ray then finds nothing and the table keeps a
                // 1e-6 spike that smoothing spreads into a dip. The slack
                // counts a vertex hit on both segments; the max is the same.
                if !(-1e-9..=1.0 + 1e-9).contains(&t) {
                    continue;
                }
                let hit = (p[0] + t * ex) * cos_a + (p[1] + t * ey) * sin_a;
                if hit > *slot {
                    *slot = hit;
                }
            }
        }
        // Any ray still without a crossing takes its nearest found neighbour.
        let found: Vec<bool> = r.iter().map(|&v| v > 1e-5).collect();
        if found.iter().any(|&f| f) && !found.iter().all(|&f| f) {
            let src = r;
            for i in 0..OUTLINE_STEPS {
                if found[i] {
                    continue;
                }
                for d in 1..OUTLINE_STEPS {
                    let (a, b) = ((i + d) % OUTLINE_STEPS, (i + OUTLINE_STEPS - d) % OUTLINE_STEPS);
                    if found[a] || found[b] {
                        r[i] = if found[a] && found[b] { 0.5 * (src[a] + src[b]) } else if found[a] { src[a] } else { src[b] };
                        break;
                    }
                }
            }
        }
        Self { r }
    }

    /// Normalized distance: 1 on the outline, 0 at the centre.
    fn distance(&self, x: f64, y: f64) -> f64 {
        let rad = (x * x + y * y).sqrt();
        if rad <= 1e-12 {
            return 0.0;
        }
        let a = y.atan2(x).rem_euclid(std::f64::consts::TAU);
        let t = a / std::f64::consts::TAU * OUTLINE_STEPS as f64;
        let i = (t.floor() as usize) % OUTLINE_STEPS;
        let f = t - t.floor();
        let edge = self.r[i] * (1.0 - f) + self.r[(i + 1) % OUTLINE_STEPS] * f;
        rad / edge.max(1e-9)
    }
}

/// The classic heart, `(x² + y² − 1)³ = x²y³`, with its dimple at +y.
///
/// Solved for the radius along each ray rather than written as one, because the
/// closed forms that are easy to write are not this curve. The one this replaced
/// returned **zero at the dimple** — a cusp running all the way to the centre —
/// and the outline that came out of it had a spike for a point: measured against
/// a real heart signet's plate, its upper boundary was at 0.60 of full reach a
/// fifth of the way out where the reference holds 0.90.
fn heart_radius(a: f64) -> f64 {
    let (s, c) = a.sin_cos();
    // Negative inside the curve and positive outside, so a bisection converges
    // on the boundary from either end.
    let f = |r: f64| {
        let t = r * r - 1.0;
        t * t * t - r.powi(5) * c * c * s * s * s
    };
    let (mut lo, mut hi) = (0.0, 2.0);
    for _ in 0..60 {
        let m = 0.5 * (lo + hi);
        if f(m) <= 0.0 {
            lo = m;
        } else {
            hi = m;
        }
    }
    0.5 * (lo + hi)
}

/// A crest: flat shoulders, straight sides, a point at the bottom.
///
/// Half-planes rather than floored reciprocals. The floors made every
/// constraint slack near the diagonals, which let the corners out to meet and
/// left a square wearing a shield's name.
fn shield_radius(a: f64) -> f64 {
    let (s, c) = a.sin_cos();
    // `(nx, ny)` of `nx*x + ny*y <= 1`: a flat top, two straight sides, and two
    // lines running down from the waist to a point.
    const EDGES: [(f64, f64); 5] =
        [(0.0, 1.0), (1.0, 0.0), (-1.0, 0.0), (0.85, -1.0), (-0.85, -1.0)];
    EDGES.iter().fold(f64::MAX, |acc, &(nx, ny)| {
        let d = nx * c + ny * s;
        if d > 1e-9 { acc.min(1.0 / d) } else { acc }
    })
}

/// Regular polygon boundary with `n` sides.
fn polygon_radius(a: f64, n: f64) -> f64 {
    let seg = std::f64::consts::TAU / n;
    let half = seg * 0.5;
    1.0 / (a.rem_euclid(seg) - half).cos().abs().max(1e-6)
}

fn heart_table() -> &'static PolarOutline {
    static T: std::sync::OnceLock<PolarOutline> = std::sync::OnceLock::new();
    T.get_or_init(|| PolarOutline::build(heart_radius))
}

fn shield_table() -> &'static PolarOutline {
    static T: std::sync::OnceLock<PolarOutline> = std::sync::OnceLock::new();
    T.get_or_init(|| PolarOutline::build(shield_radius))
}

fn octagon_table() -> &'static PolarOutline {
    static T: std::sync::OnceLock<PolarOutline> = std::sync::OnceLock::new();
    T.get_or_init(|| PolarOutline::build(|a| polygon_radius(a, 8.0)))
}

impl SignetOutline {
    pub const ALL: &'static [SignetOutline] = &[
        SignetOutline::Oval,
        SignetOutline::Round,
        SignetOutline::Cushion,
        SignetOutline::Rectangle,
        SignetOutline::Hexagon,
        SignetOutline::Octagon,
        SignetOutline::Marquise,
        SignetOutline::Shield,
        SignetOutline::Heart,
        SignetOutline::Diamond,
        SignetOutline::Cross,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SignetOutline::Oval => "Oval",
            SignetOutline::Round => "Round",
            SignetOutline::Cushion => "Cushion",
            SignetOutline::Rectangle => "Rectangle",
            SignetOutline::Hexagon => "Hexagon",
            SignetOutline::Heart => "Heart",
            SignetOutline::Shield => "Shield",
            SignetOutline::Octagon => "Octagon",
            SignetOutline::Marquise => "Marquise",
            SignetOutline::Diamond => "Diamond",
            SignetOutline::Cross => "Cross",
            SignetOutline::Custom(_) => "Custom",
        }
    }

    /// Fullness of the outline as a superellipse exponent, which is also what
    /// the signet shank uses for the head silhouette: 2 oval, 4 cushion,
    /// 8 rectangle. Shapes with their own boundary report the nearest match.
    pub fn exponent(self) -> f64 {
        match self {
            SignetOutline::Oval | SignetOutline::Round => 2.0,
            SignetOutline::Cushion => 4.0,
            SignetOutline::Rectangle => 8.0,
            SignetOutline::Hexagon => 2.0,
            SignetOutline::Marquise => 1.4,
            SignetOutline::Heart => 2.6,
            SignetOutline::Shield => 3.2,
            SignetOutline::Octagon => 5.0,
            // Exponent 1 is a straight-sided rhombus; a touch above bulges
            // the sides and takes the worst off the corners.
            SignetOutline::Diamond => 1.15,
            SignetOutline::Cross => 2.0,
            // Resolved through the registry on the geometry path; this is
            // the same ellipse fallback as `extent`.
            SignetOutline::Custom(_) => 2.0,
        }
    }

    /// Boundary table for outlines that are not a plain superellipse.
    fn polar(self) -> Option<&'static PolarOutline> {
        match self {
            SignetOutline::Heart => Some(heart_table()),
            SignetOutline::Shield => Some(shield_table()),
            SignetOutline::Octagon => Some(octagon_table()),
            _ => None,
        }
    }

    /// Position in [`SignetOutline::ALL`], for indexing the cached tables.
    fn index(self) -> usize {
        Self::ALL.iter().position(|&o| o == self).unwrap_or(0)
    }

    /// Length-to-width ratio the shape wants — around the ring against across
    /// the band — so picking an outline can size a head that reads as that
    /// shape rather than a stretched one.
    ///
    /// The upright shapes are under 1 because their long axis runs across the
    /// band: a crest is taller up the finger than it is wide round it.
    pub fn head_aspect(self) -> f64 {
        match self {
            SignetOutline::Shield => 0.85,
            // The reference plate is 18.53 mm round the ring by 17.64 across,
            // which is 1.05 — and the classic curve's own box is 1.02.
            SignetOutline::Heart => 1.05,
            SignetOutline::Round => 1.0,
            SignetOutline::Octagon => 1.1,
            SignetOutline::Cushion => 1.2,
            SignetOutline::Rectangle => 1.25,
            SignetOutline::Hexagon => 1.3,
            SignetOutline::Oval => 1.35,
            SignetOutline::Marquise => 1.9,
            SignetOutline::Diamond => 1.1,
            SignetOutline::Cross => 1.0,
            // The registry carries the imported shape's own box ratio;
            // resolve through `ShankStyle::outline_aspect`.
            SignetOutline::Custom(_) => 1.0,
        }
    }

    /// Whether the outline's own "up" runs across the band rather than round
    /// the ring.
    ///
    /// A crest has to read up the finger — the flat top toward one edge of the
    /// band and the point toward the other — because that is the way it is
    /// looked at. Turned the other way a shield lies on its side. It only
    /// matters for shapes that are not symmetric end to end; an oval reads the
    /// same whichever way it is turned.
    pub fn upright(self) -> bool {
        matches!(self, SignetOutline::Heart | SignetOutline::Shield)
    }

    /// Normalized outline distance at a point already scaled to the extents: 0
    /// at the centre, 1 on the outline. `x` runs around the ring, `y` across
    /// the band.
    pub fn distance_norm(self, x: f64, y: f64) -> f64 {
        if let Some(table) = self.polar() {
            return if self.upright() {
                // A quarter turn, so the shape's own +y runs across the band
                // and toward the low edge: a crest stands with its top up the
                // finger and its point down it, and a heart with its lobes up.
                table.distance(x, -y)
            } else {
                table.distance(y, x)
            };
        }
        match self {
            // Three slabs 60 degrees apart, scaled to the extents: flat sides,
            // points at the length ends.
            SignetOutline::Hexagon => y.abs().max(x.abs() + 0.5 * y.abs()),
            // Union of two rounded bars: the plus is inside where either bar
            // is, so its distance is the smaller of the two.
            SignetOutline::Cross => {
                let w = 0.38;
                let n = 6.0f64;
                let bar = |long: f64, short: f64| {
                    (long.abs().powf(n) + (short.abs() / w).powf(n)).powf(1.0 / n)
                };
                bar(x, y).min(bar(y, x))
            }
            // A pointed ellipse: two arcs meeting at the length ends.
            SignetOutline::Marquise => {
                let n = 1.4f64;
                (x.abs().powf(n) + y.abs().powf(n)).powf(1.0 / n)
            }
            _ => {
                let n = self.exponent().max(1e-3);
                (x.abs().powf(n) + y.abs().powf(n)).powf(1.0 / n)
            }
        }
    }

    /// The outline's reach across the band at a station around the ring, as
    /// `(low, high)` — both normalized to the outline's own extents.
    ///
    /// An interval, not a half-width, because an upright shape does not reach
    /// the same distance both ways: a shield stands its flat top against one
    /// band edge and its point against the other. That is also why a signet
    /// head needs [`crate::profile::ShankMod::z_center_frac`] — a swept band is
    /// centred on its own mid-plane unless something moves it.
    ///
    /// This is the silhouette a swept band can carry. A band has one section
    /// per angle, so what it can follow is the outline's furthest reach either
    /// way, not where the shape happens to be hollow at that station.
    pub fn extent(self, x: f64) -> (f64, f64) {
        silhouette(self.builtin_or_oval()).at(x)
    }

    /// The variant the static caches can serve. `Custom` has no slot there —
    /// its table lives on the design — so bare access reads the oval instead
    /// of panicking; the geometry path resolves customs through
    /// [`ShankStyle::outline_extent`](crate::profile::ShankStyle::outline_extent).
    fn builtin_or_oval(self) -> SignetOutline {
        if matches!(self, SignetOutline::Custom(_)) { SignetOutline::Oval } else { self }
    }

    /// The **body's** reach at the same station: the face's own, faired out so
    /// it carries none of the face's detail.
    ///
    /// A signet's face is a facet cut across the crown of a wider body, and the
    /// two are not the same shape. Extruding the face down to the finger gives a
    /// heart-shaped prism — the dimple runs the whole depth of the ring, and the
    /// lobes leave creases down the flank. This is the shape the *bore* takes,
    /// with the face left on the table where it belongs.
    ///
    /// Contains [`SignetOutline::extent`] everywhere, which is what keeps the
    /// flank drafted rather than leaning back under the table.
    pub fn body_extent(self, x: f64) -> (f64, f64) {
        silhouette(self.builtin_or_oval()).body_at(x)
    }

    /// Width the outline leaves across the band, as a fraction of the head's
    /// half-width.
    pub fn half_extent(self, x: f64) -> f64 {
        let (lo, hi) = self.extent(x);
        (hi - lo) * 0.5
    }
}

/// Steps across an extent table, over `x` in -1..=1. Read by interpolation, so
/// this has to out-resolve the sweep: the table's own facets would otherwise
/// show up as slope steps in the band's silhouette.
const SILHOUETTE_STEPS: usize = 1025;

/// Radius the body fairs the face's hollows with, in stations — a station is a
/// half-length, so this is a share of the head's own reach.
///
/// Big enough to bridge a heart's notch, which is the point: a dimple is a
/// feature of the **face**, and a ring that carries it down to the finger is a
/// heart-shaped prism rather than a signet.
const BODY_FAIR_R: f64 = 0.75;
/// Radius the body's own corners are rounded over, in the same units. Small —
/// it is paid for in width, and its job is to take the edge off what the fairing
/// leaves convex, not to reshape the head.
const BODY_ROUND_X: f64 = 0.06;

/// The outline's reach across the band, per station around the ring — the sharp
/// face, and the faired body the face is a facet of.
#[derive(Clone)]
struct Silhouette {
    lo: [f64; SILHOUETTE_STEPS],
    hi: [f64; SILHOUETTE_STEPS],
    body_lo: [f64; SILHOUETTE_STEPS],
    body_hi: [f64; SILHOUETTE_STEPS],
}

/// Running maximum (or minimum) over a window, clamped at the ends.
fn sweep(src: &[f64; SILHOUETTE_STEPS], rad: isize, max: bool) -> [f64; SILHOUETTE_STEPS] {
    let n = SILHOUETTE_STEPS as isize;
    let mut out = [0.0f64; SILHOUETTE_STEPS];
    for i in 0..n {
        let mut m = src[i as usize];
        for k in -rad..=rad {
            let v = src[(i + k).clamp(0, n - 1) as usize];
            m = if max { m.max(v) } else { m.min(v) };
        }
        out[i as usize] = m;
    }
    out
}

/// Dilate (or erode) by a paraboloid of radius `r` — a ball rolled along the
/// curve rather than a window slid along it.
///
/// The shape of the structuring element is the whole difference between a
/// fairing and a plateau. A flat window fills every hollow to the level of its
/// rim and holds it there: measured on a heart, closing with one took the whole
/// dimple side of the head to a straight parallel edge and left a cliff where
/// it met the shoulder. A paraboloid bridges a hollow with an arc of its own
/// radius and leaves everything it cannot reach alone, which is what filleting
/// one means.
fn roll(src: &[f64; SILHOUETTE_STEPS], r: f64, up: bool) -> [f64; SILHOUETTE_STEPS] {
    let n = SILHOUETTE_STEPS as isize;
    // `x` spans 2 over n-1 steps.
    let step = 2.0 / (n - 1) as f64;
    let mut out = [0.0f64; SILHOUETTE_STEPS];
    for i in 0..n {
        let mut m = if up { f64::MIN } else { f64::MAX };
        for j in 0..n {
            let t = (j - i) as f64 * step;
            let bump = t * t / (2.0 * r.max(1e-6));
            let v = src[j as usize];
            m = if up { m.max(v - bump) } else { m.min(v + bump) };
        }
        out[i as usize] = m;
    }
    out
}

/// Box blur, run twice: one pass leaves the curvature stepping wherever the
/// input's own slope changes, and a step in curvature is a facet you can see.
fn blur(src: &[f64; SILHOUETTE_STEPS], rad: isize) -> [f64; SILHOUETTE_STEPS] {
    let n = SILHOUETTE_STEPS as isize;
    let half = (rad / 2).max(1);
    let mut out = *src;
    for _ in 0..2 {
        let prev = out;
        for i in 0..n {
            let mut s = 0.0;
            for k in -half..=half {
                s += prev[(i + k).clamp(0, n - 1) as usize];
            }
            out[i as usize] = s / (2 * half + 1) as f64;
        }
    }
    out
}

/// The body the face is a facet of: the face's own reach with its hollows faired
/// out and its corners taken off, carrying none of the face's detail.
///
/// A **closing** — dilate then erode — and not a blur. Closing fills what is
/// concave at its own radius and leaves everything convex exactly where it was,
/// so a heart's notch fairs over while its lobes, its point and the head's whole
/// plan size stay put. Blurring instead pulls the peaks in as well, and the head
/// comes out a blob with the face lost in it.
///
/// **Containment is not decoration.** A body narrower than the table it carries
/// leans the flank back over the mould half it sits in, which is an undercut by
/// construction rather than by accident. Closing is extensive, so it can only
/// add — and it stays extensive with the ball's reach truncated at the head's
/// ends, because the erosion's own station is always one of the samples it
/// minimises over. The rounding that follows dilates by its own radius *before*
/// blurring over no more than that, which makes every sample the blur averages a
/// maximum taken over a window still holding this station — so that cannot fall
/// below the face either.
///
/// `sign` is 1 for the reach toward the high band edge and -1 for the low one,
/// so one pass does both: the low edge fairs by taking minima.
fn fair(src: &[f64; SILHOUETTE_STEPS], sign: f64, fair_r: f64) -> [f64; SILHOUETTE_STEPS] {
    let n = SILHOUETTE_STEPS as isize;
    // `x` spans 2 over n-1 steps, so a radius in stations is half that in cells.
    let cells = |r: f64| ((r * 0.5 * (n - 1) as f64) as isize).max(1);
    let mut v = *src;
    for x in v.iter_mut() {
        *x *= sign;
    }

    let closed = roll(&roll(&v, fair_r, true), fair_r, false);
    let round_r = cells(BODY_ROUND_X);
    let mut out = blur(&sweep(&closed, round_r, true), round_r);
    for x in out.iter_mut() {
        *x *= sign;
    }
    out
}

impl Silhouette {
    /// Scanned inward from each extent in turn, then bisected onto the
    /// crossing. The boundary is not monotone in `y` for every outline — a
    /// heart's two lobes leave a gap at their own height — so the outermost
    /// reach has to be found from outside; bisecting only the bracket the scan
    /// lands in keeps that while costing nothing in precision.
    ///
    /// Precision is not cosmetic here. A quantized extent puts a step in the
    /// band's width, and a step in width is a step in slope, which is a facet
    /// you can see running round the head.
    fn build(o: SignetOutline) -> Self {
        Self::build_from(&|x, y| o.distance_norm(x, y), BODY_FAIR_R)
    }

    /// The same scan over any normalized distance — 1 on the outline — which
    /// is what lets an imported table build its silhouette through the exact
    /// machinery the builtins are held to.
    ///
    /// `fair_r` is the rolling ball's radius. [`BODY_FAIR_R`] is calibrated
    /// on the heart's two gentle lobes; a deeply lobed import — a clover, a
    /// rosette — wants a bigger ball, so its notches bridge flat and the
    /// lobed detail lives at the table's rim instead of riding the whole
    /// flank as ripples. Closing is extensive at any radius, so containment
    /// is not a function of the choice.
    fn build_from(dist: &dyn Fn(f64, f64) -> f64, fair_r: f64) -> Self {
        const SCAN: usize = 256;
        const BISECT: usize = 40;
        let mut lo = [0.0f64; SILHOUETTE_STEPS];
        let mut hi = [0.0f64; SILHOUETTE_STEPS];
        let mut found = [false; SILHOUETTE_STEPS];
        for i in 0..SILHOUETTE_STEPS {
            let x = -1.0 + 2.0 * i as f64 / (SILHOUETTE_STEPS - 1) as f64;
            // `side` is which end we walk in from, so one pass does both.
            for (side, slot) in [(1.0f64, &mut hi[i]), (-1.0, &mut lo[i])] {
                for j in 0..=SCAN {
                    let y = side * (1.0 - j as f64 / SCAN as f64);
                    if dist(x, y) > 1.0 {
                        continue;
                    }
                    let (mut inside, mut outside) = (y, y + side / SCAN as f64);
                    for _ in 0..BISECT {
                        let mid = 0.5 * (inside + outside);
                        if dist(x, mid) <= 1.0 {
                            inside = mid;
                        } else {
                            outside = mid;
                        }
                    }
                    *slot = inside;
                    found[i] = true;
                    break;
                }
            }
        }
        // A station the scan finds nothing at — the tangent sliver at x = ±1,
        // where the interval is a point — must inherit its neighbour, not
        // hold (0, 0): the zeroed cell collapses the span to a bogus centred
        // point, and the sweep reads that as a 4 mm yank inside one table
        // cell — measured as 128 degree grid folds at the face's end.
        for i in 1..SILHOUETTE_STEPS {
            if !found[i] && found[i - 1] {
                lo[i] = lo[i - 1];
                hi[i] = hi[i - 1];
                found[i] = true;
            }
        }
        for i in (0..SILHOUETTE_STEPS - 1).rev() {
            if !found[i] && found[i + 1] {
                lo[i] = lo[i + 1];
                hi[i] = hi[i + 1];
                found[i] = true;
            }
        }
        let r = fair_r.clamp(0.1, 4.0);
        let (body_lo, body_hi) = (fair(&lo, -1.0, r), fair(&hi, 1.0, r));
        Self { lo, hi, body_lo, body_hi }
    }

    fn at(&self, x: f64) -> (f64, f64) {
        self.sample(&self.lo, &self.hi, x)
    }

    fn body_at(&self, x: f64) -> (f64, f64) {
        self.sample(&self.body_lo, &self.body_hi, x)
    }

    /// Read with a Catmull-Rom spline rather than a chord.
    ///
    /// Linear interpolation reconstructs a table as facets, and a facet in the
    /// extent is a step in the band's slope that no amount of sweep resolution
    /// smooths out — it is in the function, not the sampling. A cubic
    /// reconstruction is C¹, so the silhouette the sweep follows is as smooth
    /// as the outline it came from.
    fn sample(
        &self,
        lo: &[f64; SILHOUETTE_STEPS],
        hi: &[f64; SILHOUETTE_STEPS],
        x: f64,
    ) -> (f64, f64) {
        let last = SILHOUETTE_STEPS - 1;
        let t = (x.clamp(-1.0, 1.0) + 1.0) * 0.5 * last as f64;
        let i = (t.floor() as usize).min(last - 1);
        let f = t - i as f64;
        let read = |table: &[f64; SILHOUETTE_STEPS]| {
            let p = |k: isize| table[(i as isize + k).clamp(0, last as isize) as usize];
            let (p0, p1, p2, p3) = (p(-1), p(0), p(1), p(2));
            p1 + 0.5
                * f
                * ((p2 - p0)
                    + f * ((2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3)
                        + f * (3.0 * (p1 - p2) + p3 - p0)))
        };
        (read(lo).clamp(-1.0, 0.0), read(hi).clamp(0.0, 1.0))
    }
}

/// One table per outline, built on first use. Per variant rather than all at
/// once: a design uses one outline and building the other eight would be work
/// nothing asked for.
fn silhouette(o: SignetOutline) -> &'static Silhouette {
    static T: [std::sync::OnceLock<Silhouette>; 11] =
        [const { std::sync::OnceLock::new() }; 11];
    T[o.index()].get_or_init(|| Silhouette::build(o))
}

/// An imported signet plan: a polar boundary table, carried in the design.
///
/// The table is the **source of truth** — 720 radii normalized to the unit
/// box, ~3 KB in the file — so a design with a custom head renders
/// identically on any machine with no asset or rasterizer in the loop.
/// Importers (a decoded factory curve, a traced SVG) produce the table once;
/// the derived silhouette is rebuilt on first use and never persisted, the
/// same way the tiling SDFs are.
///
/// [`SignetOutline::Custom`] indexes
/// [`ShankStyle::custom_outlines`](crate::profile::ShankStyle::custom_outlines),
/// and the head construction resolves it there. Everything downstream — the
/// rolling-ball body fairing, containment, the crest-span clamps — is the
/// same code the builtin outlines run, so the castability story is unchanged.
#[derive(Serialize, Deserialize)]
pub struct CustomOutline {
    pub name: String,
    /// Boundary radius per [`OUTLINE_STEPS`] direction, unit-box normalized.
    pub r: Vec<f32>,
    /// Source bounding box, length round the ring over width across the band
    /// — what [`crate::profile::SignetHead::fit_length_to`] wants to know.
    pub aspect: f64,
    /// Rolling-ball radius the body fairs this plan's hollows with, in
    /// stations. [`BODY_FAIR_R`] suits gently lobed shapes; importers raise
    /// it for deeply lobed ones so the flank stays one smooth surface and
    /// the lobes read only at the table's rim.
    #[serde(default = "default_fair_r")]
    pub fair_r: f64,
    #[serde(skip)]
    cache: std::sync::OnceLock<Silhouette>,
}

fn default_fair_r() -> f64 {
    BODY_FAIR_R
}

impl std::fmt::Debug for CustomOutline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomOutline")
            .field("name", &self.name)
            .field("aspect", &self.aspect)
            .finish_non_exhaustive()
    }
}

/// `1 − area / hull area`: how deeply lobed a closed plan is. A circle or a
/// square is ~0, a four-leaf clover ~0.2.
pub fn hull_defect(pts: &[[f64; 2]]) -> f64 {
    fn cross(o: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    }
    fn area(poly: &[[f64; 2]]) -> f64 {
        let n = poly.len();
        if n < 3 {
            return 0.0;
        }
        (0..n)
            .map(|i| {
                let (p, q) = (poly[i], poly[(i + 1) % n]);
                p[0] * q[1] - q[0] * p[1]
            })
            .sum::<f64>()
            .abs()
            * 0.5
    }
    let mut p: Vec<[f64; 2]> = pts.to_vec();
    p.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    p.dedup();
    if p.len() < 3 {
        return 0.0;
    }
    let mut lo: Vec<[f64; 2]> = Vec::new();
    for &q in &p {
        while lo.len() >= 2 && cross(lo[lo.len() - 2], lo[lo.len() - 1], q) <= 0.0 {
            lo.pop();
        }
        lo.push(q);
    }
    let mut up: Vec<[f64; 2]> = Vec::new();
    for &q in p.iter().rev() {
        while up.len() >= 2 && cross(up[up.len() - 2], up[up.len() - 1], q) <= 0.0 {
            up.pop();
        }
        up.push(q);
    }
    lo.pop();
    up.pop();
    lo.extend(up);
    let hull = area(&lo);
    if hull < 1e-12 { 0.0 } else { (1.0 - area(pts) / hull).max(0.0) }
}

/// The fairing ball for a plan this lobed: the calibrated default for a
/// convex plan, rising to 2.5 half-lengths at a 15% hull defect, so a
/// clover's notches bridge flat in the body and the lobes read at the
/// table's rim instead of rippling the whole flank.
pub fn fair_r_for(defect: f64) -> f64 {
    let t = ((defect - 0.02) / 0.13).clamp(0.0, 1.0);
    default_fair_r() + t * 1.75
}

impl Clone for CustomOutline {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            r: self.r.clone(),
            aspect: self.aspect,
            fair_r: self.fair_r,
            cache: self.cache.clone(),
        }
    }
}

impl PartialEq for CustomOutline {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.r == other.r
            && self.aspect == other.aspect
            && self.fair_r == other.fair_r
    }
}

impl CustomOutline {
    /// Build from a closed boundary polyline in the shape's own frame,
    /// `x` along the ring and `y` across the band toward the low edge.
    /// `None` when the boundary is degenerate.
    pub fn from_points(name: impl Into<String>, pts: &[[f64; 2]]) -> Option<Self> {
        if pts.len() < 8 {
            return None;
        }
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in pts {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        if !(w > 1e-9 && h > 1e-9) {
            return None;
        }
        // Uniform-parameter curve samples cluster at dense knot regions and
        // leave long chords elsewhere; a long chord is a curvature step in
        // the polar table, and a curvature step sweeps a ripple band down a
        // head's wall. Near-uniform arc length, then a circular Gaussian:
        // measured on a four-lobe plan, max second difference 0.0121 at 256
        // uniform samples, 0.0004 after.
        let dense = densify_closed(pts, OUTLINE_DENSIFY);
        let mut table = PolarOutline::from_boundary(&dense);
        smooth_circular(&mut table.r, OUTLINE_SMOOTH_SIGMA);
        Some(Self {
            name: name.into(),
            r: table.r.iter().map(|&v| v as f32).collect(),
            aspect: (w / h).clamp(0.05, 20.0),
            fair_r: fair_r_for(hull_defect(&dense)),
            cache: std::sync::OnceLock::new(),
        })
    }

    fn table(&self) -> PolarOutline {
        let mut r = [1e-6f64; OUTLINE_STEPS];
        for (slot, &v) in r.iter_mut().zip(self.r.iter()) {
            *slot = (v as f64).max(1e-6);
        }
        PolarOutline { r }
    }

    /// Mirror-average the table so the plan is symmetric: `across_band`
    /// folds it about the ring axis (y → −y), `along_ring` about the band
    /// axis (x → −x). Opt-in — a heart or a shield is asymmetric by design.
    /// For a curve drawn a hair off: a factory cushion sits 0.008 of its
    /// half-length fuller on one side at its tip, which on a lofted head
    /// stands the ridge's apex 0.6 mm off the parting plane.
    pub fn symmetrize(&mut self, across_band: bool, along_ring: bool) {
        let n = self.r.len();
        if n == 0 {
            return;
        }
        if across_band {
            let src = self.r.clone();
            for i in 0..n {
                self.r[i] = 0.5 * (src[i] + src[(n - i) % n]);
            }
        }
        if along_ring {
            let src = self.r.clone();
            let half = n / 2;
            for i in 0..n {
                self.r[i] = 0.5 * (src[i] + src[(half + n - i) % n]);
            }
        }
        self.cache = std::sync::OnceLock::new();
    }

    fn silhouette(&self) -> &Silhouette {
        self.cache.get_or_init(|| {
            let table = self.table();
            // The shape's own +y reads across the band toward the low edge,
            // the way a crest stands up the finger — the upright convention.
            Silhouette::build_from(&|x, y| table.distance(x, -y), self.fair_r)
        })
    }

    /// The face's reach across the band at a station, like
    /// [`SignetOutline::extent`].
    pub fn extent_at(&self, x: f64) -> (f64, f64) {
        self.silhouette().at(x)
    }

    /// The faired body's reach, like [`SignetOutline::body_extent`].
    pub fn body_extent_at(&self, x: f64) -> (f64, f64) {
        self.silhouette().body_at(x)
    }
}

/// A raised flat table pad standing on the band, faired into it.
///
/// **This is not how to make a signet.** A signet's head is the band's own swell
/// — [`crate::profile::SignetHead`] on the shank — and its outline is the band's
/// plan silhouette. This pad sits *on top of* whatever is under it, which is the
/// right thing for a flat facet on an otherwise ordinary band and the wrong
/// thing for a signet, where it leaves a disc glued to a ring.
///
/// Displacement is solved per point rather than held constant, so the face is a
/// true plane: a uniform offset of a curved band stays curved, and on a size 7
/// half-round a 12 x 9 mm table would stand 2.15 mm out of flat. The shoulder
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
    ///
    /// Table and shoulder together have to fit the room, or the fairing runs
    /// off the surface holding it up and walls up instead. The shoulder is
    /// taken out of the room first, so this can never produce a table that its
    /// own [`SignetLayer::overhangs`] then complains about.
    pub fn fitted_to(ctx: &FieldContext) -> Self {
        let room = Self::room_across(ctx).max(0.0);
        let shoulder = (room * 0.225).clamp(0.3, Self::default().shoulder_mm);
        let width = (room - 2.0 * shoulder).clamp(1.0, 14.0);
        Self {
            v_mm: ctx.crest_v_mm,
            width_mm: width,
            length_mm: width * 1.6,
            shoulder_mm: shoulder,
            ..Default::default()
        }
    }

    /// Surface across the band a table can stand on, mm.
    ///
    /// The run around the crest still flat enough to carry a plane. Past that
    /// the base has dropped so far below the table that the shoulder has to
    /// claw back the difference over its own short width, which is a wall, not
    /// a fairing — and it is why a half-round is a poor base for a signet and a
    /// flat crest a good one.
    ///
    /// Measured from the crest outward in both directions and taken as twice
    /// the shorter, because a crest sitting off centre — a flange takes it to
    /// one side — leaves less room than the band width says.
    pub fn room_across(ctx: &FieldContext) -> f64 {
        if ctx.surface.is_empty() || ctx.band_v_len_mm <= 1e-9 {
            return ctx.band_v_len_mm.max(0.0);
        }
        let span = ctx.band_v_len_mm;
        let flat = |v: f64| {
            ctx.surface.draft_deg(v, span).is_some_and(|d| d <= TABLE_MAX_DRAFT_DEG)
        };
        let n = ctx.surface.samples.len();
        let step = span / (n - 1) as f64;
        let crest = (ctx.crest_v_mm / step).round().clamp(0.0, (n - 1) as f64) as usize;
        let mut lo = crest;
        while lo > 0 && flat(at_step(lo - 1, step)) {
            lo -= 1;
        }
        let mut hi = crest;
        while hi + 1 < n && flat(at_step(hi + 1, step)) {
            hi += 1;
        }
        let half = (ctx.crest_v_mm - at_step(lo, step))
            .min(at_step(hi, step) - ctx.crest_v_mm)
            .max(0.0);
        (half * 2.0).min(span)
    }

    /// Whether the table, shoulder included, reaches past the surface that can
    /// support it — what makes it bow away from a true plane and wall up.
    ///
    /// Measured on a squared-sided band, undercut starts once the reach passes
    /// about 1.05 of the half-room. Reporting at 1.0 leaves a margin on the safe
    /// side, so this warns a little early and never late.
    pub fn overhangs(&self, ctx: &FieldContext) -> bool {
        self.reach_mm() > Self::room_across(ctx) * 0.5
    }

    /// How far the table and its shoulder extend either side of centre, mm.
    pub fn reach_mm(&self) -> f64 {
        self.width_mm.max(0.0) * 0.5 + self.shoulder_mm.max(0.0)
    }

    /// Grow the table to fill the head, the way a real signet's does.
    ///
    /// Measured on a flat crest: 0.55 of the room is clean but leaves an obvious
    /// margin, 0.70 is clean and reads as a signet, 0.82 starts to bow at
    /// 0.05%, and 0.92 walls up at -36 degrees.
    pub fn fill_head(&mut self, ctx: &FieldContext) {
        let room = Self::room_across(ctx);
        self.width_mm = (room * SIGNET_TABLE_FRAC).max(2.0);
        self.length_mm = self.width_mm * 1.55;
        self.shoulder_mm = self.shoulder_mm.min((room - self.width_mm) * 0.5).max(0.4);
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
        self.outline.distance_norm(x_mm / half_u, y_mm / half_v)
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

/// Smoothstep with the second derivative flat at both ends too.
///
/// Where a blend joins two curves that are going somewhere, C¹ is not enough:
/// `smoothstep` leaves a step in curvature at each end of its window, and a step
/// in curvature is a crease you can see under a light even though the surface is
/// smooth. Measured on a heart head, the two ends of the edge break each left
/// one.
pub fn smootherstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-12)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
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
            bore_radius_mm: 8.5,
            side_faces_cache: Default::default(),
        }
    }

    /// The doctrine in two acts: a deep carve on a side face releases at any
    /// depth, and the same carve turned onto the crown is caught by the
    /// field verdict — which is why the preset gates to a side face.
    #[test]
    fn openwork_carves_deep_on_a_side_face_and_the_crown_is_caught() {
        use crate::alpha::AlphaLibrary;
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.width_mm = 8.0;
        d.profile.thickness_mm = 2.4;
        d.profile.flatten_sides();
        let fc = d.field_context();

        let mut lib = AlphaLibrary::builtin();
        // One large round window per tile: distance to the rim comfortably
        // exceeds the wall ramp, so the full depth is reached.
        let n = 128usize;
        let mut mask = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let dx = x as f64 / n as f64 - 0.5;
                let dy = y as f64 / n as f64 - 0.5;
                if (dx * dx + dy * dy).sqrt() < 0.36 {
                    mask[y * n + x] = 1.0;
                }
            }
        }
        lib.insert(crate::alpha::Alpha::new("window", n, n, mask));
        let mut t = crate::tiling::TilingLayer::default_for("window", &fc);
        t.fit_to_side_faces(&fc, SIDE_FACE_MIN_DRAFT_DEG);
        t.repeats_around = 9;
        t.height_mm = 1.0;
        t.edge_mm = 0.35;
        t.feather_mm = 0.0;
        t.continuous = false;
        let o = OpenworkLayer { tiling: t, depth_mm: 1.2, keep_mm: 1.0 };
        let mut e = LayerEntry::new("Openwork", Layer::Openwork(o));
        // Carving must Add: the default Max would swallow a negative layer.
        e.blend = Blend::Add;
        e.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
        d.layers.layers.push(e);
        d.bake_all(&mut lib);

        // Full commanded depth on the gated face.
        let (lo, hi) = fc.side_faces_std().and_then(|f| f.wider()).expect("side face");
        let v = 0.5 * (lo + hi);
        let mut deepest = 0.0f64;
        for k in 0..2000 {
            let u = k as f64 / 2000.0 * fc.circumference_mm;
            deepest = deepest.min(d.layers.height(Uv { u, v }, &fc, &lib));
        }
        assert!(
            (deepest + 1.2).abs() < 0.1,
            "side-face carve missed its depth: {deepest}"
        );

        // Deep on a face parallel to the pull: still releases.
        let out = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams { theta_steps: 192, profile_steps: 96, ..Default::default() },
        );
        assert!(out.report.validation.watertight);
        let field = crate::castability::analyze_field(&d, &lib, &d.draft, 160, 96);
        assert_ne!(
            field.verdict,
            crate::castability::Verdict::NotCastable,
            "{:?}",
            field.notes
        );

        // Act two: the same carve across the crown locks in the sand, and
        // the verdict says so — the floor cap over the bore still holds.
        let Layer::Openwork(o) = &mut d.layers.layers[0].layer else { panic!() };
        o.tiling.v_center_mm = fc.crest_v_mm;
        o.tiling.v_span_mm = 4.5;
        o.tiling.mirror_v = false;
        o.depth_mm = 6.0;
        d.layers.layers[0].window.v_gate = VGate::Off;
        let v = fc.crest_v_mm;
        let mut deepest = 0.0f64;
        for k in 0..2000 {
            let u = k as f64 / 2000.0 * fc.circumference_mm;
            deepest = deepest.min(d.layers.height(Uv { u, v }, &fc, &lib));
        }
        let (r, _) = fc.surface.at(v, fc.band_v_len_mm).expect("surface");
        let metal_left = (r - fc.bore_radius_mm) + deepest;
        assert!(
            metal_left > 0.97,
            "floor broke through: {metal_left:.3} mm"
        );
        let field = crate::castability::analyze_field(&d, &lib, &d.draft, 160, 96);
        assert_eq!(
            field.verdict,
            crate::castability::Verdict::NotCastable,
            "a deep crown carve should be caught: {:?}",
            field.notes
        );
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
        assert!(!s.overhangs(&ctx), "the default fit must not warn about itself");
        assert!(s.reach_mm() <= room * 0.5, "fitted reach {:.2} exceeds half the room", s.reach_mm());
        let (lo, hi) = (s.v_mm - s.width_mm * 0.5, s.v_mm + s.width_mm * 0.5);
        let low_end = faces.low.map_or(0.0, |(_, e)| e);
        let high_start = faces.high.map_or(ctx.band_v_len_mm, |(s, _)| s);
        assert!(lo > low_end, "table starts at {lo:.2} mm, inside the side face ending {low_end:.2}");
        assert!(hi < high_start, "table ends at {hi:.2} mm, inside the side face from {high_start:.2}");
    }

    #[test]
    fn a_v_band_gate_holds_inside_and_dies_past_the_fade() {
        let c = ctx();
        let mut w = Window::default();
        w.v_gate = VGate::Band { center_mm: 4.0, span_mm: 2.0, fade_mm: 0.5 };
        let at = |v: f64| w.mask(Uv { u: 0.0, v }, &c);
        assert!((at(4.0) - 1.0).abs() < 1e-12);
        assert!((at(4.9) - 1.0).abs() < 1e-12, "inside the held span");
        assert_eq!(at(5.6), 0.0, "past the fade");
        let mid = at(5.25);
        assert!(mid > 0.0 && mid < 1.0, "fading at {mid}");
        // The angular window is off, and the gate still applies.
        assert!(!w.enabled);
    }

    #[test]
    fn a_side_face_gate_tracks_the_profile() {
        let mut d = crate::RingDesign::default();
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 3.4;
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let c = d.field_context();
        let faces = c.side_faces_std().expect("squared sides have faces");
        let (lo, hi) = faces.low.expect("low face present");

        let mut w = Window::default();
        w.v_gate = VGate::SideFaces(SideFacePick::Low);
        let at = |v: f64| w.mask(Uv { u: 0.0, v }, &c);
        assert!((at((lo + hi) * 0.5) - 1.0).abs() < 1e-9, "full in the run's middle");
        assert_eq!(at(c.crest_v_mm), 0.0, "nothing on the crest");
        assert_eq!(at(hi + 0.5), 0.0, "nothing past the run");

        // A dome has no side faces, and the gate honestly passes nothing.
        let dome = crate::RingDesign::default();
        let mut dc = dome.field_context();
        dc.side_faces_cache = Default::default();
        if dc.side_faces_std().is_none() {
            assert_eq!(w.v_gate.mask(dc.crest_v_mm, &dc), 0.0);
        }
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
        assert_eq!(Blend::Add.apply(1.0, 2.0, 0.0), 3.0);
        assert_eq!(Blend::Max.apply(1.0, 2.0, 0.0), 2.0);
        assert_eq!(Blend::Subtract.apply(1.0, 2.0, 0.0), -1.0);
        assert_eq!(Blend::Replace.apply(1.0, 2.0, 0.0), 2.0);
    }

    #[test]
    fn remap_reshapes_within_bounds_and_terraces_hold_flat() {
        let span = 0.4;
        for remap in [Remap::cushion(span), Remap::chamfer(span)] {
            assert_eq!(remap.apply(0.0), 0.0);
            assert!((remap.apply(span) - span).abs() < 1e-9, "full height maps to itself");
            let mut prev = 0.0;
            for i in 1..=64 {
                let h = span * i as f64 / 64.0;
                let out = remap.apply(h);
                assert!((0.0..=span + 1e-9).contains(&out), "left the span: {out}");
                assert!(out >= prev - 1e-9, "remap must stay monotone");
                prev = out;
            }
        }

        let t = Remap::Terrace { steps: 4, span_mm: 0.4, riser: 0.3 };
        let q = 0.1;
        // Mid-tread inputs sit exactly on the tread below.
        assert!((t.apply(0.130) - q).abs() < 1e-9, "{}", t.apply(0.130));
        assert!((t.apply(0.230) - 2.0 * q).abs() < 1e-9);
        // The top of each tread has risen to the next.
        assert!((t.apply(0.2) - 2.0 * q).abs() < 1e-9);
        // Negative and zero pass through, so carving is unaffected.
        assert_eq!(t.apply(-0.2), -0.2);
    }

    /// A graduated row's seats shrink but its stations did not move, so the
    /// metal between them grew with every step — 0.42 mm at the large pole
    /// against 3.05 mm at the small one, a sevenfold spread down what is
    /// meant to read as one continuous line of stones. Holding the bridge
    /// constant instead is a closed-form warp, and the identity when the row
    /// is not graded.
    #[test]
    fn a_graduated_run_holds_its_bridge_constant() {
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let c = d.field_context();

        let build = |taper: f64| {
            let mut r = SeatRunLayer::default();
            r.gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 1.5);
            r.seat.v_mm = c.crest_v_mm;
            r.bridge_mm = 0.4;
            r.taper = taper;
            r.solve_spacing(&c);
            r
        };
        // The seat scales whole, as the field scales it.
        let bridges = |r: &SeatRunLayer, warped: bool| -> Vec<f64> {
            let n = r.count as usize;
            let radius = c.crest_radius_mm * c.arc_scale(r.seat.v_mm);
            let at = |k: f64| {
                if warped { r.theta_of_station(k, &c) } else { k * 360.0 / n as f64 }
            };
            (0..n)
                .map(|k| {
                    let (a, b) = (at(k as f64), at(k as f64 + 1.0));
                    let half = |t: f64| r.seat_span_mm() * 0.5 * r.scale_at(t);
                    wrap_delta(b - a, 360.0).abs().to_radians() * radius - half(a) - half(b)
                })
                .collect()
        };
        let spread = |v: &[f64]| {
            let lo = v.iter().cloned().fold(f64::MAX, f64::min);
            let hi = v.iter().cloned().fold(0.0f64, f64::max);
            (lo, hi / lo)
        };

        // Ungraded: the warp is the identity, station for station.
        let plain = build(0.0);
        for k in 0..plain.count {
            let want = k as f64 * 360.0 / plain.count as f64;
            assert!((plain.theta_of_station(k as f64, &c) - want).abs() < 1e-12);
            assert!((plain.station_of_theta(want, &c) - k as f64).abs() < 1e-12);
        }
        assert!((spread(&bridges(&plain, true)).1 - 1.0).abs() < 1e-9);

        for taper in [0.4, 0.85] {
            let r = build(taper);
            let (lo, ratio) = spread(&bridges(&r, true));
            assert!(ratio < 1.25, "taper {taper}: bridges still run {ratio:.2}x");
            assert!(lo > 0.3, "and none of them is a feather: {lo:.3} mm");

            // The lattice this replaces, so the test cannot rot into a
            // tautology: uniform stations on the count the old law solved.
            let mut old = r;
            old.taper = 0.0;
            old.solve_spacing(&c);
            old.taper = taper;
            assert!(
                spread(&bridges(&old, false)).1 > 2.0,
                "taper {taper}: the uniform lattice was fine after all?"
            );

            // What the report says is what the row holds.
            let said = r.bridge_at(&c);
            assert!((said - lo).abs() / lo < 0.08, "reports {said:.3}, holds {lo:.3}");

            // Every step advances round the ring — the sequence wraps once
            // and only once — and the warp inverts.
            let mut total = 0.0;
            for k in 0..r.count {
                let t = r.theta_of_station(k as f64, &c);
                let next = r.theta_of_station(k as f64 + 1.0, &c);
                let step = wrap_delta(next - t, 360.0);
                assert!(step > 0.0, "stations must advance: {step} at {k}");
                total += step;
                assert!(
                    (wrap_delta(
                        r.theta_of_station(r.station_of_theta(t, &c).round(), &c) - t,
                        360.0
                    ))
                    .abs()
                        < 1e-9,
                    "the warp must invert at {t}"
                );
            }
            assert!((total - 360.0).abs() < 1e-9, "one turn, exactly: {total}");
            let close = r.theta_of_station(r.count as f64, &c);
            assert!(
                wrap_delta(close - r.theta_of_station(0.0, &c), 360.0).abs() < 1e-9,
                "the row closes on itself: {close}"
            );
            assert!(r.count >= 3);
        }

        // And it still releases.
        let mut d2 = d.clone();
        d2.layers.layers.push(LayerEntry::new("Graded", Layer::SeatRun(build(0.85))));
        let lib = crate::AlphaLibrary::builtin();
        let v = crate::castability::analyze_field(&d2, &lib, &d2.draft, 256, 128);
        assert!(
            v.undercut_fraction() < 0.001,
            "a graded row locks: {:.4}%",
            v.undercut_fraction() * 100.0
        );
    }

    /// `u` is arc at the crest radius, so it is the true metal only on the
    /// crest. Everything a run reports on a side face was 17–20% optimistic,
    /// in the unsafe direction, on exactly the surfaces the doctrine sends
    /// all ornament to.
    #[test]
    fn the_bridge_a_run_reports_is_metal_not_arc() {
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 5.0;
        let c = d.field_context();
        let face = c.side_faces_std().and_then(|sf| sf.wider()).expect("a squared band has one");
        let v_face = 0.5 * (face.0 + face.1);

        // On the crest the chart is the metal — to the sampled profile's own
        // resolution, which is where the last nanometre goes.
        assert!((c.arc_scale(c.crest_v_mm) - 1.0).abs() < 1e-6);
        let k = c.arc_scale(v_face);
        assert!(k < 0.85, "a squared band's side face runs well inside the crest: {k:.4}");

        let run = |v: f64| {
            let mut r = SeatRunLayer::default();
            r.gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Round, 1.6);
            r.seat.v_mm = v;
            r.bridge_mm = 0.4;
            r.solve_spacing(&c);
            r
        };

        // The chart figure, and the metal it really is.
        let on_face = run(v_face);
        let chart = c.circumference_mm / on_face.count as f64 - on_face.seat_span_mm();
        let metal = on_face.bridge_at(&c);
        assert!((metal - chart * k).abs() < 1e-9, "{metal} vs {chart} x {k}");
        assert!(chart - metal > 0.08, "the correction is worth saying: {:.3} mm", chart - metal);

        // And solve_spacing never lands *under* the bridge it was asked for,
        // in metal — the invariant the two have to share, or the report
        // starts warning about spacing it just solved. The slack above it is
        // the floor to a whole station, which is one pitch spread over the
        // ring.
        for v in [v_face, c.crest_v_mm] {
            let r = run(v);
            let got = r.bridge_at(&c);
            let pitch = c.circumference_mm * c.arc_scale(v) / r.count as f64;
            let slack = pitch * pitch / (c.circumference_mm * c.arc_scale(v));
            assert!(
                got >= r.bridge_mm - 1e-9 && got <= r.bridge_mm + slack,
                "asked {:.2} mm, got {got:.3} at v {v:.2} (slack {slack:.3})",
                r.bridge_mm
            );
        }

        // A crest run is untouched: the whole correction is 1 there.
        let crest = run(c.crest_v_mm);
        let chart_crest = c.circumference_mm / crest.count as f64 - crest.seat_span_mm();
        assert!((crest.bridge_at(&c) - chart_crest).abs() < 1e-6);
    }

    #[test]
    fn an_eternity_run_closes_seamlessly_and_releases() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let fc = d.field_context();
        let mut run = SeatRunLayer::default();
        run.seat.v_mm = fc.crest_v_mm;
        run.solve_spacing(&fc);
        assert!(run.count >= 8, "a size-7 band fits a real row: {}", run.count);
        assert!(run.bridge_at(&fc) > 0.0, "stones must not touch");

        for dv in [-1.0, 0.0, 1.0] {
            let v = fc.crest_v_mm + dv;
            let a = run.height(Uv { u: 0.0, v }, &fc);
            let b = run.height(Uv { u: fc.circumference_mm - 1e-9, v }, &fc);
            assert!((a - b).abs() < 1e-6, "joint mismatch at v {v}");
        }

        d.layers.layers.push(LayerEntry::new("eternity", Layer::SeatRun(run)));
        let out = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams { theta_steps: 384, profile_steps: 96, ..Default::default() },
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert!(
            cast.undercut_fraction() < 0.001,
            "a gypsy-mound row locks: {:.3}%",
            cast.undercut_fraction() * 100.0
        );
    }

    /// A stone is not a circle, and its stock should not be either. The pad
    /// carries the girdle's own plan — its aspect, its superellipse exponent
    /// and its bearing — and every measurement downstream reads the real
    /// footprint rather than a diameter.
    #[test]
    fn an_elongated_seat_carries_its_stone_and_not_a_circle_round_it() {
        let c = ctx();
        let gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Marquise, 3.0);
        assert!((gem.l_mm - 6.0).abs() < 1e-9, "a 3 mm marquise is 6 mm long");

        let mut pad = SeatPadLayer { v_mm: 4.0, style: SeatStyle::Boss, ..Default::default() };
        pad.fit_stone(gem);
        // The stock allowance is a constant width all round, so the pad is
        // less elongated than the stone it holds — but it still contains it.
        let (ra, rb) = pad.semi_axes_mm();
        assert!(ra * 2.0 >= gem.l_mm, "pad {:.2} mm long for a {:.2} mm stone", ra * 2.0, gem.l_mm);
        assert!(rb * 2.0 >= gem.w_mm, "pad {:.2} mm wide for a {:.2} mm stone", rb * 2.0, gem.w_mm);
        assert!(pad.elong > 1.4 && pad.elong < gem.l_mm / gem.w_mm, "elong {}", pad.elong);
        assert_eq!(pad.plan_pow, crate::gem::GemCut::Marquise.plan_pow());

        // A round pad is exactly what it always was.
        let round = SeatPadLayer { v_mm: 4.0, ..Default::default() };
        assert_eq!(round.half_extents_mm().0, round.diameter_mm * 0.5);
        assert_eq!(round.half_extents_mm().1, round.diameter_mm * 0.5);

        let u0 = c.u_of_theta(pad.theta_deg);
        let at = |l: &SeatPadLayer, du: f64, dv: f64| {
            l.height(Uv { u: u0 + du, v: 4.0 + dv }, &c)
        };
        // Metal reaches down the length and stops across the width.
        assert!(at(&pad, ra - 0.3, 0.0) > 0.0, "no stock at the stone's point");
        assert!(at(&pad, 0.0, rb + 0.05 + pad.blend_mm) <= 1e-9, "stock past the girdle's side");

        // Turning the seat turns its reach with it, exactly.
        let mut across = pad;
        across.rot_deg = 90.0;
        let (hu, hv) = across.half_extents_mm();
        assert!((hu - rb).abs() < 1e-9 && (hv - ra).abs() < 1e-9, "turned extents {hu} {hv}");
        assert!(at(&across, 0.0, ra - 0.3) > 0.0, "the length should now run across the band");

        // The extent formula is the outline's own support function: check it
        // against the sampled plan at a handful of bearings, every exponent.
        for &n in &[1.5, 2.0, 3.2, 6.0] {
            for &rot in &[0.0, 17.0, 45.0, 90.0, 123.0] {
                let mut l = pad;
                l.plan_pow = n;
                l.rot_deg = rot;
                let (ra, rb) = l.semi_axes_mm();
                let (mut mu, mut mv) = (0.0f64, 0.0f64);
                for i in 0..2048 {
                    let t = i as f64 / 2048.0 * std::f64::consts::TAU;
                    let (sn, cs) = t.sin_cos();
                    // A point on the plan outline, in the pad's own frame.
                    let (a, b) = (
                        ra * cs.abs().powf(2.0 / n) * cs.signum(),
                        rb * sn.abs().powf(2.0 / n) * sn.signum(),
                    );
                    let (s2, c2) = rot.to_radians().sin_cos();
                    mu = mu.max((a * c2 - b * s2).abs());
                    mv = mv.max((a * s2 + b * c2).abs());
                }
                let (hu, hv) = l.half_extents_mm();
                assert!((hu - mu).abs() < 5e-3, "n {n} rot {rot}: u {hu} vs sampled {mu}");
                assert!((hv - mv).abs() < 5e-3, "n {n} rot {rot}: v {hv} vs sampled {mv}");
            }
        }
    }

    /// The whole point of the plan being convex: a mound built on it is
    /// still a monotone drop from a single crest in every direction, so an
    /// elongated seat releases exactly where a round one does.
    #[test]
    fn an_elongated_gypsy_row_still_releases() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let fc = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = crate::gem::Gem::calibrated(crate::gem::GemCut::Emerald, 1.8);
        run.seat.v_mm = fc.crest_v_mm;
        run.solve_spacing(&fc);
        // A row of step cuts packs by its length, so it holds fewer stones
        // than the same width of rounds would.
        assert!(run.seat.elong > 1.2, "the seat took the stone's aspect");
        assert!(run.bridge_at(&fc) > 0.0, "stones must not touch");

        for dv in [-0.8, 0.0, 0.8] {
            let v = fc.crest_v_mm + dv;
            let a = run.height(Uv { u: 0.0, v }, &fc);
            let b = run.height(Uv { u: fc.circumference_mm - 1e-9, v }, &fc);
            assert!((a - b).abs() < 1e-6, "joint mismatch at v {v}");
        }

        d.layers.layers.push(LayerEntry::new("eternity", Layer::SeatRun(run)));
        let v = crate::castability::analyze_field(&d, &lib, &d.draft, 192, 96);
        assert!(
            v.undercut_fraction() < 0.001,
            "an elongated gypsy row locks: {:.3}%",
            v.undercut_fraction() * 100.0
        );
    }

    #[test]
    fn seat_styles_shape_as_promised_and_release_where_they_belong() {
        let c = ctx();
        // Bezel: rim at full height, pocket floor recessed inside it.
        let bezel = SeatPadLayer {
            v_mm: 4.0,
            style: SeatStyle::Bezel,
            diameter_mm: 5.0,
            height_mm: 1.0,
            recess_mm: 0.4,
            ..Default::default()
        };
        let u0 = c.u_of_theta(90.0);
        let at = |l: &SeatPadLayer, du: f64| l.height(Uv { u: u0 + du, v: 4.0 }, &c);
        let rim = at(&bezel, 2.2);
        let pocket = at(&bezel, 0.0);
        assert!((rim - 1.0).abs() < 1e-6, "rim carries full height: {rim}");
        assert!((pocket - 0.6).abs() < 1e-6, "pocket floor recessed: {pocket}");

        // Prongs stand above the pad; sample on the ring their apexes sit on.
        let pronged = SeatPadLayer { v_mm: 4.0, prongs: 4, prong_mm: 0.8, ..Default::default() };
        let prong_r = (0.28 * 2.5f64).clamp(0.35, 0.8);
        let ring_r = 2.5 - prong_r * 0.6;
        let mut peak = 0.0f64;
        for i in 0..720 {
            let a = i as f64 / 720.0 * std::f64::consts::TAU;
            let (du, dv) = (ring_r * a.cos(), ring_r * a.sin());
            peak = peak.max(pronged.height(Uv { u: u0 + du, v: 4.0 + dv }, &c));
        }
        assert!(
            (peak - (pronged.height_mm + 0.8)).abs() < 0.05,
            "a prong tip should top the pad: {peak}"
        );

        // Every style builds watertight; boss and mound release on a dome
        // crown, and a bezel on a squared side face releases too.
        let lib = crate::AlphaLibrary::builtin();
        let params =
            crate::BuildParams { theta_steps: 256, profile_steps: 96, ..Default::default() };
        for style in [SeatStyle::Boss, SeatStyle::GypsyMound] {
            let mut d = crate::RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::LowDome);
            let crest = d.field_context().crest_v_mm;
            let pad = SeatPadLayer { v_mm: crest, style, ..Default::default() };
            d.layers.layers.push(LayerEntry::new("pad", Layer::SeatPad(pad)));
            let out = crate::mesh::build(&d, &lib, params);
            assert!(out.report.validation.watertight, "{style:?}");
            let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
            assert!(
                cast.undercut_fraction() < 0.004,
                "{style:?} locks: {:.3}%",
                cast.undercut_fraction() * 100.0
            );
        }

        let mut d = crate::RingDesign::default();
        d.profile.width_mm = 8.0;
        d.profile.thickness_mm = 3.0;
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let fc = d.field_context();
        let (lo, hi) = fc.side_faces_std().expect("faces").wider().unwrap();
        let pad = SeatPadLayer {
            v_mm: 0.5 * (lo + hi),
            style: SeatStyle::Bezel,
            diameter_mm: (hi - lo) * 0.8,
            height_mm: 0.8,
            blend_mm: 0.4,
            ..Default::default()
        };
        let mut entry = LayerEntry::new("bezel", Layer::SeatPad(pad));
        // Held to the run, so the skirt cannot spill over the band edge.
        entry.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
        d.layers.layers.push(entry);
        let out = crate::mesh::build(&d, &lib, params);
        assert!(out.report.validation.watertight);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        assert!(
            cast.undercut_fraction() < 0.001,
            "side-face bezel locks: {:.3}%",
            cast.undercut_fraction() * 100.0
        );
    }

    #[test]
    fn a_decal_stamps_where_placed_and_nowhere_else() {
        let c = ctx();
        let mut lib = AlphaLibrary::default();
        lib.insert(crate::Alpha::new("solid", 8, 8, vec![1.0; 64]));

        let d = DecalLayer {
            alpha: "solid".into(),
            decals: vec![Decal { theta_deg: 90.0, v_mm: 4.0, size_mm: 4.0, ..Default::default() }],
            feather_mm: 0.5,
            invert: false,
        };
        let u0 = c.u_of_theta(90.0);
        let centre = d.height(Uv { u: u0, v: 4.0 }, &c, &lib);
        assert!((centre - 0.35).abs() < 1e-9, "full relief at the stamp centre: {centre}");
        assert_eq!(d.height(Uv { u: u0 + 5.0, v: 4.0 }, &c, &lib), 0.0, "clear of the stamp");
        let near_edge = d.height(Uv { u: u0 + 1.9, v: 4.0 }, &c, &lib);
        assert!(near_edge > 0.0 && near_edge < 0.35 * 0.5, "feathered border: {near_edge}");

        // A stamp near the joint reaches across it.
        let wrapped = DecalLayer {
            decals: vec![Decal { theta_deg: 1.0, ..d.decals[0] }],
            ..d.clone()
        };
        let h = wrapped.height(Uv { u: c.circumference_mm - 0.5, v: 4.0 }, &c, &lib);
        assert!(h > 0.0, "stamp did not wrap the joint");

        // A missing alpha contributes nothing rather than panicking.
        let missing = DecalLayer { alpha: "nope".into(), ..d };
        assert_eq!(missing.height(Uv { u: u0, v: 4.0 }, &c, &lib), 0.0);
    }

    #[test]
    fn flutes_peak_mid_cell_and_close_at_the_joint() {
        let c = ctx();
        let f = FlutesLayer { count: 60, ..Default::default() };
        let cell = 1.0; // 60 mm circumference / 60
        let mid = f.height(Uv { u: 0.5 * cell, v: 4.0 }, &c);
        assert!((mid - f.height_mm).abs() < 1e-9, "cell centre carries full depth: {mid}");
        assert_eq!(f.height(Uv { u: 0.02, v: 4.0 }, &c), 0.0, "bare band between flutes");
        for v in [1.0, 4.0, 7.0] {
            let a = f.height(Uv { u: 0.0, v }, &c);
            let b = f.height(Uv { u: 60.0 - 1e-9, v }, &c);
            assert!((a - b).abs() < 1e-6, "joint mismatch at v {v}");
        }
        // Lean shifts the phase across the band without opening the joint.
        let leaned = FlutesLayer { lean: 2.5, ..f };
        let a = leaned.height(Uv { u: 0.0, v: 2.0 }, &c);
        let b = leaned.height(Uv { u: 60.0 - 1e-9, v: 2.0 }, &c);
        assert!((a - b).abs() < 1e-6, "leaned joint mismatch");
    }

    #[test]
    fn a_reeded_band_builds_watertight_without_undercuts() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.layers.layers.push(LayerEntry::new("reeding", Layer::Flutes(FlutesLayer::default())));
        let out = crate::mesh::build(
            &d,
            &lib,
            crate::BuildParams { theta_steps: 384, profile_steps: 96, ..Default::default() },
        );
        assert!(out.report.validation.watertight, "{:?}", out.report.validation);
        let cast = crate::castability::analyze(&out.mesh, &d.draft, d.inner_radius_mm());
        // Reed walls face around the ring, parallel to the pull: they drag,
        // they do not lock.
        assert!(
            cast.undercut_fraction() < 0.002,
            "reeding locked: {:.4}%",
            cast.undercut_fraction() * 100.0
        );
    }

    #[test]
    fn a_group_composites_inside_and_replace_cannot_leak_out() {
        let c = ctx();
        let lib = AlphaLibrary::default();
        let base = LayerEntry::new(
            "base pad",
            Layer::SeatPad(SeatPadLayer { theta_deg: 270.0, v_mm: 4.0, ..Default::default() }),
        );

        // A group whose only child is a Replace border of height zero: inside
        // the group it wipes the group's own composite, and the group then
        // contributes nothing — the base pad underneath must survive.
        let mut wipe = LayerEntry::new(
            "wipe",
            Layer::Border(BorderLayer { height_mm: 0.0, mirror: false, ..Default::default() }),
        );
        wipe.blend = Blend::Replace;
        let mut grp = LayerEntry::new(
            "group",
            Layer::Group(GroupLayer {
                stack: LayerStack { layers: vec![wipe] },
                recipe: None,
            }),
        );
        grp.blend = Blend::Max;

        let stack = LayerStack { layers: vec![base.clone(), grp] };
        let u = c.u_of_theta(270.0);
        let alone = LayerStack { layers: vec![base] }.height(Uv { u, v: 4.0 }, &c, &lib);
        let with_group = stack.height(Uv { u, v: 4.0 }, &c, &lib);
        assert!(alone > 0.0);
        assert_eq!(with_group, alone, "the group's internal Replace stayed internal");
    }

    #[test]
    fn a_painted_mask_scales_the_layer_and_a_missing_one_passes() {
        let c = ctx();
        let mut lib = AlphaLibrary::default();
        // Left half of the band 0, right half 1.
        let data: Vec<f32> =
            (0..64 * 64).map(|i| if (i % 64) < 32 { 0.0 } else { 1.0 }).collect();
        lib.insert(crate::Alpha::new("halves", 64, 64, data));

        let mut e = LayerEntry::new(
            "bordered",
            Layer::Border(BorderLayer { mirror: false, v_mm: 4.0, ..Default::default() }),
        );
        let peak = |e: &LayerEntry, u: f64| {
            let uv = Uv { u, v: 4.0 };
            e.layer.height(uv, &c, &lib) * e.mask_at(uv, &c, &lib)
        };
        let bare = peak(&e, 45.0);
        assert!(bare > 0.0);

        e.mask = Some("halves".into());
        assert_eq!(peak(&e, 15.0), 0.0, "masked-out arc");
        assert!((peak(&e, 45.0) - bare).abs() < 1e-9, "masked-in arc unchanged");

        e.mask = Some("no such alpha".into());
        assert!((peak(&e, 15.0) - bare).abs() < 1e-9, "missing mask passes 1.0");
    }

    #[test]
    fn smooth_max_is_exact_on_ties_and_dominance_and_never_overshoots() {
        let r = 0.4;
        for a in [-0.5, 0.0, 0.7, 1.3] {
            assert_eq!(Blend::SmoothMax.apply(a, a, r), a, "tie at {a}");
        }
        assert_eq!(Blend::SmoothMax.apply(1.0, 0.2, r), 1.0, "clear dominance is exact");
        assert_eq!(Blend::SmoothMin.apply(1.0, 0.2, r), 0.2);
        for i in 0..100 {
            let b = i as f64 * 0.02;
            let m = Blend::SmoothMax.apply(1.0, b, r);
            assert!(m <= 1.0f64.max(b) + 1e-12, "overshoot at b={b}: {m}");
            assert!(m >= 1.0f64.min(b) - 1e-12, "undershoot at b={b}: {m}");
        }
        // Zero radius degenerates to the hard modes.
        assert_eq!(Blend::SmoothMax.apply(1.0, 0.99, 0.0), 1.0);
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

#[cfg(test)]
mod silhouette_tests {
    use super::*;

    /// The extent tables, which are what a signet head's plan silhouette
    /// follows. Printed because reading them is how four separate bugs were
    /// found: a heart squashed to a lens, a shield that was a square, an
    /// outline folded about its own centre, and a crest lying on its side.
    #[test]
    fn every_outline_gives_a_silhouette_of_its_own_shape() {
        let sample = |o: SignetOutline| -> Vec<(f64, f64)> {
            (0..=16).map(|i| o.extent(i as f64 / 8.0 - 1.0)).collect()
        };
        for &o in SignetOutline::ALL {
            let row = sample(o);
            let width: Vec<f64> = row.iter().map(|&(lo, hi)| (hi - lo) * 0.5).collect();
            let text: Vec<String> = width.iter().map(|v| format!("{v:.2}")).collect();
            println!("{:<10} {}", o.label(), text.join(" "));

            // Fills the extents it was given. Not "some station is full
            // width": a heart's widest column runs from its point up to the
            // notch between its lobes, and never spans the whole box at once.
            let top = row.iter().map(|&(_, hi)| hi).fold(0.0f64, f64::max);
            let bottom = row.iter().map(|&(lo, _)| lo).fold(0.0f64, f64::min);
            assert!(
                (top - 1.0).abs() < 0.02 && (bottom + 1.0).abs() < 0.02,
                "{o:?} spans {bottom:.3}..{top:.3} across the band, not -1..1"
            );
            // Wide enough over the middle to be a face rather than a spine.
            assert!(width[8] > 0.55, "{o:?} is pinched at its centre: {:.2}", width[8]);

            if o.upright() {
                // An upright face leaves through its own sides, so it need not
                // close — but it does stand off the band's mid-plane, and that
                // offset is the whole reason its ends carry any width at all.
                let centre: Vec<f64> = row.iter().map(|&(lo, hi)| (hi + lo) * 0.5).collect();
                let worst = centre.iter().cloned().fold(0.0f64, |a, c| a.max(c.abs()));
                assert!(worst > 0.15, "{o:?} is upright but sits centred: {centre:?}");
            } else {
                assert!(
                    width[0] < 0.15 && width[16] < 0.15,
                    "{o:?} does not close at its ends: {text:?}"
                );
            }
        }

        // A crest tapers to its point, so what is left of it out at the sides —
        // where the shank leaves — is its top half, not a centred strip. Get
        // the quarter turn wrong and this comes out symmetric.
        for o in [SignetOutline::Shield, SignetOutline::Heart] {
            let row = sample(o);
            println!(
                "  {o:?} spans {:?} at its sides, {:?} down the middle",
                row[2].0.max(-9.0), row[8]
            );
            for (name, (lo, hi)) in [("-0.75", row[2]), ("+0.75", row[14])] {
                // The wide end sits against the low band edge and the point
                // against the high one, so out at the sides — where the shank
                // leaves — what is left is the top half, not a centred strip.
                let centre = (hi + lo) * 0.5;
                assert!(
                    centre < -0.15 && lo < -0.7,
                    "{o:?} at x {name} spans {lo:.2}..{hi:.2}: it is not standing up"
                );
            }
            // ...and it does reach its point somewhere down the middle.
            assert!(row[8].1 > 0.85, "{o:?} has no point: {:?}", row[8]);
        }
    }

    /// Rescaling an outline to fill its extents moves every boundary point, so
    /// the table has to be read off the moved boundary. A heart is the case
    /// that catches it: its point reaches four times as far as its lobes.
    #[test]
    fn a_polar_outline_fills_its_own_extents() {
        for (name, table) in [("heart", heart_table()), ("shield", shield_table())] {
            let mut lo = [f64::MAX; 2];
            let mut hi = [f64::MIN; 2];
            for (i, &r) in table.r.iter().enumerate() {
                let a = std::f64::consts::TAU * i as f64 / OUTLINE_STEPS as f64;
                for (k, v) in [r * a.cos(), r * a.sin()].into_iter().enumerate() {
                    lo[k] = lo[k].min(v);
                    hi[k] = hi[k].max(v);
                }
            }
            for k in 0..2 {
                assert!(
                    (lo[k] + 1.0).abs() < 0.02 && (hi[k] - 1.0).abs() < 0.02,
                    "{name} spans {:.3}..{:.3} on axis {k}, not -1..1",
                    lo[k],
                    hi[k]
                );
            }
        }
    }

    /// The body a face is a facet of has to **contain** it, everywhere.
    ///
    /// This is the castability of the head's flank, not a nicety: a body
    /// narrower than the table it carries leans that flank back over the mould
    /// half it sits in. Closing is extensive, which is why it can be asserted
    /// rather than tuned.
    ///
    /// Sample by sample it is exact. The tolerance is for the Catmull-Rom
    /// reconstruction, which can dip a few parts in ten million below the
    /// samples where the two curves touch — a heart's lobe, where the body *is*
    /// the face. Three nanometres on a 13 mm head, and `head_at` clamps the
    /// crest into the bore regardless.
    #[test]
    fn the_body_contains_the_face_it_carries() {
        for &o in SignetOutline::ALL {
            let mut worst = (0.0f64, 0.0);
            for i in 0..=2000 {
                let x = -1.0 + 2.0 * i as f64 / 2000.0;
                let (fl, fh) = o.extent(x);
                let (bl, bh) = o.body_extent(x);
                // The body has to reach *further* both ways: lower on the low
                // edge, higher on the high one.
                let out = (bl - fl).max(fh - bh);
                if out > worst.0 {
                    worst = (out, x);
                }
            }
            assert!(
                worst.0 < 1e-5,
                "{o:?}: the face reaches {:.3e} past its body at x {:.3}",
                worst.0,
                worst.1
            );
        }
    }

    /// An imported plan runs the same machinery the builtins are held to, so
    /// it gets the same guarantees: the faired body contains the face it
    /// carries, the head releases from the sand, and the table survives the
    /// file round-trip without its derived silhouette.
    ///
    /// The outline here is deliberately hostile: a five-pointed star with one
    /// lobe clipped, so it is asymmetric, concave at every notch, and not
    /// star-shaped from its own centroid.
    /// A superellipse |x|^4 + |y|^4 = 1 as a closed polygon, optionally tilted
    /// at its +x tip the way a factory cushion is.
    fn superellipse_points(tilt: f64) -> Vec<[f64; 2]> {
        (0..256)
            .map(|k| {
                let a = k as f64 / 256.0 * std::f64::consts::TAU;
                let (s, c) = a.sin_cos();
                let r = 1.0 / (c.abs().powi(4) + s.abs().powi(4)).powf(0.25);
                let (x, y) = (r * c, r * s);
                if x > 0.0 { [x * (1.0 + tilt * y), y] } else { [x, y] }
            })
            .collect()
    }

    #[test]
    fn a_symmetric_plan_reads_symmetric() {
        let poly = superellipse_points(0.0);
        let o = CustomOutline::from_points("sq4", &poly).unwrap();
        for x in [0.9, 0.95, 0.98, 1.0] {
            let (lo, hi) = o.extent_at(x);
            assert!((lo + hi).abs() < 1e-3, "face at {x}: {lo} {hi}");
            let (lo, hi) = o.body_extent_at(x);
            assert!((lo + hi).abs() < 1e-3, "body at {x}: {lo} {hi}");
        }
        // The same shape reversed or turned half round gives the same table.
        let rev: Vec<[f64; 2]> = poly.iter().rev().copied().collect();
        let rot: Vec<[f64; 2]> = poly.iter().map(|p| [-p[0], -p[1]]).collect();
        let o2 = CustomOutline::from_points("rev", &rev).unwrap();
        let o3 = CustomOutline::from_points("rot", &rot).unwrap();
        for i in 0..o.r.len() {
            assert!((o.r[i] - o2.r[i]).abs() < 1e-6, "reversed differs at {i}: {} vs {}", o.r[i], o2.r[i]);
            assert!((o.r[i] - o3.r[i]).abs() < 1e-6, "rotated differs at {i}: {} vs {}", o.r[i], o3.r[i]);
        }
    }

    #[test]
    fn symmetrize_removes_a_drawn_tilt() {
        let mut o = CustomOutline::from_points("tilt", &superellipse_points(0.015)).unwrap();
        let (lo, hi) = o.extent_at(0.98);
        assert!((lo + hi).abs() > 0.05, "the tilt should read: {lo} {hi}");
        o.symmetrize(true, false);
        let (lo, hi) = o.extent_at(0.98);
        assert!((lo + hi).abs() < 1e-3, "{lo} {hi}");
        // Folding along the ring too leaves a symmetric shape symmetric.
        o.symmetrize(true, true);
        let (lo, hi) = o.extent_at(0.9);
        assert!((lo + hi).abs() < 1e-3, "{lo} {hi}");
    }

    /// The import sizes the fairing ball from the plan's own hull defect,
    /// as the exporter does, so a drawn clover goes onto the cut dome
    /// without anyone setting a number.
    #[test]
    fn fair_r_follows_the_hull_defect() {
        let ring = |f: &dyn Fn(f64) -> f64| -> Vec<[f64; 2]> {
            (0..360).map(|k| { let t = (k as f64).to_radians(); let r = f(t); [r * t.cos(), r * t.sin()] }).collect()
        };
        let circle = ring(&|_| 1.0);
        assert!(hull_defect(&circle) < 0.01);
        assert!((fair_r_for(hull_defect(&circle)) - default_fair_r()).abs() < 1e-9, "a convex plan keeps the default");
        let square: Vec<[f64; 2]> = vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        assert!(hull_defect(&square) < 1e-9);
        let clover = ring(&|t| 0.62 + 0.38 * (4.0 * t).cos());
        let d = hull_defect(&clover);
        assert!(d > 0.15, "four deep lobes: defect {d}");
        assert_eq!(fair_r_for(d), default_fair_r() + 1.75);
        let o = CustomOutline::from_points("Clover", &clover).unwrap();
        assert!(o.fair_r > 1.2, "{}", o.fair_r);
        let mut shank = crate::profile::ShankStyle::default();
        let v = shank.adopt_outline(o);
        assert_eq!(shank.suggest_dome(v), 1.0);
        let oval = CustomOutline::from_points("Oval", &ring(&|t| 1.0 / (t.cos().powi(2) + (1.4 * t.sin()).powi(2)).sqrt())).unwrap();
        assert!((oval.fair_r - default_fair_r()).abs() < 1e-9);
    }

    #[test]
    fn a_drawn_outline_makes_a_head_that_pulls() {
        // The hostile plan, as a closed polyline.
        let pts: Vec<[f64; 2]> = (0..720)
            .map(|i| {
                let a = i as f64 / 720.0 * std::f64::consts::TAU;
                let five = 0.62 + 0.38 * (5.0 * a).cos();
                // One lobe clipped: flatten the reach on a 70 degree window.
                let clip = 1.0 - 0.55 * crate::field::smoothstep(0.0, 1.0,
                    1.0 - (wrap_delta(a.to_degrees() - 36.0, 360.0).abs() / 35.0).min(1.0));
                let r = five * clip;
                [r * a.cos(), r * a.sin()]
            })
            .collect();
        let co = CustomOutline::from_points("Clipped star", &pts).expect("a real boundary");
        assert!(co.aspect > 0.1 && co.aspect < 10.0);

        // Containment: the same assertion the builtins are pinned by.
        let mut worst = (0.0f64, 0.0);
        for i in 0..=2000 {
            let x = -1.0 + 2.0 * i as f64 / 2000.0;
            let (fl, fh) = co.extent_at(x);
            let (bl, bh) = co.body_extent_at(x);
            let out = (bl - fl).max(fh - bh);
            if out > worst.0 {
                worst = (out, x);
            }
        }
        assert!(worst.0 < 1e-5, "the face reaches {:.3e} past its body at x {:.3}", worst.0, worst.1);

        // On a head: castable, and what little is reported sits on the crest
        // line — the phantom-vs-real discriminator.
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.width_mm = 8.0;
        d.profile.thickness_mm = 2.2;
        d.profile.flatten_sides();
        d.shank.apply_signet(8.0);
        d.shank.head.loft = 0.0; // The prism and the cut dome are what this tests.
        let o = d.shank.adopt_outline(co.clone());
        d.shank.head.outline = o;
        d.shank.head.length_mm = (8.0 * d.shank.outline_aspect(o)).clamp(2.0, 40.0);
        let lib = crate::AlphaLibrary::builtin();
        let v = crate::castability::analyze_field(&d, &lib, &d.draft, 288, 144);
        assert_ne!(v.verdict, crate::castability::Verdict::NotCastable, "{:?}", v.notes);
        assert!(
            v.undercut_fraction() < 5e-4,
            "a custom head locks: {:.4}%",
            v.undercut_fraction() * 100.0
        );

        // Round-trip: the table survives, the derived silhouette does not.
        let json = serde_json::to_string(&d).unwrap();
        assert!(!json.contains("cache"), "the derived table must not persist");
        let back: crate::RingDesign = serde_json::from_str(&json).unwrap();
        let c2 = back.shank.custom_outline(o).expect("the registry travels");
        assert_eq!(c2.r, d.shank.custom_outline(o).unwrap().r);
        for i in 0..=64 {
            let x = -1.0 + 2.0 * i as f64 / 64.0;
            let (a0, b0) = d.shank.outline_extent(o, x);
            let (a1, b1) = back.shank.outline_extent(o, x);
            assert!((a0 - a1).abs() < 1e-12 && (b0 - b1).abs() < 1e-12, "rebuilt table differs at {x}");
        }

        // A deeply lobed plan defaults onto the cut dome — the body is one
        // smooth lens and the lobes read in the arris — and it fields clean
        // there too.
        let mut lobed = co.clone();
        assert!(lobed.fair_r > 1.2, "a clipped star's hull defect sizes its own ball: {}", lobed.fair_r);
        lobed.fair_r = 2.0;
        let mut d2 = d.clone();
        let o2 = d2.shank.adopt_outline(lobed);
        d2.shank.head.outline = o2;
        assert_eq!(d2.shank.suggest_dome(o2), 1.0, "a clipped star is lobed");
        d2.shank.head.dome = 1.0;
        let v3 = crate::castability::analyze_field(&d2, &lib, &d2.draft, 192, 96);
        assert!(
            v3.undercut_fraction() < 5e-4,
            "the domed custom head locks: {:.4}%",
            v3.undercut_fraction() * 100.0
        );

        // And a design with no registry entry falls back instead of panicking.
        let mut bare = crate::RingDesign::default();
        bare.shank.apply_signet(6.0);
        bare.shank.head.outline = SignetOutline::Custom(7);
        let _ = bare.shank.outline_extent(SignetOutline::Custom(7), 0.3);
        let v2 = crate::castability::analyze_field(&bare, &lib, &bare.draft, 96, 48);
        assert_ne!(v2.verdict, crate::castability::Verdict::NotCastable);
    }

    /// A heart's dimple belongs to the **face**. Carried down to the finger it
    /// is a heart-shaped prism, which is what a signet is not.
    ///
    /// It reads on the low edge: the two lobes reach -1 at a third of the way
    /// out either side, and the notch between them holds that edge back to
    /// -0.79 at the head's centre.
    #[test]
    fn the_body_has_no_dimple() {
        let o = SignetOutline::Heart;
        let notch = |e: &dyn Fn(f64) -> (f64, f64)| {
            e(0.0).0 - (0..=400).map(|i| e(i as f64 / 400.0).0).fold(0.0f64, f64::min)
        };
        let (face, body) = (notch(&|x| o.extent(x)), notch(&|x| o.body_extent(x)));
        println!("heart notch: face {face:.4}, body {body:.4}");
        assert!(face > 0.2, "the face has no notch to fair: {face:.4}");
        assert!(
            body < face * 0.45 && body < 0.08,
            "the body still carries the dimple: {body:.4} of the face's {face:.4}"
        );

        // And it fairs rather than plateaus, which is the difference between a
        // rolling ball and a flat window. A flat one fills a hollow to the level
        // of its rim and holds it *exactly* there, so the run of identical
        // samples is what gives it away — with one, the low edge held the same
        // value over 122 of these stations, a straight parallel band edge with a
        // cliff where it met the shoulder. A ball leaves an arc, which is never
        // twice the same.
        let lo = |i: usize| o.body_extent(i as f64 / 200.0).0;
        let (mut run, mut worst) = (0, 0);
        for i in 1..=200 {
            run = if (lo(i) - lo(i - 1)).abs() < 1e-12 { run + 1 } else { 0 };
            worst = worst.max(run);
        }
        assert!(worst < 8, "the body's low edge holds one value over {worst} stations");
    }
}

#[cfg(test)]
mod tilt_tests {
    use super::*;
    use crate::gem::{Gem, GemCut};
    use crate::{LayerEntry, RingDesign};

    fn princess_row(d: &RingDesign, v_mm: f64, tilt: f64) -> SeatRunLayer {
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Princess, 2.5);
        run.seat.v_mm = v_mm;
        run.tilt_deg = tilt;
        run.solve_spacing(&ctx);
        run
    }

    #[test]
    fn a_tilted_run_turns_every_stone_and_repacks_to_its_reach() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let straight = princess_row(&d, ctx.crest_v_mm, 0.0);
        let tilted = princess_row(&d, ctx.crest_v_mm, 45.0);
        assert!(tilted.count < straight.count, "a square on the diagonal spans more of the ring: {} vs {}", tilted.count, straight.count);
        // A princess plan is a rounded square: its diagonal support is 1.19x, not the box's 1.41x.
        assert!(tilted.seat_span_mm() > straight.seat_span_mm() * 1.1);
        assert!(tilted.turned(tilted.seat).half_extents_mm().1 > straight.seat.half_extents_mm().1, "the reach across the band grows too");
        assert!(tilted.bridge_at(&ctx) >= tilted.bridge_mm - 1e-6, "the row holds at least the bridge it solved for");
        assert!(tilted.bridge_at(&ctx) < tilted.bridge_mm + 0.6);
        // The mound's peak is the seat's height whichever way it is turned.
        for run in [&straight, &tilted] {
            let uv = Uv { u: ctx.u_of_theta(run.theta_of_station(0.0, &ctx)), v: run.seat.v_mm };
            assert!((run.height(uv, &ctx) - run.seat.height_mm).abs() < 1e-6);
        }
        d.layers.layers.push(LayerEntry::new("Row", Layer::SeatRun(tilted)));
        let stones = crate::setstone::set_stones(&d);
        assert_eq!(stones.len() as u32, tilted.count);
        assert!(stones.iter().all(|s| (s.seat.rot_deg - (tilted.seat.rot_deg + 45.0)).abs() < 1e-9));
        let report = crate::stones::report(&d, 0.0).unwrap();
        assert!((report.seats[0].bridge_mm.unwrap() - tilted.bridge_at(&ctx)).abs() < 1e-9, "the report says what the row holds");
    }

    #[test]
    fn a_tilted_row_on_the_crest_and_on_a_side_face_still_pulls() {
        let lib = crate::AlphaLibrary::builtin();
        let mut crest = RingDesign::default();
        let ctx = crest.field_context();
        let run = princess_row(&crest, ctx.crest_v_mm, 45.0);
        crest.layers.layers.push(LayerEntry::new("Row", Layer::SeatRun(run)));
        let f = crate::castability::analyze_field(&crest, &lib, &crest.draft, 192, 128);
        assert!(f.undercut_fraction() < 5e-4, "crest: {}", f.undercut_fraction());

        // A face wide enough to hold the row: the 7x5 squared band of the
        // arc-scale table, 4 mm of face against the turned stone's 2 mm reach.
        let mut side = RingDesign::default();
        side.profile.width_mm = 7.0;
        side.profile.thickness_mm = 5.0;
        side.profile.apply_style(crate::ProfileStyle::Flat);
        side.profile.flatten_sides();
        let ctx = side.field_context();
        let sf = ctx.side_faces_std().expect("a squared band has side faces");
        let face = sf.low.or(sf.high).expect("a side run");
        let run = princess_row(&side, 0.5 * (face.0 + face.1), 45.0);
        side.layers.layers.push(LayerEntry::new("Row", Layer::SeatRun(run)));
        let f = crate::castability::analyze_field(&side, &lib, &side.draft, 192, 128);
        assert!(f.undercut_fraction() < 5e-4, "side face: {}", f.undercut_fraction());
    }

    #[test]
    fn a_run_saved_without_a_tilt_reads_zero() {
        let mut v = serde_json::to_value(SeatRunLayer::default()).unwrap();
        v.as_object_mut().unwrap().remove("tilt_deg");
        let run: SeatRunLayer = serde_json::from_value(v).unwrap();
        assert_eq!(run.tilt_deg, 0.0);
    }
}
