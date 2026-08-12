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
                    min_feature_mm: cw.min(ch).max(0.1),
                    u_mm: None,
                    v_mm: l.v_bounds(),
                }]
            }
            Layer::Border(l) => {
                let v = (l.v_mm - l.width_mm * 0.5, l.v_mm + l.width_mm * 0.5);
                let f = |v| FeatureFootprint {
                    min_feature_mm: l.width_mm.max(0.1),
                    u_mm: None,
                    v_mm: v,
                };
                if l.mirror { vec![f(v), f(mirrored(v))] } else { vec![f(v)] }
            }
            Layer::Milgrain(l) => {
                let half = l.bead_diameter_mm * 0.5;
                let v = (l.v_mm - half, l.v_mm + half);
                let f = |v| FeatureFootprint {
                    min_feature_mm: l.bead_diameter_mm.max(0.1),
                    u_mm: None,
                    v_mm: v,
                };
                if l.mirror { vec![f(v), f(mirrored(v))] } else { vec![f(v)] }
            }
            Layer::SeatPad(l) => {
                let reach = l.diameter_mm * 0.5 + l.blend_mm.max(0.0);
                let u0 = ctx.u_of_theta(l.theta_deg);
                vec![FeatureFootprint {
                    min_feature_mm: l.blend_mm.clamp(0.15, l.diameter_mm.max(0.15)),
                    u_mm: Some((u0 - reach, u0 + reach)),
                    v_mm: (l.v_mm - reach, l.v_mm + reach),
                }]
            }
            Layer::Signet(l) => {
                let reach = l.reach_mm();
                let u0 = ctx.u_of_theta(l.theta_deg);
                let half_w = l.width_mm * 0.5 + l.shoulder_mm;
                vec![FeatureFootprint {
                    min_feature_mm: l.shoulder_mm.max(0.15),
                    u_mm: Some((u0 - reach, u0 + reach)),
                    v_mm: (l.v_mm - half_w, l.v_mm + half_w),
                }]
            }
            Layer::Group(g) => g.stack.feature_footprints(ctx),
            Layer::Curve(l) => l.feature_footprints(ctx),
            Layer::Flutes(l) => vec![FeatureFootprint {
                min_feature_mm: l.width_mm.max(0.1),
                u_mm: None,
                v_mm: (0.0, ctx.band_v_len_mm),
            }],
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
#[derive(Clone, Copy, Debug)]
pub struct FeatureFootprint {
    /// Smallest feature the layer produces here, mm.
    pub min_feature_mm: f64,
    /// Arc extent around the ring, mm at the crest radius; may extend past the
    /// wrap. `None` covers the whole ring.
    pub u_mm: Option<(f64, f64)>,
    /// Extent across the band surface, mm.
    pub v_mm: (f64, f64),
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
}

fn default_bezel_wall() -> f64 {
    0.5
}

fn default_recess() -> f64 {
    0.4
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
        }
    }
}

impl SeatPadLayer {
    /// Diameter of the largest stone this pad can reasonably seat, mm.
    pub fn suggested_stone_mm(&self) -> f64 {
        match self.style {
            SeatStyle::Bezel => (self.diameter_mm - 2.0 * self.bezel_wall_mm).max(0.5),
            _ => (self.diameter_mm - 1.2).max(0.5),
        }
    }

    /// Size the pad for a chosen stone instead of inferring the stone from
    /// the pad — a bezel needs its walls around the girdle, a boss needs its
    /// drilling allowance around the seat.
    pub fn fit_stone(&mut self, gem: crate::gem::Gem) {
        let w = gem.w_mm.max(0.5);
        self.diameter_mm = match self.style {
            SeatStyle::Bezel => w + 2.0 * self.bezel_wall_mm.max(0.2),
            SeatStyle::Boss => w + 1.2,
            SeatStyle::GypsyMound => w + 1.8,
        };
        if self.style == SeatStyle::GypsyMound {
            self.crown = 1.0;
        }
        self.gem = Some(gem);
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let r = (self.diameter_mm * 0.5).max(1e-6);
        let blend = self.blend_mm.max(0.0);
        let u0 = ctx.u_of_theta(self.theta_deg);
        let du = wrap_delta(uv.u - u0, ctx.circumference_mm);
        let dv = uv.v - self.v_mm;
        let d = (du * du + dv * dv).sqrt();
        if d > r + blend + self.prong_mm {
            return 0.0;
        }

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
        // Drafted cone bumps on the seat circle, nearest-centre like milgrain.
        let prong_r = (0.28 * r).clamp(0.35, 0.8);
        let ring_r = (r - prong_r * 0.6).max(0.2);
        let a0 = dv.atan2(du);
        let step = std::f64::consts::TAU / n as f64;
        let a_near = (a0 / step).round() * step;
        let (px, py) = (ring_r * a_near.cos(), ring_r * a_near.sin());
        let dp = ((du - px).powi(2) + (dv - py).powi(2)).sqrt();
        let t = (dp / prong_r).clamp(0.0, 1.0);
        let prong =
            (self.height_mm + self.prong_mm) * (0.5 + 0.5 * (std::f64::consts::PI * t).cos());
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
                FeatureFootprint {
                    min_feature_mm: (d.size_mm * 0.15).max(0.15),
                    u_mm: Some((u0 - reach, u0 + reach)),
                    v_mm: (d.v_mm - reach, d.v_mm + reach),
                }
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
        Self { seat, count: 18, gem, bridge_mm: 0.4 }
    }
}

impl SeatRunLayer {
    /// Solve the count from the stone: fit the seat first, then take the most
    /// stations of that seat plus the bridge that fit the ring.
    pub fn solve_spacing(&mut self, ctx: &FieldContext) {
        self.seat.fit_stone(self.gem);
        let pitch = self.seat.diameter_mm.max(0.5) + self.bridge_mm.max(0.0);
        self.count = ((ctx.circumference_mm / pitch).floor() as u32).clamp(3, 200);
    }

    /// Bridge actually left between neighbouring seats at the current count.
    pub fn bridge_at(&self, ctx: &FieldContext) -> f64 {
        let pitch = ctx.circumference_mm / self.count.max(1) as f64;
        pitch - self.seat.diameter_mm
    }

    pub fn height(&self, uv: Uv, ctx: &FieldContext) -> f64 {
        let n = self.count.clamp(1, 200) as f64;
        if !(ctx.circumference_mm > 1e-9) || !uv.u.is_finite() {
            return 0.0;
        }
        let pitch_deg = 360.0 / n;
        let theta = uv.u / ctx.circumference_mm * 360.0;
        // Nearest station and its neighbours, so a generous skirt cannot
        // clip at the cell boundary.
        let k = (theta / pitch_deg).round();
        let mut h: f64 = 0.0;
        for dk in [-1.0, 0.0, 1.0] {
            let mut s = self.seat;
            s.theta_deg = (k + dk) * pitch_deg;
            s.v_mm = self.seat.v_mm;
            h = h.max(s.height(uv, ctx));
        }
        h
    }

    pub fn feature_footprints(&self, _ctx: &FieldContext) -> Vec<FeatureFootprint> {
        let reach = self.seat.diameter_mm * 0.5 + self.seat.blend_mm;
        vec![FeatureFootprint {
            min_feature_mm: (self.seat.diameter_mm * 0.2).max(0.15),
            u_mm: None,
            v_mm: (self.seat.v_mm - reach, self.seat.v_mm + reach),
        }]
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
}

/// Steps in a polar boundary table. One per 0.5 degree.
const OUTLINE_STEPS: usize = 720;

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
        let mut lo = [f64::MAX; 2];
        let mut hi = [f64::MIN; 2];
        for p in &raw {
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
        let mut r = [1e-6f64; OUTLINE_STEPS];
        for (i, slot) in r.iter_mut().enumerate() {
            let a = i as f64 * step;
            let (sin_a, cos_a) = a.sin_cos();
            for k in 0..OUTLINE_STEPS {
                let (p, q) = (b[k], b[(k + 1) % OUTLINE_STEPS]);
                let (ex, ey) = (q[0] - p[0], q[1] - p[1]);
                // Ray x segment: the ray's own cross product vanishes on it.
                let den = cos_a * ey - sin_a * ex;
                if den.abs() <= 1e-12 {
                    continue;
                }
                let t = (sin_a * p[0] - cos_a * p[1]) / den;
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let hit = (p[0] + t * ex) * cos_a + (p[1] + t * ey) * sin_a;
                if hit > *slot {
                    *slot = hit;
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
        silhouette(self).at(x)
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
        silhouette(self).body_at(x)
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
fn fair(src: &[f64; SILHOUETTE_STEPS], sign: f64) -> [f64; SILHOUETTE_STEPS] {
    let n = SILHOUETTE_STEPS as isize;
    // `x` spans 2 over n-1 steps, so a radius in stations is half that in cells.
    let cells = |r: f64| ((r * 0.5 * (n - 1) as f64) as isize).max(1);
    let mut v = *src;
    for x in v.iter_mut() {
        *x *= sign;
    }

    let closed = roll(&roll(&v, BODY_FAIR_R, true), BODY_FAIR_R, false);
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
                    if o.distance_norm(x, y) > 1.0 {
                        continue;
                    }
                    let (mut inside, mut outside) = (y, y + side / SCAN as f64);
                    for _ in 0..BISECT {
                        let mid = 0.5 * (inside + outside);
                        if o.distance_norm(x, mid) <= 1.0 {
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
        let (body_lo, body_hi) = (fair(&lo, -1.0), fair(&hi, 1.0));
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
    static T: [std::sync::OnceLock<Silhouette>; 9] =
        [const { std::sync::OnceLock::new() }; 9];
    T[o.index()].get_or_init(|| Silhouette::build(o))
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
            Layer::Group(GroupLayer { stack: LayerStack { layers: vec![wipe] } }),
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
