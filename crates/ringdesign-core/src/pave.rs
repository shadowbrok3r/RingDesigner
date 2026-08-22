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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PaveRegion {
    /// A band of `v`, centred at `center_mm`, `width_mm` across.
    VBand { center_mm: f64, width_mm: f64 },
    /// A side-face run, resolved from the profile at generation time.
    SideFace(SideFacePick),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// Turn every seat about its own normal, degrees. 0 lays an elongated
    /// stone along the ring; 90 stands it across the band.
    #[serde(default)]
    pub rot_deg: f64,
    /// Seats the user owns, and the room the packer must leave them.
    #[serde(default)]
    pub pinned: Vec<PinnedSeat>,
}

/// A place in the fill the packer does not decide.
///
/// A pin and a deletion are the same thing said twice — *the packer may not
/// place here* — so they are one mechanism, and a pin merely also emits a
/// seat. Stated as a **region**, never as the identity of a station: a
/// generated station is recomputed from scratch every time the band moves,
/// so matching a stored one by position would quietly cull a neighbour
/// instead the moment the pitch changed.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PinnedSeat {
    /// Position around the ring, degrees. 90 is the top.
    pub theta_deg: f64,
    /// Position across the band, mm of `v`.
    pub v_mm: f64,
    /// The seat to place here. `None` is a hole: the packer keeps clear and
    /// nothing is placed.
    pub seat: Option<SeatPadLayer>,
    /// How much room the packer must leave, mm. 0 takes the seat's own
    /// reach, which is what a pinned seat wants; a hole needs its own.
    #[serde(default)]
    pub clear_mm: f64,
}

impl PinnedSeat {
    /// The room this pin claims, mm — along the ring, then across the band.
    fn reach_mm(&self) -> (f64, f64) {
        match (&self.seat, self.clear_mm) {
            (_, c) if c > 0.0 => (c, c),
            (Some(s), _) => s.half_extents_mm(),
            (None, _) => (0.0, 0.0),
        }
    }
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
            rot_deg: 0.0,
            pinned: Vec::new(),
        }
    }
}

/// What [`fill`] produced, beyond the layer itself.
#[derive(Clone, Debug)]
pub struct PaveOutcome {
    pub seats: usize,
    pub rows: usize,
    /// How many of the seats are the user's own rather than the packer's.
    pub pinned: usize,
    /// Seats the cap, the region or a pin refused — said, not silent.
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
    proto.rot_deg = spec.rot_deg;
    // Pack by the seat's own reach: an elongated stone spends its length
    // along the ring and its width across it, so the two axes pitch apart.
    let (span_u, span_v) = {
        let (hu, hv) = proto.half_extents_mm();
        (hu * 2.0, hv * 2.0)
    };
    let pitch = span_u + spec.bridge_mm.max(0.0);
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
    if v_hi - v_lo < span_v {
        return None;
    }

    // Rows across the band at hex spacing, centred in the region.
    let v_pitch = span_v + spec.bridge_mm.max(0.0);
    let row_gap = if spec.stagger { v_pitch * 0.866 } else { v_pitch };
    let usable = v_hi - v_lo - span_v;
    let rows = (usable / row_gap).floor() as usize + 1;
    let v0 = v_lo + span_v * 0.5 + (usable - (rows - 1) as f64 * row_gap) * 0.5;

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
    // The user's own seats and holes claim their ground; the packer yields.
    let before = seats.len();
    seats.retain(|s| !blocked(spec, &ctx, s));
    let yielded = before - seats.len();
    let mut pinned = 0usize;
    for p in &spec.pinned {
        if let Some(seat) = &p.seat {
            push_seat(&mut seats, seat, p.theta_deg, p.v_mm, &mut refused);
            pinned += 1;
        }
    }
    if seats.is_empty() {
        return None;
    }

    let mut notes = Vec::new();
    if refused > 0 {
        notes.push(format!("{refused} seats past the {MAX_SEATS}-seat cap were dropped"));
    }
    if yielded > 0 {
        notes.push(format!("{yielded} yielded to {} pinned", spec.pinned.len()));
    }
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    let outcome = PaveOutcome { seats: seats.len(), rows, pinned, note };

    let mut stack = LayerStack::default();
    // Pins keep their own numbering so the layer list does not renumber them
    // when the packer re-solves around them.
    let generated = seats.len() - pinned;
    for (i, s) in seats.into_iter().enumerate() {
        let name = if i < generated {
            format!("Seat {}", i + 1)
        } else {
            format!("Pinned {}", i - generated + 1)
        };
        stack.layers.push(LayerEntry::new(name, Layer::SeatPad(s)));
    }
    let entry = LayerEntry::new(
        format!("Pavé {} ({})", spec.gem.display(), outcome.seats),
        Layer::Group(GroupLayer { stack, recipe: Some(GenRecipe::Pave(spec.clone())) }),
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
        Layer::Group(GroupLayer {
            stack,
            recipe: Some(GenRecipe::Channel(ChannelSpec { gem, recess_mm: recess })),
        }),
    );
    entry.window.v_gate = VGate::SideFaces(SideFacePick::Wider);
    Some(entry)
}

/// A halo: a centre stone on a domed plate, ringed by bead-set accents.
///
/// The construction is what a cast halo actually is, and what the field
/// verdict forces. A ring of *proud* accent mounds does not cast: each melee
/// mound sits off the crest and forms a two-flange valley with the centre —
/// measured 1.4% undercut at −33° — the same wall a signet's cleft or a
/// two-flange shank raises. So the halo casts as a **clean gypsy plate**
/// (one gentle dome, 0.000%) carrying the centre seat, and the accent ring
/// rides the plate as **bench-set markers** (zero-height seats): the report
/// counts them, the gem preview stands each stone on the plate surface, and
/// the setter drills and beads them into the cast dome — which is how fine
/// halo melee is set regardless of process.
///
/// The one proud stone is the centre, on the crest, where a mound straddles
/// the parting plane and releases.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HaloSpec {
    pub center: Gem,
    pub accent: Gem,
    pub theta_deg: f64,
    /// Across the band; `None` sits it on the crest, the one place a proud
    /// centre mound releases.
    pub v_mm: Option<f64>,
    /// Metal between the centre girdle and the accent ring, mm.
    pub gap_mm: f64,
    /// Metal between neighbouring accents, mm.
    pub bridge_mm: f64,
    /// Accents in the ring; 0 solves the count from the halo outline.
    pub count: u32,
    /// Turn the centre stone and its halo about the seat normal, degrees.
    /// 0 lays an elongated centre along the ring.
    #[serde(default)]
    pub rot_deg: f64,
}

/// The halo outline, sampled so accents land at equal arc length round it.
/// A circle when the centre stone is round, which is the common case and
/// costs the same.
struct HaloRing {
    /// Cumulative arc length at each sample, first 0 and last the perimeter.
    arc: Vec<f64>,
    pts: Vec<(f64, f64)>,
    perimeter_mm: f64,
}

impl HaloRing {
    const STEPS: usize = 256;

    fn new(ra: f64, rb: f64, rot_deg: f64) -> Self {
        let (sin, cos) = rot_deg.to_radians().sin_cos();
        let pts: Vec<(f64, f64)> = (0..=Self::STEPS)
            .map(|i| {
                let t = i as f64 / Self::STEPS as f64 * std::f64::consts::TAU;
                let (a, b) = (ra * t.cos(), rb * t.sin());
                (a * cos - b * sin, a * sin + b * cos)
            })
            .collect();
        let mut arc = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        arc.push(0.0);
        for w in pts.windows(2) {
            acc += ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
            arc.push(acc);
        }
        Self { arc, pts, perimeter_mm: acc }
    }

    /// The point at `frac` of the way round by arc length.
    fn at(&self, frac: f64) -> (f64, f64) {
        let target = frac.rem_euclid(1.0) * self.perimeter_mm;
        let i = self.arc.partition_point(|&a| a < target).clamp(1, self.pts.len() - 1);
        let (a0, a1) = (self.arc[i - 1], self.arc[i]);
        let t = if a1 > a0 { (target - a0) / (a1 - a0) } else { 0.0 };
        let (p0, p1) = (self.pts[i - 1], self.pts[i]);
        (p0.0 + (p1.0 - p0.0) * t, p0.1 + (p1.1 - p0.1) * t)
    }
}

impl Default for HaloSpec {
    fn default() -> Self {
        Self {
            center: Gem::calibrated(crate::gem::GemCut::Round, 5.0),
            accent: Gem::calibrated(crate::gem::GemCut::Round, 1.1),
            theta_deg: crate::profile::TOP_DEG,
            v_mm: None,
            gap_mm: 0.3,
            bridge_mm: 0.25,
            count: 0,
            rot_deg: 0.0,
        }
    }
}

/// Build the halo group. `None` when the plate will not fit across the band.
///
/// Under [`CastProcess::LostWax`](crate::castability::CastProcess) the halo
/// takes its classic form instead — the centre mound ringed by *proud*
/// accent mounds, no plate — because the investment burns out of the
/// two-flange valleys a sand pull cannot clear.
pub fn halo(design: &RingDesign, spec: &HaloSpec) -> Option<(LayerEntry, u32)> {
    let ctx = design.field_context();
    let lost_wax =
        design.draft.process == crate::castability::CastProcess::LostWax;
    let v_center = spec.v_mm.unwrap_or(ctx.crest_v_mm);

    let mut center = SeatPadLayer {
        style: SeatStyle::GypsyMound,
        height_mm: 0.9,
        crown: 1.0,
        blend_mm: 2.0,
        ..Default::default()
    };
    center.fit_stone(spec.center);
    center.rot_deg = spec.rot_deg;
    // Melee footprint for the accent markers: a tight ring the bench drills.
    let acc_dia = spec.accent.w_mm.max(0.5) + 0.7;

    // The halo follows the centre's own outline, so an oval stone gets an
    // oval halo rather than a circle drawn round its length.
    let (ca, cb) = center.semi_axes_mm();
    let grow = spec.gap_mm.max(0.0) + acc_dia * 0.5;
    let (ha, hb) = (ca + grow, cb + grow);
    if hb <= 1e-3 {
        return None;
    }
    // The domed plate carries the whole cluster; it must fit the band with a
    // gentle skirt to the crown. The lost-wax form has no plate, so only the
    // accent ring itself must fit.
    let (pa, pb) = (ha + acc_dia * 0.5 + 0.8, hb + acc_dia * 0.5 + 0.8);
    let plate = SeatPadLayer {
        theta_deg: spec.theta_deg.rem_euclid(360.0),
        v_mm: v_center,
        diameter_mm: pb * 2.0,
        elong: pa / pb,
        rot_deg: spec.rot_deg,
        height_mm: 0.6,
        crown: 1.0,
        blend_mm: pb * 0.6,
        style: SeatStyle::GypsyMound,
        ..Default::default()
    };
    let reach = if lost_wax {
        crate::field::plan_half_extents_mm(ha, hb, 2.0, spec.rot_deg).1 + acc_dia * 0.5 + 0.3
    } else {
        plate.half_extents_mm().1
    };
    if v_center - reach < 0.0 || v_center + reach > ctx.band_v_len_mm {
        return None;
    }

    let ring = HaloRing::new(ha, hb, spec.rot_deg);
    let pitch = acc_dia + spec.bridge_mm.max(0.0);
    let n = if spec.count >= 3 {
        spec.count
    } else {
        ((ring.perimeter_mm / pitch).floor() as u32).max(6)
    };

    let mut stack = LayerStack::default();
    if !lost_wax {
        // The plate: one gentle dome, the clean stock the melee is cut into.
        stack.layers.push(LayerEntry::new("Plate", Layer::SeatPad(plate)));
    }

    let mut cs = center;
    cs.theta_deg = spec.theta_deg.rem_euclid(360.0);
    cs.v_mm = v_center;
    // The centre rides proud of the plate; its own skirt fairs into the dome.
    cs.height_mm = 0.9;
    stack.layers.push(LayerEntry::new("Centre", Layer::SeatPad(cs)));

    let crest_r = ctx.crest_radius_mm.max(1e-6);
    for k in 0..n {
        // Equally spaced by arc length round the halo, so the bridges
        // between accents are even on an oval as they are on a round.
        let (off_u, off_v) = ring.at(k as f64 / n as f64);
        // The halo is a few mm across, so its own outline is locally flat in
        // (arc-u, v): the u offset becomes an angle at the crest radius.
        let dtheta = off_u / crest_r * 180.0 / std::f64::consts::PI;
        // Sand: a zero-height marker — it carries the stone for the report
        // and the preview but raises no proud geometry, because a proud
        // accent ring is the undercut; the plate is the stock and the bench
        // cuts the seat. Lost wax: the classic proud melee mound.
        let s = SeatPadLayer {
            theta_deg: (spec.theta_deg + dtheta).rem_euclid(360.0),
            v_mm: v_center + off_v,
            diameter_mm: acc_dia,
            height_mm: if lost_wax { 0.5 } else { 0.0 },
            crown: 1.0,
            blend_mm: if lost_wax { 0.3 } else { 0.2 },
            style: SeatStyle::GypsyMound,
            gem: Some(spec.accent),
            ..Default::default()
        };
        stack.layers.push(LayerEntry::new(format!("Accent {}", k + 1), Layer::SeatPad(s)));
    }

    let entry = LayerEntry::new(
        format!("Halo {} + {}x {}", spec.center.display(), n, spec.accent.display()),
        Layer::Group(GroupLayer { stack, recipe: Some(GenRecipe::Halo(spec.clone())) }),
    );
    Some((entry, n))
}

/// Channel-set parameters, so a channel group can stay live like the rest.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChannelSpec {
    pub gem: Gem,
    pub recess_mm: f64,
}

/// The generator a live [`GroupLayer`] was made by — the CrossGems lesson,
/// in this model's idiom: a group is either *live* (this recipe owns its
/// stack, and editors re-run it when the recipe or the band changes) or
/// *baked* (recipe removed, layers hand-owned). Builds never regenerate;
/// the stored stack is what a file means.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GenRecipe {
    Pave(PaveSpec),
    Halo(HaloSpec),
    Channel(ChannelSpec),
}

impl GenRecipe {
    pub fn kind_label(&self) -> &'static str {
        match self {
            GenRecipe::Pave(_) => "Pavé",
            GenRecipe::Halo(_) => "Halo",
            GenRecipe::Channel(_) => "Channel set",
        }
    }

    /// Run the generator this recipe names against the current band.
    pub fn generate(&self, design: &RingDesign) -> Option<LayerEntry> {
        match self {
            GenRecipe::Pave(spec) => fill(design, spec).map(|(e, _)| e),
            GenRecipe::Halo(spec) => halo(design, spec).map(|(e, _)| e),
            GenRecipe::Channel(spec) => channel_set(design, spec.gem, spec.recess_mm),
        }
    }
}

/// Re-run every live generator group against the current band, replacing
/// each one's stack and generated name in place. The entry's own window,
/// blend, mask and opacity are the user's and stay put. A recipe that no
/// longer fits keeps its old stack and says so in the returned notes —
/// non-destructive, like every refusal in this app.
///
/// Editors call this after any change that moves the ground under a
/// generator: the recipe itself, the profile, the shank, the process.
/// Builds and analysis never do — a design file renders as saved.
pub fn regenerate_live(design: &mut RingDesign) -> Vec<String> {
    let live: Vec<(usize, GenRecipe)> = design
        .layers
        .layers
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match &e.layer {
            Layer::Group(g) => g.recipe.clone().map(|r| (i, r)),
            _ => None,
        })
        .collect();
    let mut notes = Vec::new();
    for (i, recipe) in live {
        match recipe.generate(design) {
            Some(fresh) => {
                let Layer::Group(fresh_group) = fresh.layer else { continue };
                let name = fresh.name;
                if let Some(entry) = design.layers.layers.get_mut(i) {
                    if let Layer::Group(g) = &mut entry.layer {
                        g.stack = fresh_group.stack;
                        entry.name = name;
                    }
                }
            }
            None => {
                if let Some(entry) = design.layers.layers.get(i) {
                    notes.push(format!(
                        "{} no longer fits this band — kept as generated",
                        entry.name
                    ));
                }
            }
        }
    }
    notes
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

/// Whether a pin claims this station. Measured in the chart, ellipse against
/// ellipse, with the spec's own bridge between them — the same metal the
/// packer leaves between two of its own seats.
fn blocked(spec: &PaveSpec, ctx: &crate::field::FieldContext, seat: &SeatPadLayer) -> bool {
    if spec.pinned.is_empty() {
        return false;
    }
    let (hu, hv) = seat.half_extents_mm();
    let bridge = spec.bridge_mm.max(0.0);
    spec.pinned.iter().any(|p| {
        let (pu, pv) = p.reach_mm();
        let (ru, rv) = (hu + pu + bridge, hv + pv + bridge);
        if !(ru > 0.0 && rv > 0.0) {
            return false;
        }
        let du = crate::field::wrap_delta(seat.theta_deg - p.theta_deg, 360.0).to_radians()
            * ctx.crest_radius_mm;
        let dv = seat.v_mm - p.v_mm;
        (du / ru).powi(2) + (dv / rv).powi(2) < 1.0
    })
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
    fn a_halo_casts_on_the_crown_and_stays_editable() {
        use crate::castability::{self, Verdict};
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::LowDome);
        d.profile.width_mm = 9.0;
        d.profile.thickness_mm = 2.4;
        let spec = HaloSpec {
            center: Gem::calibrated(crate::gem::GemCut::Round, 4.0),
            accent: Gem::calibrated(crate::gem::GemCut::Round, 1.0),
            ..Default::default()
        };
        let (entry, n) = halo(&d, &spec).expect("halo fits a 9 mm band");
        assert!(n >= 6, "solved {n} accents");
        let Layer::Group(g) = &entry.layer else { panic!("not a group") };
        assert_eq!(g.stack.layers.len(), n as usize + 2, "plate, centre, then the ring");
        // The accents are bench-set markers: they carry a stone but raise no
        // proud geometry, because a proud accent ring is the undercut.
        for e in g.stack.layers.iter().filter(|e| e.name.starts_with("Accent")) {
            let Layer::SeatPad(s) = &e.layer else { panic!() };
            assert_eq!(s.height_mm, 0.0, "accent must be a flat marker");
            assert!(s.gem.is_some(), "marker still carries its stone");
        }

        d.layers.layers.push(entry);
        let lib = crate::alpha::AlphaLibrary::builtin();
        let field = castability::analyze_field(&d, &lib, &d.draft, 220, 128);
        assert_ne!(
            field.verdict,
            Verdict::NotCastable,
            "halo of mounds must release on the crown: {:.3}% at {:.1} deg, {:?}",
            field.undercut_fraction() * 100.0,
            field.worst_draft_deg,
            field.notes,
        );

        // A halo too wide for the band refuses rather than running off the edge.
        let mut narrow = RingDesign::default();
        narrow.profile.apply_style(ProfileStyle::LowDome);
        narrow.profile.width_mm = 3.0;
        assert!(halo(&narrow, &spec).is_none());
    }

    /// The live-group lifecycle: a generated group carries its recipe, the
    /// regenerate pass re-solves it when the band changes, a recipe that no
    /// longer fits refuses non-destructively, baking detaches it, and the
    /// recipe survives the file round-trip.
    /// A pin is the user's ground, and the packer yields to it. The whole
    /// point is that a hand edit inside a live group used to be destroyed by
    /// the next regeneration — the panel offered a seat editor and a delete
    /// button whose every change `g.stack = fresh.stack` wiped on the same
    /// frame.
    #[test]
    fn pinned_seats_survive_regeneration_and_the_packer_yields_to_them() {
        let mut d = flat_design();
        let crest = d.field_context().crest_v_mm;
        let base = PaveSpec {
            gem: Gem::calibrated(crate::gem::GemCut::Round, 1.2),
            region: PaveRegion::VBand { center_mm: crest, width_mm: 5.0 },
            stagger: false,
            ..Default::default()
        };
        let (entry, plain) = fill(&d, &base).expect("fits");
        let Layer::Group(g) = &entry.layer else { panic!() };
        let step = {
            // The lattice the packer laid down, for the seamlessness check.
            let mut ts: Vec<f64> = g
                .stack
                .layers
                .iter()
                .filter_map(|e| match &e.layer {
                    Layer::SeatPad(s) if (s.v_mm - crest).abs() < 1e-9 => Some(s.theta_deg),
                    _ => None,
                })
                .collect();
            ts.sort_by(f64::total_cmp);
            assert!(ts.len() > 8, "a real row: {}", ts.len());
            360.0 / ts.len() as f64
        };

        // One pin deliberately off the lattice, and one hole.
        let mut held = SeatPadLayer {
            style: SeatStyle::GypsyMound,
            height_mm: 0.6,
            crown: 1.0,
            blend_mm: 0.4,
            ..Default::default()
        };
        held.fit_stone(Gem::calibrated(crate::gem::GemCut::Round, 2.0));
        let pin_theta = 0.37 * step;
        let hole_theta = 180.0;
        let spec = PaveSpec {
            pinned: vec![
                PinnedSeat {
                    theta_deg: pin_theta,
                    v_mm: crest,
                    seat: Some(held),
                    clear_mm: 0.0,
                },
                PinnedSeat {
                    theta_deg: hole_theta,
                    v_mm: crest,
                    seat: None,
                    clear_mm: 2.0,
                },
            ],
            ..base.clone()
        };
        let (entry, out) = fill(&d, &spec).expect("fits");
        assert_eq!(out.pinned, 1);
        assert!(out.note.as_deref().is_some_and(|n| n.contains("yielded")), "{:?}", out.note);
        assert!(out.seats < plain.seats, "the pin and the hole cost stations");
        d.layers.layers.push(entry);

        let seats = |d: &RingDesign| -> Vec<SeatPadLayer> {
            let Layer::Group(g) = &d.layers.layers[0].layer else { panic!() };
            g.stack
                .layers
                .iter()
                .filter_map(|e| match &e.layer {
                    Layer::SeatPad(s) => Some(*s),
                    _ => None,
                })
                .collect()
        };

        let found = |d: &RingDesign, theta: f64| {
            seats(d).into_iter().find(|s| (s.theta_deg - theta).abs() < 1e-9)
        };
        // Pinned means pinned: exactly where it was put, not near it.
        let held_now = found(&d, pin_theta).expect("the pin is on the ring");
        assert_eq!(held_now.gem, held.gem, "and it is the user's stone");

        // The hole is empty, and nothing the packer placed crowds the pin.
        for s in seats(&d) {
            let dh = crate::field::wrap_delta(s.theta_deg - hole_theta, 360.0).abs();
            assert!(dh > 1e-6 || s.v_mm != crest, "a seat landed in the hole");
        }
        let r = crate::stones::report(&d, 0.0).expect("stones");
        assert!(
            r.closest.as_ref().is_some_and(|c| c.gap_mm > 0.0),
            "the packer left the pin room: {:?}",
            r.closest
        );

        // Regenerating with a different band re-packs around the pin, and
        // the pin does not move.
        d.profile.width_mm = 12.0;
        let notes = regenerate_live(&mut d);
        assert!(notes.is_empty(), "{notes:?}");
        let after = found(&d, pin_theta).expect("the pin survived the band change");
        assert_eq!(after.gem, held.gem);
        assert!((after.v_mm - crest).abs() < 1e-9, "and it did not drift");

        // Every seat the packer still owns sits on an integer lattice — the
        // seamlessness guarantee, and what would break if this ever reached
        // for a relaxation solver instead of yielding.
        let mut ts: Vec<f64> = seats(&d)
            .iter()
            .filter(|s| s.gem != held.gem)
            .map(|s| s.theta_deg)
            .collect();
        ts.sort_by(f64::total_cmp);
        let n = (360.0 / (ts[1] - ts[0])).round();
        for t in &ts {
            let off = (t / (360.0 / n)).fract();
            assert!(off < 1e-6 || off > 1.0 - 1e-6, "{t} is off the lattice of {n}");
        }

        // The report rolls the fill up by seat shape rather than printing a
        // row per station, and a pin with its own stone is its own shape.
        assert_eq!(r.seats.len(), 2, "{:?}", r.seats.iter().map(|s| &s.label).collect::<Vec<_>>());

        // Pins survive the file, and a design written before them still loads.
        let dir = std::env::temp_dir().join("ringdesign-pin-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pinned.ring.json");
        crate::library::save_design(&path, &d).unwrap();
        let back = crate::library::load_design(&path).unwrap();
        let Layer::Group(g) = &back.layers.layers[0].layer else { panic!() };
        let Some(GenRecipe::Pave(sp)) = &g.recipe else { panic!() };
        assert_eq!(sp.pinned.len(), 2, "pins lost in the file");
        let _ = std::fs::remove_dir_all(&dir);

        let old = serde_json::to_string(&base).unwrap();
        assert!(!old.is_empty());
        let bare: PaveSpec =
            serde_json::from_str(&old.replace(",\"pinned\":[]", "")).expect("a file with no pins");
        assert!(bare.pinned.is_empty());

        let lib = crate::AlphaLibrary::builtin();
        let v = crate::castability::analyze_field(&d, &lib, &d.draft, 192, 96);
        assert!(v.verdict != crate::castability::Verdict::NotCastable, "{:?}", v.verdict);
    }

    #[test]
    fn live_groups_regenerate_with_the_band_and_bake_detaches() {
        let mut d = flat_design();
        let spec = PaveSpec {
            gem: Gem::calibrated(crate::gem::GemCut::Round, 1.2),
            region: PaveRegion::VBand {
                center_mm: d.field_context().crest_v_mm,
                width_mm: 5.0,
            },
            ..Default::default()
        };
        let (entry, out) = fill(&d, &spec).expect("fits");
        let Layer::Group(g) = &entry.layer else { panic!() };
        assert!(matches!(g.recipe, Some(GenRecipe::Pave(_))), "generated groups are live");
        d.layers.layers.push(entry);
        let before = out.seats;

        // Editing the stored recipe — the editor's flow — re-packs the group.
        d.profile.width_mm = 12.0;
        let crest = d.field_context().crest_v_mm;
        let Layer::Group(g) = &mut d.layers.layers[0].layer else { panic!() };
        let Some(GenRecipe::Pave(sp)) = &mut g.recipe else { panic!() };
        sp.region = PaveRegion::VBand { center_mm: crest, width_mm: 8.0 };
        let notes = regenerate_live(&mut d);
        assert!(notes.is_empty(), "{notes:?}");
        let Layer::Group(g) = &d.layers.layers[0].layer else { panic!() };
        assert!(
            g.stack.layers.len() > before,
            "{} seats after widening the region, was {before}",
            g.stack.layers.len()
        );

        // The recipe survives the file round-trip.
        let dir = std::env::temp_dir().join("ringdesign-live-group-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("live.ring.json");
        crate::library::save_design(&path, &d).unwrap();
        let back = crate::library::load_design(&path).unwrap();
        let Layer::Group(g2) = &back.layers.layers[0].layer else { panic!() };
        assert!(matches!(g2.recipe, Some(GenRecipe::Pave(_))), "recipe lost in the file");
        let _ = std::fs::remove_dir_all(&dir);

        // A region the recipe no longer fits: the note says so, the stack stays.
        let kept = g.stack.layers.len();
        let Layer::Group(g) = &mut d.layers.layers[0].layer else { panic!() };
        let Some(GenRecipe::Pave(sp)) = &mut g.recipe else { panic!() };
        sp.region = PaveRegion::VBand { center_mm: 2.0, width_mm: 0.8 };
        let notes = regenerate_live(&mut d);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].contains("no longer fits"));
        let Layer::Group(g) = &d.layers.layers[0].layer else { panic!() };
        assert_eq!(g.stack.layers.len(), kept, "refusal must not destroy the stack");

        // Baked: the recipe is gone and regeneration leaves the layers alone.
        let Layer::Group(g) = &mut d.layers.layers[0].layer else { panic!() };
        g.recipe = None;
        let fingerprint = g.stack.layers.len();
        let notes = regenerate_live(&mut d);
        assert!(notes.is_empty());
        let Layer::Group(g) = &d.layers.layers[0].layer else { panic!() };
        assert_eq!(g.stack.layers.len(), fingerprint, "a baked group is hand-owned");
    }

    /// The process mode: the same proud-accent halo that locks in sand is
    /// the classic lost-wax cluster, and the verdict knows the difference.
    #[test]
    fn lost_wax_frees_the_halo_and_the_verdict_says_which_is_which() {
        use crate::castability::{self, CastProcess, Verdict};
        let mut d = RingDesign::default();
        d.profile.apply_style(ProfileStyle::LowDome);
        d.profile.width_mm = 11.0;
        d.profile.thickness_mm = 2.4;
        let spec = HaloSpec::default();
        let lib = crate::alpha::AlphaLibrary::builtin();

        // Sand: plate + zero-height markers, no proud accents.
        let (entry, _) = halo(&d, &spec).unwrap();
        let Layer::Group(g) = &entry.layer else { panic!() };
        assert!(g.stack.layers.iter().any(|e| e.name == "Plate"));

        // Lost wax: the classic proud ring, no plate — and the same design
        // that would lock in sand fields Castable with the honest note.
        d.draft.process = CastProcess::LostWax;
        let (entry, n) = halo(&d, &spec).unwrap();
        let Layer::Group(g) = &entry.layer else { panic!() };
        assert!(!g.stack.layers.iter().any(|e| e.name == "Plate"));
        let proud = g
            .stack
            .layers
            .iter()
            .filter(|e| matches!(&e.layer, Layer::SeatPad(s) if s.height_mm > 0.0))
            .count();
        assert_eq!(proud, n as usize + 1, "centre plus every accent stands proud");

        d.layers.layers.push(entry);
        let f = castability::analyze_field(&d, &lib, &d.draft, 220, 128);
        assert_eq!(f.verdict, Verdict::Castable, "{:?}", f.notes);
        assert!(
            f.undercut_area_mm2 > 0.0,
            "the pull statistics still report the sand-hostile geometry"
        );
        assert!(f.notes.iter().any(|n| n.contains("cannot move to sand")), "{:?}", f.notes);

        // Back in sand mode the same proud design is refused.
        d.draft.process = CastProcess::SandTwoPart;
        let f = castability::analyze_field(&d, &lib, &d.draft, 220, 128);
        assert_ne!(f.verdict, Verdict::Castable);
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
