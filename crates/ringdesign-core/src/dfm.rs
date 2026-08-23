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

/// [`findings`] plus what the textures measure: a tiling's or openwork's
/// mask read by granulometry ([`crate::alpha::Alpha::min_feature_px`]) at
/// the layer's own cell scale, so a fine-lined alpha on coarse cells is
/// still caught. The analytic check sees only the cell pitch.
pub fn findings_in(design: &RingDesign, lib: &crate::AlphaLibrary) -> Vec<DfmFinding> {
    let mut out = findings(design);
    let ctx = design.field_context();
    let min = design.draft.min_detail_mm.max(0.0);
    if min <= 0.0 {
        return out;
    }
    fn tilings<'a>(stack: &'a crate::field::LayerStack, out: &mut Vec<&'a crate::tiling::TilingLayer>) {
        for e in stack.layers.iter().filter(|e| e.enabled) {
            match &e.layer {
                Layer::Tiling(t) => out.push(t),
                Layer::Openwork(o) => out.push(&o.tiling),
                Layer::Group(g) => tilings(&g.stack, out),
                _ => {}
            }
        }
    }
    for (i, entry) in design.layers.layers.iter().enumerate() {
        if !entry.enabled || out.iter().any(|f| f.layer == i) {
            continue;
        }
        let mut ts = Vec::new();
        let one = crate::field::LayerStack { layers: vec![entry.clone()] };
        tilings(&one, &mut ts);
        for t in ts {
            let Some(alpha) = lib.get(&t.alpha) else { continue };
            let shaped = if t.invert || (t.contrast - 1.0).abs() > 1e-9 || t.bias.abs() > 1e-9 {
                let data = alpha.data.iter().map(|&v| alpha.shaped(v, t.contrast, t.bias, t.invert) as f32).collect();
                crate::alpha::Alpha::new(format!("{} shaped", alpha.name), alpha.width, alpha.height, data)
            } else {
                alpha.clone()
            };
            let Some((ink_px, gap_px)) = shaped.min_feature_px() else { continue };
            let (cw, ch) = t.cell_size(&ctx);
            let scale = (cw / alpha.width.max(1) as f64).min(ch / alpha.height.max(1) as f64);
            let (ink, gap) = (ink_px * scale, gap_px * scale);
            let (what, finest) = if ink <= gap { ("strokes", ink) } else { ("gaps", gap) };
            if finest >= min {
                continue;
            }
            out.push(DfmFinding {
                layer: i,
                label: entry.name.clone(),
                message: format!(
                    "the {} texture's finest {what} measure {finest:.2} mm on {cw:.1} x {ch:.1} mm cells against the sand's {min:.2} mm floor — they will cast as mush. Coarsen the pattern, use fewer repeats, or accept the softness.",
                    t.alpha
                ),
            });
            break;
        }
    }
    out
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
            .map(|f| f.metal_feature_mm(&ctx))
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

#[cfg(test)]
mod measured_tests {
    use super::*;
    use crate::field::LayerEntry;
    use crate::tiling::TilingLayer;

    #[test]
    fn a_fine_lined_texture_on_honest_cells_is_caught_by_the_measure() {
        let lib = crate::AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut t = TilingLayer::default_for("Greek Key", &ctx);
        t.repeats_around = 12;
        t.rows = 1;
        t.v_center_mm = ctx.crest_v_mm;
        t.v_span_mm = 2.0;
        d.layers.layers.push(LayerEntry::new("Key", Layer::Tiling(t)));
        let (cw, _) = match &d.layers.layers[0].layer { Layer::Tiling(t) => t.cell_size(&ctx), _ => unreachable!() };
        assert!(cw > 2.0, "cells are coarser than the floor: {cw}");
        assert!(findings(&d).is_empty(), "the cell pitch alone passes");
        let measured = findings_in(&d, &lib);
        assert_eq!(measured.len(), 1, "{measured:?}");
        assert!(measured[0].message.contains("Greek Key"), "{}", measured[0].message);
        d.draft.min_detail_mm = 0.0;
        assert!(findings_in(&d, &lib).is_empty(), "no floor, no finding");
    }

    /// What the measure says about the shipped templates, printed under
    /// `--nocapture`; the analytic check stays clean on all of them.
    #[test]
    fn the_templates_measured() {
        let lib = crate::AlphaLibrary::builtin();
        for t in crate::templates::all() {
            let d = t.design();
            assert!(findings(&d).is_empty(), "{}: {:?}", t.name, findings(&d));
            for f in findings_in(&d, &lib) {
                eprintln!("{}: {} — {}", t.name, f.label, f.message);
            }
        }
    }
}
