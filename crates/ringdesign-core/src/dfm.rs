//! Design-for-manufacture checks: layer feature sizes against the sand.
//!
//! Layers are analytic, so their finest feature is known without measuring
//! any mesh — a bead's diameter, a tile cell's pitch, a wire's width are all
//! parameters. Anything finer than [`crate::castability::DraftSettings::min_detail_mm`]
//! reproduces in the sand as mush, and it is cheaper to say so in the layer
//! list than to discover it in the pour.

use crate::field::Layer;
use crate::RingDesign;

/// [`DfmFinding::layer`] of a finding about one of the design's stamps, which are not layers.
pub const STAMP: usize = usize::MAX;

#[derive(Clone, Debug)]
pub struct DfmFinding {
    /// Index of the top-level layer entry the finding belongs to, or [`STAMP`].
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
        if !entry.enabled || entry.bench_only {
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
            let k = if design.imported_base.is_some() {
                ctx.station_stretch(d.theta_deg)
            } else {
                design.profile.sample_mod(inner_r, 96, &m).surface_len_mm / ctx.band_v_len_mm.max(1e-9)
            };
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
        if !entry.enabled || entry.bench_only || out.iter().any(|f| f.layer == i) {
            continue;
        }
        let mut ts = Vec::new();
        let one = crate::field::LayerStack { layers: vec![entry.clone()] };
        tilings(&one, &mut ts);
        let window = worst_arc_ratio(design, entry, &ctx, lib, None);
        for t in ts {
            // One tile round the whole ring, a hide, lays each column at its own
            // angle: it is judged where it carries ink, not where its window reaches.
            let (ratio, at_deg) = if t.repeats_around == 1 { worst_arc_ratio(design, entry, &ctx, lib, Some(t)) } else { window };
            let Some((finest, what)) = tiling_finest_mm_at(t, lib, &ctx, ratio) else { continue };
            let (cw, ch) = t.cell_size(&ctx);
            let ch = ch * ratio;
            if finest >= min {
                continue;
            }
            let where_ = if ratio < 0.98 {
                format!(" at its tightest station, {at_deg:.0}°,")
            } else {
                String::new()
            };
            out.push(DfmFinding {
                layer: i,
                label: entry.name.clone(),
                message: format!(
                    "the {} texture's finest {what}{where_} measure {finest:.2} mm on {cw:.1} x {ch:.1} mm cells against the sand's {min:.2} mm floor — they will cast as mush. Coarsen the pattern, use fewer repeats, or accept the softness.",
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
        // A layer cut at the bench is not poured: the sand's floor is not its floor.
        if !entry.enabled || entry.bench_only {
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
    // A stamp cut at the bench is not poured either.
    let mut frames = crate::setting::StampFrames::new(design, &ctx);
    for (k, s) in design.stamps.iter().enumerate().filter(|(_, s)| !s.bench) {
        let Some(finest) = stamp_finest_mm(&s.outline, min) else { continue };
        if finest < min {
            out.push(DfmFinding {
                layer: STAMP,
                label: s.name.clone(),
                message: format!(
                    "the stamp's finest strokes measure {finest:.2} mm against the sand's {min:.2} mm floor — \
                     they will cast as mush. Bolden the outline or accept the softness."
                ),
            });
            continue;
        }
        // Measures what higher tiers leave of it: a ledge round a stamp on it, a wall beside a cut in it.
        let Some(exposed) = stamp_exposed_mm(design, &mut frames, k, min) else { continue };
        if exposed < min {
            out.push(DfmFinding {
                layer: STAMP,
                label: s.name.clone(),
                message: format!(
                    "what the stamps struck on it leave of it measures {exposed:.2} mm against the sand's {min:.2} mm floor — \
                     the ledge will cast as mush. Grow it past them or shrink them."
                ),
            });
        }
    }
    out
}

/// Finest stroke of stamp `k` left uncovered by the poured higher-tier stamps standing on it, drawn into its plane; `None` when none do.
fn stamp_exposed_mm(design: &RingDesign, frames: &mut crate::setting::StampFrames, k: usize, floor: f64) -> Option<f64> {
    use crate::setting::{drawn_into, may_touch, same_ground};
    let s = &design.stamps[k];
    let uppers: Vec<usize> = design.stamps.iter().enumerate().filter(|(_, u)| u.tier > s.tier && !u.bench && may_touch(design, s, u)).map(|(j, _)| j).collect();
    if s.cut || uppers.is_empty() {
        return None;
    }
    let frame = frames.get(k);
    let bounds = |o: &[[f64; 2]]| o.iter().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let (lo, hi) = bounds(&s.outline);
    let holes: Vec<Vec<[f64; 2]>> = uppers.into_iter().filter_map(|j| {
        let u = &design.stamps[j];
        let f = frames.get(j);
        if !same_ground(s, &frame, u, &f) {
            return None;
        }
        let plan = drawn_into(&frame, u, &f);
        let (plo, phi) = bounds(&plan);
        (plo[0] < hi[0] && phi[0] > lo[0] && plo[1] < hi[1] && phi[1] > lo[1]).then_some(plan)
    }).collect();
    if holes.is_empty() {
        return None;
    }
    plan_finest_mm(&s.outline, &holes, floor)
}

/// Finest stroke of a stamp's outline in mm: its plan rasterized fine enough for `floor` and read by the
/// granulometry a texture's mask is, so a pointed tip costs nothing and a thin arm is found. `None` for an
/// outline that encloses nothing.
pub fn stamp_finest_mm(outline: &[[f64; 2]], floor: f64) -> Option<f64> {
    plan_finest_mm(outline, &[], floor)
}

/// [`stamp_finest_mm`] of the outline less every polygon in `holes`.
pub fn plan_finest_mm(outline: &[[f64; 2]], holes: &[Vec<[f64; 2]>], floor: f64) -> Option<f64> {
    let n = outline.len();
    if n < 3 || !(floor > 0.0) {
        return None;
    }
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in outline {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
    if !(extent.is_finite() && extent > 0.0) {
        return None;
    }
    // Six texels to the floor, and no side much past five hundred.
    let px = (floor / 6.0).max(extent / 480.0);
    const MARGIN: usize = 4;
    let w = ((hi[0] - lo[0]) / px).ceil() as usize + 2 * MARGIN;
    let h = ((hi[1] - lo[1]) / px).ceil() as usize + 2 * MARGIN;
    let mut data = vec![0.0f32; w * h];
    let mut xs = Vec::new();
    for row in 0..h {
        let y = lo[1] + (row as f64 + 0.5 - MARGIN as f64) * px;
        xs.clear();
        for i in 0..n {
            let (a, b) = (outline[i], outline[(i + 1) % n]);
            if (a[1] > y) != (b[1] > y) {
                xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(f64::total_cmp);
        for span in xs.chunks_exact(2) {
            let from = ((span[0] - lo[0]) / px + MARGIN as f64 - 0.5).ceil().max(0.0) as usize;
            let to = ((span[1] - lo[0]) / px + MARGIN as f64 - 0.5).floor();
            if to < 0.0 {
                continue;
            }
            for col in from..=(to as usize).min(w - 1) {
                data[row * w + col] = 1.0;
            }
        }
        for hole in holes {
            let m = hole.len();
            xs.clear();
            for i in 0..m {
                let (a, b) = (hole[i], hole[(i + 1) % m]);
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
                }
            }
            xs.sort_by(f64::total_cmp);
            for span in xs.chunks_exact(2) {
                let from = ((span[0] - lo[0]) / px + MARGIN as f64 - 0.5).ceil().max(0.0) as usize;
                let to = ((span[1] - lo[0]) / px + MARGIN as f64 - 0.5).floor();
                if to < 0.0 {
                    continue;
                }
                for col in from..=(to as usize).min(w - 1) {
                    data[row * w + col] = 0.0;
                }
            }
        }
    }
    let (ink, _) = crate::alpha::Alpha::new("stamp", w, h, data).min_feature_px()?;
    Some(ink * px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry, MilgrainLayer};
    use crate::tiling::TilingLayer;

    /// A stamp's plan is its metal: a thin arm is found, a pointed tip is not held against it, and one cut
    /// at the bench is not the sand's to judge.
    #[test]
    fn a_stamp_is_judged_by_its_strokes() {
        let bar = |w: f64| vec![[-1.5, -w / 2.0], [1.5, -w / 2.0], [1.5, w / 2.0], [-1.5, w / 2.0]];
        for w in [0.2, 0.5] {
            let got = stamp_finest_mm(&bar(w), 0.3).unwrap();
            assert!((got - w).abs() < 0.06, "a {w} mm bar measures {got}");
        }
        // Five arms that come to points, each far wider than the floor where it leaves the body.
        let star: Vec<[f64; 2]> = (0..10).map(|i| {
            let r = if i % 2 == 0 { 1.5 } else { 0.8 };
            let a = std::f64::consts::PI * i as f64 / 5.0;
            [r * a.cos(), r * a.sin()]
        }).collect();
        assert!(stamp_finest_mm(&star, 0.3).unwrap() > 0.4);
        let mut d = RingDesign::default();
        d.draft.min_detail_mm = 0.3;
        let hairline = crate::setting::Stamp {
            name: "Hairline".into(), theta_deg: 90.0, v_mm: 1.0, rot_deg: 0.0, outline: bar(0.2), height_mm: 0.3,
            sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: Default::default(),
        };
        d.stamps = vec![hairline.clone(), crate::setting::Stamp { name: "Graver line".into(), bench: true, ..hairline }];
        let f = findings(&d);
        assert!(f.iter().any(|f| f.layer == STAMP && f.label == "Hairline"), "{f:?}");
        assert!(!f.iter().any(|f| f.label == "Graver line"));
    }

    /// A higher tier's ledge or wall on the stamp beneath is measured by granulometry.
    #[test]
    fn a_tier_is_judged_by_what_it_leaves_of_the_stamp_beneath() {
        use crate::setting::{Stamp, StampTop};
        let mut d = RingDesign::default();
        d.draft.min_detail_mm = 0.3;
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 7.0;
        d.profile.thickness_mm = 2.4;
        let v = d.field_context().crest_v_mm;
        let disc = |name: &str, dia: f64, tier: u8, cut: bool| Stamp {
            name: name.into(), theta_deg: 90.0, v_mm: v, rot_deg: 0.0, outline: crate::outline::circle(dia), height_mm: 0.3,
            sink_mm: 0.3, draft_deg: 0.0, cut, bench: false, along_pull: false, tier, top: StampTop::Flat,
        };
        let mut ledge = |upper: Stamp| {
            d.stamps = vec![disc("Plate", 3.0, 0, false), upper];
            findings(&d).into_iter().filter(|f| f.label == "Plate").map(|f| f.message).collect::<Vec<_>>()
        };
        assert!(ledge(disc("Boss", 1.6, 1, false)).is_empty(), "a 0.7 mm ledge holds");
        let thin = ledge(disc("Boss", 2.84, 1, false));
        assert!(thin.len() == 1 && thin[0].starts_with("what the stamps struck on it leave"), "a 0.08 mm ledge is found: {thin:?}");
        assert!(!ledge(disc("Well", 2.84, 1, true)).is_empty(), "a cut leaving a 0.08 mm wall is found");
        assert!(ledge(disc("Boss", 2.84, 0, false)).is_empty(), "a stamp on the same tier is not standing on it");
        assert!(ledge(Stamp { theta_deg: 270.0, ..disc("Boss", 2.84, 1, false) }).is_empty(), "a boss on the palm stands on nothing of the plate");
        let exposed = plan_finest_mm(&crate::outline::circle(3.0), &[crate::outline::circle(1.6)], 0.3).unwrap();
        assert!((exposed - 0.7).abs() < 0.08, "{exposed}");
        assert_eq!(plan_finest_mm(&crate::outline::circle(3.0), &[crate::outline::circle(3.4)], 0.3), None, "nothing left, nothing to judge");
        // On a squared band a boss on the high face stands on nothing of a plate on the low one.
        let mut sq = RingDesign::default();
        sq.draft.min_detail_mm = 0.3;
        sq.profile.apply_style(crate::ProfileStyle::Flat);
        sq.profile.thickness_mm = 3.0;
        sq.profile.flatten_sides();
        let faces = sq.field_context().side_faces_std().unwrap();
        let (lo, hi) = (faces.low.unwrap(), faces.high.unwrap());
        let face = |name: &str, dia: f64, tier: u8, v: (f64, f64)| Stamp { v_mm: 0.5 * (v.0 + v.1), along_pull: true, ..disc(name, dia, tier, false) };
        sq.stamps = vec![face("Plate", 2.0, 0, lo), face("Boss", 1.84, 1, hi)];
        assert!(findings(&sq).iter().all(|f| f.label != "Plate"), "{:?}", findings(&sq));
        sq.stamps[1] = face("Boss", 1.84, 1, lo);
        assert!(findings(&sq).iter().any(|f| f.label == "Plate"), "the same boss on the plate's own face leaves a 0.08 mm ledge");
    }

    /// 24 gabled keels on 24 plates make a point each and at most a section each on a band first seen, plain or a signet's, and nothing judged again.
    #[test]
    fn tiered_stamps_are_judged_at_the_cost_of_their_frames() {
        use crate::setting::{Stamp, StampTop};
        let mut band = RingDesign::default();
        band.profile.apply_style(crate::ProfileStyle::LowDome);
        band.profile.width_mm = 7.0;
        band.profile.thickness_mm = 2.4;
        let signet = crate::templates::fixture("Heart signet").unwrap();
        for base in [band, signet] {
            let rows = |tiered: bool, salt: u32| {
                let mut d = base.clone();
                d.profile.width_mm += 1e-9 * salt as f64;
                d.draft.min_detail_mm = 0.3;
                let v = d.field_context().crest_v_mm;
                for k in 0..24 {
                    let plate = Stamp {
                        name: format!("Plate {k}"), theta_deg: 90.0 + 15.0 * k as f64, v_mm: v, rot_deg: 0.0, outline: crate::outline::circle(2.4),
                        height_mm: 0.3, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, tier: 0, top: StampTop::Flat,
                    };
                    let keel = Stamp {
                        name: format!("Keel {k}"), outline: crate::outline::keel(1.6, 0.8, 0.18), tier: u8::from(tiered),
                        top: StampTop::Gable { rise_mm: 0.2, axis_deg: 0.0 }, ..plate.clone()
                    };
                    d.stamps.extend([plate, keel]);
                }
                d
            };
            let judge = |d: &RingDesign| {
                let before = crate::setting::MADE.with(|m| m.get());
                assert!(findings(d).iter().all(|f| f.layer != STAMP));
                let after = crate::setting::MADE.with(|m| m.get());
                [after[0] - before[0], after[1] - before[1]]
            };
            let fewest = |runs: &mut dyn Iterator<Item = [usize; 2]>| runs.fold([usize::MAX; 2], |a, b| [a[0].min(b[0]), a[1].min(b[1])]);
            let flat = fewest(&mut (0..3).map(|_| judge(&rows(false, 0))));
            // A band a hair wider each call, none of its surface kept yet.
            let first = fewest(&mut (1..=3).map(|k| judge(&rows(true, k))));
            let again = rows(true, 0);
            judge(&again);
            let warm = fewest(&mut (0..5).map(|_| judge(&again)));
            eprintln!("{}: points and sections made: one tier {flat:?}, tiered first {first:?}, judged again {warm:?}", base.name);
            assert_eq!(flat, [0, 0], "{}: one tier reads no frame", base.name);
            assert!(first[0] == 24 && first[1] <= 24, "{}: {first:?} made on first sight, a point per keel", base.name);
            assert_eq!(warm, [0, 0], "{}: judged again reads every frame from the store", base.name);
        }
    }

    /// The solver is the checker read backwards: fitting to the sand's own
    /// floor must silence the finding it was derived from, and repeats past it
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

        // The solve reads the texels at their finer pitch, as the check did
        // before it read each axis at its own: on these tall texels it is safe
        // and the check lets a few repeats more through before it flags.
        let flags = |m: u32| {
            let mut over = t.clone();
            over.repeats_around = m;
            let mut bad = wide.clone();
            bad.layers.layers.push(LayerEntry::new("Pattern", Layer::Tiling(over)));
            !findings_in(&bad, &lib).is_empty()
        };
        let (cw, ch) = t.cell_size(&ctx);
        let alpha = lib.get(&t.alpha).unwrap();
        let tall = (ch / alpha.height as f64) / (cw / alpha.width as f64);
        let first = (n + 1..=2 * n).find(|&m| flags(m)).expect("enough repeats past the solve flag");
        eprintln!("Chevron: texels {tall:.2} times taller than wide, solved {n}, flags from {first}");
        assert!(tall > 1.05 && first > n + 1, "tall {tall}, first {first}");
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

    /// A mask on texels four times taller than wide reads its stripes at their
    /// own pitch: bars across the band at the tall pitch, bars along it at the
    /// narrow one. The disc in texels read both at the narrow pitch.
    #[test]
    fn a_mask_on_tall_texels_reads_each_axis_at_its_own_pitch() {
        let (n, bar) = (64usize, 8usize);
        let mut lib = crate::AlphaLibrary::builtin();
        let stripes = |name: &str, across: bool| {
            let data = (0..n * n).map(|i| {
                let k = if across { i / n } else { i % n };
                if k % (2 * bar) < bar { 1.0 } else { 0.0 }
            }).collect();
            crate::alpha::Alpha::new(name, n, n, data)
        };
        lib.insert(stripes("Rungs", true));
        lib.insert(stripes("Rails", false));
        let d = RingDesign::default();
        let ctx = d.field_context();
        let tile = |alpha: &str, tall: f64| {
            let mut t = TilingLayer::default_for(alpha, &ctx);
            t.repeats_around = 40;
            t.rows = 1;
            t.v_span_mm = t.cell_size(&ctx).0 * tall;
            t
        };
        let sx = ctx.circumference_mm / 40.0 / n as f64;
        let (rungs, _) = tiling_finest_mm(&tile("Rungs", 4.0), &lib, &ctx).unwrap();
        assert!((rungs / (bar as f64 * 4.0 * sx) - 1.0).abs() < 0.1, "rungs {rungs} against {}", bar as f64 * 4.0 * sx);
        let (rails, _) = tiling_finest_mm(&tile("Rails", 4.0), &lib, &ctx).unwrap();
        assert!((rails / (bar as f64 * sx) - 1.0).abs() < 0.1, "rails {rails} against {}", bar as f64 * sx);
        // Within 5% of square the texels are read as they are.
        let near = tile("Rungs", 1.04);
        let (px, _) = tiling_feature_px(&near, &lib).unwrap();
        assert_eq!(tiling_finest_mm(&near, &lib, &ctx).unwrap().0, px * sx);
    }

    /// A hide is judged at the tightest station its ink reaches, not the
    /// tightest its window does: ink at the head of a band that pinches at the
    /// palm is not read at the palm's pitch.
    #[test]
    fn a_hide_is_judged_where_it_carries_ink() {
        use crate::profile::{ShankKey, ShankKind};
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 2.4;
        d.shank.kind = ShankKind::Keyframes;
        d.shank.amount = 1.0;
        d.shank.keys = vec![
            ShankKey { theta_deg: 90.0, width_scale: 1.3, thickness_scale: 1.2, crown_scale: 1.0 },
            ShankKey { theta_deg: 270.0, width_scale: 0.5, thickness_scale: 0.6, crown_scale: 1.0 },
        ];
        let (w, h) = (360usize, 64usize);
        let ctx = d.field_context();
        let mut lib = crate::AlphaLibrary::builtin();
        let entry = crate::skin::hide_layer(&d, "Table", 0.3, crate::field::Window::default());
        let Layer::Tiling(t) = &entry.layer else { unreachable!() };
        // Ink in the columns the hide lays between 70° and 110°.
        let data = (0..w * h).map(|i| {
            let theta = (i % w) as f64 + 0.5;
            if (70.0..110.0).contains(&theta) && (i / w) % 8 < 4 { 1.0 } else { 0.0 }
        }).collect();
        lib.insert(crate::alpha::Alpha::new("Table", w, h, data));
        let (window, _) = worst_arc_ratio(&d, &entry, &ctx, &lib, None);
        let (inked, at) = worst_arc_ratio(&d, &entry, &ctx, &lib, Some(t));
        assert!((60.0..=120.0).contains(&at), "judged at {at}°");
        assert!(inked > 1.2 * window, "the table's {inked:.2} against the window's {window:.2}");
        let mut on = d.clone();
        on.layers.layers.push(entry.clone());
        let at_ink = tiling_finest_mm_at(t, &lib, &ctx, inked).unwrap().0;
        let at_palm = tiling_finest_mm_at(t, &lib, &ctx, window).unwrap().0;
        on.draft.min_detail_mm = 0.5 * (at_ink + at_palm);
        assert!(findings_in(&on, &lib).is_empty(), "{:?}", findings_in(&on, &lib));
        on.draft.min_detail_mm = 1.05 * at_ink;
        assert_eq!(findings_in(&on, &lib).len(), 1);
    }

    /// A made part's sections by rays: a claw's diameter and a collet's wall
    /// read as the Bestiarium writes them, and a point shows as area under the
    /// floor rather than as the part's section.
    #[test]
    fn a_part_reads_its_sections_by_rays() {
        use crate::csg::{P3, Solid};
        use std::f64::consts::TAU;
        // A lathe solid about z from `(r, z)` rings, outward wound, closed by fans where r > 0 at the ends.
        let lathe = |rings: &[(f64, f64)], n: usize| {
            let mut s = Solid::default();
            for &(r, z) in rings {
                for j in 0..n {
                    let a = TAU * (j as f64 + 0.37) / n as f64;
                    s.v.push([r * a.cos(), r * a.sin(), z]);
                }
            }
            let (m, n32) = (rings.len() as u32, n as u32);
            let at = |i: u32, j: u32| i * n32 + j % n32;
            for i in 0..m - 1 {
                for j in 0..n32 {
                    s.f.push([at(i, j), at(i, j + 1), at(i + 1, j + 1)]);
                    s.f.push([at(i, j), at(i + 1, j + 1), at(i + 1, j)]);
                }
            }
            s
        };
        let capped = |rings: &[(f64, f64)], n: usize| {
            let mut s = lathe(rings, n);
            let m = rings.len() as u32;
            let (lo, hi) = (s.v.len() as u32, s.v.len() as u32 + 1);
            s.v.push([0.0, 0.0, rings[0].1]);
            s.v.push([0.0, 0.0, rings[rings.len() - 1].1]);
            for j in 0..n as u32 {
                let k = (j + 1) % n as u32;
                s.f.push([lo, k, j]);
                s.f.push([hi, (m - 1) * n as u32 + j, (m - 1) * n as u32 + k]);
            }
            s
        };
        // Arachne's claw: a tapered wire 0.9 to 0.4 mm in radius, domed at its tip.
        let mut claw: Vec<(f64, f64)> = (0..=10).map(|i| (0.9 - 0.05 * i as f64, 0.4 * i as f64)).collect();
        claw.extend((1..12).map(|i| {
            let a = std::f64::consts::FRAC_PI_2 * i as f64 / 12.0;
            (0.4 * a.cos(), 4.0 + 0.4 * a.sin())
        }));
        let claw = capped(&claw, 64);
        assert_eq!(claw.open_edges(), (0, 0));
        for up in [None, Some([0.0, 0.0, 1.0])] {
            let (min, under) = part_sections(&claw, up, 0.75);
            assert!((min - 0.8).abs() < 0.03, "claw {min} mm, up {up:?}");
            assert_eq!(under, 0.0);
        }
        // Manticora's collet: a 0.8 mm wall round a 3 mm seat, 2 mm tall.
        let mut collet = lathe(&[(2.3, 0.0), (2.3, 2.0), (1.5, 2.0), (1.5, 0.0)], 96);
        let n = 96u32;
        for j in 0..n {
            let k = (j + 1) % n;
            collet.f.push([3 * n + j, 3 * n + k, k]);
            collet.f.push([3 * n + j, k, j]);
        }
        assert_eq!(collet.open_edges(), (0, 0));
        let (wall, under) = part_sections(&collet, Some([0.0, 0.0, 1.0]), 0.75);
        assert!((wall - 0.8).abs() < 0.01, "collet wall {wall} mm");
        assert_eq!(under, 0.0);
        // A 2 mm cone to a point: thin only near the point.
        let cone = capped(&(0..=20).map(|i| (1.0 - 0.05 * i as f64 + 1e-3, 0.3 * i as f64)).collect::<Vec<_>>(), 64);
        let (least, under) = part_sections(&cone, None, 0.8);
        let area = |s: &Solid| s.f.iter().map(|f| {
            let [a, b, c]: [P3; 3] = f.map(|i| s.v[i as usize]);
            let (u, v): (P3, P3) = (std::array::from_fn(|k| b[k] - a[k]), std::array::from_fn(|k| c[k] - a[k]));
            0.5 * ((u[1] * v[2] - u[2] * v[1]).powi(2) + (u[2] * v[0] - u[0] * v[2]).powi(2) + (u[0] * v[1] - u[1] * v[0]).powi(2)).sqrt()
        }).sum::<f64>();
        assert!(least < 0.1 && under > 0.0 && under < 0.5 * area(&cone), "point {least} mm, {under} of {} mm² under", area(&cone));
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

/// The thinnest section of one made part (a claw, a collet, a loft) and the
/// area of its surface whose section is under `floor_mm`, in mm and mm².
///
/// Each face is read by one ray from its centroid along its inward normal to
/// where it leaves the metal, so a wall reads its thickness, a round wire its
/// diameter and a dome through its centre; a point or a burnished lip shows as
/// a little area under the floor, which is what a lost-wax gate excepts by
/// name. With `up`, the part's own axis, faces turned more toward either end
/// of it than across it are not read: a short claw's foot or a collet's table
/// measure its height, not a section. `(f64::MAX, 0.0)` for a solid with no
/// face a ray leaves by.
pub fn part_sections(solid: &crate::csg::Solid, up: Option<crate::csg::P3>, floor_mm: f64) -> (f64, f64) {
    use crate::interaction::bvh::Bvh;
    let mesh = crate::mesh::Mesh {
        vertices: solid.v.iter().map(|p| crate::mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: solid.f.clone(),
        ..Default::default()
    };
    let bvh = Bvh::build(&mesh);
    let up = up.and_then(|u| {
        let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
        (l > 1e-12).then(|| u.map(|x| x / l))
    });
    // Far enough in that the face's own f32 plane is behind the ray.
    const IN: f64 = 1e-4;
    let (mut min, mut under) = (f64::MAX, 0.0);
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let (e1, e2) = (std::array::from_fn::<f64, 3, _>(|k| b[k] - a[k]), std::array::from_fn::<f64, 3, _>(|k| c[k] - a[k]));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !(twice > 1e-14) {
            continue;
        }
        let inward = n.map(|x| -x / twice);
        if up.is_some_and(|u| (inward[0] * u[0] + inward[1] * u[1] + inward[2] * u[2]).abs() > std::f64::consts::FRAC_1_SQRT_2) {
            continue;
        }
        let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0 + IN * inward[k]);
        let Some((_, t)) = bvh.ray(&mesh, o, inward) else { continue };
        let section = t + IN;
        min = min.min(section);
        if section < floor_mm {
            under += 0.5 * twice;
        }
    }
    (min, under)
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
    tiling_finest_mm_at(t, lib, ctx, 1.0)
}

/// [`tiling_finest_mm`] with the cell's height taken at `v_scale` of the
/// reference section's arc.
///
/// The chart's `v` is the section's own arc normalized, so a layer's cell is
/// only the reference height where the band is the reference thickness. A
/// shoulder that narrows to two thirds of it carries cells two thirds as tall,
/// and the mask's strokes with them — which the measurement missed entirely
/// while it read the reference context alone. A *decal* already did this per
/// station; a tiling covers an arc, so what matters is the tightest station
/// in it.
pub fn tiling_finest_mm_at(
    t: &crate::tiling::TilingLayer,
    lib: &crate::alpha::AlphaLibrary,
    ctx: &crate::field::FieldContext,
    v_scale: f64,
) -> Option<(f64, &'static str)> {
    let alpha = lib.get(&t.alpha)?;
    let (cw, ch) = t.cell_size(ctx);
    let ch = ch * v_scale.clamp(0.05, 8.0);
    let (sx, sy) = (cw / alpha.width.max(1) as f64, ch / alpha.height.max(1) as f64);
    // Granulometry reads a round disc in texels, so on texels far from square
    // one axis was read at the other's pitch: the coarser axis is repeated out
    // to the finer pitch first and the disc is round in millimetres.
    let (ink_px, gap_px, scale) = if sx.is_finite() && sy > 0.0 && (sx / sy - 1.0).abs() > 0.05 {
        let square = stretched(&shaped_mask(t, lib)?, sx / sy);
        let (ink_px, gap_px) = square.min_feature_px()?;
        (ink_px, gap_px, (cw / square.width as f64).min(ch / square.height as f64))
    } else {
        let (ink_px, gap_px) = tiling_feature_px(t, lib)?;
        (ink_px, gap_px, sx.min(sy))
    };
    let (ink, gap) = (ink_px * scale, gap_px * scale);
    Some(if ink <= gap { (ink, "strokes") } else { (gap, "gaps") })
}

/// `alpha` with its coarser axis repeated nearest-texel to the finer pitch,
/// `sx_over_sy` being a texel's width over its height in millimetres, and no
/// side past 4096. Whole periods are kept, so a seamless mask stays seamless.
fn stretched(alpha: &crate::alpha::Alpha, sx_over_sy: f64) -> crate::alpha::Alpha {
    let (w, h) = (alpha.width.max(1), alpha.height.max(1));
    let (nw, nh) = if sx_over_sy > 1.0 {
        (((w as f64 * sx_over_sy).round() as usize).clamp(w, w.max(4096)), h)
    } else {
        (w, ((h as f64 / sx_over_sy).round() as usize).clamp(h, h.max(4096)))
    };
    let mut data = Vec::with_capacity(nw * nh);
    for row in 0..nh {
        let y = (((row as f64 + 0.5) / nh as f64 * h as f64) as usize).min(h - 1);
        data.extend((0..nw).map(|col| alpha.data[y * w + (((col as f64 + 0.5) / nw as f64 * w as f64) as usize).min(w - 1)]));
    }
    crate::alpha::Alpha::new(format!("{} {nw}x{nh}", alpha.name), nw, nh, data)
}

/// The tightest section a layer's window covers, as a ratio of the reference
/// section's arc, and the angle it is at.
///
/// A modulated band is not one section: `sample_mod`'s `surface_len_mm` moves
/// with the shank, and every layer measured against the reference alone was
/// judged on a band it does not sit on everywhere. With `ink`, a station is
/// counted only where that tiling stands at least half its height within
/// half a station either side.
fn worst_arc_ratio(
    design: &RingDesign,
    entry: &crate::field::LayerEntry,
    ctx: &crate::field::FieldContext,
    lib: &crate::AlphaLibrary,
    ink: Option<&crate::tiling::TilingLayer>,
) -> (f64, f64) {
    const STATIONS: usize = 72;
    let inner_r = design.inner_radius_mm();
    let crest_r = inner_r + design.profile.thickness_mm;
    let reference = ctx.band_v_len_mm.max(1e-9);
    // A hide wholly on stations thicker than the reference is judged there, not at the reference.
    let mut worst = if ink.is_some() { (f64::INFINITY, 0.0) } else { (1.0f64, 0.0f64) };
    for k in 0..STATIONS {
        let theta = k as f64 / STATIONS as f64 * 360.0;
        // The layer's own gate, read through the mask it actually uses: a
        // station the window keeps out cannot be the one that fails.
        let u = theta / 360.0 * ctx.circumference_mm;
        let uv = crate::field::Uv { u, v: ctx.band_v_len_mm * 0.5 };
        if entry.window.enabled && entry.window.mask(uv, ctx) <= 1e-6 {
            continue;
        }
        if design.imported_base.is_some() && entry.mask.is_some()
            && !(0..=128).any(|j| entry.mask_at(crate::Uv { u, v: reference*j as f64/128. }, ctx, lib)>1e-3)
        {
            continue;
        }
        if let Some(t) = ink {
            let step = ctx.circumference_mm / STATIONS as f64;
            let half = 0.5 * t.height_mm.abs();
            let inked = (0..8).any(|i| {
                let u = u + step * ((i as f64 + 0.5) / 8.0 - 0.5);
                (0..=64).any(|j| t.height(crate::field::Uv { u, v: ctx.band_v_len_mm * j as f64 / 64.0 }, ctx, lib).abs() >= half)
            });
            if !inked {
                continue;
            }
        }
        let m = design.modulation_at(theta, inner_r, crest_r);
        let len = design.profile.sample_mod(inner_r, 96, &m).surface_len_mm;
        let ratio = if design.imported_base.is_some() { ctx.station_stretch(theta) } else { len / reference };
        if ratio.is_finite() && ratio < worst.0 {
            worst = (ratio, theta);
        }
    }
    if worst.0.is_infinite() { (1.0, 0.0) } else { worst }
}

/// The mask's finest ink and gap in texels, after the layer's own shaping.
fn tiling_feature_px(t: &crate::tiling::TilingLayer, lib: &crate::alpha::AlphaLibrary) -> Option<(f64, f64)> {
    shaped_mask(t, lib)?.min_feature_px()
}

/// The layer's mask after its own contrast/bias/invert.
fn shaped_mask(t: &crate::tiling::TilingLayer, lib: &crate::alpha::AlphaLibrary) -> Option<crate::alpha::Alpha> {
    let alpha = lib.get(&t.alpha)?;
    Some(if t.invert || (t.contrast - 1.0).abs() > 1e-9 || t.bias.abs() > 1e-9 {
        let data = alpha.data.iter().map(|&v| alpha.shaped(v, t.contrast, t.bias, t.invert) as f32).collect();
        crate::alpha::Alpha::new(format!("{} shaped", alpha.name), alpha.width, alpha.height, data)
    } else {
        alpha.clone()
    })
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

#[cfg(test)]
mod flute_tests {
    use crate::field::{FlutesLayer, FluteProfile, Layer, LayerEntry};
    use crate::RingDesign;

    /// `FeatureFootprint::across` sets `feature_u_mm = INFINITY`, and
    /// `metal_feature_mm` scales only the `u` side by the arc ratio — so a
    /// flute filed as `across` was reported at its chart width, never at the
    /// metal width. `u` is arc at the crest radius and everything else sits
    /// inside it: on a squared band the side face runs at ~0.85 of that, so
    /// the figure was 15-20% optimistic, in the unsafe direction, on the
    /// surface the doctrine sends all ornament to.
    #[test]
    fn a_flute_is_measured_around_the_ring_and_at_its_metal_width() {
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.width_mm = 6.0;
        d.profile.thickness_mm = 3.0;
        d.profile.flatten_sides();
        let ctx = d.field_context();

        let flutes = FlutesLayer {
            count: 30,
            profile: FluteProfile::Round,
            width_mm: 1.2,
            height_mm: 0.3,
            ..Default::default()
        };
        let entry = LayerEntry::new("reeding", Layer::Flutes(flutes));
        let fp = entry.layer.feature_footprints(&ctx);
        assert_eq!(fp.len(), 1);

        // Narrow around the ring, unlimited across the band — the other way
        // round from a rail or a milgrain line.
        assert!(fp[0].feature_u_mm.is_finite(), "a flute is narrow in u");
        assert!(fp[0].feature_v_mm.is_infinite(), "and runs the band in v");

        // And the arc correction now actually reaches it.
        let chart = fp[0].min_feature_mm();
        let metal = fp[0].metal_feature_mm(&ctx);
        assert!(
            metal < chart * 0.999,
            "metal {metal:.4} must be under the chart figure {chart:.4}"
        );
    }

    /// A dense reeding fails on the bare band between two cuts long before it
    /// fails on the cut, and only the cut was ever measured.
    #[test]
    fn the_land_between_flutes_is_a_feature_too() {
        let d = RingDesign::default();
        let ctx = d.field_context();
        let pitch = ctx.circumference_mm / 60.0;
        // Cuts wide enough that the land between them is the finer of the two.
        let width = pitch * 0.85;
        let flutes = FlutesLayer {
            count: 60,
            profile: FluteProfile::Round,
            width_mm: width,
            height_mm: 0.25,
            ..Default::default()
        };
        let entry = LayerEntry::new("dense", Layer::Flutes(flutes));
        let fp = entry.layer.feature_footprints(&ctx);
        let land = pitch - width;
        assert!(
            (fp[0].feature_u_mm - land).abs() < 1e-6,
            "the land ({land:.4}) is finer than the cut ({width:.4}) and must be what is reported, got {:.4}",
            fp[0].feature_u_mm
        );
    }
}
