//! Stones in the cast report: per-seat bench checks and carat totals.
//!
//! Analytic, not mesh-read: the layers say where every seat is and what it
//! holds, and the base profile says what it sits on, so each check is a
//! number about the design rather than a measurement of one build of it.
//! Stones themselves are never cast — what is checked is the *stock*: the
//! pad's footing, the metal a pavilion needs, the bridge between neighbours.

use crate::castability::draft_angle;
use crate::field::{
    FieldContext, Layer, LayerEntry, LayerStack, SeatPadLayer, SeatStyle, Uv,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use crate::gem::Gem;
use crate::mesh::MIN_WALL_MM;
use crate::profile::MIN_EDGE_MM;
use crate::RingDesign;

/// What the base surface under a seat is, castability-wise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SeatFooting {
    /// A face parallel to the pull: displacement along its normal is castable
    /// by construction.
    SideFace,
    /// On the outer surface, with this much signed draft, degrees.
    Crown(f64),
}

/// One seat layer's line in the report. A run is one line with `count`
/// stations; its checks are the worst over the stations its window keeps.
#[derive(Clone, Debug)]
pub struct SeatCheck {
    /// Layer name, prefixed by its group path.
    pub label: String,
    pub style: SeatStyle,
    /// Stations this seat actually occupies (a run windowed to half the ring
    /// keeps half its count; a pad gated out by its own window keeps none).
    pub count: u32,
    pub seat_diameter_mm: f64,
    pub gem: Option<Gem>,
    pub footing: SeatFooting,
    /// Metal from the pad's foot to the nearer band edge, mm.
    pub edge_clearance_mm: f64,
    /// Metal available under the seat for a pavilion, mm — along the seat's
    /// normal to the far wall, less the minimum wall, plus the stand-off the
    /// pad itself adds.
    pub depth_available_mm: f64,
    /// Runs only: metal left between neighbouring seats, mm.
    pub bridge_mm: Option<f64>,
    /// Graduated runs only: the summed graded carats, which count times the
    /// largest stone would overstate.
    pub carats_override: Option<f64>,
    /// Shared-prong runs only: (pairs, post diameter mm, proud mm).
    pub shared_prongs: Option<(u32, f64, f64)>,
    pub warnings: Vec<String>,
}

impl SeatCheck {
    pub fn carats(&self) -> f64 {
        if let Some(c) = self.carats_override {
            return c;
        }
        self.gem.map_or(0.0, |g| g.carats() * self.count as f64)
    }
}

/// Two stones and the metal between them.
///
/// Every seat in the design is a station somewhere on the band, and stones
/// set from different layers know nothing about each other — a pad beside a
/// run, two runs at different `v`, a halo's melee against its centre. This
/// is the pairwise census that catches what a per-layer bridge cannot.
#[derive(Clone, Debug)]
pub struct StonePair {
    pub a: String,
    pub b: String,
    pub a_theta_deg: f64,
    pub b_theta_deg: f64,
    /// Metal between the two girdles, mm. Negative means they overlap.
    pub gap_mm: f64,
    /// The same gap at the shallower of the two culets, where the ring's
    /// own curvature has closed the arc in. Straight-walled: a step cut
    /// keeps its width all the way down, and step cuts are the population
    /// that gets set tight.
    pub gap_deep_mm: f64,
}

impl StonePair {
    /// The gap that decides, mm — the tighter of the two.
    pub fn worst_mm(&self) -> f64 {
        self.gap_mm.min(self.gap_deep_mm)
    }
}

/// How many crowded pairs the report will print before it stops listing.
const MAX_PAIRS: usize = 12;

#[derive(Clone, Debug, Default)]
pub struct StonesReport {
    pub seats: Vec<SeatCheck>,
    pub stone_count: u32,
    pub total_carats: f64,
    /// The tightest neighbours in the whole design, worst first — at most
    /// [`MAX_PAIRS`], because a 240-seat pavé is not a list.
    pub crowding: Vec<StonePair>,
    /// Pairs under the bench floor, including any past the printed few.
    pub tight_pairs: usize,
    /// The closest two stones in the design whether or not they are tight —
    /// the headline number, and the one a probe or a banner wants.
    pub closest: Option<StonePair>,
}

impl StonesReport {
    pub fn any_warnings(&self) -> bool {
        self.seats.iter().any(|s| !s.warnings.is_empty()) || self.tight_pairs > 0
    }

    /// The one line the sheet and the banner want.
    pub fn crowding_note(&self) -> Option<String> {
        let worst = self.crowding.first()?;
        (worst.worst_mm() < CROWD_TIGHT_MM).then(|| {
            format!(
                "{} tight pair{}: {} and {} leave {:.2} mm at the girdle, {:.2} mm at depth",
                self.tight_pairs,
                if self.tight_pairs == 1 { "" } else { "s" },
                worst.a,
                worst.b,
                worst.gap_mm,
                worst.gap_deep_mm
            )
        })
    }
}

/// Metal between two stones the sand still fills, mm. Under this the report
/// says so; under [`MIN_EDGE_MM`] it will not fill at all.
pub const CROWD_TIGHT_MM: f64 = 0.3;

/// Every seat in the design, checked. `None` when the stack carries no seats.
///
/// `parting_z_mm` is the plane the draft numbers are signed against — pass
/// the cast report's when there is one; 0 is the crest plane every profile
/// puts its crest on by construction.
pub fn report(design: &RingDesign, parting_z_mm: f64) -> Option<StonesReport> {
    let ctx = design.field_context();
    let inner_r = design.inner_radius_mm();
    let crest_r = ctx.crest_radius_mm;

    let mut acc = Acc::default();
    walk(design, &ctx, inner_r, crest_r, parting_z_mm, &design.layers, "", &mut acc);
    let Acc { seats, stations } = acc;
    if seats.is_empty() {
        return None;
    }
    let stone_count = seats.iter().filter(|s| s.gem.is_some()).map(|s| s.count).sum();
    let total_carats = seats.iter().map(|s| s.carats()).sum();
    let (crowding, tight_pairs, closest) =
        crowding(design, &ctx, inner_r, crest_r, &stations);
    Some(StonesReport { seats, stone_count, total_carats, crowding, tight_pairs, closest })
}

/// One stone, wherever it came from — the unit the pairwise census works in.
struct Station {
    label: String,
    theta_deg: f64,
    v_mm: f64,
    gem: crate::gem::Gem,
    /// Height of the girdle over the bare band, mm.
    stand_off: f64,
    /// Bearing of the stone's long axis in the chart, degrees.
    rot_deg: f64,
}

#[derive(Default)]
struct Acc {
    seats: Vec<SeatCheck>,
    stations: Vec<Station>,
}

impl Acc {
    fn station(
        &mut self,
        label: &str,
        seat: &SeatPadLayer,
        gem: crate::gem::Gem,
        theta_deg: f64,
        v_mm: f64,
    ) {
        self.stations.push(Station {
            label: label.to_string(),
            theta_deg,
            v_mm,
            gem,
            stand_off: stand_off(seat),
            rot_deg: seat.rot_deg,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    design: &RingDesign,
    ctx: &FieldContext,
    inner_r: f64,
    crest_r: f64,
    parting_z: f64,
    stack: &LayerStack,
    prefix: &str,
    acc: &mut Acc,
) {
    for entry in &stack.layers {
        if !entry.enabled {
            continue;
        }
        match &entry.layer {
            Layer::SeatPad(seat) => {
                let kept = station_kept(entry, ctx, seat.theta_deg, seat.v_mm);
                let mut check = check_seat(
                    design,
                    ctx,
                    inner_r,
                    crest_r,
                    parting_z,
                    seat,
                    &[(seat.theta_deg, seat.v_mm)],
                    format!("{prefix}{}", entry.name),
                );
                check.count = kept as u32;
                if !kept {
                    check.warnings.push("gated out by its own window — not on the ring".into());
                }
                if let (true, Some(gem)) = (kept, seat.gem) {
                    acc.station(&check.label, seat, gem, seat.theta_deg, seat.v_mm);
                }
                acc.seats.push(check);
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let pitch = 360.0 / n as f64;
                let stations: Vec<(f64, f64)> = (0..n)
                    .map(|k| (k as f64 * pitch, run.seat.v_mm))
                    .filter(|&(t, v)| station_kept(entry, ctx, t, v))
                    .collect();
                let mut seat = run.seat;
                seat.fit_stone(run.gem);
                let mut check = check_seat(
                    design,
                    ctx,
                    inner_r,
                    crest_r,
                    parting_z,
                    &seat,
                    &stations,
                    format!("{prefix}{}", entry.name),
                );
                check.count = stations.len() as u32;
                // A graduated run's carats sum the graded stones, not count
                // times the largest; the check's headline gem stays the
                // largest, which is the one the pavilion depth must clear.
                if run.taper > 0.0 {
                    check.carats_override = Some(
                        stations.iter().map(|&(t, _)| run.gem_at(t).carats()).sum(),
                    );
                }
                let bridge = run.bridge_at(ctx);
                check.bridge_mm = Some(bridge);
                if bridge < MIN_EDGE_MM {
                    check.warnings.push(format!(
                        "bridge {bridge:.2} mm between stones will not fill (min {MIN_EDGE_MM} mm)"
                    ));
                } else if bridge < 0.3 {
                    check
                        .warnings
                        .push(format!("bridge {bridge:.2} mm is tight for sand — 0.3 mm is safer"));
                }
                if check.count == 0 {
                    check.warnings.push("window keeps no stations — not on the ring".into());
                }
                if run.shared_prong_mm > 1e-9 {
                    check.shared_prongs = Some((
                        check.count,
                        run.prong_r_mm() * 2.0,
                        run.shared_prong_mm,
                    ));
                    let sand = design.draft.process == crate::castability::CastProcess::SandTwoPart;
                    if sand && matches!(check.footing, SeatFooting::Crown(_)) {
                        check.warnings.push(format!(
                            "shared prongs {:.2} mm proud flank the stone column off the \
                             parting plane — posts on a crown lean under a two-part pull. \
                             Cast flush (0 mm) and bead-set, or judge for lost wax",
                            run.shared_prong_mm
                        ));
                    }
                    let floor = design.draft.min_detail_mm;
                    if run.prong_r_mm() * 2.0 < floor {
                        check.warnings.push(format!(
                            "post Ø{:.2} mm is under the process detail floor ({floor} mm)",
                            run.prong_r_mm() * 2.0
                        ));
                    }
                }
                // Every station a run keeps is a stone in its own right;
                // the census reads them against everything else on the band.
                for &(t, v) in &stations {
                    let graded = run.gem_at(t);
                    let mut seat = run.seat;
                    seat.fit_stone(graded);
                    acc.station(&check.label, &seat, graded, t, v);
                }
                acc.seats.push(check);
            }
            Layer::Group(g) => {
                // A uniform seat group — a pavé fill — rolls up to one line:
                // two hundred rows of "Seat 137" is not a report.
                if let Some(stations) = uniform_seats(entry, &g.stack, ctx) {
                    let Some(Layer::SeatPad(first)) = g
                        .stack
                        .layers
                        .iter()
                        .find(|e| e.enabled)
                        .map(|e| &e.layer)
                    else {
                        continue;
                    };
                    let mut check = check_seat(
                        design,
                        ctx,
                        inner_r,
                        crest_r,
                        parting_z,
                        first,
                        &stations,
                        format!("{prefix}{}", entry.name),
                    );
                    check.count = stations.len() as u32;
                    if let Some(gem) = first.gem {
                        for &(t, v) in &stations {
                            acc.station(&check.label, first, gem, t, v);
                        }
                    }
                    acc.seats.push(check);
                    continue;
                }
                let path = format!("{prefix}{} / ", entry.name);
                walk(design, ctx, inner_r, crest_r, parting_z, &g.stack, &path, acc);
            }
            _ => {}
        }
    }
}

/// Every stone against every other, in millimetres of real metal.
///
/// The per-layer bridge only knows about a run's own neighbours, so a pad
/// beside a run, two runs at different `v`, or a halo's melee against its
/// centre all go unchecked — CrossGems solved the same problem with a
/// separate proximity pass over the whole gem set, and this is that pass in
/// the terms of this model. Analytic like everything else here: the station
/// positions come from the modulated bare profile and the layers, so the
/// census costs nothing and cannot disagree with the design.
///
/// Two numbers per pair. At the girdle it is the plain surface-to-surface
/// gap. At depth the ring's own curvature has closed the arc in — pitch `p`
/// at crest radius `r` is only `p (r - t) / r` at depth `t` — and a stone
/// with straight pavilion walls keeps its full width the whole way down. On
/// a size-7 band a 16-stone row of 2.5 mm step cuts loses 0.38 mm of its
/// bridge that way, which is nearly twice `MIN_EDGE_MM`.
fn crowding(
    design: &RingDesign,
    ctx: &FieldContext,
    inner_r: f64,
    crest_r: f64,
    stations: &[Station],
) -> (Vec<StonePair>, usize, Option<StonePair>) {
    if stations.len() < 2 {
        return (Vec::new(), 0, None);
    }
    let frames: Vec<Frame> = stations
        .iter()
        .map(|st| frame_at(design, ctx, inner_r, crest_r, st))
        .collect();

    let mut pairs: Vec<StonePair> = Vec::new();
    let mut tight = 0usize;
    let mut closest: Option<StonePair> = None;
    for i in 0..stations.len() {
        for j in (i + 1)..stations.len() {
            let (fa, fb) = (&frames[i], &frames[j]);
            let d = sub(fb.girdle, fa.girdle);
            let dist = norm(d);
            // Nothing can be closer than the centres are, less both reaches;
            // anything further apart than a stone is not a neighbour.
            let floor = dist - fa.reach - fb.reach;
            if floor > closest.as_ref().map_or(f64::MAX, |c| c.worst_mm())
                && floor > CROWD_TIGHT_MM
            {
                continue;
            }
            let gap = dist - fa.plan_r(d) - fb.plan_r(neg(d));
            // At the shallower culet both stones still have metal beside
            // them; past it only one of them does.
            let deep = fa.pavilion.min(fb.pavilion);
            let dd = sub(axial(fb.girdle, fb.normal, deep), axial(fa.girdle, fa.normal, deep));
            let gap_deep = norm(dd) - fa.plan_r(dd) - fb.plan_r(neg(dd));
            let pair = StonePair {
                a: stations[i].label.clone(),
                b: stations[j].label.clone(),
                a_theta_deg: stations[i].theta_deg,
                b_theta_deg: stations[j].theta_deg,
                gap_mm: gap,
                gap_deep_mm: gap_deep,
            };
            if closest.as_ref().is_none_or(|c| pair.worst_mm() < c.worst_mm()) {
                closest = Some(pair.clone());
            }
            if pair.worst_mm() >= CROWD_TIGHT_MM {
                continue;
            }
            tight += 1;
            pairs.push(pair);
        }
    }
    pairs.sort_by(|x, y| x.worst_mm().total_cmp(&y.worst_mm()));
    pairs.truncate(MAX_PAIRS);
    (pairs, tight, closest)
}

/// A stone standing on the band: where its girdle is, which way it faces,
/// and the plan it presents in every direction.
struct Frame {
    girdle: [f64; 3],
    normal: [f64; 3],
    /// The stone's long axis and its short one, in world space.
    long: [f64; 3],
    short: [f64; 3],
    semi: (f64, f64),
    plan_pow: f64,
    /// The furthest the girdle reaches from its centre, mm.
    reach: f64,
    pavilion: f64,
}

impl Frame {
    /// The girdle's own radius toward `d`, mm.
    fn plan_r(&self, d: [f64; 3]) -> f64 {
        crate::field::superellipse_radius_mm(
            dot(d, self.long),
            dot(d, self.short),
            self.semi.0,
            self.semi.1,
            self.plan_pow,
        )
    }
}

fn frame_at(
    design: &RingDesign,
    ctx: &FieldContext,
    inner_r: f64,
    crest_r: f64,
    st: &Station,
) -> Frame {
    let b = base_at(design, inner_r, crest_r, ctx, st.theta_deg, st.v_mm);
    let (sin, cos) = st.theta_deg.to_radians().sin_cos();
    let normal = [b.nr * cos, b.nr * sin, b.nz];
    let girdle = [
        b.r * cos + normal[0] * st.stand_off,
        b.r * sin + normal[1] * st.stand_off,
        b.z + normal[2] * st.stand_off,
    ];
    // The band's own two tangents: along the ring, and across the section.
    let t = [-sin, cos, 0.0];
    let across = [-b.nz * cos, -b.nz * sin, b.nr];
    let (rs, rc) = st.rot_deg.to_radians().sin_cos();
    let long = [
        t[0] * rc + across[0] * rs,
        t[1] * rc + across[1] * rs,
        t[2] * rc + across[2] * rs,
    ];
    let short = [
        -t[0] * rs + across[0] * rc,
        -t[1] * rs + across[1] * rc,
        -t[2] * rs + across[2] * rc,
    ];
    let semi = (st.gem.l_mm * 0.5, st.gem.w_mm * 0.5);
    let n = st.gem.cut.plan_pow();
    let reach = if n <= 2.0 { semi.0 } else { (semi.0 * semi.0 + semi.1 * semi.1).sqrt() };
    Frame {
        girdle,
        normal,
        long,
        short,
        semi,
        plan_pow: n,
        reach,
        pavilion: st.gem.pavilion_mm(),
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn neg(a: [f64; 3]) -> [f64; 3] {
    [-a[0], -a[1], -a[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// A point moved along a normal by `d`, into the metal.
fn axial(p: [f64; 3], n: [f64; 3], d: f64) -> [f64; 3] {
    [p[0] - n[0] * d, p[1] - n[1] * d, p[2] - n[2] * d]
}

/// The `(theta, v)` stations of a group made only of identical seats —
/// same stone, style and diameter — or `None` when it is any other group.
fn uniform_seats(
    entry: &LayerEntry,
    stack: &LayerStack,
    ctx: &FieldContext,
) -> Option<Vec<(f64, f64)>> {
    let mut proto: Option<&SeatPadLayer> = None;
    let mut stations = Vec::new();
    for e in &stack.layers {
        if !e.enabled {
            continue;
        }
        let Layer::SeatPad(s) = &e.layer else { return None };
        match proto {
            None => proto = Some(s),
            Some(p) => {
                let same = p.gem == s.gem
                    && p.style == s.style
                    && (p.diameter_mm - s.diameter_mm).abs() < 1e-9
                    && (p.elong - s.elong).abs() < 1e-9
                    && (p.rot_deg - s.rot_deg).abs() < 1e-9;
                if !same {
                    return None;
                }
            }
        }
        if station_kept(entry, ctx, s.theta_deg, s.v_mm) {
            stations.push((s.theta_deg, s.v_mm));
        }
    }
    (proto.is_some() && stations.len() > 1).then_some(stations)
}

/// Whether a station survives the entry's angular window.
fn station_kept(entry: &LayerEntry, ctx: &FieldContext, theta_deg: f64, v_mm: f64) -> bool {
    let uv = Uv { u: ctx.u_of_theta(theta_deg.rem_euclid(360.0)), v: v_mm };
    entry.window.mask(uv, ctx) > 0.5
}

/// The checks for one seat shape at a set of stations; every measured number
/// is the worst over them, because a run on a tapered shank meets a different
/// band at every station.
#[allow(clippy::too_many_arguments)]
fn check_seat(
    design: &RingDesign,
    ctx: &FieldContext,
    inner_r: f64,
    crest_r: f64,
    parting_z: f64,
    seat: &SeatPadLayer,
    stations: &[(f64, f64)],
    label: String,
) -> SeatCheck {
    let mut footing = SeatFooting::Crown(90.0);
    let mut worst_draft = f64::MAX;
    let mut side = true;
    let mut clearance = f64::MAX;
    let mut depth = f64::MAX;

    let probe: Vec<(f64, f64)> = if stations.is_empty() {
        vec![(seat.theta_deg, seat.v_mm)]
    } else {
        stations.to_vec()
    };
    for &(theta, v_here) in &probe {
        let b = base_at(design, inner_r, crest_r, ctx, theta, v_here);
        // Side-face-ness by the same measure the side-face walk uses: how far
        // the outward normal leans along the pull.
        let lean = b.nz.abs().asin().to_degrees();
        side &= lean >= SIDE_FACE_MIN_DRAFT_DEG;
        let radial = [b.nr * theta.to_radians().cos(), b.nr * theta.to_radians().sin(), b.nz];
        worst_draft = worst_draft.min(draft_angle(radial, b.z, parting_z));

        // Foot to the nearer band edge, in reference v like the layer itself.
        // An elongated seat reaches across the band by its own `v` extent,
        // which is its length when the stone is turned to face the edges.
        let foot = seat.half_extents_mm().1 + seat.blend_mm.max(0.0);
        clearance = clearance
            .min(v_here - foot)
            .min(ctx.band_v_len_mm - v_here - foot);

        // Metal along the seat's normal: to the bore wall on the crown, across
        // the band on a side face — the drill goes where the normal points.
        let through = if b.nz.abs() > b.nr.abs() { b.width } else { b.r - inner_r };
        depth = depth.min(through - MIN_WALL_MM + stand_off(seat));
    }
    if side {
        footing = SeatFooting::SideFace;
    } else if worst_draft < f64::MAX {
        footing = SeatFooting::Crown(worst_draft);
    }

    let mut warnings = Vec::new();
    match footing {
        SeatFooting::SideFace => {}
        SeatFooting::Crown(d) => {
            if seat.style == SeatStyle::Bezel {
                warnings.push(
                    "bezel pocket on the crown — its walls turn to ceilings; move it to a side face"
                        .into(),
                );
            } else if seat.crown < 0.7 && d < design.draft.min_draft_deg {
                // The measured hazard is a flat top's rim on a curved base:
                // 8.6% at -51 degrees for boss rows on a dome, 0.000% for
                // fully-domed gypsy mounds on the same base. A domed pad has
                // no rim to lock.
                warnings.push(format!(
                    "flat-topped pad on {d:+.0}° of base draft (min {:.0}°) — its rim can lock; \
                     dome the pad or move it to a side face",
                    design.draft.min_draft_deg
                ));
            }
        }
    }
    if clearance < MIN_EDGE_MM {
        warnings.push(format!(
            "foot reaches within {clearance:.2} mm of the band edge — feather edges will not fill"
        ));
    }
    if let Some(gem) = seat.gem {
        let need = gem.pavilion_mm();
        if need > depth {
            warnings.push(format!(
                "culet needs {need:.2} mm; {depth:.2} mm before the {MIN_WALL_MM} mm wall — \
                 shallower stone or taller seat"
            ));
        }
    }

    SeatCheck {
        label,
        style: seat.style,
        count: probe.len() as u32,
        seat_diameter_mm: seat.diameter_mm,
        gem: seat.gem,
        footing,
        edge_clearance_mm: if clearance == f64::MAX { 0.0 } else { clearance },
        depth_available_mm: if depth == f64::MAX { 0.0 } else { depth },
        bridge_mm: None,
        carats_override: None,
        shared_prongs: None,
        warnings,
    }
}

/// Height the girdle sits above the base surface: the pad top for a drilled
/// seat, the pocket floor for a bezel.
fn stand_off(seat: &SeatPadLayer) -> f64 {
    match seat.style {
        SeatStyle::Bezel => (seat.height_mm - seat.recess_mm).max(0.0),
        _ => seat.height_mm.max(0.0),
    }
}

struct BasePoint {
    r: f64,
    z: f64,
    nr: f64,
    nz: f64,
    /// Band width at this station, mm.
    width: f64,
}

/// The bare band under a point, from the modulated profile — the same
/// reference-normalized `v` the layer itself is evaluated at, so the checks
/// follow the band as it tapers exactly like the seat does.
fn base_at(
    design: &RingDesign,
    inner_r: f64,
    crest_r: f64,
    ctx: &FieldContext,
    theta_deg: f64,
    v_mm: f64,
) -> BasePoint {
    let m = design.modulation_at(theta_deg, inner_r, crest_r);
    let l = design.profile.sample_mod(inner_r, 192, &m);
    let v_norm = (v_mm / ctx.band_v_len_mm.max(1e-9)).clamp(0.0, 1.0);
    let target = v_norm * l.surface_len_mm;

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    let mut best: Option<&crate::profile::ProfileSample> = None;
    let mut best_d = f64::MAX;
    for p in l.pts.iter() {
        lo = lo.min(p.z);
        hi = hi.max(p.z);
        if p.surface {
            let d = (p.v_mm - target).abs();
            if d < best_d {
                best_d = d;
                best = Some(p);
            }
        }
    }
    match best {
        Some(p) => BasePoint { r: p.r, z: p.z, nr: p.nr, nz: p.nz, width: (hi - lo).max(0.0) },
        None => BasePoint { r: inner_r, z: 0.0, nr: 1.0, nz: 0.0, width: 0.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Blend, SeatRunLayer, VGate, Window};
    use crate::gem::{Gem, GemCut};

    fn with_run() -> RingDesign {
        let mut d = RingDesign::default();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.0);
        let ctx = d.field_context();
        run.seat.v_mm = ctx.band_v_len_mm * 0.5;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Eternity", Layer::SeatRun(run)));
        d
    }

    /// A cabochon is flat-backed, so a seat owes it a bed and not a hole.
    /// Reading the faceted 0.65-of-depth pavilion there refused the single
    /// easiest stone a cast band can carry.
    #[test]
    fn a_cabochon_asks_for_a_bed_and_not_a_pavilion() {
        use crate::field::{SeatPadLayer, SeatStyle};
        use crate::gem::GemForm;

        let stone_mm = 6.0;
        let seated = |gem: Gem| {
            let mut d = RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::LowDome);
            d.profile.thickness_mm = 1.6;
            d.profile.width_mm = 10.0;
            let ctx = d.field_context();
            let mut pad = SeatPadLayer {
                theta_deg: 90.0,
                v_mm: ctx.crest_v_mm,
                style: SeatStyle::GypsyMound,
                ..Default::default()
            };
            pad.fit_stone(gem);
            pad.height_mm = 0.5;
            d.layers.layers.push(LayerEntry::new("Cab", Layer::SeatPad(pad)));
            report(&d, 0.0).unwrap().seats.remove(0)
        };

        let faceted = seated(Gem::calibrated(GemCut::Round, stone_mm));
        assert!(
            faceted.warnings.iter().any(|w| w.contains("culet")),
            "a 6 mm brilliant really does want more metal than a 2 mm band has: {:?}",
            faceted.warnings
        );

        let cab = Gem::cabochon(GemCut::Round, stone_mm);
        assert_eq!(cab.form, GemForm::Cabochon);
        assert_eq!(cab.pavilion_mm(), crate::gem::BED_CLEARANCE_MM);
        let check = seated(cab);
        assert!(
            !check.warnings.iter().any(|w| w.contains("culet")),
            "a cabochon needs no metal under it: {:?}",
            check.warnings
        );

        // The plan is fatter than the faceted make of the same name, which is
        // the whole content of their separate cabochon table.
        let m = Gem::cabochon(GemCut::Marquise, 4.0);
        assert!((m.l_mm - 5.0).abs() < 1e-9, "a cab marquise is 1.25:1, not 2:1");
        assert!(Gem::cabochon(GemCut::Round, 4.0).l_mm == 4.0, "a round cab stays round");

        // A medium dome is shallower than a brilliant of the same width —
        // there is no pavilion under it — but it is solid where a brilliant
        // is a cone, so it does not weigh less.
        assert!(cab.depth_mm() < Gem::calibrated(GemCut::Round, stone_mm).depth_mm());
        assert!(cab.carats() > 0.0);
        assert!(cab.display().contains("cabochon"));
    }

    /// The pairwise census: two layers that know nothing about each other,
    /// and the arc that closes under a row of step cuts.
    #[test]
    fn the_census_measures_metal_between_stones_no_layer_can_see() {
        use crate::field::{SeatPadLayer, SeatStyle};

        // Two independent pads six degrees apart on a size-7 crest. Their
        // girdles ride the seat stand-off, so the arc between them is the
        // one at *that* radius, not at the band's.
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let ctx = d.field_context();
        let pad = |theta: f64| {
            let mut p = SeatPadLayer {
                theta_deg: theta,
                v_mm: ctx.crest_v_mm,
                style: SeatStyle::GypsyMound,
                ..Default::default()
            };
            p.fit_stone(Gem::calibrated(GemCut::Round, 3.0));
            p
        };
        d.layers.layers.push(LayerEntry::new("A", Layer::SeatPad(pad(90.0))));
        d.layers.layers.push(LayerEntry::new("B", Layer::SeatPad(pad(96.0))));
        let r = report(&d, 0.0).unwrap();
        let hit = r.crowding.first().expect("two stones 1.2 mm apart are a pair");
        assert_eq!(r.tight_pairs, 1);
        assert!(hit.gap_mm < 0.0, "these two overlap: {:.3} mm", hit.gap_mm);
        // Chord at the girdle radius, less both stones' half-widths.
        let girdle_r = ctx.crest_radius_mm + 1.2;
        let want = 2.0 * girdle_r * 3.0f64.to_radians().sin() - 3.0;
        assert!((hit.gap_mm - want).abs() < 0.02, "{:.3} vs {want:.3}", hit.gap_mm);
        assert!(r.crowding_note().is_some());

        // A row of step cuts that clears at the girdle and does not at the
        // culet. The arc closes by pitch * pavilion / crest_radius, which on
        // this band is 0.38 mm — nearly twice MIN_EDGE_MM, and it is metal
        // the bridge measured at the girdle never sees.
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Emerald, 2.5);
        run.seat.v_mm = ctx.crest_v_mm;
        run.count = 16;
        run.seat.fit_stone(run.gem);
        d.layers.layers.push(LayerEntry::new("Step row", Layer::SeatRun(run)));
        let r = report(&d, 0.0).unwrap();
        let hit = r.crowding.first().expect("the row is tight at depth");
        assert!(hit.gap_mm > CROWD_TIGHT_MM, "clears at the girdle: {:.3}", hit.gap_mm);
        assert!(hit.gap_deep_mm < CROWD_TIGHT_MM, "and not at depth: {:.3}", hit.gap_deep_mm);
        let pitch = ctx.circumference_mm / 16.0;
        let loss = pitch * run.gem.pavilion_mm() / ctx.crest_radius_mm;
        assert!(
            ((hit.gap_mm - hit.gap_deep_mm) - loss).abs() < 0.02,
            "arc loss {:.3} vs predicted {loss:.3}",
            hit.gap_mm - hit.gap_deep_mm
        );

        // A well-spaced row of rounds is not a finding.
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.0);
        run.seat.v_mm = ctx.crest_v_mm;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Eternity", Layer::SeatRun(run)));
        let r = report(&d, 0.0).unwrap();
        assert_eq!(r.tight_pairs, 0, "{:?}", r.crowding.first());
        assert!(r.crowding_note().is_none());
    }

    /// A graduated run: the field tapers the seats with their stones, the
    /// carats sum the graded sizes, and the row still fields clean — a
    /// graded mound is still a mound.
    #[test]
    fn a_graduated_run_grades_sizes_carats_and_still_casts() {
        use crate::castability::{self, Verdict};
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 4.6;
        d.profile.thickness_mm = 2.5;
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.2);
        run.seat.v_mm = ctx.crest_v_mm;
        run.taper = 0.45;
        run.solve_spacing(&ctx);
        let n = run.count;

        // Largest at the top, smallest opposite, seamless scale.
        assert!((run.scale_at(90.0) - 1.0).abs() < 1e-12);
        assert!((run.scale_at(270.0) - 0.55).abs() < 1e-9);
        assert!((run.scale_at(269.9) - run.scale_at(270.1)).abs() < 1e-3);

        let graded: f64 = (0..n)
            .map(|k| run.gem_at(k as f64 * 360.0 / n as f64).carats())
            .sum();
        let flat = run.gem.carats() * n as f64;
        assert!(graded < flat * 0.85, "graded {graded:.3} vs flat {flat:.3}");

        d.layers.layers.push(LayerEntry::new("Graded", Layer::SeatRun(run)));
        let r = report(&d, 0.0).unwrap();
        assert!((r.total_carats - graded).abs() < 1e-9, "report carries graded carats");

        let lib = crate::alpha::AlphaLibrary::builtin();
        let f = castability::analyze_field(&d, &lib, &d.draft, 220, 128);
        assert_ne!(f.verdict, Verdict::NotCastable, "{:?}", f.notes);
    }

    /// Shared prongs: one post pair per boundary between neighbouring
    /// stones — the CrossGems Prongs_Row rule (pair each gem with its
    /// shift-by-one neighbour, prong the boundary, cull only when a row is
    /// open) read into the height field, where a full-ring run keeps every
    /// boundary and the window handles open arcs. Proud posts flank the
    /// column off the parting plane, so they are lost-wax stock: in sand
    /// the report says so and the field sees the lean (measured 2.8–3.0%
    /// at −62° on a low dome, converging — `examples/prong_probe.rs`);
    /// under lost wax the same design is Castable with the pull stats in
    /// the notes.
    #[test]
    fn shared_prongs_are_lost_wax_stock_and_the_report_says_so() {
        use crate::castability::{self, CastProcess, Verdict};
        use crate::field::Uv;
        let mut d = crate::RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::LowDome);
        d.profile.width_mm = 4.6;
        d.profile.thickness_mm = 2.5;
        let ctx = d.field_context();
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Round, 2.2);
        run.seat.v_mm = ctx.crest_v_mm;
        run.solve_spacing(&ctx);
        run.shared_prong_mm = 0.9;
        let n = run.count;

        // Posts stand at the boundaries, not at the stations.
        let pitch = ctx.circumference_mm / n as f64;
        let off = run.prong_off_mm();
        let seat_only = {
            let mut r0 = run;
            r0.shared_prong_mm = 0.0;
            r0
        };
        let boundary = Uv { u: pitch * 0.5, v: ctx.crest_v_mm + off };
        let station = Uv { u: 0.0, v: ctx.crest_v_mm + off };
        assert!(
            run.height(boundary, &ctx) > seat_only.height(boundary, &ctx) + 0.5,
            "post proud at the boundary"
        );
        assert!(run.height(station, &ctx) < run.height(boundary, &ctx));

        // Graduation scales the posts with their stones.
        let mut graded = run;
        graded.taper = 0.5;
        let far = Uv {
            u: ctx.circumference_mm * 0.75 + pitch * 0.5,
            v: ctx.crest_v_mm + off * graded.scale_at(271.0),
        };
        let near = run.height(boundary, &ctx);
        assert!(graded.height(far, &ctx) < near * 0.75, "far post grades down");

        d.layers.layers.push(LayerEntry::new("Shared", Layer::SeatRun(run)));

        // Sand: prong info on the report line, and the honest warning.
        let r = report(&d, 0.0).unwrap();
        let s = &r.seats[0];
        let (pairs, dia, proud) = s.shared_prongs.expect("prong info");
        assert_eq!(pairs, n);
        assert!(dia > 0.5 && (proud - 0.9).abs() < 1e-12);
        assert!(
            s.warnings.iter().any(|w| w.contains("lost wax")),
            "sand warns: {:?}",
            s.warnings
        );
        let lib = crate::alpha::AlphaLibrary::builtin();
        let sand = castability::analyze_field(&d, &lib, &d.draft, 256, 144);
        assert!(
            sand.undercut_fraction() > 5e-4,
            "posts lean in sand: {:.4}% at {:.0}°",
            sand.undercut_fraction() * 100.0,
            sand.worst_draft_deg
        );

        // Lost wax: no warning, and the verdict carries them.
        d.draft.process = CastProcess::LostWax;
        let r = report(&d, 0.0).unwrap();
        assert!(
            !r.seats[0].warnings.iter().any(|w| w.contains("lost wax")),
            "{:?}",
            r.seats[0].warnings
        );
        let lw = castability::analyze_field(&d, &lib, &d.draft, 256, 144);
        assert_eq!(lw.verdict, Verdict::Castable, "{:?}", lw.notes);
    }

    #[test]
    fn a_run_reports_its_stations_and_carats() {
        let d = with_run();
        let r = report(&d, 0.0).expect("seats exist");
        assert_eq!(r.seats.len(), 1);
        let s = &r.seats[0];
        assert!(s.count >= 3, "count {}", s.count);
        assert_eq!(r.stone_count, s.count);
        let per = Gem::calibrated(GemCut::Round, 2.0).carats();
        assert!((r.total_carats - per * s.count as f64).abs() < 1e-9);
        assert!(s.bridge_mm.unwrap() > 0.0);
        // A gypsy run over the crest of a plain band sits fine.
        assert!(
            s.warnings.is_empty(),
            "unexpected warnings: {:?}",
            s.warnings
        );
    }

    #[test]
    fn a_windowed_run_keeps_only_its_arc() {
        let mut d = with_run();
        let full = report(&d, 0.0).unwrap().stone_count;
        let entry = d.layers.layers.last_mut().unwrap();
        entry.window = Window::around(90.0, 120.0);
        let half = report(&d, 0.0).unwrap().stone_count;
        assert!(half < full, "windowed {half} vs full {full}");
        assert!(half > 0);
    }

    #[test]
    fn a_bezel_on_the_crown_is_flagged_and_on_a_side_face_is_not() {
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.flatten_sides();
        let ctx = d.field_context();
        let mut seat = SeatPadLayer::default();
        seat.style = SeatStyle::Bezel;
        seat.fit_stone(Gem::calibrated(GemCut::Round, 3.0));
        seat.theta_deg = 90.0;
        seat.v_mm = ctx.crest_v_mm;
        d.layers.layers.push(LayerEntry::new("Crown bezel", Layer::SeatPad(seat)));
        let r = report(&d, 0.0).unwrap();
        assert!(
            r.seats[0].warnings.iter().any(|w| w.contains("bezel")),
            "crown bezel not flagged: {:?}",
            r.seats[0].warnings
        );

        // The same bezel on a side face passes the footing check.
        let sf = ctx.side_faces_std().expect("flat profile has side faces");
        let run = sf.low.or(sf.high).expect("a squared band has a side run");
        let mid = 0.5 * (run.0 + run.1);
        let entry = d.layers.layers.last_mut().unwrap();
        if let Layer::SeatPad(s) = &mut entry.layer {
            s.v_mm = mid;
        }
        let r = report(&d, 0.0).unwrap();
        assert_eq!(r.seats[0].footing, SeatFooting::SideFace, "{:?}", r.seats[0]);
        assert!(
            !r.seats[0].warnings.iter().any(|w| w.contains("bezel")),
            "side-face bezel flagged: {:?}",
            r.seats[0].warnings
        );
    }

    #[test]
    fn a_deep_stone_on_a_thin_band_warns_about_its_culet() {
        let mut d = RingDesign::default();
        d.profile.thickness_mm = 1.2;
        let ctx = d.field_context();
        let mut seat = SeatPadLayer::default();
        seat.height_mm = 0.3;
        seat.fit_stone(Gem::calibrated(GemCut::Princess, 5.0));
        seat.theta_deg = 90.0;
        seat.v_mm = ctx.crest_v_mm;
        d.layers.layers.push(LayerEntry::new("Big princess", Layer::SeatPad(seat)));
        let r = report(&d, 0.0).unwrap();
        assert!(
            r.seats[0].warnings.iter().any(|w| w.contains("culet")),
            "no culet warning: {:?}",
            r.seats[0].warnings
        );
    }

    #[test]
    fn a_pave_group_rolls_up_to_one_line() {
        let mut d = RingDesign::default();
        d.profile.apply_style(crate::ProfileStyle::Flat);
        d.profile.width_mm = 8.0;
        let spec = crate::pave::PaveSpec {
            region: crate::pave::PaveRegion::VBand {
                center_mm: d.field_context().crest_v_mm,
                width_mm: 5.0,
            },
            ..Default::default()
        };
        let (entry, out) = crate::pave::fill(&d, &spec).unwrap();
        d.layers.layers.push(entry);
        let r = report(&d, 0.0).unwrap();
        assert_eq!(r.seats.len(), 1, "a fill must not be a page of rows");
        assert_eq!(r.seats[0].count as usize, out.seats);
        assert_eq!(r.stone_count as usize, out.seats);
    }

    #[test]
    fn no_seats_no_report() {
        let d = RingDesign::default();
        assert!(report(&d, 0.0).is_none());
        let mut d2 = RingDesign::default();
        d2.layers.layers.push(LayerEntry {
            blend: Blend::Add,
            ..LayerEntry::new("Muted", Layer::SeatPad(SeatPadLayer::default()))
        });
        d2.layers.layers.last_mut().unwrap().enabled = false;
        assert!(report(&d2, 0.0).is_none());
        let _ = VGate::Off;
    }
}
