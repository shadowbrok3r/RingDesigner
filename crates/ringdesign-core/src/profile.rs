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
            ProfileStyle::Custom => "Hand-tuned superellipse exponents.",
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

    /// Normalized drop from the crest, `x` in 0..1. Monotonically increasing.
    pub fn drop(&self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        let a = self.shape_a.max(0.05);
        let b = self.shape_b.max(0.05);
        (1.0 - (1.0 - x.powf(a)).max(0.0).powf(1.0 / b)).clamp(0.0, 1.0)
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
        self.sample_spaced(inner_r, n, m, None)
    }

    /// Sample the cross-section, optionally placing the vertices by height
    /// field detail across `v` rather than at equal arc length.
    ///
    /// `field_v` is indexed by normalized `v`, which is what the layer stack is
    /// evaluated against, so one density computed from the reference profile
    /// applies to every modulated cross-section.
    pub fn sample_spaced(
        &self,
        inner_r: f64,
        n: usize,
        m: &ShankMod,
        field_v: Option<&Density>,
    ) -> ProfileLoop {
        let n = n.clamp(MIN_PROFILE_STEPS, MAX_PROFILE_STEPS);
        let width = (self.width_mm * m.width_scale).max(0.4);
        let thickness = (self.thickness_mm * m.thickness_scale).max(0.3);

        let hw = width * 0.5;
        let comfort = self.comfort_fit_mm.clamp(0.0, hw * 0.8);
        // The comfort dome eats into the band from the inside, so the crown may
        // only take what it leaves. Without this the bore reaches past the outer
        // surface at the band edge and the cross-section folds over itself — a
        // 0.25 mm comfort fit does not fit inside a 0.2 mm edge.
        let crown = (self.crown_mm * m.thickness_scale * m.crown_scale)
            .clamp(0.0, (thickness - comfort - MIN_EDGE_MM).max(0.0));
        let edge_t = (thickness - crown).max(MIN_EDGE_MM + comfort);
        let draft = self.side_draft_deg.clamp(-20.0, 30.0).to_radians();
        // Side faces slope inward over the edge thickness, narrowing the band.
        let side_inset = (edge_t * draft.tan()).clamp(-hw * 0.4, hw * 0.4);
        let hwo = (hw - side_inset).max(hw * 0.15);

        let base_crest_t = (0.5 + 0.5 * self.crest_bias.clamp(-1.0, 1.0)).clamp(0.06, 0.94);
        let flange_v = self.flange.v_pos.clamp(0.0, 1.0);
        // A castable flange position takes the crest with it.
        let crest_t = match self.flange.enabled {
            true if flange_v <= EDGE_FLANGE_T => 0.0,
            true if flange_v >= 1.0 - EDGE_FLANGE_T => 1.0,
            true if (flange_v - base_crest_t).abs() <= CREST_FLANGE_T => flange_v,
            _ => base_crest_t,
        };
        let crest_z = -hwo + 2.0 * hwo * crest_t;

        let cap_r = |r: f64| match m.outer_max_r {
            Some(cap) => r.min(cap.max(inner_r + MIN_EDGE_MM)),
            None => r,
        };
        let r_at = |z: f64| -> f64 {
            let z = z.clamp(-hwo, hwo);
            let x = if z <= crest_z {
                (crest_z - z) / (crest_z + hwo).max(1e-9)
            } else {
                (z - crest_z) / (hwo - crest_z).max(1e-9)
            };
            cap_r(inner_r + thickness - crown * self.drop(x))
        };
        let bore_r = |z: f64| -> f64 { inner_r + comfort * (z / hw.max(1e-9)).powi(2) };

        // --- Flange band, clamped to sit inside the outer profile. ---
        let flange = self.flange.enabled.then(|| {
            let max_t = 2.0 * hwo * 0.8;
            let t = self.flange.thickness_mm.clamp(MIN_EDGE_MM.min(max_t), max_t);
            let z_c = -hwo + 2.0 * hwo * flange_v;
            let z_lo = (z_c - 0.5 * t).clamp(-hwo, hwo - t);
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
                let z = hw - 2.0 * hw * (i as f64 / nb as f64);
                [bore_r(z), z]
            })
            .collect();

        // --- Surface span: bottom side face, over the crown, top side face. ---
        let corner_b = [inner_r + edge_t, -hwo];
        let corner_t = [inner_r + edge_t, hwo];
        let side_b_start = [bore_r(-hw), -hw];
        let side_t_end = [bore_r(hw), hw];

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

        let outer: Vec<[f64; 2]> = match &flange {
            None => dome(-hwo, hwo, ns),
            Some(f) => {
                let lo_span = f.z_lo + hwo;
                let hi_span = hwo - f.z_hi;
                let span = (lo_span + hi_span).max(1e-9);
                let steps = |s: f64| ((ns as f64 * s / span) as usize).max(2);
                let fillet = |dome_span: f64, flat: f64| {
                    self.flange.edge_round_mm.clamp(0.0, (dome_span.min(flat) * 0.4).max(0.0))
                };
                let mut o: Vec<[f64; 2]> = Vec::with_capacity(ns + 4 * ARC_STEPS);
                if lo_span > 1e-9 {
                    let fr = fillet(lo_span, f.rim_r - r_at(f.z_lo));
                    let d = dome(-hwo, f.z_lo - fr, steps(lo_span));
                    let p0 = *d.last().unwrap_or(&corner_b);
                    o.extend(d);
                    push_arc(&mut o, p0, [r_at(f.z_lo), f.z_lo], [r_at(f.z_lo) + fr, f.z_lo]);
                } else {
                    o.push(corner_b);
                }
                o.push([f.rim_r, f.z_lo]);
                o.push([f.rim_r, f.z_hi]);
                if hi_span > 1e-9 {
                    let fr = fillet(hi_span, f.rim_r - r_at(f.z_hi));
                    let d = dome(f.z_hi + fr, hwo, steps(hi_span));
                    let p0 = [r_at(f.z_hi) + fr, f.z_hi];
                    o.push(p0);
                    push_arc(&mut o, p0, [r_at(f.z_hi), f.z_hi], d[0]);
                    o.extend(d.into_iter().skip(1));
                } else {
                    o.push(corner_t);
                }
                o
            }
        };

        let er = self.edge_round_mm.clamp(0.0, thickness.min(hwo) * 0.45);
        // Each end fillet is capped by the dome the flange leaves at that edge.
        let (er_b, er_t) = match &flange {
            Some(f) => (er.min((f.z_lo + hwo) * 0.4), er.min((hwo - f.z_hi) * 0.4)),
            None => (er, er),
        };
        let mut surface: Vec<[f64; 2]> = Vec::with_capacity(outer.len() + 4 * DENSE + 4);
        surface.push(side_b_start);
        push_fillet(&mut surface, side_b_start, corner_b, &outer, false, er_b);
        surface.extend_from_slice(trim_outer(&outer, er_b, er_t));
        push_fillet(&mut surface, side_t_end, corner_t, &outer, true, er_t);
        surface.push(side_t_end);
        dedup(&mut surface);

        // --- Place the vertex budget. ---
        let len_b = polyline_len(&bore);
        let len_s = polyline_len(&surface);

        let pts: Vec<ProfileSample> = match field_v {
            // Equal arc length: the budget goes by span length and the samples
            // sit evenly along each.
            None => {
                let total = (len_b + len_s).max(1e-9);
                let n_s = ((n as f64 * len_s / total).round() as usize).clamp(12, n - 12);
                let even = |c: usize| (0..c).map(|i| i as f64 / c as f64).collect::<Vec<f64>>();
                let bore_pts = resample_at(&bore, &even(n - n_s));
                let surf_pts = resample_at(&surface, &even(n_s));
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

        finish_loop(pts)
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
    Cathedral,
    EuroFlat,
    Signet,
}

impl ShankKind {
    pub const ALL: &'static [ShankKind] = &[
        ShankKind::Uniform,
        ShankKind::Tapered,
        ShankKind::ReverseTaper,
        ShankKind::Cathedral,
        ShankKind::EuroFlat,
        ShankKind::Signet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShankKind::Uniform => "Uniform",
            ShankKind::Tapered => "Tapered",
            ShankKind::ReverseTaper => "Reverse Taper",
            ShankKind::Cathedral => "Cathedral",
            ShankKind::EuroFlat => "Euro (flat bottom)",
            ShankKind::Signet => "Signet",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ShankKind::Uniform => "Constant section all the way around.",
            ShankKind::Tapered => "Narrows toward the bottom of the finger.",
            ShankKind::ReverseTaper => "Narrows toward the top, widening at the palm.",
            ShankKind::Cathedral => "Shoulders swell toward the top of the ring.",
            ShankKind::EuroFlat => "Flat chord across the bottom so the ring will not spin.",
            ShankKind::Signet => {
                "Narrow shank swelling into a broad head at the top. The band width is the \
                 head outline, so set Width to the head and let this taper the rest."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ShankStyle {
    pub kind: ShankKind,
    /// Strength of the modulation, 0..1.
    pub amount: f64,
    /// Arc the head spans before it has fallen to shank width, degrees.
    /// Signet only.
    #[serde(default = "default_head_span")]
    pub head_span_deg: f64,
    /// Fullness of the head outline, as the superellipse exponent of its plan
    /// view: 2 is an oval, 4 a cushion, 8 a rectangle. Signet only.
    #[serde(default = "default_head_shape")]
    pub head_shape_a: f64,
}

fn default_head_span() -> f64 {
    HEAD_SPAN_DEG
}

fn default_head_shape() -> f64 {
    HEAD_SHAPE_A
}

/// Arc a signet head spans before it reaches shank width, degrees.
pub const HEAD_SPAN_DEG: f64 = 104.0;
/// Default head outline fullness: an oval.
pub const HEAD_SHAPE_A: f64 = 2.0;
/// Shank width as a fraction of the head at full strength.
pub const SIGNET_MIN_SHANK_FRAC: f64 = 0.16;
/// How much a signet shank rounds off as it narrows. The crown clamp caps it.
pub const SIGNET_SHANK_ROUNDING: f64 = 9.0;

impl Default for ShankStyle {
    fn default() -> Self {
        Self {
            kind: ShankKind::Uniform,
            amount: 0.5,
            head_span_deg: HEAD_SPAN_DEG,
            head_shape_a: HEAD_SHAPE_A,
        }
    }
}

impl ShankStyle {
    /// Fraction of the band width left at a ring angle by a signet taper.
    ///
    /// Seen from outside, the band's own silhouette *is* the head outline, so
    /// the width follows a superellipse in plan: `1 - x^a` over the head arc,
    /// full width at the top falling to shank width at its edge. `a` is the
    /// same fullness exponent the table outlines use — 2 oval, 4 cushion, 8
    /// rectangle.
    pub fn signet_width_frac(&self, theta_deg: f64) -> f64 {
        let k = self.amount.clamp(0.0, 1.0);
        let shank = 1.0 - (1.0 - SIGNET_MIN_SHANK_FRAC) * k;
        let d = crate::field::wrap_delta(theta_deg - TOP_DEG, 360.0).abs();
        let half = (self.head_span_deg.max(1.0) * 0.5).min(180.0);
        if d >= half {
            return shank;
        }
        let x = (d / half).clamp(0.0, 1.0);
        let a = self.head_shape_a.clamp(1.0, 12.0);
        let head = (1.0 - x.powf(a)).clamp(0.0, 1.0);
        shank + (1.0 - shank) * head
    }
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
    /// Hard radial cap, used by the Euro flat chord.
    pub outer_max_r: Option<f64>,
}

impl ShankMod {
    pub fn identity() -> Self {
        Self { width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0, outer_max_r: None }
    }
}

impl ShankStyle {
    /// Modulation at a ring angle. `base_outer_r` is the unmodulated crest
    /// radius, used to position the Euro chord.
    pub fn modulation(&self, theta_deg: f64, base_outer_r: f64) -> ShankMod {
        let k = self.amount.clamp(0.0, 1.0);
        // 0 at the top of the ring, 1 at the bottom of the shank.
        let d = ((theta_deg - TOP_DEG).to_radians().cos() * -0.5 + 0.5).clamp(0.0, 1.0);
        match self.kind {
            ShankKind::Uniform => ShankMod::identity(),
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
                let w = self.signet_width_frac(theta_deg);
                // The shank keeps most of its thickness as it narrows, which is
                // what leaves a round wire at the back rather than a ribbon.
                // The narrowing section rounds off toward a wire, so a flat
                // head can sit on a round shank. The crown clamp caps it at a
                // full dome, so this only ever means "more domed here".
                ShankMod {
                    width_scale: w,
                    thickness_scale: 1.0 - 0.22 * k * (1.0 - w),
                    crown_scale: 1.0 + SIGNET_SHANK_ROUNDING * k * (1.0 - w),
                    outer_max_r: None,
                }
            }
        }
    }
}

// --- Polyline helpers ------------------------------------------------------

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
fn push_fillet(
    out: &mut Vec<[f64; 2]>,
    side_end: [f64; 2],
    corner: [f64; 2],
    outer: &[[f64; 2]],
    top: bool,
    er: f64,
) {
    if er <= 1e-9 || outer.len() < 4 {
        if !top {
            out.push(corner);
        }
        return;
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
fn finish_loop(mut pts: Vec<ProfileSample>) -> ProfileLoop {
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
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn a_signet_shank_is_widest_at_the_top_and_narrowest_at_the_bottom() {
        let sh = ShankStyle { kind: ShankKind::Signet, amount: 0.85, ..Default::default() };
        let top = sh.signet_width_frac(TOP_DEG);
        let bottom = sh.signet_width_frac(TOP_DEG + 180.0);
        assert!((top - 1.0).abs() < 1e-12, "the head is not full width: {top}");
        assert!(bottom < 0.30, "the shank is not narrow enough: {bottom}");

        // Monotone from head to shank, and symmetric either side.
        let mut last = top;
        for step in 1..=36 {
            let d = step as f64 * 5.0;
            let w = sh.signet_width_frac(TOP_DEG + d);
            assert!(w <= last + 1e-12, "width grew again at {d} deg: {w} after {last}");
            let mirror = sh.signet_width_frac(TOP_DEG - d);
            assert!((w - mirror).abs() < 1e-12, "lopsided at {d} deg: {w} vs {mirror}");
            last = w;
        }
    }

    #[test]
    fn a_fuller_head_exponent_holds_the_width_further_round() {
        let base = ShankStyle { kind: ShankKind::Signet, amount: 0.85, ..Default::default() };
        let oval = ShankStyle { head_shape_a: 2.0, ..base };
        let cushion = ShankStyle { head_shape_a: 4.0, ..base };
        let rect = ShankStyle { head_shape_a: 8.0, ..base };
        let at = TOP_DEG + base.head_span_deg * 0.25;
        let (o, c, r) = (
            oval.signet_width_frac(at),
            cushion.signet_width_frac(at),
            rect.signet_width_frac(at),
        );
        assert!(o < c && c < r, "fullness did not order: oval {o}, cushion {c}, rect {r}");
        // All still reach full width at the top and shank width at the edge.
        for sh in [oval, cushion, rect] {
            assert!((sh.signet_width_frac(TOP_DEG) - 1.0).abs() < 1e-12);
            assert!(sh.signet_width_frac(TOP_DEG + 180.0) < 0.30);
        }
    }

    #[test]
    fn a_signet_shank_rounds_off_as_it_narrows() {
        let mut p = BandProfile::default();
        p.apply_style(ProfileStyle::Flat);
        p.width_mm = 12.0;
        p.thickness_mm = 2.8;
        let sh = ShankStyle { kind: ShankKind::Signet, amount: 0.85, ..Default::default() };
        let head = sh.modulation(TOP_DEG, 10.0);
        let back = sh.modulation(TOP_DEG + 180.0, 10.0);
        assert!((head.crown_scale - 1.0).abs() < 1e-12, "the head must keep its flat crest");
        assert!(back.crown_scale > 3.0, "the shank did not round off: {}", back.crown_scale);

        // The crown clamp keeps that from eating the section.
        let loop_ = p.sample_mod(9.0, 128, &back);
        let thickness = p.thickness_mm * back.thickness_scale;
        let depth = loop_.crest_radius_mm - 9.0;
        assert!(depth <= thickness + 1e-6, "section grew: {depth} vs {thickness}");
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
            crate::BuildParams { theta_steps: 128, profile_steps: 96, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true },
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
                    crate::BuildParams { theta_steps: 128, profile_steps: steps, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true },
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
                    crate::BuildParams { theta_steps: 128, profile_steps: 96, min_wall_mm: crate::mesh::MIN_WALL_MM, adaptive: true },
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
