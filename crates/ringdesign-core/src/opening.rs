//! A band left open over part of its circumference — a cuff — and the arc that is swept instead of the whole ring.

use serde::{Deserialize, Serialize};

use crate::profile::{CrestShank, ProfileStyle, ShankKind, TOP_DEG};
use crate::{BandProfile, RingDesign, RingSize};

/// Narrowest gap an opening keeps, mm.
pub const MIN_GAP_MM: f64 = 0.2;

/// The measured cuff's gap over its bore: 31 mm across 63.5 mm.
const CUFF_GAP_SHARE: f64 = 31.0 / 63.5;

/// A gap in the band, a chord across the bore centred on a ring angle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Opening {
    /// Width of the gap, a chord across the bore, mm.
    pub gap_mm: f64,
    /// Ring angle of the gap's centre, degrees; 270 is the back, opposite the top.
    #[serde(default = "back_deg")]
    pub centre_deg: f64,
}

/// The back of the ring, opposite [`TOP_DEG`].
pub fn back_deg() -> f64 {
    TOP_DEG + 180.0
}

impl Opening {
    /// A cuff's gap at the back, in the measured cuff's proportion to its bore.
    pub fn for_bore(bore_diameter_mm: f64) -> Self {
        Self { gap_mm: (bore_diameter_mm * CUFF_GAP_SHARE).max(MIN_GAP_MM), centre_deg: back_deg() }
    }

    /// The gap's width held between [`MIN_GAP_MM`] and the bore's diameter.
    pub fn gap_held(&self, bore_radius_mm: f64) -> f64 {
        let diameter = 2.0 * bore_radius_mm.max(MIN_GAP_MM);
        if self.gap_mm.is_finite() { self.gap_mm.clamp(MIN_GAP_MM, diameter) } else { MIN_GAP_MM }
    }

    /// Half the gap's angle at the bore, degrees.
    pub fn half_angle_deg(&self, bore_radius_mm: f64) -> f64 {
        let r = bore_radius_mm.max(MIN_GAP_MM);
        (self.gap_held(r) * 0.5 / r).clamp(0.0, 1.0).asin().to_degrees()
    }

    /// The arc swept round a bore of this radius: from one side of the gap the long way round to the other.
    pub fn arc(&self, bore_radius_mm: f64) -> SweepArc {
        let half = self.half_angle_deg(bore_radius_mm);
        let centre = if self.centre_deg.is_finite() { self.centre_deg } else { back_deg() };
        SweepArc { start_deg: (centre + half).rem_euclid(360.0), span_deg: 360.0 - 2.0 * half }
    }
}

/// The trident bracelet's plain cuff: a flat 32.45 x 3.9 mm band on a 63.5 mm bore, the default crest and a 31 mm gap at the back.
pub fn measured_cuff() -> RingDesign {
    let mut profile = BandProfile { width_mm: 32.45, thickness_mm: 3.9, ..BandProfile::default() };
    profile.apply_style(ProfileStyle::Flat);
    profile.crown_mm = 0.0;
    profile.edge_round_mm = 0.45;
    profile.comfort_fit_mm = 0.0;
    profile.side_draft_deg = 0.0;
    let mut d = RingDesign { name: "Cuff".into(), size: RingSize::from_diameter_mm_exact(63.5), profile, ..RingDesign::default() };
    d.shank.kind = ShankKind::Crest;
    d.shank.crest = Some(CrestShank::default());
    d.opening = Some(Opening { gap_mm: 31.0, centre_deg: back_deg() });
    d
}

/// The arc a band is swept over: the whole ring, or from `start_deg` through `span_deg` to the far side of a gap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepArc {
    /// Ring angle of the first section, degrees.
    pub start_deg: f64,
    /// Degrees swept; 360 closes the ring.
    pub span_deg: f64,
}

/// Angular slack in [`SweepArc::contains`], degrees.
const ARC_SLACK_DEG: f64 = 1e-6;

impl SweepArc {
    pub const FULL: SweepArc = SweepArc { start_deg: 0.0, span_deg: 360.0 };

    pub fn is_closed(&self) -> bool {
        !(self.span_deg < 360.0)
    }

    /// Ring angle of the last section, degrees, 0..360.
    pub fn end_deg(&self) -> f64 {
        (self.start_deg + self.span_deg).rem_euclid(360.0)
    }

    /// Whether a ring angle falls on metal: anywhere on a closed ring, within the arc on an open one.
    pub fn contains(&self, theta_deg: f64) -> bool {
        if self.is_closed() {
            return true;
        }
        let d = (theta_deg - self.start_deg).rem_euclid(360.0);
        d <= self.span_deg + ARC_SLACK_DEG || d >= 360.0 - ARC_SLACK_DEG
    }

    /// The nearest ring angle on metal: `theta_deg` itself, or the nearer end of the arc.
    pub fn clamp(&self, theta_deg: f64) -> f64 {
        if self.contains(theta_deg) {
            return theta_deg;
        }
        let past_end = (theta_deg - self.start_deg - self.span_deg).rem_euclid(360.0);
        let before_start = (self.start_deg - theta_deg).rem_euclid(360.0);
        if past_end <= before_start { self.end_deg() } else { self.start_deg }
    }

    /// Columns a sweep of `per_turn` steps round the ring lays over this arc: both ends, and an odd count so one stands opposite the gap.
    pub fn columns(&self, per_turn: usize) -> usize {
        if self.is_closed() {
            return per_turn;
        }
        let steps = ((per_turn as f64 * self.span_deg / 360.0).ceil() as usize).max(8);
        steps + steps % 2 + 1
    }

    /// `n` evenly spaced stations as `(theta_deg, share of the turn)`: round the whole ring, or along the arc with both ends on it.
    pub fn stations(&self, n: usize) -> Vec<(f64, f64)> {
        if self.is_closed() {
            return (0..n)
                .map(|i| {
                    let f = i as f64 / n as f64;
                    (f * 360.0, f)
                })
                .collect();
        }
        let last = n.max(2) - 1;
        (0..=last)
            .map(|i| {
                let theta = (self.start_deg + self.span_deg * i as f64 / last as f64).rem_euclid(360.0);
                (theta, theta / 360.0)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::castability::{analyze_field, parting_line, section_at, write_parting_svg};
    use crate::{AlphaLibrary, BuildParams, Mesh};

    fn crossings(m: &Mesh) -> usize {
        let v = m.vertices.iter().map(|p| [f64::from(p.0), f64::from(p.1), f64::from(p.2)]).collect();
        crate::csg::self_crossings(&crate::csg::Solid { v, f: m.faces.clone() })
    }

    /// Six times the volume with its sign: positive when every face winds outward.
    fn signed_volume6(m: &Mesh) -> f64 {
        m.faces
            .iter()
            .filter_map(|f| m.triangle(f))
            .map(|(a, b, c)| a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]))
            .sum()
    }

    fn on_arc(arc: &SweepArc, x: f64, y: f64) -> bool {
        let d = (y.atan2(x).to_degrees() - arc.start_deg).rem_euclid(360.0);
        d <= arc.span_deg + 1e-3 || d >= 360.0 - 1e-3
    }

    fn open(d: &RingDesign) -> RingDesign {
        RingDesign { opening: Some(Opening::for_bore(d.size.inner_diameter_mm())), ..d.clone() }
    }

    /// An open band is its arc of the closed one shut by two flat caps: watertight, uncrossed, wound outward, nothing past its ends.
    #[test]
    fn an_open_band_is_closed_by_two_flat_caps() {
        use crate::field::{BorderLayer, BorderProfile, Layer, LayerEntry};
        let lib = AlphaLibrary::builtin();
        let mut ring = RingDesign::default();
        let rail = BorderLayer { v_mm: ring.field_context().band_v_len_mm * 0.5, width_mm: 1.0, height_mm: 0.4, profile: BorderProfile::Round, mirror: false, rope_twists: 0 };
        let opened = open(&ring);
        let arc = opened.sweep_arc();
        let refined = BuildParams { refine: Some(crate::refine::RefineParams::preset("Coarse").unwrap()), ..Default::default() };
        for (layers, params) in [
            (false, BuildParams { theta_steps: 192, profile_steps: 64, ..Default::default() }),
            (true, BuildParams { theta_steps: 128, profile_steps: 48, adaptive: true, ..Default::default() }),
            (true, refined),
        ] {
            ring.layers.layers = if layers { vec![LayerEntry::new("Rail", Layer::Border(rail.clone()))] } else { Vec::new() };
            let d = RingDesign { layers: ring.layers.clone(), crisp_relief: layers, ..opened.clone() };
            let built = crate::mesh::build(&d, &lib, params);
            let v = built.mesh.validate();
            assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{params:?}: {v:?}");
            assert_eq!(crossings(&built.mesh), 0, "{params:?}");
            assert!(signed_volume6(&built.mesh) > 0.0, "faces wind outward");
            assert!(built.mesh.vertices.iter().all(|p| on_arc(&arc, p.0 as f64, p.1 as f64)), "nothing is swept across the gap");
            assert!(!built.mesh.corner_normals.is_empty() && built.mesh.corner_normals.windows(2).all(|w| w[0].0 < w[1].0), "the caps keep a crease, sorted by face");
            assert_eq!(built.mesh.normals.len(), built.mesh.vertices.len());
            assert!(built.report.refine.is_none(), "an open band is swept, never refined");
            if !layers {
                let closed = crate::mesh::build(&RingDesign { opening: None, ..d.clone() }, &lib, params);
                let share = built.report.volume_mm3 / closed.report.volume_mm3;
                assert!((share - arc.span_deg / 360.0).abs() < 2e-3, "the arc's share of the metal: {share} for {}", arc.span_deg / 360.0);
            }
        }
        // Refinement asked for directly sweeps the arc too.
        let refined = crate::refine::build(&opened, &lib, crate::refine::RefineParams::preset("Coarse").unwrap(), 0.5);
        assert!(refined.mesh.validate().watertight && refined.stats.leaves == 0);
        assert!(refined.mesh.vertices.iter().all(|p| on_arc(&arc, p.0 as f64, p.1 as f64)));
        // An imported stock cannot open: refused by name rather than built closed.
        let mut stock = RingDesign::default();
        crate::imported_base::ImportedBase::attach(&mut stock, crate::imported_base::PRESETS[0].load().unwrap()).unwrap();
        stock.opening = opened.opening;
        let refused = crate::mesh::try_build(&stock, &lib, BuildParams { theta_steps: 96, profile_steps: 48, ..Default::default() }).err().expect("refused");
        assert!(refused.to_string().contains("cannot open"), "{refused}");
    }

    /// The verdict reads only the metal there is: sections along the arc, the two end faces as vertical walls.
    #[test]
    fn the_verdict_reads_the_arc_and_its_end_faces_as_walls() {
        let lib = AlphaLibrary::builtin();
        let ring = RingDesign::default();
        let opened = open(&ring);
        let arc = opened.sweep_arc();
        let (closed, field) = (analyze_field(&ring, &lib, &ring.draft, 192, 96), analyze_field(&opened, &lib, &opened.draft, 192, 96));
        assert_eq!(field.undercut_area_mm2, 0.0, "{:?}", field.notes);
        assert!(field.notes.iter().any(|n| n.starts_with("Open band")), "{:?}", field.notes);
        let end = |theta: f64| {
            let s = section_at(&opened, &lib, theta, 96);
            let n = s.points.len();
            (0..n).map(|i| s.points[i].r * s.points[(i + 1) % n].z - s.points[(i + 1) % n].r * s.points[i].z).sum::<f64>().abs() * 0.5
        };
        let ends = end(arc.start_deg) + end(arc.end_deg());
        let share = arc.span_deg / 360.0;
        assert!(ends > 1.0, "{ends}");
        assert!((field.total_area_mm2 - (closed.total_area_mm2 * share + ends)).abs() < 0.01 * closed.total_area_mm2, "{} vs {}", field.total_area_mm2, closed.total_area_mm2 * share + ends);
        assert!(field.vertical_area_mm2 >= ends - 1e-9, "the end faces count as walls");
        assert!((field.thinnest_wall_mm - closed.thinnest_wall_mm).abs() < 1e-6);
        assert!(section_at(&opened, &lib, back_deg(), 96).points.is_empty(), "no metal across the gap");
        assert!(!section_at(&opened, &lib, TOP_DEG, 96).points.is_empty());
        let line = parting_line(&opened, &lib, 64, 96);
        assert_eq!(line.len(), 64);
        assert!(line.iter().all(|p| on_arc(&arc, p[0], p[1])));
        assert!(crate::castability::modulus_scan(&opened, &lib, 32).iter().all(|&(theta, m)| arc.contains(theta) && m > 0.0));
        let dir = std::env::temp_dir().join("ringdesign-open-parting-test");
        std::fs::create_dir_all(&dir).unwrap();
        let svg = dir.join("open.svg");
        write_parting_svg(&svg, &line, opened.inner_radius_mm(), "open").unwrap();
        let text = std::fs::read_to_string(&svg).unwrap();
        assert!(!text.contains("Z\""), "the plan does not close across the gap");
        let closed_line = parting_line(&ring, &lib, 64, 96);
        write_parting_svg(&svg, &closed_line, ring.inner_radius_mm(), "closed").unwrap();
        assert!(std::fs::read_to_string(&svg).unwrap().contains("Z\""), "a ring's plan closes");
    }

    /// The crest widens the cuff to 50 mm and 0.4 mm proud at its tip, symmetric, back to the band past its flank, and pulls.
    #[test]
    fn the_crest_shank_widens_to_its_tip_and_pulls() {
        let lib = AlphaLibrary::builtin();
        let d = measured_cuff();
        let (inner, band) = (d.inner_radius_mm(), d.profile);
        let at = |phi: f64| {
            let m = d.modulation_at(TOP_DEG + phi, inner, inner + band.thickness_mm);
            (band.width_mm * m.width_scale, band.thickness_mm * m.thickness_scale)
        };
        let (w, t) = at(0.0);
        assert!((w - 50.0).abs() < 1e-9 && (t - 4.3).abs() < 1e-9, "{w} x {t}");
        for phi in [1.0, 10.0, 30.0, 59.0] {
            assert_eq!(at(phi), at(-phi), "the crest is symmetric about its tip");
        }
        let widths: Vec<f64> = (0..=60).map(|k| at(k as f64).0).collect();
        assert!(widths.windows(2).all(|w| w[1] <= w[0]), "the flank only falls");
        assert_eq!(at(61.0), (band.width_mm, band.thickness_mm), "past the flank it is the band");
        let field = analyze_field(&d, &lib, &d.draft, 360, 128);
        assert_eq!(field.undercut_area_mm2, 0.0, "{:?}", field.notes);
        let built = crate::mesh::build(&d, &lib, BuildParams { theta_steps: 360, profile_steps: 96, ..Default::default() });
        assert!(built.mesh.validate().watertight);
        assert_eq!(crossings(&built.mesh), 0);
        assert!((built.report.inner_diameter_mm - 63.5).abs() < 1e-3);
        let none = RingDesign { shank: crate::ShankStyle { crest: None, ..d.shank.clone() }, ..d.clone() };
        let m = none.modulation_at(TOP_DEG, inner, inner + band.thickness_mm);
        assert!((band.width_mm * m.width_scale - 50.0).abs() < 1e-9, "unset, the crest is the measured one in proportion");
    }

    /// An opening or a crest writes the design at 6 and fences a graph carrying either; a plain design stays at 5.
    #[test]
    fn an_opening_and_a_crest_are_fenced() {
        use crate::library::{FORMAT_VERSION, PLAIN_FORMAT_VERSION, format_version_for, template_features_in_json};
        let plain = RingDesign::default();
        assert_eq!(format_version_for(&plain), PLAIN_FORMAT_VERSION);
        assert!(serde_json::to_value(&plain).unwrap().get("opening").is_none());
        assert!(serde_json::to_value(&plain).unwrap()["shank"].get("crest").is_none());
        let opened = open(&plain);
        let mut crest = plain.clone();
        crest.shank.apply_crest(&crest.profile);
        for d in [&opened, &crest, &measured_cuff()] {
            assert_eq!(format_version_for(d), FORMAT_VERSION);
            let text = crate::library::design_json(d).unwrap();
            assert_eq!(serde_json::to_value(crate::library::load_design_str(&text).unwrap()).unwrap(), serde_json::to_value(d).unwrap());
            assert!(template_features_in_json(&serde_json::to_value(d).unwrap()));
        }
        let node = |kind: &str, inputs: serde_json::Value| serde_json::json!({"nodes":[{"id":1,"kind":kind,"inputs":inputs}],"wires":[]});
        for (g, fenced) in [
            (node("design.new", serde_json::json!({"open_gap_mm": 31.0})), true),
            (node("design.new", serde_json::json!({"size": 7.0})), false),
            (node("shank", serde_json::json!({"kind": "Crest"})), true),
            (node("shank", serde_json::json!({"kind": "Bypass"})), false),
            (node("shank", serde_json::json!({"crest": {"width_mm": 9.0}})), true),
            (node("design.set", serde_json::json!({"pointer": "/opening", "value": {"gap_mm": 3.0}})), true),
            (node("design.set", serde_json::json!({"pointer": "/opening"})), false),
        ] {
            assert_eq!(template_features_in_json(&g), fenced, "{g}");
        }
    }

    #[test]
    fn a_gap_opens_the_back_by_its_chord() {
        let o = Opening { gap_mm: 31.0, centre_deg: back_deg() };
        let arc = o.arc(31.75);
        let half = (15.5f64 / 31.75).asin().to_degrees();
        assert!((arc.start_deg - (270.0 + half)).abs() < 1e-9);
        assert!((arc.span_deg - (360.0 - 2.0 * half)).abs() < 1e-9);
        assert!(!arc.is_closed());
        assert!(arc.contains(90.0) && arc.contains(0.0) && arc.contains(arc.start_deg) && arc.contains(arc.end_deg()));
        assert!(!arc.contains(270.0));
        assert!((arc.clamp(275.0) - arc.start_deg).abs() < 1e-9 && (arc.clamp(265.0) - arc.end_deg()).abs() < 1e-9);
        let stations = arc.stations(5);
        assert_eq!(stations.len(), 5);
        assert!((stations[0].0 - arc.start_deg).abs() < 1e-9 && (stations[4].0 - arc.end_deg()).abs() < 1e-9);
        assert!(stations.iter().all(|&(t, f)| (0.0..360.0).contains(&t) && (f * 360.0 - t).abs() < 1e-9));
        assert_eq!(SweepArc::FULL.stations(4), vec![(0.0, 0.0), (90.0, 0.25), (180.0, 0.5), (270.0, 0.75)]);
        for per_turn in [24, 96, 512, 1080] {
            let columns = arc.columns(per_turn);
            assert_eq!(columns % 2, 1, "{per_turn}");
            assert!((arc.stations(columns)[columns / 2].0 - TOP_DEG).abs() < 1e-9, "a column stands at the top, opposite the gap");
        }
    }

    #[test]
    fn a_hostile_gap_still_opens_and_never_takes_the_whole_ring() {
        for gap in [f64::NAN, -3.0, 0.0, 1e300, f64::INFINITY] {
            let arc = Opening { gap_mm: gap, centre_deg: f64::NAN }.arc(8.0);
            assert!(!arc.is_closed() && arc.span_deg >= 180.0 - 1e-9, "{gap}: {arc:?}");
            assert!(arc.start_deg.is_finite());
        }
    }
}
