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
    // A stamp is a texture too: its footprint guesses 15% of the stamp,
    // but the alpha can be measured at each stamp's own mm per texel, and
    // the measurement replaces the guess either way.
    for (i, entry) in design.layers.layers.iter().enumerate() {
        let Layer::Decals(dl) = &entry.layer else { continue };
        if !entry.enabled {
            continue;
        }
        let Some(alpha) = lib.get(&dl.alpha) else { continue };
        // The chart's v is the section's arc normalized, so on a station
        // thicker than the reference a stamp stands taller than it is wide
        // by that ratio: measure the alpha stretched the same way, and the
        // metal's own features come out, squashed art included.
        let inner_r = design.inner_radius_mm();
        let crest_r = inner_r + design.profile.thickness_mm;
        let mut finest_of: Option<(&str, f64)> = None;
        for d in dl.decals.iter().take(crate::field::MAX_DECALS) {
            let m = design.modulation_at(d.theta_deg, inner_r, crest_r);
            let k = design.profile.sample_mod(inner_r, 96, &m).surface_len_mm / ctx.band_v_len_mm.max(1e-9);
            let k = if k.is_finite() { k.clamp(0.25, 8.0) } else { 1.0 };
            let (w, h) = (alpha.width.max(1), alpha.height.max(1));
            let hs = ((h as f64 * k).round() as usize).clamp(1, 4096);
            let stretched = if hs == h {
                alpha.clone()
            } else {
                let mut data = Vec::with_capacity(w * hs);
                for row in 0..hs {
                    let src = ((row as f64 + 0.5) / hs as f64 * h as f64) as usize;
                    data.extend_from_slice(&alpha.data[src.min(h - 1) * w..src.min(h - 1) * w + w]);
                }
                crate::alpha::Alpha::new(format!("{} x{k:.2}", alpha.name), w, hs, data)
            };
            let Some((ink_px, gap_px)) = stretched.min_feature_px() else { continue };
            let scale = d.size_mm / w as f64 * ctx.arc_scale(d.v_mm);
            if !(scale.is_finite() && scale > 0.0) {
                continue;
            }
            let (ink, gap) = (ink_px * scale, gap_px * scale);
            let (what, f) = if ink <= gap { ("strokes", ink) } else { ("gaps", gap) };
            if finest_of.is_none_or(|(_, best)| f < best) {
                finest_of = Some((what, f));
            }
        }
        let Some((what, finest)) = finest_of else { continue };
        out.retain(|f| f.layer != i);
        if finest < min {
            let smallest = dl.decals.iter().map(|d| d.size_mm).fold(f64::MAX, f64::min);
            out.push(DfmFinding {
                layer: i,
                label: entry.name.clone(),
                message: format!(
                    "the {} stamp's finest {what} measure {finest:.2} mm at its smallest, {smallest:.1} mm, against the sand's {min:.2} mm floor — they will cast as mush. Enlarge the stamp, bolden the art, or accept the softness.",
                    dl.alpha
                ),
            });
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
            let Some((finest, what)) = tiling_finest_mm(t, lib, &ctx) else { continue };
            let (cw, ch) = t.cell_size(&ctx);
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

    /// The solver is the checker read backwards: fitting to the sand's own
    /// floor must silence the finding it was derived from, and one repeat more
    /// must bring it back. A mask too fine for the face reports the face it
    /// would need instead of a count.
    #[test]
    fn fitting_to_the_floor_silences_the_finding() {
        let lib = crate::alpha::AlphaLibrary::builtin();
        // A side face's usable width is `thickness - crown`, so thickness is
        // the dimension that decides whether a mask fits, not band width.
        let band = |t: f64| {
            let mut d = RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::Flat);
            d.profile.width_mm = 6.0;
            d.profile.thickness_mm = t;
            d.profile.flatten_sides();
            d
        };
        let make = |d: &RingDesign, reps: u32| {
            let ctx = d.field_context();
            let mut t = TilingLayer::default_for("Chevron", &ctx);
            t.height_mm = 0.3;
            t.fit_to_side_faces(&ctx, crate::field::SIDE_FACE_MIN_DRAFT_DEG);
            t.repeats_around = reps;
            t
        };

        // A narrow face cannot hold this mask at any count, and says how wide it must be.
        let narrow = band(2.2);
        let mut t = make(&narrow, 60);
        let want = match fit_to_floor(&mut t, &lib, &narrow.field_context(), narrow.draft.min_detail_mm) {
            FloorFit::NeedsTallerCell { min_cell_h_mm } => min_cell_h_mm,
            other => panic!("a 6 mm band's face should be too narrow for Chevron, got {other:?}"),
        };
        assert!(want > 0.0 && want.is_finite(), "expected a usable figure, got {want}");

        // Give it a face that clears the figure and the solve lands.
        let wide = band(want * 1.4 + 1.0);
        let ctx = wide.field_context();
        let mut t = make(&wide, 400);
        let n = match fit_to_floor(&mut t, &lib, &ctx, wide.draft.min_detail_mm) {
            FloorFit::Repeats(n) => n,
            other => panic!("a face sized from the figure should solve, got {other:?}"),
        };
        assert!(n < 400, "the solver must actually coarsen: got {n}");

        let mut ok = wide.clone();
        ok.layers.layers.push(LayerEntry::new("Pattern", Layer::Tiling(t.clone())));
        assert!(findings_in(&ok, &lib).is_empty(), "solved layer still flagged: {:?}", findings_in(&ok, &lib));

        let mut over = t.clone();
        over.repeats_around = n + 1;
        let mut bad = wide.clone();
        bad.layers.layers.push(LayerEntry::new("Pattern", Layer::Tiling(over)));
        assert!(!findings_in(&bad, &lib).is_empty(), "one repeat past the solve should flag");
    }

    /// Leaning a flute turns part of each wall to face across the band, and
    /// on a dome the downhill flank then leans back. Pins the limit so a
    /// future change cannot quietly make diagonal reeding look free.
    #[test]
    fn flute_lean_costs_draft_past_the_sand_limit() {
        let lib = crate::alpha::AlphaLibrary::builtin();
        let build = |lean: f64| {
            let mut d = RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::LowDome);
            d.profile.width_mm = 7.0;
            d.profile.thickness_mm = 2.6;
            let mut f = crate::field::FlutesLayer::default();
            f.count = 30;
            f.width_mm = 1.2;
            f.height_mm = 0.3;
            f.along = false;
            f.lean = lean;
            d.layers.layers.push(LayerEntry::new("Reeding", Layer::Flutes(f)));
            crate::castability::analyze_field(&d, &lib, &d.draft, 256, 128).undercut_fraction()
        };
        assert!(build(crate::field::SAND_MAX_LEAN) < 5e-4, "reeding at the limit must be clean");
        assert!(build(1.5) > 0.02, "a hard lean must show as real undercut");
    }

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

    /// A stamp is measured at its own mm per texel, on the section as
    /// modulated at its station: a bold hook passes where the 15% guess
    /// said mush, and the same art at half the size fails.
    #[test]
    fn a_stamp_is_measured_not_guessed() {
        let hook: String = {
            let pts: Vec<String> = (0..=120)
                .map(|i| {
                    let t = i as f64 / 120.0;
                    let a = t * std::f64::consts::TAU;
                    let r = 6.0 + 40.0 * t;
                    format!("{:.1} {:.1}", 50.0 + r * a.cos(), 50.0 + r * a.sin())
                })
                .collect();
            format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><path d="M{}" fill="none" stroke="#000" stroke-width="20" stroke-linecap="round"/></svg>"##,
                pts.join(" L")
            )
        };
        let mut lib = crate::AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 2.0;
        d.profile.flatten_sides();
        d.svgs.push(crate::svg::SvgAlpha { name: "Hook".into(), svg: hook, invert: false });
        d.bake_all(&mut lib);
        let ctx = d.field_context();
        let stamp = |size: f64| crate::field::DecalLayer {
            alpha: "Hook".into(),
            decals: vec![crate::field::Decal { theta_deg: crate::profile::TOP_DEG, v_mm: ctx.crest_v_mm, size_mm: size, ..Default::default() }],
            ..Default::default()
        };
        d.layers.layers.push(LayerEntry::new("Bold", Layer::Decals(stamp(2.25))));
        assert!(!findings(&d).is_empty(), "the 15% guess calls a 2.25 mm stamp mush");
        assert!(findings_in(&d, &lib).is_empty(), "measured, a 0.45 mm stroke and gap pass: {:?}", findings_in(&d, &lib));
        d.layers.layers[0].layer = Layer::Decals(stamp(1.0));
        let f = findings_in(&d, &lib);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].message.contains("measure") && f[0].message.contains("Hook"), "{}", f[0].message);
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

/// A tiling's finest measured feature in millimetres of metal, and whether it
/// is the ink or the gaps that runs finest.
///
/// The mask is shaped by the layer's own contrast/bias/invert first, measured
/// by granulometry, then scaled to the cell the layer actually lays down.
/// [`findings_in`] and [`coarsen_to_floor`] both read this, so the check and
/// the solver cannot drift apart.
pub fn tiling_finest_mm(
    t: &crate::tiling::TilingLayer,
    lib: &crate::alpha::AlphaLibrary,
    ctx: &crate::field::FieldContext,
) -> Option<(f64, &'static str)> {
    let (ink_px, gap_px) = tiling_feature_px(t, lib)?;
    let alpha = lib.get(&t.alpha)?;
    let (cw, ch) = t.cell_size(ctx);
    let scale = (cw / alpha.width.max(1) as f64).min(ch / alpha.height.max(1) as f64);
    let (ink, gap) = (ink_px * scale, gap_px * scale);
    Some(if ink <= gap { (ink, "strokes") } else { (gap, "gaps") })
}

/// The mask's finest ink and gap in texels, after the layer's own shaping.
fn tiling_feature_px(t: &crate::tiling::TilingLayer, lib: &crate::alpha::AlphaLibrary) -> Option<(f64, f64)> {
    let alpha = lib.get(&t.alpha)?;
    let shaped = if t.invert || (t.contrast - 1.0).abs() > 1e-9 || t.bias.abs() > 1e-9 {
        let data = alpha.data.iter().map(|&v| alpha.shaped(v, t.contrast, t.bias, t.invert) as f32).collect();
        crate::alpha::Alpha::new(format!("{} shaped", alpha.name), alpha.width, alpha.height, data)
    } else {
        alpha.clone()
    };
    shaped.min_feature_px()
}

/// What the sand's detail floor allows a tiling to be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloorFit {
    /// The layer was set to this many repeats and now clears the floor.
    Repeats(u32),
    /// No repeat count clears it, because the cell's *height* binds: the mask
    /// runs finer across the band than along it. The cell must be at least
    /// this tall — widen the face, or drop a row — before any count helps.
    NeedsTallerCell { min_cell_h_mm: f64 },
    /// The mask has no measurable feature (blank, or not in the library).
    Unmeasurable,
}

/// Fit a tiling to the sand's detail floor: set `repeats_around` to the most
/// repeats whose finest feature still clears `floor_mm`.
///
/// The measurement is linear in the cell and the cell's width is the
/// circumference over the repeat count, so the admissible count is closed
/// form — no search. When the answer is [`FloorFit::NeedsTallerCell`] the
/// layer is left untouched and the figure is what a face must measure for the
/// pattern to be usable at all, which is the number worth designing the band
/// around.
pub fn fit_to_floor(
    t: &mut crate::tiling::TilingLayer,
    lib: &crate::alpha::AlphaLibrary,
    ctx: &crate::field::FieldContext,
    floor_mm: f64,
) -> FloorFit {
    if floor_mm <= 0.0 {
        return FloorFit::Repeats(t.repeats_around.max(1));
    }
    let Some((ink_px, gap_px)) = tiling_feature_px(t, lib) else { return FloorFit::Unmeasurable };
    let f_px = ink_px.min(gap_px);
    let Some(alpha) = lib.get(&t.alpha) else { return FloorFit::Unmeasurable };
    let (w, h) = (alpha.width.max(1) as f64, alpha.height.max(1) as f64);
    if f_px <= 0.0 || !ctx.circumference_mm.is_finite() {
        return FloorFit::Unmeasurable;
    }
    let min_cell_h = floor_mm * h / f_px;
    if t.v_span_mm / (t.rows.max(1) as f64) < min_cell_h {
        return FloorFit::NeedsTallerCell { min_cell_h_mm: min_cell_h };
    }
    let n = (ctx.circumference_mm / (floor_mm * w / f_px)).floor();
    if !(n >= 1.0) {
        return FloorFit::NeedsTallerCell { min_cell_h_mm: min_cell_h };
    }
    t.repeats_around = (n as i64).clamp(1, 4096) as u32;
    FloorFit::Repeats(t.repeats_around)
}
