//! Stones in the cast report: per-seat bench checks and carat totals.
//!
//! Analytic, not mesh-read: the layers say where every seat is and what it
//! holds, and the base profile says what it sits on, so each check is a
//! number about the design rather than a measurement of one build of it.
//! Stones themselves are never cast — what is checked is the *stock*: the
//! pad's footing, the metal a pavilion needs, the bridge between neighbours.

use crate::castability::draft_angle;
use crate::field::{
    FieldContext, Layer, LayerEntry, LayerStack, SeatPadLayer, SeatStyle,
    SIDE_FACE_MIN_DRAFT_DEG,
};
use crate::gem::Gem;
use crate::mesh::MIN_WALL_MM;
use crate::profile::MIN_EDGE_MM;
use crate::setstone::SetStone;
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
    /// The fill floor these numbers were judged against, mm — the design's
    /// own `min_section_mm`. Carried so the sheet and the setter's map quote
    /// the same figure the census used, instead of each printing a constant.
    pub fill_floor_mm: f64,
}

impl StonesReport {
    pub fn any_warnings(&self) -> bool {
        self.seats.iter().any(|s| !s.warnings.is_empty()) || self.tight_pairs > 0
    }

    /// Metal below which the sand will not fill at all, mm.
    pub fn will_not_fill_mm(&self) -> f64 {
        self.fill_floor_mm
    }

    /// Metal the pour fills but the bench should know about, mm.
    pub fn tight_mm(&self) -> f64 {
        self.fill_floor_mm * TIGHT_MULTIPLE
    }

    /// The one line the sheet and the banner want.
    pub fn crowding_note(&self) -> Option<String> {
        let worst = self.crowding.first()?;
        (worst.worst_mm() < self.tight_mm()).then(|| {
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

/// How far above the fill floor a bridge is still worth remarking on.
///
/// The census used to judge stone-to-stone metal against a hardcoded 0.3 mm
/// and the run bridge against [`MIN_EDGE_MM`], neither of which has anything
/// to do with the sand the ring is cast in — Delft clay's own floor is 0.8 mm
/// and Petrobond's 0.6. So a 0.35 mm bridge was reported as merely "tight"
/// when the sand physically cannot fill it, which is the unsafe direction on
/// a check whose own doctrine says it is *said in the `min_section_mm`
/// voice*. Both thresholds now come off the design's floor.
pub const TIGHT_MULTIPLE: f64 = 1.5;

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
    let Acc { seats } = acc;
    if seats.is_empty() {
        return None;
    }
    // Every stone in its own right, from the one record every consumer
    // reads; the census measures them against each other.
    let stations = crate::setstone::set_stones(design);
    let stone_count = seats.iter().filter(|s| s.gem.is_some()).map(|s| s.count).sum();
    let total_carats = seats.iter().map(|s| s.carats()).sum();
    let (crowding, tight_pairs, closest) =
        crowding(design, &ctx, inner_r, crest_r, &stations);
    Some(StonesReport {
        seats,
        stone_count,
        total_carats,
        crowding,
        tight_pairs,
        closest,
        fill_floor_mm: design.draft.min_section_mm.max(0.0),
    })
}

#[derive(Default)]
struct Acc {
    seats: Vec<SeatCheck>,
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
    let fill_floor = design.draft.min_section_mm.max(0.0);
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
                acc.seats.push(check);
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let stations: Vec<(f64, f64)> = (0..n)
                    .map(|k| (run.theta_of_station(k as f64, ctx), run.seat.v_mm))
                    .filter(|&(t, v)| station_kept(entry, ctx, t, v))
                    .collect();
                let mut seat = run.seat;
                seat.fit_stone(run.gem);
                let seat = run.turned(seat);
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
                if bridge < fill_floor {
                    check.warnings.push(format!(
                        "bridge {bridge:.2} mm between stones will not fill (this sand fills {fill_floor:.2} mm)"
                    ));
                } else if bridge < fill_floor * TIGHT_MULTIPLE {
                    check.warnings.push(format!(
                        "bridge {bridge:.2} mm is tight — {:.2} mm is safer in this sand",
                        fill_floor * TIGHT_MULTIPLE
                    ));
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
                acc.seats.push(check);
            }
            Layer::Group(g) => {
                // A seat group — a pavé fill — rolls up to one line per seat
                // shape: two hundred rows of "Seat 137" is not a report.
                if let Some(shapes) = seats_by_shape(entry, &g.stack, ctx) {
                    let many = shapes.len() > 1;
                    for (k, (seat, stations)) in shapes.iter().enumerate() {
                        if stations.is_empty() {
                            continue;
                        }
                        let label = if many {
                            format!("{prefix}{} ({})", entry.name, k + 1)
                        } else {
                            format!("{prefix}{}", entry.name)
                        };
                        let mut check = check_seat(
                            design, ctx, inner_r, crest_r, parting_z, seat, stations, label,
                        );
                        check.count = stations.len() as u32;
                        acc.seats.push(check);
                    }
                    continue;
                }
                let path = format!("{prefix}{} / ", entry.name);
                walk(design, ctx, inner_r, crest_r, parting_z, &g.stack, &path, acc);
            }
            _ => {}
        }
    }
}

/// Every stone the design sets with its girdle frame, in the order the
/// record lists them.
pub fn stone_frames(design: &RingDesign) -> Vec<(SetStone, StoneFrame)> {
    let ctx = design.field_context();
    let inner_r = design.inner_radius_mm();
    let crest_r = ctx.crest_radius_mm;
    crate::setstone::set_stones(design)
        .into_iter()
        .map(|st| {
            let f = frame_at(design, &ctx, inner_r, crest_r, &st);
            (st, f)
        })
        .collect()
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
    stations: &[SetStone],
) -> (Vec<StonePair>, usize, Option<StonePair>) {
    if stations.len() < 2 {
        return (Vec::new(), 0, None);
    }
    let frames: Vec<StoneFrame> = stations
        .iter()
        .map(|st| frame_at(design, ctx, inner_r, crest_r, st))
        .collect();

    // Both thresholds come off the sand the design is judged for, not off a
    // constant: Delft fills 0.8 mm and Petrobond 0.6, and the default is 0.7.
    let tight_floor = design.draft.min_section_mm.max(0.0) * TIGHT_MULTIPLE;
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
                && floor > tight_floor
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
            if pair.worst_mm() >= tight_floor {
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
/// A stone's girdle in space: where the census measures it and where the
/// map draws it.
#[derive(Clone, Debug)]
pub struct StoneFrame {
    pub girdle: [f64; 3],
    pub normal: [f64; 3],
    /// The stone's long axis and its short one, in world space.
    pub long: [f64; 3],
    pub short: [f64; 3],
    pub semi: (f64, f64),
    pub plan_pow: f64,
    /// The furthest the girdle reaches from its centre, mm.
    pub reach: f64,
    pub pavilion: f64,
}

impl StoneFrame {
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
    st: &SetStone,
) -> StoneFrame {
    let b = base_at(design, inner_r, crest_r, ctx, st.theta_deg, st.v_mm);
    let (sin, cos) = st.theta_deg.to_radians().sin_cos();
    let normal = [b.nr * cos, b.nr * sin, b.nz];
    let girdle = [
        b.r * cos + normal[0] * st.stand_off_mm(),
        b.r * sin + normal[1] * st.stand_off_mm(),
        b.z + normal[2] * st.stand_off_mm(),
    ];
    // The band's own two tangents: along the ring, and across the section.
    let t = [-sin, cos, 0.0];
    let across = [-b.nz * cos, -b.nz * sin, b.nr];
    let (rs, rc) = st.rot_deg().to_radians().sin_cos();
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
    StoneFrame {
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

/// A group of seats, rolled up by shape: one entry per distinct seat, with
/// the `(theta, v)` stations it stands at. `None` when the group holds
/// anything that is not a seat.
///
/// By shape and not by one prototype, because a pinned seat carrying a
/// different stone is the ordinary case now — demanding a single prototype
/// would collapse the rollup and print two hundred rows of "Seat 137",
/// which is not a report.
fn seats_by_shape<'a>(
    entry: &LayerEntry,
    stack: &'a LayerStack,
    ctx: &FieldContext,
) -> Option<Vec<(&'a SeatPadLayer, Vec<(f64, f64)>)>> {
    let mut out: Vec<(&SeatPadLayer, Vec<(f64, f64)>)> = Vec::new();
    for e in &stack.layers {
        if !e.enabled {
            continue;
        }
        let Layer::SeatPad(s) = &e.layer else { return None };
        let same = |p: &SeatPadLayer| {
            p.gem == s.gem
                && p.style == s.style
                && (p.diameter_mm - s.diameter_mm).abs() < 1e-9
                && (p.elong - s.elong).abs() < 1e-9
                && (p.rot_deg - s.rot_deg).abs() < 1e-9
                && (p.height_mm - s.height_mm).abs() < 1e-9
        };
        let slot = match out.iter().position(|(p, _)| same(p)) {
            Some(k) => k,
            None => {
                out.push((s, Vec::new()));
                out.len() - 1
            }
        };
        if station_kept(entry, ctx, s.theta_deg, s.v_mm) {
            out[slot].1.push((s.theta_deg, s.v_mm));
        }
    }
    // A single seat is an ordinary pad, not a group worth rolling up.
    let stations: usize = out.iter().map(|(_, v)| v.len()).sum();
    (stations > 1).then_some(out)
}

/// Whether a station survives the entry's angular window.
fn station_kept(entry: &LayerEntry, ctx: &FieldContext, theta_deg: f64, v_mm: f64) -> bool {
    crate::setstone::kept(entry, ctx, theta_deg, v_mm)
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

        // Foot to the nearer band edge, on the section as modulated at this
        // station: a widened top carries a seat the reference band could not.
        // An elongated seat reaches across the band by its own `v` extent,
        // which is its length when the stone is turned to face the edges.
        // The section arcs are metal mm, so the foot reads metal mm too.
        let foot = seat.metal_foot_v_mm(ctx, theta);
        clearance = clearance
            .min(b.along - foot)
            .min(b.surface_len - b.along - foot);

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

/// Height the girdle sits above the base surface, mm — the pad's stand-off
/// less how deep the stone is set into it. A seat with no stone assigned is
/// judged on the one it says it could hold.
/// Height of the girdle over the bare band for the seat's own stone, or a
/// round suggested by its size when it carries none.
fn stand_off(seat: &SeatPadLayer) -> f64 {
    let gem = seat.gem.unwrap_or_else(|| {
        crate::gem::Gem::calibrated(crate::gem::GemCut::Round, seat.suggested_stone_mm())
    });
    seat.stand_off_mm(gem)
}

struct BasePoint {
    r: f64,
    z: f64,
    nr: f64,
    nz: f64,
    /// Band width at this station, mm.
    width: f64,
    /// Arc from the low band edge to this point along the station's own
    /// section, and that section's whole surface arc, mm.
    along: f64,
    surface_len: f64,
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
        Some(p) => BasePoint { r: p.r, z: p.z, nr: p.nr, nz: p.nz, width: (hi - lo).max(0.0), along: target, surface_len: l.surface_len_mm },
        None => BasePoint { r: inner_r, z: 0.0, nr: 1.0, nz: 0.0, width: 0.0, along: v_mm, surface_len: ctx.band_v_len_mm },
    }
}

#[cfg(test)]
mod tests {
    /// The chart's `v` is arc normalized, so a pad on a keyframed lobe casts
    /// the station's stretch times its drawn size, and the report used to
    /// read its foot in chart mm — measured on a lofted head, a 1.5 mm boss
    /// mid-wall cast 2.6 mm tall while the clearance printed a chart figure.
    /// A metal-true pad casts as drawn; either way the clearance and the DFM
    /// footprint now speak metal mm.
    #[test]
    fn a_pad_on_a_stretched_lobe_is_judged_in_metal_mm() {
        use crate::field::{Layer, LayerEntry, Uv};
        use crate::profile::{ShankKey, ShankKind};

        let build = |metal_true: bool| {
            let mut d = crate::RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::Flat);
            d.profile.width_mm = 6.0;
            d.profile.thickness_mm = 2.0;
            d.shank.kind = ShankKind::Keyframes;
            d.shank.amount = 1.0;
            d.shank.keys = vec![
                ShankKey { theta_deg: 90.0, width_scale: 1.5, thickness_scale: 1.2, crown_scale: 1.0 },
                ShankKey { theta_deg: 270.0, width_scale: 1.0, thickness_scale: 1.0, crown_scale: 1.0 },
            ];
            let ctx = d.field_context();
            let mut pad = SeatPadLayer::default();
            pad.theta_deg = 90.0;
            pad.v_mm = ctx.crest_v_mm;
            pad.diameter_mm = 2.0;
            pad.height_mm = 0.8;
            pad.blend_mm = 0.5;
            pad.metal_true = metal_true;
            d.layers.layers.push(LayerEntry::new("Boss", Layer::SeatPad(pad)));
            d
        };

        let d = build(true);
        let ctx = d.field_context();
        let inner = d.inner_radius_mm();
        let m = d.modulation_at(90.0, inner, inner + d.profile.thickness_mm);
        let k_true = d.profile.sample_mod(inner, 512, &m).surface_len_mm / ctx.band_v_len_mm;
        assert!(k_true > 1.3, "the lobe stretches the section: {k_true}");
        let k = ctx.station_stretch(90.0);
        assert!(
            (k - k_true).abs() < 0.02 * k_true,
            "the table reads the true stretch: {k} against {k_true}"
        );

        // Physical reach across the band: the chart span the pad raises,
        // read back in the lobe's own millimetres.
        let lib = crate::AlphaLibrary::builtin();
        let span_mm = |d: &crate::RingDesign| {
            let ctx = d.field_context();
            let u0 = ctx.u_of_theta(90.0);
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for i in 0..=4096 {
                let v = ctx.band_v_len_mm * i as f64 / 4096.0;
                if d.layers.height(Uv { u: u0, v }, &ctx, &lib) > 1e-4 {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            (hi - lo) * k_true
        };
        let drawn = 2.0 + 2.0 * 0.5;
        let true_span = span_mm(&d);
        assert!(
            (true_span - drawn).abs() < 0.1,
            "a metal-true pad casts its drawn reach: {true_span:.3} against {drawn}"
        );
        let chart = build(false);
        let chart_span = span_mm(&chart);
        assert!(
            (chart_span - drawn * k_true).abs() < 0.1,
            "a chart-drawn pad casts the stretch times its numbers: {chart_span:.3} against {:.3}",
            drawn * k_true
        );

        // The clearance figure is metal mm for both: the modulated section's
        // own arc to the edge, less the foot's reach in the same millimetres.
        let len = d.profile.sample_mod(inner, 192, &m).surface_len_mm;
        let along = ctx.crest_v_mm / ctx.band_v_len_mm * len;
        let near = along.min(len - along);
        let clearance =
            |d: &crate::RingDesign| super::report(d, 0.0).unwrap().seats[0].edge_clearance_mm;
        let c_true = clearance(&d);
        let c_chart = clearance(&chart);
        assert!(
            (c_true - (near - 1.5)).abs() < 0.02,
            "a metal-true foot reaches its drawn 1.5 mm: {c_true:.3} against {:.3}",
            near - 1.5
        );
        assert!(
            (c_chart - (near - 1.5 * k)).abs() < 0.02,
            "a chart foot reaches k times further: {c_chart:.3} against {:.3}",
            near - 1.5 * k
        );
        assert!(c_chart < c_true - 0.5, "the two reads differ by the stretch");

        // The footprint carries the station's stretch, so the detail floor
        // judges the skirt in metal — and a waist, where the chart overstates
        // the metal, raises a finding the chart figure alone never would.
        assert!((d.layers.layers[0].layer.feature_footprints(&ctx)[0].v_stretch - k).abs() < 1e-9);
        let mut waist = build(false);
        waist.shank.keys[0].width_scale = 0.55;
        waist.shank.keys[0].thickness_scale = 0.8;
        waist.draft.min_detail_mm = 0.4;
        if let Layer::SeatPad(p) = &mut waist.layers.layers[0].layer {
            p.blend_mm = 0.45;
        }
        let wctx = waist.field_context();
        assert!(wctx.station_stretch(90.0) < 0.85, "the waist compresses: {}", wctx.station_stretch(90.0));
        let f = waist.layers.layers[0].layer.feature_footprints(&wctx)[0];
        assert!(f.min_feature_mm() >= waist.draft.min_detail_mm, "the chart figure passes the floor");
        assert!(f.metal_feature_mm(&wctx) < waist.draft.min_detail_mm, "the metal figure does not");
        assert!(!crate::dfm::findings(&waist).is_empty(), "and the finding says so");
    }

    /// The census is a *fill* rule — a thin bridge does not lock the mould, it
    /// comes out of the flask as two stones sharing a hole — so it has to move
    /// with the sand. It used to be a hardcoded 0.3 mm, which is under every
    /// sand this shop pours and so reported "tight" where the metal will not
    /// go at all.
    #[test]
    fn the_census_moves_with_the_sand() {
        use crate::castability::SandProcess;
        use crate::field::{Layer, LayerEntry, SeatRunLayer};
        use crate::gem::{Gem, GemCut};

        let build = |sand: SandProcess| {
            let mut d = crate::RingDesign::default();
            d.profile.apply_style(crate::ProfileStyle::LowDome);
            sand.apply(&mut d.draft);
            let ctx = d.field_context();
            let mut run = SeatRunLayer::default();
            run.gem = Gem::calibrated(GemCut::Emerald, 2.5);
            run.seat.v_mm = ctx.crest_v_mm;
            run.count = 16;
            run.seat.fit_stone(run.gem);
            d.layers.layers.push(LayerEntry::new("Step row", Layer::SeatRun(run)));
            super::report(&d, 0.0).unwrap()
        };

        let coarse = build(SandProcess::Petrobond);
        let fine = build(SandProcess::DelftClay);
        assert_eq!(coarse.fill_floor_mm, 0.6);
        assert_eq!(fine.fill_floor_mm, 0.8);
        // Identical geometry, different sand: the finer sand's higher fill
        // floor has to make the same row read tighter, not the same.
        assert!(
            fine.tight_pairs >= coarse.tight_pairs,
            "Delft ({} pairs at a {:.2} mm floor) cannot be more forgiving than \
             Petrobond ({} at {:.2})",
            fine.tight_pairs,
            fine.fill_floor_mm,
            coarse.tight_pairs,
            coarse.fill_floor_mm
        );
        assert!(fine.tight_mm() > coarse.tight_mm());
        // And the sheet's heading and the census must quote one number.
        assert_eq!(fine.tight_mm(), fine.fill_floor_mm * super::TIGHT_MULTIPLE);
    }

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

    /// How deep a stone is set was two numbers that disagreed: the preview
    /// sank the girdle by a fraction of the stone's depth while the report
    /// credited the whole pad height as metal under it. One authored number
    /// now, and both read it.
    #[test]
    fn how_deep_the_stone_sits_is_one_number() {
        use crate::field::{SeatPadLayer, SeatStyle};

        let gem = Gem::calibrated(GemCut::Round, 3.0);
        let mut boss = SeatPadLayer { style: SeatStyle::Boss, height_mm: 1.2, ..Default::default() };
        boss.fit_stone(gem);
        // A drilled pad takes the stone a whisker in; the report credits the
        // pad height less that, not the whole of it.
        let drop = boss.girdle_drop_mm(gem);
        assert!(drop > 0.0 && drop < boss.height_mm, "{drop}");
        assert!((boss.stand_off_mm(gem) - (boss.height_mm - drop)).abs() < 1e-12);
        assert!((stand_off(&boss) - boss.stand_off_mm(gem)).abs() < 1e-12);

        // A bezel's girdle lands on its pocket floor, by construction.
        let mut bezel = SeatPadLayer {
            style: SeatStyle::Bezel,
            height_mm: 1.2,
            recess_mm: 0.4,
            ..Default::default()
        };
        bezel.fit_stone(gem);
        assert!((bezel.girdle_drop_mm(gem) - 0.4).abs() < 1e-12);

        // A cabochon rests on its bed.
        let cab = Gem::cabochon(GemCut::Round, 3.0);
        let mut bed = SeatPadLayer { style: SeatStyle::GypsyMound, ..Default::default() };
        bed.fit_stone(cab);
        assert_eq!(bed.girdle_drop_mm(cab), 0.0);

        // Authored outright, and never past the pad's own top.
        let mut set = boss;
        set.set_depth_mm = Some(5.0);
        assert!((set.girdle_drop_mm(gem) - set.height_mm).abs() < 1e-12);
        set.set_depth_mm = Some(-1.0);
        assert_eq!(set.girdle_drop_mm(gem), 0.0);
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
        // Chord at the girdle radius, less both stones' half-widths. The
        // girdle rides the pad's stand-off, which is its height less how
        // deep the stone is set into it.
        let seat = pad(90.0);
        let girdle_r = ctx.crest_radius_mm + seat.stand_off_mm(seat.gem.unwrap());
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
        // The subject here is the arc loss, not the threshold: the ring's own
        // curvature closes the gap between the girdle and the culet, and the
        // culet is the one that decides. Measured on this row, 0.51 mm at the
        // girdle against 0.26 at depth — and against a real sand floor of
        // 0.70 mm *both* are tight, which is the whole point of judging them
        // against the process instead of against 0.3.
        assert!(hit.gap_mm > hit.gap_deep_mm, "the arc closes: {:.3} -> {:.3}", hit.gap_mm, hit.gap_deep_mm);
        assert!(hit.gap_deep_mm < r.will_not_fill_mm(), "at depth: {:.3}", hit.gap_deep_mm);
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

        let ctx = d.field_context();
        let graded: f64 = (0..n)
            .map(|k| run.gem_at(run.theta_of_station(k as f64, &ctx)).carats())
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

        // Graduation scales the posts with their stones. The far boundary
        // is wherever the station warp now puts it, not at a uniform pitch.
        let mut graded = run;
        graded.taper = 0.5;
        let half = graded.count as f64 * 0.5;
        let theta_far = graded.theta_of_station(half.floor() + 0.5, &ctx);
        let far = Uv {
            u: ctx.u_of_theta(theta_far.rem_euclid(360.0)),
            v: ctx.crest_v_mm + off * graded.scale_at(theta_far),
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
        // The one warning it does carry is the truth this check was blind to
        // while the threshold was a hardcoded 0.3 mm: `SeatRunLayer::default()`
        // asks for a 0.4 mm bridge, and no sand this shop pours fills that —
        // Delft is 0.8, Petrobond 0.6, the default 0.7. The default row is
        // asking for metal the process cannot give it.
        assert_eq!(s.warnings.len(), 1, "{:?}", s.warnings);
        assert!(
            s.warnings[0].contains("will not fill"),
            "the default bridge is under the sand's floor: {:?}",
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
