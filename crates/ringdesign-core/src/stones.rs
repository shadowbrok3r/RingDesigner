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
    pub warnings: Vec<String>,
}

impl SeatCheck {
    pub fn carats(&self) -> f64 {
        self.gem.map_or(0.0, |g| g.carats() * self.count as f64)
    }
}

#[derive(Clone, Debug, Default)]
pub struct StonesReport {
    pub seats: Vec<SeatCheck>,
    pub stone_count: u32,
    pub total_carats: f64,
}

impl StonesReport {
    pub fn any_warnings(&self) -> bool {
        self.seats.iter().any(|s| !s.warnings.is_empty())
    }
}

/// Every seat in the design, checked. `None` when the stack carries no seats.
///
/// `parting_z_mm` is the plane the draft numbers are signed against — pass
/// the cast report's when there is one; 0 is the crest plane every profile
/// puts its crest on by construction.
pub fn report(design: &RingDesign, parting_z_mm: f64) -> Option<StonesReport> {
    let ctx = design.field_context();
    let inner_r = design.inner_radius_mm();
    let crest_r = ctx.crest_radius_mm;

    let mut seats = Vec::new();
    walk(design, &ctx, inner_r, crest_r, parting_z_mm, &design.layers, "", &mut seats);
    if seats.is_empty() {
        return None;
    }
    let stone_count = seats.iter().filter(|s| s.gem.is_some()).map(|s| s.count).sum();
    let total_carats = seats.iter().map(|s| s.carats()).sum();
    Some(StonesReport { seats, stone_count, total_carats })
}

fn walk(
    design: &RingDesign,
    ctx: &FieldContext,
    inner_r: f64,
    crest_r: f64,
    parting_z: f64,
    stack: &LayerStack,
    prefix: &str,
    out: &mut Vec<SeatCheck>,
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
                    &[seat.theta_deg],
                    format!("{prefix}{}", entry.name),
                );
                check.count = kept as u32;
                if !kept {
                    check.warnings.push("gated out by its own window — not on the ring".into());
                }
                out.push(check);
            }
            Layer::SeatRun(run) => {
                let n = run.count.clamp(1, 200);
                let pitch = 360.0 / n as f64;
                let stations: Vec<f64> = (0..n)
                    .map(|k| k as f64 * pitch)
                    .filter(|&t| station_kept(entry, ctx, t, run.seat.v_mm))
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
                out.push(check);
            }
            Layer::Group(g) => {
                let path = format!("{prefix}{} / ", entry.name);
                walk(design, ctx, inner_r, crest_r, parting_z, &g.stack, &path, out);
            }
            _ => {}
        }
    }
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
    stations: &[f64],
    label: String,
) -> SeatCheck {
    let mut footing = SeatFooting::Crown(90.0);
    let mut worst_draft = f64::MAX;
    let mut side = true;
    let mut clearance = f64::MAX;
    let mut depth = f64::MAX;

    let probe: Vec<f64> = if stations.is_empty() { vec![seat.theta_deg] } else { stations.to_vec() };
    for &theta in &probe {
        let b = base_at(design, inner_r, crest_r, ctx, theta, seat.v_mm);
        // Side-face-ness by the same measure the side-face walk uses: how far
        // the outward normal leans along the pull.
        let lean = b.nz.abs().asin().to_degrees();
        side &= lean >= SIDE_FACE_MIN_DRAFT_DEG;
        let radial = [b.nr * theta.to_radians().cos(), b.nr * theta.to_radians().sin(), b.nz];
        worst_draft = worst_draft.min(draft_angle(radial, b.z, parting_z));

        // Foot to the nearer band edge, in reference v like the layer itself.
        let foot = seat.diameter_mm * 0.5 + seat.blend_mm.max(0.0);
        clearance = clearance
            .min(seat.v_mm - foot)
            .min(ctx.band_v_len_mm - seat.v_mm - foot);

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
