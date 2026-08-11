//! Auto-pavé: pack a region with seats, as editable layers.
//!
//! A generator, not a primitive: the output is an ordinary [`Layer::Group`]
//! full of gypsy [`SeatPadLayer`]s, so every seat can be nudged, deleted or
//! restyled afterwards and the stones report reads them like anything else.
//! Rows wrap in `u` by construction — a full-ring row gets an integer count —
//! and stagger by half a pitch, which is the hexagonal packing pavé means.
//!
//! Gypsy mounds because that is the measured-safe row on curved ground
//! (flat-boss rows undercut at their rims at 8.6%; mounds at 0.000%), and
//! the bench drills the seats into the mounds either way.

use crate::field::{
    Blend, BorderLayer, BorderProfile, GroupLayer, Layer, LayerEntry, LayerStack, SeatPadLayer,
    SeatStyle, SideFacePick, VGate,
};
use crate::gem::Gem;
use crate::RingDesign;

/// Hard cap on generated seats: past this a flask is a bead blanket, and the
/// report and editor stop being legible.
pub const MAX_SEATS: usize = 240;

/// Where across the band the fill goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaveRegion {
    /// A band of `v`, centred at `center_mm`, `width_mm` across.
    VBand { center_mm: f64, width_mm: f64 },
    /// A side-face run, resolved from the profile at generation time.
    SideFace(SideFacePick),
}

#[derive(Clone, Debug)]
pub struct PaveSpec {
    pub gem: Gem,
    /// Metal left between neighbouring girdles, mm.
    pub bridge_mm: f64,
    /// Arc filled, degrees. A span of 360 or more fills the whole ring with
    /// wrap-exact rows.
    pub theta_deg: f64,
    pub span_deg: f64,
    pub region: PaveRegion,
    /// Offset alternate rows by half a pitch — hexagonal packing.
    pub stagger: bool,
    pub style: SeatStyle,
}

impl Default for PaveSpec {
    fn default() -> Self {
        Self {
            gem: Gem::calibrated(crate::gem::GemCut::Round, 1.5),
            bridge_mm: 0.4,
            theta_deg: crate::profile::TOP_DEG,
            span_deg: 360.0,
            region: PaveRegion::SideFace(SideFacePick::Wider),
            stagger: true,
            style: SeatStyle::GypsyMound,
        }
    }
}

/// What [`fill`] produced, beyond the layer itself.
#[derive(Clone, Debug)]
pub struct PaveOutcome {
    pub seats: usize,
    pub rows: usize,
    /// Seats the cap or the region refused, if any — said, not silent.
    pub note: Option<String>,
}

/// Pack the region and return the group entry plus the tally.
///
/// `None` when the region resolves to nothing to fill — a side-face pick on
/// an all-dome profile, a zero span, a stone wider than the region.
pub fn fill(design: &RingDesign, spec: &PaveSpec) -> Option<(LayerEntry, PaveOutcome)> {
    let ctx = design.field_context();
    // The seat the stone needs, once — every seat in the fill is identical.
    let mut proto = SeatPadLayer {
        style: spec.style,
        height_mm: 0.6,
        crown: 1.0,
        blend_mm: 0.4,
        ..Default::default()
    };
    proto.fit_stone(spec.gem);
    let pitch = proto.diameter_mm + spec.bridge_mm.max(0.0);
    if !(pitch > 0.2) {
        return None;
    }

    let (v_lo, v_hi) = match spec.region {
        PaveRegion::VBand { center_mm, width_mm } => {
            (center_mm - width_mm * 0.5, center_mm + width_mm * 0.5)
        }
        PaveRegion::SideFace(pick) => {
            let sf = ctx.side_faces_std()?;
            match pick {
                SideFacePick::Low => sf.low?,
                SideFacePick::High => sf.high?,
                SideFacePick::Wider | SideFacePick::Both => sf.wider()?,
            }
        }
    };
    let v_lo = v_lo.max(0.0);
    let v_hi = v_hi.min(ctx.band_v_len_mm);
    if v_hi - v_lo < proto.diameter_mm {
        return None;
    }

    // Rows across the band at hex spacing, centred in the region.
    let row_gap = if spec.stagger { pitch * 0.866 } else { pitch };
    let usable = v_hi - v_lo - proto.diameter_mm;
    let rows = (usable / row_gap).floor() as usize + 1;
    let v0 = v_lo + proto.diameter_mm * 0.5 + (usable - (rows - 1) as f64 * row_gap) * 0.5;

    let full = spec.span_deg >= 360.0 - 1e-9;
    let mut seats: Vec<SeatPadLayer> = Vec::new();
    let mut refused = 0usize;
    for r in 0..rows {
        let v = v0 + r as f64 * row_gap;
        let stagger = spec.stagger && r % 2 == 1;
        if full {
            // Wrap-exact: an integer count around the ring, alternate rows
            // rotated half a step.
            let n = (ctx.circumference_mm / pitch).floor() as usize;
            if n < 3 {
                continue;
            }
            let step = 360.0 / n as f64;
            for k in 0..n {
                let theta = k as f64 * step + if stagger { step * 0.5 } else { 0.0 };
                push_seat(&mut seats, &proto, theta, v, &mut refused);
            }
        } else {
            // A centred run along the arc. Arc length is measured at the
            // crest radius, same as `u`.
            let arc_mm = spec.span_deg.to_radians() * ctx.crest_radius_mm;
            let n = (arc_mm / pitch).floor() as usize;
            if n == 0 {
                continue;
            }
            let step_deg = pitch / ctx.crest_radius_mm.max(1e-9) * 180.0 / std::f64::consts::PI;
            let offset = if stagger { 0.5 } else { 0.0 };
            let base = spec.theta_deg - (n as f64 - 1.0 + 2.0 * offset) * 0.5 * step_deg;
            for k in 0..n {
                let theta = base + (k as f64 + offset) * step_deg;
                push_seat(&mut seats, &proto, theta, v, &mut refused);
            }
        }
    }
    if seats.is_empty() {
        return None;
    }

    let note = (refused > 0)
        .then(|| format!("{refused} seats past the {MAX_SEATS}-seat cap were dropped"));
    let outcome = PaveOutcome { seats: seats.len(), rows, note };

    let mut stack = LayerStack::default();
    for (i, s) in seats.into_iter().enumerate() {
        stack
            .layers
            .push(LayerEntry::new(format!("Seat {}", i + 1), Layer::SeatPad(s)));
    }
    let entry = LayerEntry::new(
        format!("Pavé {} ({})", spec.gem.display(), outcome.seats),
        Layer::Group(GroupLayer { stack }),
    );
    Some((entry, outcome))
}

/// Channel-set stock: two rails flanking a recessed channel, as one group
/// gated to the wider side face.
///
/// The recess's walls stand on a face parallel to the mould pull, which is
/// the one place a channel is castable — on the crown they lean back over
/// the sand. Hence `None` when the profile has no side face, or the face is
/// too narrow for the stone plus its rails. The bench cuts the seats into
/// the channel's rails; the ring casts the stock.
pub fn channel_set(design: &RingDesign, gem: Gem, recess_mm: f64) -> Option<LayerEntry> {
    let ctx = design.field_context();
    let (lo, hi) = ctx.side_faces_std()?.wider()?;
    let stone = gem.w_mm.max(0.8);
    let rail_w = (stone * 0.45).clamp(0.5, 1.2);
    if hi - lo < stone + 2.0 * rail_w {
        return None;
    }
    let vc = 0.5 * (lo + hi);
    let offset = 0.5 * (stone + rail_w);
    let recess = recess_mm.clamp(0.1, 1.0);
    let rail = |v_mm: f64| BorderLayer {
        v_mm,
        width_mm: rail_w,
        height_mm: 0.3,
        profile: BorderProfile::Round,
        mirror: false,
        rope_twists: 0,
    };
    let mut stack = LayerStack::default();
    stack.layers.push(LayerEntry::new("Low rail", Layer::Border(rail(vc - offset))));
    stack.layers.push(LayerEntry::new("High rail", Layer::Border(rail(vc + offset))));
    let mut channel = LayerEntry::new(
        "Channel",
        Layer::Border(BorderLayer {
            v_mm: vc,
            width_mm: stone,
            height_mm: recess,
            profile: BorderProfile::Flat,
            mirror: false,
            rope_twists: 0,
        }),
    );
    channel.blend = Blend::Subtract;
    stack.layers.push(channel);

    let mut entry = LayerEntry::new(
        format!("Channel set {}", gem.display()),
        Layer::Group(GroupLayer { stack }),
    );
    entry.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
    Some(entry)
}

fn push_seat(
    seats: &mut Vec<SeatPadLayer>,
    proto: &SeatPadLayer,
    theta_deg: f64,
    v_mm: f64,
    refused: &mut usize,
) {
    if seats.len() >= MAX_SEATS {
        *refused += 1;
        return;
    }
    let mut s = *proto;
    s.theta_deg = theta_deg.rem_euclid(360.0);
    s.v_mm = v_mm;
    seats.push(s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProfileStyle;

    fn flat_design() -> RingDesign {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::Flat);
        d.profile.width_mm = 8.0;
        d
    }

    #[test]
    fn a_full_ring_fill_wraps_exactly_and_stays_editable() {
        let d = flat_design();
        let spec = PaveSpec {
            region: PaveRegion::VBand {
                center_mm: d.field_context().crest_v_mm,
                width_mm: 5.0,
            },
            ..Default::default()
        };
        let (entry, out) = fill(&d, &spec).expect("fill");
        assert!(out.seats >= 6, "{out:?}");
        assert!(out.note.is_none(), "{out:?}");
        let Layer::Group(g) = &entry.layer else { panic!("not a group") };
        assert_eq!(g.stack.layers.len(), out.seats);
        // Wrap-exact: every row's thetas are an integer division of 360.
        let mut by_v: std::collections::BTreeMap<i64, Vec<f64>> = Default::default();
        for e in &g.stack.layers {
            let Layer::SeatPad(s) = &e.layer else { panic!() };
            by_v.entry((s.v_mm * 100.0).round() as i64).or_default().push(s.theta_deg);
        }
        assert_eq!(by_v.len(), out.rows);
        for (_, thetas) in by_v {
            let n = thetas.len() as f64;
            let step = 360.0 / n;
            let mut sorted = thetas.clone();
            sorted.sort_by(f64::total_cmp);
            for w in sorted.windows(2) {
                assert!((w[1] - w[0] - step).abs() < 1e-6, "uneven step");
            }
        }
    }

    #[test]
    fn an_arc_fill_stays_inside_its_span_and_the_cap_reports_itself() {
        let d = flat_design();
        let spec = PaveSpec {
            span_deg: 90.0,
            region: PaveRegion::VBand {
                center_mm: d.field_context().crest_v_mm,
                width_mm: 5.0,
            },
            ..Default::default()
        };
        let (entry, out) = fill(&d, &spec).unwrap();
        let Layer::Group(g) = &entry.layer else { panic!() };
        for e in &g.stack.layers {
            let Layer::SeatPad(s) = &e.layer else { panic!() };
            let off = crate::field::wrap_delta(s.theta_deg - spec.theta_deg, 360.0).abs();
            assert!(off <= 46.0, "seat at {:.1} outside the 90 deg span", s.theta_deg);
        }
        assert!(out.seats > 3);

        // A melee blanket over an absurdly wide band hits the cap and says so.
        let mut big = flat_design();
        big.profile.width_mm = 20.0;
        let mut wide = PaveSpec::default();
        wide.gem = Gem::calibrated(crate::gem::GemCut::Round, 1.0);
        wide.bridge_mm = 0.1;
        wide.style = SeatStyle::Boss;
        wide.region = PaveRegion::VBand {
            center_mm: big.field_context().crest_v_mm,
            width_mm: 19.0,
        };
        let (_, out) = fill(&big, &wide).unwrap();
        assert_eq!(out.seats, MAX_SEATS);
        assert!(out.note.is_some());
    }

    #[test]
    fn a_side_face_pick_on_an_all_dome_profile_refuses() {
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::HalfRound);
        let spec = PaveSpec::default();
        assert!(fill(&d, &spec).is_none());
    }

    #[test]
    fn a_channel_set_casts_on_a_side_face_and_refuses_a_dome() {
        use crate::castability::{self, Verdict};
        // A channel needs stone plus two rails of side face, so it is a
        // thick-band feature: 1.5 mm stone + 0.675 rails wants ~2.9 mm.
        let mut d = flat_design();
        d.profile.thickness_mm = 4.0;
        d.profile.flatten_sides();
        let gem = Gem::calibrated(crate::gem::GemCut::Round, 1.5);
        let entry = channel_set(&d, gem, 0.6).expect("thick squared band has a side face");
        let Layer::Group(g) = &entry.layer else { panic!("not a group") };
        assert_eq!(g.stack.layers.len(), 3);
        d.layers.layers.push(entry);
        let lib = crate::alpha::AlphaLibrary::builtin();
        let field = castability::analyze_field(&d, &lib, &d.draft, 160, 96);
        assert_ne!(field.verdict, Verdict::NotCastable, "{:?}", field.notes);

        let mut dome = RingDesign::default();
        dome.profile.apply_style(ProfileStyle::HalfRound);
        assert!(channel_set(&dome, gem, 0.6).is_none());
    }
}
