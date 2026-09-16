//! Reversible repair candidates. Applying a candidate is the caller's undoable transaction.

use super::Setup;
use crate::{AlphaLibrary, Layer, RingDesign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Repair {
    ReduceRelief,
    MoveToSide,
    DeferToBench,
    SquareSides,
    SuggestedParting,
}
impl Repair {
    pub const ALL: [Self; 5] = [
        Self::ReduceRelief,
        Self::MoveToSide,
        Self::DeferToBench,
        Self::SquareSides,
        Self::SuggestedParting,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::ReduceRelief => "Halve layer relief",
            Self::MoveToSide => "Move layer to side faces",
            Self::DeferToBench => "Defer layer to bench",
            Self::SquareSides => "Square band sides",
            Self::SuggestedParting => "Use suggested parting plane",
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RankedRepair {
    pub repair: Repair,
    pub status: super::release::Status,
    pub obstructions: usize,
    pub blocked_area_mm2: f64,
    pub low_draft_area_mm2: f64,
    pub volume_change_pct: f64,
}
/// A deterministic baseline: compare actual candidates, then prefer fewer
/// obstructions, less locked area, and less alteration. Nothing is auto-applied.
pub fn rank(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    selected: Option<usize>,
    params: crate::BuildParams,
) -> anyhow::Result<Vec<RankedRepair>> {
    let baseline = super::inspect(d, lib, setup, params)?;
    let volume = baseline.prepared.mesh.volume_mm3();
    let mut rows = Vec::new();
    for repair in Repair::ALL {
        let Ok(next) = candidate(
            d,
            setup,
            selected,
            repair,
            baseline.release.suggested_parting_mm,
        ) else {
            continue;
        };
        let s = next.manufacturing.as_ref().unwrap_or(setup);
        let Ok(i) = super::inspect(&next, lib, s, params) else {
            continue;
        };
        rows.push(RankedRepair {
            repair,
            status: i.release.status,
            obstructions: i.release.obstructions.len(),
            blocked_area_mm2: i
                .release
                .obstructions
                .iter()
                .map(|o| o.projected_area_mm2)
                .sum(),
            low_draft_area_mm2: i.release.low_draft_area_mm2,
            volume_change_pct: (i.prepared.mesh.volume_mm3() / volume - 1.0) * 100.0,
        });
    }
    let severity = |s| match s {
        super::release::Status::Invalid => 3,
        super::release::Status::Blocked => 2,
        super::release::Status::Review => 1,
        _ => 0,
    };
    rows.sort_by(|a, b| {
        severity(a.status)
            .cmp(&severity(b.status))
            .then(a.obstructions.cmp(&b.obstructions))
            .then(a.blocked_area_mm2.total_cmp(&b.blocked_area_mm2))
            .then(
                a.volume_change_pct
                    .abs()
                    .total_cmp(&b.volume_change_pct.abs()),
            )
    });
    Ok(rows)
}

pub fn candidate(
    d: &RingDesign,
    setup: &Setup,
    selected: Option<usize>,
    repair: Repair,
    suggested: f64,
) -> anyhow::Result<RingDesign> {
    anyhow::ensure!(
        d.graph.is_none() || repair == Repair::SuggestedParting,
        "This design is driven by a graph. Make an editable copy or change its exposed inputs first."
    );
    let mut next = d.clone();
    next.manufacturing = Some(setup.clone());
    match repair {
        Repair::SquareSides => next.profile.flatten_sides(),
        Repair::SuggestedParting => {
            let s = next.manufacturing.as_mut().unwrap();
            s.auto_parting = false;
            s.parting_mm = suggested;
        }
        _ => {
            let ctx = next.field_context();
            let e = next
                .layers
                .layers
                .get_mut(selected.ok_or_else(|| anyhow::anyhow!("Select a layer to repair"))?)
                .ok_or_else(|| anyhow::anyhow!("Selected layer no longer exists"))?;
            anyhow::ensure!(e.enabled, "Selected layer is disabled");
            match repair {
                Repair::ReduceRelief => e.opacity *= 0.5,
                Repair::DeferToBench => e.bench_only = true,
                Repair::MoveToSide => match &mut e.layer {
                    Layer::Tiling(t) => anyhow::ensure!(
                        t.fit_to_side_faces(&ctx, crate::field::SIDE_FACE_MIN_DRAFT_DEG),
                        "The profile has no suitable side face"
                    ),
                    Layer::Curve(c) => {
                        let faces = ctx.side_faces_std().ok_or_else(|| {
                            anyhow::anyhow!("The profile has no suitable side face")
                        })?;
                        let (lo, hi) = faces.wider().ok_or_else(|| {
                            anyhow::anyhow!("The profile has no suitable side face")
                        })?;
                        anyhow::ensure!(
                            hi - lo > c.width_mm,
                            "The side face is narrower than the wire"
                        );
                        let old_lo = c.points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                        let old_hi = c
                            .points
                            .iter()
                            .map(|p| p[1])
                            .fold(f64::NEG_INFINITY, f64::max);
                        for p in &mut c.points {
                            p[1] = lo
                                + c.width_mm * 0.5
                                + (p[1] - old_lo) / (old_hi - old_lo).max(1e-9)
                                    * (hi - lo - c.width_mm);
                        }
                        c.mirror_v = false;
                    }
                    _ => anyhow::bail!(
                        "Side placement is available for tiled ornament and curve wires; use the layer editor for this feature"
                    ),
                },
                _ => unreachable!(),
            }
        }
    }
    Ok(next)
}

/// Approximate ownership, useful for selecting a candidate, not proof of causation.
pub fn layer_at(d: &RingDesign, lib: &AlphaLibrary, world: [f64; 3]) -> Option<usize> {
    let theta = world[1].atan2(world[0]).to_degrees().rem_euclid(360.0);
    let ctx = d.field_context();
    let r = world[0].hypot(world[1]);
    let section = crate::castability::section_at(d, lib, theta, 192);
    let points: Vec<_> = section.points.iter().filter(|p| p.surface).collect();
    let total = points
        .windows(2)
        .map(|w| (w[1].r - w[0].r).hypot(w[1].z - w[0].z))
        .sum::<f64>();
    let (mut acc, mut best, mut at) = (0.0, f64::INFINITY, 0.0);
    for w in points.windows(2) {
        acc += (w[1].r - w[0].r).hypot(w[1].z - w[0].z);
        let dist = (w[1].r - r).hypot(w[1].z - world[2]);
        if dist < best {
            best = dist;
            at = acc;
        }
    }
    let uv = crate::Uv {
        u: ctx.u_of_theta(theta),
        v: at / total.max(1e-9) * ctx.band_v_len_mm,
    };
    d.layers
        .layers
        .iter()
        .enumerate()
        .rev()
        .find(|(_, e)| {
            e.enabled
                && !e.bench_only
                && e.layer.height(uv, &ctx, lib).abs() * e.opacity * e.mask_at(uv, &ctx, lib)
                    > 0.002
        })
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_leave_the_original_and_graph_provenance_alone() {
        let mut d = RingDesign::default();
        d.layers.layers.push(crate::field::LayerEntry::new(
            "Wire",
            Layer::Curve(Default::default()),
        ));
        let before = serde_json::to_string(&d).unwrap();
        let next = candidate(&d, &Setup::default(), Some(0), Repair::DeferToBench, 0.0).unwrap();
        assert!(next.layers.layers[0].bench_only);
        assert_eq!(serde_json::to_string(&d).unwrap(), before);
        d.graph = Some(serde_json::json!({}));
        assert!(candidate(&d, &Setup::default(), Some(0), Repair::ReduceRelief, 0.0).is_err());
    }
}
