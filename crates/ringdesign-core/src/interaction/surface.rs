//! Editable paths and pattern placement in the ring's existing surface chart.
use crate::{
    AlphaLibrary, RingDesign,
    curve::CurveLayer,
    field::{Blend, Layer, LayerEntry},
};

pub const MAX_REPEATS: u32 = 128;
/// Mirroring doubles the entry count; every copy must fit the field evaluator.
pub const MAX_STAMP_COPIES: u32 = crate::field::MAX_DECALS as u32 / 2;

/// Unwrap a picked angle around its neighbour, so 359 -> 1 draws across the seam.
pub fn near_turn(turn: f64, neighbour: f64) -> f64 {
    neighbour + (turn - neighbour + 0.5).rem_euclid(1.0) - 0.5
}

/// Commit ordinary Curve source, preserving the selected entry's mask/window.
pub fn apply_path(
    d: &mut RingDesign,
    curve: CurveLayer,
    engrave: bool,
    target: Option<usize>,
) -> Result<usize, String> {
    if d.graph.is_some() || d.cad.is_some() {
        return Err("Bake the driven design before editing surface paths.".into());
    }
    if !(2..=crate::curve::MAX_CURVE_POINTS).contains(&curve.points.len())
        || curve.points.iter().flatten().any(|v| !v.is_finite())
        || !curve.width_mm.is_finite()
        || !(0.1..=6.0).contains(&curve.width_mm)
        || !curve.height_mm.is_finite()
        || !(0.01..=1.6).contains(&curve.height_mm)
        || !(1..=MAX_REPEATS).contains(&curve.repeats_around)
        || !curve.taper.is_finite()
        || !(0.0..=0.5).contains(&curve.taper)
    {
        return Err("Use 2–64 finite points, a 0.1–6 mm width and 0.01–1.6 mm depth.".into());
    }
    let (lo, hi) = curve
        .points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p[0]), hi.max(p[0]))
        });
    if hi - lo > 1.000001 {
        return Err("The path is wider than one repeat. Reduce Copies or shorten the path.".into());
    }
    let band = d.field_context().band_v_len_mm;
    if curve.points.iter().any(|p| !(0.0..=band).contains(&p[1])) {
        return Err("Place every point on the outside of the band.".into());
    }
    // The field reads the owning instance and its immediate neighbours.
    let mut curve = curve;
    let shift = ((lo + hi) * 0.5).floor();
    for p in &mut curve.points {
        p[0] -= shift;
    }
    let blend = if engrave { Blend::Subtract } else { Blend::Add };
    if let Some(index) = target {
        let entry = d
            .layers
            .layers
            .get_mut(index)
            .ok_or("The selected path no longer exists.")?;
        if !matches!(entry.layer, Layer::Curve(_)) {
            return Err("The selected layer is no longer a path.".into());
        }
        entry.layer = Layer::Curve(curve);
        // Keep non-subtractive compositing on existing raised paths.
        if engrave || entry.blend == Blend::Subtract {
            entry.blend = blend;
        }
        Ok(index)
    } else {
        let mut entry = LayerEntry::new(
            if engrave {
                "Surface engraving"
            } else {
                "Surface wire"
            },
            Layer::Curve(curve),
        );
        entry.blend = blend;
        let index = d.layers.layers.len();
        d.layers.layers.push(entry);
        Ok(index)
    }
}

/// Preview points evaluated through the same profile/displacement as the mesh.
/// Inputs are turns around the ring and chart millimetres across it.
pub fn points(d: &RingDesign, lib: &AlphaLibrary, chart: &[[f64; 2]]) -> Vec<[f64; 3]> {
    let ctx = d.field_context();
    let reference = d.reference_loop();
    let inner = d.inner_radius_mm();
    let disp = crate::mesh::Displacer {
        stack: &d.layers,
        ctx: &ctx,
        lib,
        soften_mm: 0.0,
        inner_r: inner,
        min_wall: d.build.min_wall_mm.max(0.05),
    };
    chart
        .iter()
        .take(4096)
        .map(|q| {
            let theta = q[0].rem_euclid(1.0) * 360.0;
            let m = d.modulation_at(theta, inner, reference.crest_radius_mm);
            let section = d
                .profile
                .sample_spaced(inner, 160, &m, None, Some(&reference));
            let target = q[1].clamp(0.0, ctx.band_v_len_mm) / ctx.band_v_len_mm.max(1e-9)
                * section.surface_len_mm;
            let pts = &section.pts[section.surface_start..];
            let k = pts
                .partition_point(|p| p.v_mm < target)
                .clamp(1, pts.len() - 1);
            let a = &pts[k - 1];
            let b = &pts[k];
            let t = ((target - a.v_mm) / (b.v_mm - a.v_mm).max(1e-9)).clamp(0.0, 1.0);
            let a = disp.at(a, section.surface_len_mm, ctx.u_of_theta(theta));
            let b = disp.at(b, section.surface_len_mm, ctx.u_of_theta(theta));
            let r = a.r + (b.r - a.r) * t;
            let z = a.z + (b.z - a.z) * t;
            let (s, c) = theta.to_radians().sin_cos();
            [r * c, r * s, z]
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Arrangement {
    pub count: u32,
    pub span_deg: f64,
    pub mirror: bool,
}
impl Default for Arrangement {
    fn default() -> Self {
        Self {
            count: 1,
            span_deg: 360.0,
            mirror: false,
        }
    }
}
impl Arrangement {
    pub fn placements(self, theta: f64, v: f64, band: f64) -> Vec<(f64, f64, bool)> {
        if !theta.is_finite() || !v.is_finite() || !band.is_finite() || !self.span_deg.is_finite() {
            return vec![];
        }
        if band <= 0.0 || !(0.0..=band).contains(&v) {
            return vec![];
        }
        let n = self.count.clamp(1, MAX_STAMP_COPIES);
        let span = self.span_deg.clamp(0.0, 360.0);
        let step = span / if span >= 359.999 { n } else { (n - 1).max(1) } as f64;
        let mut out = Vec::new();
        for i in 0..n {
            let theta = (theta + step * i as f64).rem_euclid(360.0);
            out.push((theta, v, false));
            if self.mirror && (band - 2.0 * v).abs() > 1e-6 {
                out.push((theta, band - v, true));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seam_path_roundtrips_edits_and_undoes_without_losing_its_mask() {
        let mut d = RingDesign::default();
        let v = d.field_context().crest_v_mm;
        let curve = CurveLayer {
            points: vec![
                [359.0 / 360.0, v],
                [near_turn(1.0 / 360.0, 359.0 / 360.0), v],
            ],
            repeats_around: 1,
            ..Default::default()
        };
        let index = apply_path(&mut d, curve.clone(), false, None).unwrap();
        d.layers.layers[index].mask = Some("painted mask".into());
        d.layers.layers[index].window.enabled = true;
        let original = serde_json::to_value(&d).unwrap();
        let mut history = crate::history::History::new(&d);
        let mut edited = curve;
        edited.width_mm = 1.2;
        apply_path(&mut d, edited, true, Some(index)).unwrap();
        assert_eq!(d.layers.layers[index].mask.as_deref(), Some("painted mask"));
        assert!(d.layers.layers[index].window.enabled);
        history.commit(&d);
        assert_eq!(
            serde_json::to_value(history.undo().unwrap()).unwrap(),
            original
        );
        let saved = serde_json::to_string(&d).unwrap();
        let loaded: RingDesign = serde_json::from_str(&saved).unwrap();
        let Layer::Curve(c) = &loaded.layers.layers[index].layer else {
            panic!()
        };
        assert!((c.points[1][0] - c.points[0][0] - 2.0 / 360.0).abs() < 1e-12);
        assert_eq!(loaded.layers.layers[index].blend, Blend::Subtract);
    }
    #[test]
    fn invalid_path_does_not_mutate_the_project() {
        let mut d = RingDesign::default();
        let before = serde_json::to_value(&d).unwrap();
        for pts in [
            vec![[0.0, 1.0]],
            vec![[0.0, 1.0], [f64::NAN, 1.0]],
            vec![[0.0, 1.0], [1.2, 1.0]],
            vec![[0.0, -1.0], [0.1, 1.0]],
        ] {
            assert!(
                apply_path(
                    &mut d,
                    CurveLayer {
                        points: pts,
                        ..Default::default()
                    },
                    false,
                    None
                )
                .is_err()
            );
            assert_eq!(serde_json::to_value(&d).unwrap(), before);
        }
    }
    #[test]
    fn arrays_cover_full_and_partial_arcs_without_duplicate_seams() {
        let ring = Arrangement {
            count: 4,
            span_deg: 360.0,
            mirror: true,
        }
        .placements(350.0, 2.0, 10.0);
        assert_eq!(ring.len(), 8);
        assert_eq!(ring[0], (350.0, 2.0, false));
        assert_eq!(ring[1], (350.0, 8.0, true));
        assert_eq!(ring[6], (260.0, 2.0, false));
        let arc = Arrangement {
            count: 3,
            span_deg: 60.0,
            mirror: false,
        }
        .placements(350.0, 2.0, 10.0);
        assert_eq!(
            arc.iter().map(|p| p.0).collect::<Vec<_>>(),
            vec![350.0, 20.0, 50.0]
        );
        assert_eq!(
            Arrangement {
                count: u32::MAX,
                span_deg: 360.0,
                mirror: true
            }
            .placements(0.0, 5.0, 10.0)
            .len(),
            MAX_STAMP_COPIES as usize
        );
        assert!(
            Arrangement::default()
                .placements(f64::NAN, 1.0, 10.0)
                .is_empty()
        );
    }
}
