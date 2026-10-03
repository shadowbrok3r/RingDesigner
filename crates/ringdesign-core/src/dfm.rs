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
/// [`DfmFinding::layer`] of a finding about a CAD part, which is not a layer either.
pub const PART: usize = usize::MAX - 1;
/// [`DfmFinding::label`] of a [`cut_lands`] finding.
pub const CUT_LAND: &str = "cut land";

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
    /// Every tiling in `stack`, with the remaps its relief passes through on the way out, its own first.
    fn tilings<'a>(stack: &'a crate::field::LayerStack, outer: &[crate::field::Remap], out: &mut Vec<(&'a crate::tiling::TilingLayer, Vec<crate::field::Remap>)>) {
        for e in stack.layers.iter().filter(|e| e.enabled) {
            let chain = || std::iter::once(e.remap).chain(outer.iter().copied()).filter(|r| !r.is_off()).collect::<Vec<_>>();
            match &e.layer {
                Layer::Tiling(t) => out.push((t, chain())),
                // An openwork's cut is its mask, not its relief.
                Layer::Openwork(o) => out.push((&o.tiling, Vec::new())),
                Layer::Group(g) => tilings(&g.stack, &chain(), out),
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
        tilings(&one, &[], &mut ts);
        let window = worst_arc_ratio(design, entry, &ctx, lib, None);
        for (t, remaps) in ts {
            // One tile round the whole ring, a hide, lays each column at its own
            // angle: it is judged where it carries ink, not where its window reaches.
            let (ratio, at_deg) = if t.repeats_around == 1 { worst_arc_ratio(design, entry, &ctx, lib, Some(t)) } else { window };
            let Some((finest, what)) = tiling_finest_mm_remapped(t, lib, &ctx, ratio, &remaps) else { continue };
            let (cw, ch) = t.finest_cell_size(&ctx);
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

type P3 = [f64; 3];
fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot3(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// The distance from `p` to the segment `a b`.
fn to_segment(p: P3, a: P3, b: P3) -> f64 {
    let (d, w) = (sub3(b, a), sub3(p, a));
    let t = (dot3(w, d) / dot3(d, d).max(1e-30)).clamp(0.0, 1.0);
    let q = sub3(w, d.map(|v| v * t));
    dot3(q, q).sqrt()
}
/// The least distance between two sets of closed loops.
fn loops_apart(a: &[Vec<P3>], b: &[Vec<P3>]) -> f64 {
    let one_way = |a: &[Vec<P3>], b: &[Vec<P3>]| {
        a.iter().flatten().map(|p| b.iter().map(|l| (0..l.len()).map(|k| to_segment(*p, l[k], l[(k + 1) % l.len()])).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min)).fold(f64::INFINITY, f64::min)
    };
    one_way(a, b).min(one_way(b, a))
}
/// A box round loops: least and greatest corner.
fn box_of(loops: &[Vec<P3>]) -> (P3, P3) {
    loops.iter().flatten().fold(([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]), |(lo, hi), p| (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))))
}
/// How far apart two boxes are at the least.
fn boxes_apart(a: (P3, P3), b: (P3, P3)) -> f64 {
    let gap: P3 = std::array::from_fn(|k| (a.0[k] - b.1[k]).max(b.0[k] - a.1[k]).max(0.0));
    dot3(gap, gap).sqrt()
}
/// Points round a loop of curves about `step` apart.
fn sampled(curves: &[cadkernel::geom2d::Curve], step: f64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for c in curves {
        let n = ((c.length() / step).ceil() as usize).clamp(4, 400);
        out.extend((0..n).map(|k| c.point_at(k as f64 / n as f64)));
    }
    out
}

/// For every Cut extrusion, and every copy a Pattern makes of one, the narrowest metal it leaves:
/// between two of its regions, between it and its copies, and from a region to the band's or its host
/// part's edge, each kind reported once where it falls under `floor_mm`, labelled [`CUT_LAND`]. Lands
/// between regions are measured between their outlines in the sketch's plane, carried to each copy by
/// the copy's motion; the land to an edge is walked out from each outline in that plane until a line
/// along the plane's normal, within the cut's reach, no longer meets metal in the ring as `built`.
/// Nothing calls it unasked: a design reports what it always did until a floor is asked for.
pub fn cut_lands(design: &RingDesign, built: &crate::mesh::BuildResult, floor_mm: f64) -> Vec<DfmFinding> {
    use crate::cad::{Attach, Operation};
    let mut out = Vec::new();
    let (Some(doc), Some(e)) = (design.cad.as_ref(), built.parts.evaluated.as_ref()) else { return out };
    if !(floor_mm.is_finite() && floor_mm > 0.0) {
        return out;
    }
    let bvh = crate::interaction::bvh::Bvh::build(&built.mesh);
    let found = |who: &str, land: f64, what: String| DfmFinding { layer: PART, label: CUT_LAND.into(), message: format!("{who}: {land:.2} mm {what} (floor {floor_mm})") };
    for c in e.components.iter().filter(|c| c.attach == Attach::Cut) {
        let Some(f) = doc.feature(c.id).filter(|f| f.enabled) else { continue };
        let Operation::Extrude { height_mm, .. } = f.operation else { continue };
        let Some((plane, regions)) = crate::cad::extruded_regions(doc, &f.operation, e) else { continue };
        let Some(normal) = plane.normal() else { continue };
        let normal = c.frame.vector(normal);
        let reach = height_mm.abs() + crate::cad::CUT_CLEAR_MM + 0.5;
        let world = |uv: [f64; 2]| c.frame.point(plane.point_at(uv));
        let who = format!("Cut #{} '{}'", c.id, f.name);
        let step = (0.25 * floor_mm).min(0.05);
        let uv: Vec<Vec<Vec<[f64; 2]>>> = regions.iter().map(|r| r.loops().iter().map(|l| sampled(l, step)).collect()).collect();
        let source: Vec<Vec<Vec<P3>>> = uv.iter().map(|r| r.iter().map(|l| l.iter().map(|p| world(*p)).collect()).collect()).collect();
        // The copies every pattern of this cut makes, the source itself left out.
        let still = |m: &cadkernel::brep::Placement| dot3(m.origin, m.origin) < 1e-18 && (m.x_axis[0] - 1.0).abs() < 1e-12 && (m.y_axis[1] - 1.0).abs() < 1e-12;
        let copies: Vec<cadkernel::brep::Placement> = doc
            .features
            .iter()
            .filter(|p| p.enabled)
            .filter_map(|p| match &p.operation {
                Operation::Pattern { sources, kind } if sources.contains(&c.id) => crate::cad::pattern::copy_motions(design, built.band.as_deref(), e, c.id, kind).ok(),
                _ => None,
            })
            .flatten()
            .filter(|m| !still(m))
            .collect();
        // Between two regions of the cut.
        let boxes: Vec<(P3, P3)> = source.iter().map(|r| box_of(r)).collect();
        let mut least: Option<(f64, usize, usize)> = None;
        for i in 0..source.len() {
            for j in i + 1..source.len() {
                if boxes_apart(boxes[i], boxes[j]) >= floor_mm {
                    continue;
                }
                let d = loops_apart(&source[i], &source[j]);
                if least.is_none_or(|(best, ..)| d < best) {
                    least = Some((d, i, j));
                }
            }
        }
        if let Some((d, i, j)) = least.filter(|(d, ..)| *d < floor_mm) {
            out.push(found(&who, d, format!("between lights {} and {}", i + 1, j + 1)));
        }
        // Between the cut and its copies.
        let mut least: Option<(f64, usize, usize, usize)> = None;
        for (k, m) in copies.iter().enumerate() {
            let moved: Vec<Vec<Vec<P3>>> = source.iter().map(|r| r.iter().map(|l| l.iter().map(|p| m.point(*p)).collect()).collect()).collect();
            for (i, a) in source.iter().enumerate() {
                for (j, b) in moved.iter().enumerate() {
                    if boxes_apart(boxes[i], box_of(b)) >= floor_mm {
                        continue;
                    }
                    let d = loops_apart(a, b);
                    if least.is_none_or(|(best, ..)| d < best) {
                        least = Some((d, i, k, j));
                    }
                }
            }
        }
        let between_copies = least;
        // To the edge: out from each outline in the plane until the line along the normal leaves the metal.
        let inverse: Vec<cadkernel::brep::Placement> = copies.iter().map(crate::cad::pattern::inverse).collect();
        let (x, y, o) = (c.frame.vector(plane.x_axis), c.frame.vector(plane.y_axis), world([0.0, 0.0]));
        let back = |w: P3| {
            let d = sub3(w, o);
            [dot3(d, x) / dot3(x, x), dot3(d, y) / dot3(y, y)]
        };
        let in_a_light = |q: [f64; 2]| {
            let w = world(q);
            regions.iter().any(|r| r.contains(q)) || inverse.iter().any(|m| {
                let at = back(m.point(w));
                regions.iter().any(|r| r.contains(at))
            })
        };
        let metal = |q: [f64; 2]| {
            let w = world(q);
            [normal, normal.map(|v| -v)].iter().any(|d| bvh.ray(&built.mesh, w, *d).is_some_and(|(_, t)| t <= reach))
        };
        // The copy, and its light, whose opening the line along the normal through `q` runs through within reach:
        // a copy turned round the ring is met at the metal's depth, not in this plane.
        let through_copy = |q: [f64; 2]| {
            let w = world(q);
            let steps = (2.0 * reach / 0.05).ceil() as usize;
            inverse.iter().enumerate().find_map(|(k, m)| {
                (0..=steps).find_map(|i| {
                    let t = -reach + 2.0 * reach * i as f64 / steps as f64;
                    let at = back(m.point(std::array::from_fn(|a| w[a] + t * normal[a])));
                    regions.iter().position(|r| r.contains(at)).map(|j| (k, j))
                })
            })
        };
        let march = floor_mm / 20.0;
        let mut least: Option<(f64, usize)> = None;
        let mut copy_least: Option<(f64, usize, usize, usize)> = between_copies.filter(|(d, ..)| *d < floor_mm);
        for (i, (r, loops)) in regions.iter().zip(&uv).enumerate() {
            for l in loops {
                let n = l.len();
                for k in 0..n {
                    let (p, a, b) = (l[k], l[(k + n - 1) % n], l[(k + 1) % n]);
                    let t = [b[0] - a[0], b[1] - a[1]];
                    let len = t[0].hypot(t[1]);
                    if len < 1e-12 {
                        continue;
                    }
                    let mut away = [t[1] / len, -t[0] / len];
                    if r.contains([p[0] + 1e-4 * away[0], p[1] + 1e-4 * away[1]]) {
                        away = away.map(|v| -v);
                    }
                    let at = |s: f64| [p[0] + s * away[0], p[1] + s * away[1]];
                    let best = least.map_or(floor_mm, |(d, _)| d).min(copy_least.map_or(floor_mm, |(d, ..)| d));
                    let mut s = march;
                    while s < best {
                        let q = at(s);
                        if in_a_light(q) {
                            break;
                        }
                        if !metal(q) {
                            // Halve back to where the metal ends.
                            let (mut lo, mut hi) = (s - march, s);
                            for _ in 0..12 {
                                let mid = 0.5 * (lo + hi);
                                if metal(at(mid)) { lo = mid } else { hi = mid }
                            }
                            match through_copy(q) {
                                Some((c, j)) => copy_least = Some((hi, i, c, j)),
                                None => least = Some((hi, i)),
                            }
                            break;
                        }
                        s += march;
                    }
                }
            }
        }
        if let Some((d, i, k, j)) = copy_least {
            out.push(found(&who, d, format!("between light {} and copy {}'s light {}", i + 1, k + 2, j + 1)));
        }
        if let Some((d, i)) = least {
            out.push(found(&who, d, format!("from light {} to the edge", i + 1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry, MilgrainLayer};
    use crate::tiling::TilingLayer;

    /// A stamp's plan is its metal: a thin arm is found, a pointed tip is not held against it, and one cut
    /// at the bench is not the sand's to judge.
    /// The Court band with lights cut down through its crown from a sketch on a plane over it, `extra` after.
    fn lit(lights: &[[f64; 4]], extra: Vec<crate::cad::Feature>) -> RingDesign {
        use crate::cad::{Attach, Component, Document, Feature, Operation, Profile};
        use crate::sketch::{Sketch, Workplane};
        let mut d = crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let over = d.inner_radius_mm() + d.profile.thickness_mm + 1.0;
        // x round the ring, y along the finger: the plane's normal points down at the crown.
        let mut s = Sketch { plane: Workplane { origin: [0.0, over, 0.0], x: [1.0, 0.0, 0.0], y: [0.0, 0.0, 1.0], on_face: None }, ..Sketch::default() };
        for l in lights {
            s.add_rectangle([l[0], l[1]], [l[2], l[3]], false).unwrap();
        }
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 2, name: "Lights".into(), enabled: true, operation: Operation::Sketch { sketch: s }, component: Component::default() }).unwrap();
        let cut = Component { attach: Attach::Cut, ..Component::default() };
        doc.append(Feature { id: 3, name: "Pierce the lights".into(), enabled: true, operation: Operation::Extrude { sketch: Profile::Feature { feature: 2 }, height_mm: d.profile.thickness_mm + 2.5, draft_deg: 0.0 }, component: cut }).unwrap();
        for f in extra {
            doc.append(f).unwrap();
        }
        d.cad = Some(doc);
        d
    }

    #[test]
    fn a_cut_names_the_narrowest_land_between_its_lights_its_copies_and_the_edge() {
        let lib = crate::AlphaLibrary::builtin();
        let build = |d: &RingDesign| crate::mesh::try_build(d, &lib, crate::BuildParams::default()).unwrap();
        let bare = build(&lit(&[], vec![]));
        let half = 0.5 * crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design().profile.width_mm;
        // Two 1 mm lights 0.6 mm apart round the ring, well inside the band's width.
        let near = lit(&[[-1.3, -0.5, -0.3, 0.5], [0.3, -0.5, 1.3, 0.5]], vec![]);
        let built = build(&near);
        assert!(built.report.volume_mm3 < bare.report.volume_mm3 - 1.0, "the lights cut the crown");
        let found = cut_lands(&near, &built, 0.8);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].layer == PART && found[0].label == CUT_LAND);
        assert_eq!(found[0].message, "Cut #3 'Pierce the lights': 0.60 mm between lights 1 and 2 (floor 0.8)");
        // A floor under the land says nothing, and so does the design's own report: the check is asked for.
        assert!(cut_lands(&near, &built, 0.5).is_empty());
        assert!(findings(&near).is_empty());
        // Lights a full floor apart pass.
        let apart = lit(&[[-1.5, -0.5, -0.5, 0.5], [0.5, -0.5, 1.5, 0.5]], vec![]);
        assert!(cut_lands(&apart, &build(&apart), 0.8).is_empty());
        // A light 0.5 mm in from the band's side leaves 0.5 mm to the edge, found through the metal as built.
        let edge = lit(&[[-0.5, half - 1.5, 0.5, half - 0.5]], vec![]);
        let found = cut_lands(&edge, &build(&edge), 0.8);
        assert_eq!(found.len(), 1, "{found:?}");
        let land: f64 = found[0].message.split(": ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
        assert!(found[0].message.ends_with("mm from light 1 to the edge (floor 0.8)") && (land - 0.5).abs() <= 0.06, "{}", found[0].message);
        // A ring of 48 copies of one light: each copy stands too close to the next.
        use crate::cad::{Attach, Component, Feature, Operation, pattern::PatternKind};
        let ring = Feature { id: 4, name: "Round the ring".into(), enabled: true, operation: Operation::Pattern { sources: crate::cad::pattern::Sources(vec![3]), kind: PatternKind::Ring { count: 48, span_deg: 360.0 } }, component: Component { attach: Attach::Cut, ..Component::default() } };
        let round = lit(&[[-0.5, -0.5, 0.5, 0.5]], vec![ring]);
        let built = build(&round);
        let found = cut_lands(&round, &built, 0.8);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].message.starts_with("Cut #3 'Pierce the lights': 0.") && found[0].message.contains("between light 1 and copy"), "{}", found[0].message);
    }

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
            sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, fine_cap: false, tier: 0, top: Default::default(),
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
            sink_mm: 0.3, draft_deg: 0.0, cut, bench: false, along_pull: false, fine_cap: false, tier, top: StampTop::Flat,
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
                        height_mm: 0.3, sink_mm: 0.3, draft_deg: 0.0, cut: false, bench: false, along_pull: false, fine_cap: false, tier: 0, top: StampTop::Flat,
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

    /// A graded tiling is measured at its small pole: its finest feature falls
    /// with the grade, and a floor between the two catches only the graded
    /// layer. At taper 0 the measure is the ungraded one, bit for bit.
    #[test]
    fn a_graded_tiling_is_measured_at_its_finest_cells() {
        use crate::tiling::{GradeLaw, TileGrade};
        let lib = crate::AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut t = TilingLayer::default_for("Scales", &ctx);
        t.repeats_around = 8;
        // Isotropic, so both axes shrink; Scales' finest feature runs across the band.
        let grade = |taper: f64| Some(TileGrade { taper, theta_deg: 90.0, law: GradeLaw::Cosine, isotropic: true });
        let plain = tiling_finest_mm(&t, &lib, &ctx).unwrap().0;
        let flat = tiling_finest_mm(&TilingLayer { grade: grade(0.0), ..t.clone() }, &lib, &ctx).unwrap().0;
        assert_eq!(plain.to_bits(), flat.to_bits());
        let graded = TilingLayer { grade: grade(0.6), ..t.clone() };
        let fine = tiling_finest_mm(&graded, &lib, &ctx).unwrap().0;
        assert!(fine < plain * 0.9, "graded {fine} against ungraded {plain}");
        d.draft.min_detail_mm = 0.5 * (fine + plain);
        let mut ungraded = d.clone();
        ungraded.layers.layers.push(LayerEntry::new("Scales", Layer::Tiling(t)));
        assert!(findings_in(&ungraded, &lib).is_empty(), "{:?}", findings_in(&ungraded, &lib));
        d.layers.layers.push(LayerEntry::new("Scales", Layer::Tiling(graded)));
        assert_eq!(findings_in(&d, &lib).len(), 1, "the graded layer is caught at its small pole");
    }

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

    /// C-R6: a terrace remap cuts a smooth relief into treads, and a tread is
    /// a flat between two risers that the half-height threshold never sees.
    /// DFM measures the relief as remapped: the treads show, a remap that is
    /// off measures exactly as before, and the finding follows the layer's
    /// own remap and its group's.
    #[test]
    fn terrace_treads_are_measured_through_the_remap() {
        use crate::field::{GroupLayer, Remap};
        let lib = crate::AlphaLibrary::builtin();
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let mut t = TilingLayer::default_for("Pyramids", &ctx);
        t.repeats_around = 24;
        t.rows = 1;
        t.v_center_mm = ctx.crest_v_mm;
        t.v_span_mm = 3.0;
        let plain = tiling_finest_mm_at(&t, &lib, &ctx, 1.0).unwrap();
        assert_eq!(tiling_finest_mm_remapped(&t, &lib, &ctx, 1.0, &[]), Some(plain));
        let terrace = Remap::Terrace { steps: 8, span_mm: t.height_mm, riser: 0.2 };
        let (tread, what) = tiling_finest_mm_remapped(&t, &lib, &ctx, 1.0, &[terrace]).unwrap();
        assert_eq!(what, "treads");
        assert!(tread < plain.0 * 0.8, "treads {tread:.3} mm against the plain {:.3} mm", plain.0);
        // A floor between the two: the plain layer passes, the terraced one is caught, in a group or on its own.
        d.draft.min_detail_mm = 0.5 * (tread + plain.0);
        d.layers.layers.push(LayerEntry::new("Pyramids", Layer::Tiling(t.clone())));
        assert!(findings_in(&d, &lib).is_empty(), "{:?}", findings_in(&d, &lib));
        d.layers.layers[0].remap = terrace;
        let own = findings_in(&d, &lib);
        assert!(own.len() == 1 && own[0].message.contains("treads"), "{own:?}");
        d.layers.layers[0].remap = Remap::Off;
        let mut group = LayerEntry::new("Hide", Layer::Group(GroupLayer { stack: crate::field::LayerStack { layers: vec![d.layers.layers.remove(0)] }, ..Default::default() }));
        group.remap = terrace;
        d.layers.layers.push(group);
        assert_eq!(findings_in(&d, &lib).len(), 1);
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

    /// A pebbled hide over a thick slab: the plain census reads the granules'
    /// chords as sections near nothing, the relief-aware one sets them apart,
    /// and a thin fin standing on the slab is still read as the section it is.
    #[test]
    fn relief_is_told_from_a_section() {
        use crate::sculpt::{smin, tetra_mesh};
        // Rounded boxes, so no sharp edge reads a chord of its own.
        let rounded = |p: [f64; 3], c: [f64; 3], half: [f64; 3], r: f64| {
            let q: [f64; 3] = std::array::from_fn(|k| (p[k] - c[k]).abs() - half[k] + r);
            (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2) + q[2].max(0.0).powi(2)).sqrt() + q[0].max(q[1]).max(q[2]).min(0.0) - r
        };
        let slab = move |p: [f64; 3]| rounded(p, [0.0; 3], [1.6, 1.6, 0.8], 0.3);
        // A 0.3 mm fin standing 1.2 mm off the slab along x = 0, y > 0.6, its top and ends rounded right through.
        let fin = move |p: [f64; 3]| rounded(p, [0.0, 1.1, 1.0], [0.15, 0.5, 0.6], 0.149);
        let skin = move |p: [f64; 3]| smin(slab(p), fin(p), 0.05);
        let granules = |p: [f64; 3]| {
            let mut d = f64::MAX;
            for i in -3..=3 {
                for j in -3..=0 {
                    let c = [0.45 * i as f64, 0.45 * j as f64 + 0.2, 0.85];
                    d = d.min(((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2)).sqrt() - 0.2);
                }
            }
            d
        };
        let part = move |p: [f64; 3]| smin(skin(p), granules(p), 0.03);
        let solid = tetra_mesh([-2.0, -2.0, -1.2], [2.0, 2.0, 2.0], 0.04, &part);
        let (plain, under) = part_sections(&solid, None, 0.8);
        assert!(plain < 0.1 && under > 0.5, "plain census: {plain} mm, {under} mm² under");
        // The fin's faces, and its wall well clear of its root.
        let on = |z: f64| -> Vec<bool> { solid.f.iter().map(|f| f.iter().all(|i| solid.v[*i as usize][0].abs() < 0.2 && solid.v[*i as usize][1] > 0.55 && solid.v[*i as usize][2] > z)).collect() };
        let (on_fin, wall) = (on(0.8), on(1.0));
        let fin_least = face_sections(&solid, None).iter().zip(&wall).filter(|(_, f)| **f).filter_map(|(s, _)| *s).fold(f64::MAX, f64::min);
        assert!((fin_least - 0.3).abs() < 0.02, "the fin reads {fin_least} mm");
        let read = part_sections_relief(&solid, None, 0.8, &Relief { skin: Some(&skin), radius_mm: 0.0, max_deg: 45.0, height_mm: 0.25 });
        assert!(read.relief_mm2 > 0.5 && read.relief_thinnest_mm < 0.1, "{read:?}");
        assert!((read.thinnest_mm - 0.3).abs() < 0.02, "the fin is the thinnest section: {read:?}");
        let fin_area: f64 = solid.f.iter().zip(&on_fin).filter(|(_, f)| **f).map(|(f, _)| 0.5 * face_normal(&solid, f).1).sum();
        assert!(read.under_mm2 <= fin_area, "{read:?} against the fin's {fin_area} mm²");
        // The plain read is the fold of the per-face reads, and the relief read splits it without losing any.
        assert!((read.under_mm2 + read.relief_mm2 - under).abs() < 1e-9);
        // Without the skin the base is read off the mesh; the granules still go to relief.
        let bare = part_sections_relief(&solid, None, 0.8, &Relief { skin: None, radius_mm: 0.6, max_deg: 45.0, height_mm: 0.25 });
        assert!(bare.relief_mm2 > 0.5 * read.relief_mm2, "{bare:?} against {read:?}");
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
/// face a ray leaves by. A fold over [`face_sections`].
pub fn part_sections(solid: &crate::csg::Solid, up: Option<crate::csg::P3>, floor_mm: f64) -> (f64, f64) {
    let (mut min, mut under) = (f64::MAX, 0.0);
    for (read, twice) in face_reads(solid, up).into_iter().zip(face_twice_areas(solid)) {
        let Some(read) = read else { continue };
        min = min.min(read.section);
        if read.section < floor_mm {
            under += 0.5 * twice;
        }
    }
    (min, under)
}

/// Each face's single-ray section as [`part_sections`] reads it, in face
/// order: `None` for a degenerate face, one turned toward `up`, or one no ray
/// leaves by. A caller that names its faces (a toe, a spine, a hide) folds
/// these by name.
pub fn face_sections(solid: &crate::csg::Solid, up: Option<crate::csg::P3>) -> Vec<Option<f64>> {
    face_reads(solid, up).into_iter().map(|r| r.map(|r| r.section)).collect()
}

/// One face's ray: where it starts, which way it runs, and how far it goes in the metal.
#[derive(Clone, Copy, Debug)]
struct FaceRead {
    origin: crate::csg::P3,
    inward: crate::csg::P3,
    section: f64,
}

/// Twice each face's area, in face order.
fn face_twice_areas(solid: &crate::csg::Solid) -> Vec<f64> {
    solid.f.iter().map(|f| face_normal(solid, f).1).collect()
}

/// A face's unnormalised normal and its length, twice the face's area.
fn face_normal(solid: &crate::csg::Solid, f: &[u32; 3]) -> (crate::csg::P3, f64) {
    let [a, b, c] = f.map(|i| solid.v[i as usize]);
    let (e1, e2) = (std::array::from_fn::<f64, 3, _>(|k| b[k] - a[k]), std::array::from_fn::<f64, 3, _>(|k| c[k] - a[k]));
    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    (n, (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt())
}

/// Each face's ray, in face order, as [`face_sections`] reads it.
fn face_reads(solid: &crate::csg::Solid, up: Option<crate::csg::P3>) -> Vec<Option<FaceRead>> {
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
    solid
        .f
        .iter()
        .map(|f| {
            let [a, b, c] = f.map(|i| solid.v[i as usize]);
            let (n, twice) = face_normal(solid, f);
            if !(twice > 1e-14) {
                return None;
            }
            let inward = n.map(|x| -x / twice);
            if up.is_some_and(|u| (inward[0] * u[0] + inward[1] * u[1] + inward[2] * u[2]).abs() > std::f64::consts::FRAC_1_SQRT_2) {
                return None;
            }
            let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0 + IN * inward[k]);
            let (_, t) = bvh.ray(&mesh, o, inward)?;
            Some(FaceRead { origin: o, inward, section: t + IN })
        })
        .collect()
}

/// How [`part_sections_relief`] tells a relief's own flank from a section the
/// metal must fill.
///
/// A ray from the flank of a tubercle, a scale or a bead runs nearly along the
/// surface the relief stands on and leaves through the next flank a little
/// way off: a short chord, not a thin wall. A face is read as relief when its
/// ray runs within `max_deg` of the base surface's tangent plane and leaves
/// the metal within `height_mm` of that surface. A thin toe or a wire is read
/// across, along the base's normal, so it stays a section.
#[derive(Clone, Copy)]
pub struct Relief<'a> {
    /// The base surface as a field, negative inside: the part without its
    /// relief. Its gradient is the base's normal, and a ray leaves within
    /// `height_mm` of the base where the field there is above `-height_mm`.
    /// `None` reads the base off the mesh: each face's normal averaged over
    /// `radius_mm` round it, and the height as how far the ray sinks below
    /// the plane through its start.
    pub skin: Option<&'a (dyn Fn(crate::csg::P3) -> f64 + Sync)>,
    /// Radius the mesh's normals are averaged over when there is no skin, mm:
    /// about a relief cell, so one bump's flanks cancel.
    pub radius_mm: f64,
    /// Steepest angle between a relief ray and the base's tangent plane,
    /// degrees; 90 takes every ray the height lets through.
    pub max_deg: f64,
    /// How far below the base a relief ray may leave the metal, mm: the
    /// relief's height and a little.
    pub height_mm: f64,
}

/// A part's sections with its relief told apart: see [`Relief`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReliefSections {
    /// The thinnest section that is not relief, mm; `f64::MAX` when none is read.
    pub thinnest_mm: f64,
    /// Area under the floor that is not relief, mm².
    pub under_mm2: f64,
    /// Area under the floor read as relief, mm².
    pub relief_mm2: f64,
    /// The thinnest chord read as relief, mm; `f64::MAX` when none is.
    pub relief_thinnest_mm: f64,
}

/// [`part_sections`] with relief told apart from sections, so a pebbled or
/// scaled hide over a thick body does not read as a 0 mm wall. A face whose
/// ray reads at or over `floor_mm` counts as it does in [`part_sections`];
/// under it, a face [`Relief`] calls a relief chord goes to `relief_mm2`
/// instead of the thinnest section and the area under the floor.
pub fn part_sections_relief(solid: &crate::csg::Solid, up: Option<crate::csg::P3>, floor_mm: f64, relief: &Relief) -> ReliefSections {
    let reads = face_reads(solid, up);
    let normals: Vec<(crate::csg::P3, f64)> = solid.f.iter().map(|f| face_normal(solid, f)).collect();
    // Without a skin, the base's normal is the mesh's own averaged over the radius: face centroids on a grid that wide.
    let cell = relief.radius_mm.max(1e-3);
    let centroid = |f: &[u32; 3]| -> crate::csg::P3 { std::array::from_fn(|k| f.iter().map(|i| solid.v[*i as usize][k]).sum::<f64>() / 3.0) };
    let key = |p: crate::csg::P3| -> [i64; 3] { p.map(|x| (x / cell).floor() as i64) };
    let mut cells: std::collections::HashMap<[i64; 3], Vec<u32>> = std::collections::HashMap::new();
    let centroids: Vec<crate::csg::P3> = if relief.skin.is_none() { solid.f.iter().map(centroid).collect() } else { Vec::new() };
    for (i, c) in centroids.iter().enumerate() {
        cells.entry(key(*c)).or_default().push(i as u32);
    }
    let base = |i: usize, p: crate::csg::P3| -> Option<crate::csg::P3> {
        let n = match relief.skin {
            Some(skin) => {
                let e = 1e-3;
                std::array::from_fn(|k| {
                    let (mut a, mut b) = (p, p);
                    a[k] += e;
                    b[k] -= e;
                    (skin(a) - skin(b)) / (2.0 * e)
                })
            }
            None => {
                let (c, at) = (centroids[i], key(centroids[i]));
                let mut sum = [0.0; 3];
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            for &j in cells.get(&[at[0] + dx, at[1] + dy, at[2] + dz]).map_or(&[][..], Vec::as_slice) {
                                let q = centroids[j as usize];
                                if (0..3).map(|k| (q[k] - c[k]).powi(2)).sum::<f64>() <= relief.radius_mm * relief.radius_mm {
                                    // The unnormalised normal is already weighted by the face's area.
                                    sum = std::array::from_fn(|k| sum[k] + normals[j as usize].0[k]);
                                }
                            }
                        }
                    }
                }
                sum
            }
        };
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        (l > 1e-12).then(|| n.map(|x| x / l))
    };
    let sin_max = relief.max_deg.clamp(0.0, 90.0).to_radians().sin();
    let mut out = ReliefSections { thinnest_mm: f64::MAX, under_mm2: 0.0, relief_mm2: 0.0, relief_thinnest_mm: f64::MAX };
    for (i, read) in reads.into_iter().enumerate() {
        let Some(r) = read else { continue };
        let area = 0.5 * normals[i].1;
        if r.section >= floor_mm {
            out.thinnest_mm = out.thinnest_mm.min(r.section);
            continue;
        }
        let exit: crate::csg::P3 = std::array::from_fn(|k| r.origin[k] + (r.section - 1e-4) * r.inward[k]);
        let is_relief = base(i, r.origin).is_some_and(|n| {
            let along = (r.inward[0] * n[0] + r.inward[1] * n[1] + r.inward[2] * n[2]).abs();
            let shallow = along <= sin_max + 1e-12;
            let near = match relief.skin {
                Some(skin) => skin(exit) > -relief.height_mm,
                None => (0..3).map(|k| (r.origin[k] - exit[k]) * n[k]).sum::<f64>() < relief.height_mm,
            };
            shallow && near
        });
        if is_relief {
            out.relief_mm2 += area;
            out.relief_thinnest_mm = out.relief_thinnest_mm.min(r.section);
        } else {
            out.thinnest_mm = out.thinnest_mm.min(r.section);
            out.under_mm2 += area;
        }
    }
    out
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
    tiling_finest_mm_remapped(t, lib, ctx, v_scale, &[])
}

/// [`tiling_finest_mm_at`] measured on the relief the layer actually lays
/// down: the shaped mask at the layer's height, through `remaps` in order
/// (the layer's own, then each enclosing group's), renormalized to its
/// remapped top. A terrace last in the chain has its treads measured too,
/// each as its own ink, since a tread is a flat between two risers that the
/// half-height threshold never sees — "treads" when one runs finest. The
/// top tread is only a peak's cap and the ground is no tread, so the
/// interior treads are the ones read. With no remap on, the measurement is
/// the plain one exactly.
pub fn tiling_finest_mm_remapped(
    t: &crate::tiling::TilingLayer,
    lib: &crate::alpha::AlphaLibrary,
    ctx: &crate::field::FieldContext,
    v_scale: f64,
    remaps: &[crate::field::Remap],
) -> Option<(f64, &'static str)> {
    use crate::alpha::Alpha;
    use crate::field::Remap;
    let alpha = lib.get(&t.alpha)?;
    // A graded tiling is judged at its small pole, the finest cell it lays.
    let (cw, ch) = t.finest_cell_size(ctx);
    let ch = ch * v_scale.clamp(0.05, 8.0);
    let (sx, sy) = (cw / alpha.width.max(1) as f64, ch / alpha.height.max(1) as f64);
    // Granulometry reads a round disc in texels, so on texels far from square
    // one axis was read at the other's pitch: the coarser axis is repeated out
    // to the finer pitch first and the disc is round in millimetres.
    let measure = |mask: &Alpha| -> Option<(f64, f64)> {
        let (ink_px, gap_px, scale) = if sx.is_finite() && sy > 0.0 && (sx / sy - 1.0).abs() > 0.05 {
            let square = stretched(mask, sx / sy);
            let (ink_px, gap_px) = square.min_feature_px()?;
            (ink_px, gap_px, (cw / square.width as f64).min(ch / square.height as f64))
        } else {
            let (ink_px, gap_px) = mask.min_feature_px()?;
            (ink_px, gap_px, sx.min(sy))
        };
        Some((ink_px * scale, gap_px * scale))
    };
    let shaped = shaped_mask(t, lib)?;
    let chain = |h: f64| remaps.iter().fold(h, |h, r| r.apply(h));
    let top_in = t.height_mm;
    let top = chain(top_in);
    // A remap only reshapes relief standing proud; an engraving, or a chain that flattens everything, reads as drawn.
    if remaps.iter().all(Remap::is_off) || !(top_in > 1e-9) || !(top > 1e-9) {
        let (ink, gap) = measure(&shaped)?;
        return Some(if ink <= gap { (ink, "strokes") } else { (gap, "gaps") });
    }
    let heights: Vec<f64> = shaped.data.iter().map(|&v| chain(v as f64 * top_in)).collect();
    let (w, h) = (shaped.width, shaped.height);
    let remapped = Alpha::new(format!("{} remapped", shaped.name), w, h, heights.iter().map(|&x| (x / top) as f32).collect());
    let (ink, gap) = measure(&remapped).unwrap_or((f64::INFINITY, f64::INFINITY));
    let mut tread = f64::INFINITY;
    if let Some(&Remap::Terrace { steps, span_mm, .. }) = remaps.last() {
        let steps = steps.clamp(1, 64);
        let q = span_mm.max(1e-6) / steps as f64;
        for i in 1..steps {
            let band = Alpha::new(format!("{} tread {i}", shaped.name), w, h, heights.iter().map(|&x| if (x / q).round() as u32 == i { 1.0 } else { 0.0 }).collect());
            if let Some((width, _)) = measure(&band) {
                tread = tread.min(width);
            }
        }
    }
    if tread < ink.min(gap) {
        return Some((tread, "treads"));
    }
    if !ink.is_finite() && !gap.is_finite() {
        return None;
    }
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
