//! Design-for-manufacture checks: layer feature sizes against the sand.
//!
//! Layers are analytic, so their finest feature is known without measuring
//! any mesh — a bead's diameter, a tile cell's pitch, a wire's width are all
//! parameters. Anything finer than [`crate::castability::DraftSettings::min_detail_mm`]
//! reproduces in the sand as mush, and it is cheaper to say so in the layer
//! list than to discover it in the pour.

use crate::field::Layer;
use crate::RingDesign;

#[derive(Clone, Debug)]
pub struct DfmFinding {
    /// Index of the top-level layer entry the finding belongs to.
    pub layer: usize,
    pub label: String,
    pub message: String,
}

/// Every enabled layer whose finest feature the sand cannot hold.
pub fn findings(design: &RingDesign) -> Vec<DfmFinding> {
    let ctx = design.field_context();
    let min = design.draft.min_detail_mm.max(0.0);
    if min <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, entry) in design.layers.layers.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let finest = entry
            .layer
            .feature_footprints(&ctx)
            .iter()
            .map(|f| f.min_feature_mm)
            .fold(f64::MAX, f64::min);
        if finest == f64::MAX || finest >= min {
            continue;
        }
        let what = match &entry.layer {
            Layer::Milgrain(_) => "beads",
            Layer::Tiling(_) => "tile cells",
            Layer::Curve(_) => "the wire",
            Layer::Flutes(_) => "the flutes",
            Layer::Decals(_) => "a stamp",
            Layer::Border(_) => "the rail",
            Layer::Group(_) => "something inside",
            _ => "its finest feature",
        };
        out.push(DfmFinding {
            layer: i,
            label: entry.name.clone(),
            message: format!(
                "{what} run {finest:.2} mm against the sand's {min:.2} mm floor — \
                 they will cast as mush. Coarsen the pattern or accept the softness."
            ),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry, MilgrainLayer};
    use crate::tiling::TilingLayer;

    #[test]
    fn fine_beads_flag_and_coarse_ones_pass() {
        let mut d = RingDesign::default();
        let mut m = MilgrainLayer::default();
        m.bead_diameter_mm = 0.2;
        d.layers.layers.push(LayerEntry::new("Fine beads", Layer::Milgrain(m)));
        let f = findings(&d);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].layer, 0);
        assert!(f[0].message.contains("beads"));

        if let Layer::Milgrain(m) = &mut d.layers.layers[0].layer {
            m.bead_diameter_mm = 0.8;
        }
        assert!(findings(&d).is_empty());

        // Muted layers are not checked: they are not in the pour.
        if let Layer::Milgrain(m) = &mut d.layers.layers[0].layer {
            m.bead_diameter_mm = 0.2;
        }
        d.layers.layers[0].enabled = false;
        assert!(findings(&d).is_empty());
    }

    #[test]
    fn a_dense_tiling_flags_its_cells() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut t = TilingLayer::default_for("Rope".to_string(), &ctx);
        t.repeats_around = 380;
        t.rows = 24;
        d.layers.layers.push(LayerEntry::new("Dense", Layer::Tiling(t)));
        let f = findings(&d);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].message.contains("tile cells"));
    }
}
