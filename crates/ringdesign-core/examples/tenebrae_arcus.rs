//! Tenebrae — Arcus, the flying buttress: a cathedral solitaire whose shoulder arches are flyers, each springing from a
//! pinnacled pier on the band to the sapphire's basket, the choir. Lost wax, Gold 18k.
//! cargo build --release -p ringdesign-core --example tenebrae_arcus
//! target/release/examples/tenebrae_arcus [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, Boolean, Component, ComponentRole, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, Profile, Stage, builders, pattern::Sources},
    castability::CastProcess,
    csg, dfm, library, manufacturing as mf, mesh,
    gem::{Gem, GemCut},
    profile::{ProfileStyle, ShankKind},
    render,
    setting::{self, RowPath, Stamp, StampRow, StampTop},
    sketch::{Geometry, Id, Sketch},
    stl,
};
use serde_json::json;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
const ALLOY: &str = "Gold 18k";
/// The flyers' spread either side of the stone: where each pier stands round the ring.
const SPREAD_DEG: f64 = 50.0;
/// How far the choir lifts the sapphire over its plain basket stand-off, so the flyers have height to leap.
const CHOIR_LIFT_MM: f64 = 2.2;
/// The pier, stood upright (world-vertical) on the shoulder, in its frame (x across the finger, y out from the choir,
/// z up, its origin on the band's nominal surface at its axis): its square shaft, and its top.
const SHAFT_MM: f64 = 1.8;
const PIER_H_MM: f64 = 6.8;
/// The pier's corners rounded in plan, so its seam with the band turns no corner.
const PIER_ROUND_MM: f64 = 0.3;
/// The plinth: how far it runs out past the shaft's outer and inner faces at its set-off, how much further its foot
/// spreads, and how far its set-off stands over the band at its inner end.
const PLINTH_OUT_MM: f64 = 0.35;
const PLINTH_IN_MM: f64 = 0.7;
const PLINTH_BATTER_MM: f64 = 0.35;
const PLINTH_OVER_MM: f64 = 0.25;
/// How far the pier's foot sinks under the band's surface along its sloping bed.
const PIER_SINK_MM: f64 = 0.5;
/// The blind lancet on the shaft's outward face: its width, its sill over the plinth, its apex under the shaft's top, its depth.
const NICHE_W_MM: f64 = 0.6;
const NICHE_SILL_MM: f64 = 0.5;
const NICHE_HEAD_MM: f64 = 0.6;
const NICHE_DEEP_MM: f64 = 0.35;
/// The spire: square and turned a quarter on the shaft, across its arrises at its foot (a little over the shaft's
/// width) and at its point under the finial, and its height.
const SPIRE_FOOT_MM: f64 = 2.1;
const SPIRE_TOP_MM: f64 = 0.3;
const SPIRE_H_MM: f64 = 4.0;
/// The crockets: hooked leaves on the spire's four arrises at three heights (shares of the spire's height), each
/// standing out this far, as thick across as the investment's floor, stopping short of the tip.
const CROCKET_AT: [f64; 3] = [0.14, 0.4, 0.64];
const CROCKET_OUT_MM: f64 = 0.42;
/// The leaf's height over its unit drawing.
const CROCKET_TALL: f64 = 0.85;
/// The finial: a pointed knop on a collar over the spire's top, lofted through (height over the spire's top, radius).
const COLLAR_MM: f64 = 0.4;
const KNOP: [(f64, f64); 4] = [(0.1, 0.28), (0.4, 0.45), (0.75, 0.32), (1.25, 0.04)];
/// The flyer: a rib across the finger `RIB_MM` thick. Its extrados rakes straight from `RIB_UNDER_TOP_MM` under the
/// pier's top up to the choir's upper rail; its intrados is a segmental arch from `SPRING_MM` up the pier's inner face
/// to the lower rail, its crown `RIB_SAG_MM` over its chord. It is never shallower than `RIB_MIN_MM`, and both ends
/// sink into what they meet.
const RIB_MM: f64 = 1.2;
const SPRING_MM: f64 = 3.0;
const RIB_UNDER_TOP_MM: f64 = 0.25;
const RIB_SAG_MM: f64 = 1.4;
const RIB_MIN_MM: f64 = 0.9;
const RIB_INTO_PIER_MM: f64 = 0.4;
const RIB_INTO_RAIL_MM: f64 = 0.3;
/// The nave's blind arcade on both side faces, from pier to pier through the palm: lancets cut into the side face.
const ARCADE_FROM_DEG: f64 = 156.0;
const ARCADE_TO_DEG: f64 = 384.0;
const ARCADE_BAYS: u32 = 28;
const ARCADE_W_MM: f64 = 0.85;
const ARCADE_H_MM: f64 = 1.75;
const ARCADE_DEEP_MM: f64 = 0.45;
/// The seam bead where each pier meets the band.
const SEAM_MM: f64 = 0.15;
/// Investment fill floor.
const MIN_SECTION_MM: f64 = 0.8;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// The sapphire: an emerald cut, 6 × 8 mm, Chartres blue.
fn sapphire() -> Gem {
    Gem { preview_tint: Some([0.02, 0.06, 0.45]), ..Gem { l_mm: 8.0, ..Gem::calibrated(GemCut::Emerald, 6.0) } }
}

struct Tree {
    doc: Document,
}

impl Tree {
    fn next(&self) -> Id {
        self.doc.features.len() as Id + 1
    }
    fn add(&mut self, name: &str, operation: Operation, role: ComponentRole) -> Id {
        let id = self.next();
        self.push(Feature { id, name: name.into(), enabled: true, operation, component: Component { role, ..Component::default() } })
    }
    fn push(&mut self, mut f: Feature) -> Id {
        if !f.component.reference {
            f.component.material = ALLOY.into();
        }
        let id = f.id;
        self.doc.append(f).expect("feature appends");
        id
    }
    fn sketch(&mut self, name: &str, sketch: Sketch) -> Id {
        self.add(name, Operation::Sketch { sketch }, ComponentRole::Other)
    }
    fn extrude(&mut self, name: &str, sketch: Id, height_mm: f64, draft_deg: f64) -> Id {
        self.add(name, Operation::Extrude { sketch: Profile::Feature { feature: sketch }, height_mm, draft_deg }, ComponentRole::Other)
    }
}

fn polar(theta_deg: f64, r: f64) -> P2 {
    let a = theta_deg.to_radians();
    [r * a.cos(), r * a.sin()]
}

/// A sketch of closed polylines on the plane through `origin` spanned by `x` and `y`.
fn drawn(name: &str, origin: P3, x: P3, y: P3, loops: &[Vec<P2>]) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    s.plane.origin = origin;
    s.plane.x = x;
    s.plane.y = y;
    for ring in loops {
        let ids: Vec<Id> = ring.iter().map(|p| s.point(*p)).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s
}

/// A rectangle `x` across the finger and from `y0` to `y1` along the pier's outward axis, corners of radius `r`.
fn rounded(x: f64, y0: f64, y1: f64, r: f64) -> Vec<P2> {
    let k = 6;
    let hx = x / 2.0;
    let corners = [([hx - r, y1 - r], 0.0), ([-hx + r, y1 - r], 90.0), ([-hx + r, y0 + r], 180.0), ([hx - r, y0 + r], 270.0)];
    let mut pts: Vec<P2> = Vec::new();
    for (c, from) in corners {
        for i in 0..=k {
            let a = (from + 90.0 * i as f64 / k as f64).to_radians();
            pts.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    pts
}

/// A level plan at height `z` in the pier's frame.
fn plan_at(name: &str, z: f64, ring: Vec<P2>) -> Sketch {
    drawn(name, [0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], &[ring])
}

/// An outline on the pier's y–z plane (y out from the choir, z up), set `back` behind it across the finger.
fn elevation(name: &str, back: f64, loops: &[Vec<P2>]) -> Sketch {
    drawn(name, [-back, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], loops)
}

/// A lancet's outline: a flat sill `w` wide at `from`, straight jambs and an equilateral pointed head with its apex at
/// `to`, walked in short chords, `x` across it and `y` up it.
fn lancet(w: f64, from: f64, to: f64) -> Vec<P2> {
    let h = w / 2.0;
    let spring = to - w * 3f64.sqrt() / 2.0;
    let mut pts = vec![[-h, from], [h, from], [h, spring]];
    let steps = 10;
    for i in 1..steps {
        let a = std::f64::consts::PI / 3.0 * i as f64 / steps as f64;
        pts.push([-h + w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([0.0, to]);
    for i in 1..steps {
        let a = std::f64::consts::PI / 3.0 * (steps - i) as f64 / steps as f64;
        pts.push([h - w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([-h, spring]);
    pts
}

/// The right pier's frame: seated on the band's nominal surface at `theta` and leaned upright, so its z is the world's
/// up and its y points out, away from the choir.
struct PierFrame {
    origin: P2,
}

impl PierFrame {
    fn of(d: &RingDesign, theta: f64) -> Self {
        Self { origin: polar(theta, d.inner_radius_mm() + d.profile.thickness_mm) }
    }
    fn to_local(&self, w: P2) -> P2 {
        [-(w[0] - self.origin[0]), w[1] - self.origin[1]]
    }
    fn placement(&self, theta: f64) -> Placement {
        Placement::Ring { theta_deg: theta, across_mm: 0.0, height_mm: 0.0, spin_deg: 0.0, tilt_deg: theta - 90.0, cant_deg: 0.0 }
    }
}

/// The band's surface under the upright pier, as a height in its frame at `y`, read on the nominal section.
fn bed_z(d: &RingDesign, f: &PierFrame, y: f64) -> f64 {
    let r = d.inner_radius_mm() + d.profile.thickness_mm;
    let x = f.origin[0] - y;
    (r * r - x * x).max(0.0).sqrt() - f.origin[1]
}

/// The choir's two rails where the flyer meets them, in the arches' plane on the right of the crown, in world
/// millimetres: the upper rail's outer face and top, and the lower rail's outer face and middle.
struct Rails {
    upper: P2,
    lower: P2,
}

fn rails(d: &RingDesign, head: &str) -> Result<Rails> {
    let e = cad::evaluate(d, &AlphaLibrary::builtin(), draft_params())?;
    let c = e.components.iter().find(|c| c.name == head).context("the choir was built")?;
    let mut near: Vec<P2> = c.mesh.vertices.iter().filter(|v| (v.2 as f64).abs() < 0.3 && v.0 < 0.0).map(|v| [v.0 as f64, v.1 as f64]).collect();
    ensure!(near.len() > 8, "the choir's rails were not found in the arches' plane");
    near.sort_by(|a, b| a[1].total_cmp(&b[1]));
    // The two rails are the two runs of heights with a gap between them.
    let split = near.windows(2).enumerate().max_by(|a, b| (a.1[1][1] - a.1[0][1]).total_cmp(&(b.1[1][1] - b.1[0][1]))).map(|(i, _)| i + 1).unwrap();
    let (low, high) = near.split_at(split);
    let outer = |run: &[P2]| run.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
    let (lo_min, lo_max) = (low[0][1], low[low.len() - 1][1]);
    Ok(Rails { upper: [outer(high), high[high.len() - 1][1]], lower: [outer(low), 0.5 * (lo_min + lo_max)] })
}

/// The flyer's elevation in the pier's frame: a straight raking extrados from just under the pier's top up to the top
/// of the choir's upper rail, over a segmental intrados springing from low on the pier's inner face and landing on the
/// lower rail, its crown standing `RIB_SAG_MM` over its chord; both ends sink into what they meet.
fn rib_outline(upper: P2, lower: P2) -> Result<Vec<P2>> {
    let ys = -SHAFT_MM / 2.0 + RIB_INTO_PIER_MM;
    let ye = upper[0].min(lower[0]) - RIB_INTO_RAIL_MM;
    let s0 = [ys, SPRING_MM];
    let e0 = [ye, lower[1]];
    let (cx, cz) = (e0[0] - s0[0], e0[1] - s0[1]);
    let l = cx.hypot(cz);
    let h = RIB_SAG_MM;
    let radius = (l * l / 4.0 + h * h) / (2.0 * h);
    // The bulge's side of the chord is up; the centre stands under the chord's middle.
    let n = [cz / l, -cx / l];
    let n = if n[1] > 0.0 { n } else { [-n[0], -n[1]] };
    let mid = [(s0[0] + e0[0]) / 2.0, (s0[1] + e0[1]) / 2.0];
    let c = [mid[0] - n[0] * (radius - h), mid[1] - n[1] * (radius - h)];
    let a0 = (s0[1] - c[1]).atan2(s0[0] - c[0]);
    let mut a1 = (e0[1] - c[1]).atan2(e0[0] - c[0]);
    if a1 < a0 {
        a1 += std::f64::consts::TAU;
    }
    let k = 40;
    let intrados: Vec<P2> = (0..=k).map(|i| a0 + (a1 - a0) * i as f64 / k as f64).map(|a| [c[0] + radius * a.cos(), c[1] + radius * a.sin()]).collect();
    let (t0, t1) = ([ys, PIER_H_MM - RIB_UNDER_TOP_MM], [ye, upper[1] - 0.05]);
    let line_at = |y: f64| t0[1] + (t1[1] - t0[1]) * (y - t0[0]) / (t1[0] - t0[0]);
    let least = intrados.iter().map(|p| line_at(p[0]) - p[1]).fold(f64::MAX, f64::min);
    ensure!(least >= RIB_MIN_MM, "the flyer is {least:.2} mm deep at its thinnest, under its {RIB_MIN_MM} mm floor");
    let mut ring = vec![t0, t1];
    ring.extend(intrados.iter().rev());
    ring.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-3);
    Ok(ring)
}

/// A crocket's outline in the plane through the spire's axis and one arris (s out along the arris' bearing, z up): a
/// leaf rooted in the arris at `rho`, swelling out and curling up to a turned tip, its root kept clear of the axis so
/// the four arrises' leaves never meet.
fn crocket(rho: f64, z: f64) -> Vec<P2> {
    let root = (rho - 0.25).max(MIN_SECTION_MM / 2.0 + 0.02);
    let o = CROCKET_OUT_MM;
    let raw: Vec<P2> = [
        [root, -0.38],
        [rho + 0.2 * o, -0.34],
        [rho + 0.65 * o, -0.18],
        [rho + 0.95 * o, 0.05],
        [rho + 1.0 * o, 0.3],
        [rho + 0.8 * o, 0.48],
        [rho + 0.55 * o, 0.4],
        [rho + 0.5 * o, 0.22],
        [rho + 0.25 * o, 0.12],
        [root, 0.12],
    ]
    .into_iter()
    .map(|[s, dz]| [s, z + CROCKET_TALL * dz])
    .collect();
    // Two rounds of corner cutting, so the leaf's outline runs smooth.
    let mut pts = raw;
    for _ in 0..2 {
        let n = pts.len();
        pts = (0..n).flat_map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            [[0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]], [0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]]
        }).collect();
    }
    pts
}

/// The spire's arris at height share `t`: its distance out from the axis along the diagonal.
fn arris(t: f64) -> f64 {
    SPIRE_FOOT_MM / 2.0 + (SPIRE_TOP_MM - SPIRE_FOOT_MM) / 2.0 * t
}

/// One side's pinnacled pier and its flyer, built in the pier's own upright frame and seated at `theta`.
fn pinnacle(t: &mut Tree, d: &RingDesign, choir: &Rails, theta: f64) -> Result<Vec<Id>> {
    let f = PierFrame::of(d, theta);
    let hy = SHAFT_MM / 2.0;
    let (out_y, in_y) = (hy + PLINTH_OUT_MM, -hy - PLINTH_IN_MM);
    let hx = hy;
    let slope = (bed_z(d, &f, 0.05) - bed_z(d, &f, -0.05)) / 0.1;
    let deep = bed_z(d, &f, 0.0) - PIER_SINK_MM;
    let top = bed_z(d, &f, in_y) + PLINTH_OVER_MM;
    let r = PIER_ROUND_MM;
    // The plinth: a block drawn well under the shoulder, battered in to its set-off, its foot cut along the shoulder.
    let low = deep - 3.0;
    let batter = (PLINTH_BATTER_MM / (top - low)).atan().to_degrees();
    let b = PLINTH_BATTER_MM;
    let plan = t.sketch("Draw the plinth's plan under the shoulder", plan_at("Plinth plan", low, rounded(2.0 * hx + 0.2 + 2.0 * b, in_y - b, out_y + b, r + b)));
    let block = t.extrude("Raise the plinth, battered in to its set-off", plan, top - low, batter);
    let bed = t.add("Shape the bed under the shoulder", Operation::Box { size: [6.0, 8.0, 6.0] }, ComponentRole::Other);
    let tilt = slope.atan();
    let laid = t.add(
        "Lay the bed along the shoulder's slope, sunk under it",
        Operation::Transform { source: bed, translation: [0.0, 3.0 * tilt.sin(), deep - 3.0 * tilt.cos()], rotation_deg: [tilt.to_degrees(), 0.0, 0.0] },
        ComponentRole::Other,
    );
    let plinth = t.add("Cut the plinth's foot along the shoulder", Operation::Boolean { a: block, b: laid, kind: Boolean::Subtract }, ComponentRole::Other);
    // The shaft, with a blind lancet sunk in its outward face.
    let shaft_s = t.sketch("Draw the pier's shaft on the plinth", plan_at("Pier shaft", top - 0.05, rounded(2.0 * hx, -hy, hy, r)));
    let shaft = t.extrude("Raise the pier's shaft", shaft_s, PIER_H_MM - top + 0.05, 0.0);
    let niche_s = t.sketch(
        "Draw a blind lancet on the shaft's outward face",
        drawn("Pier lancet", [0.0, hy + 0.1, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], &[lancet(NICHE_W_MM, top + NICHE_SILL_MM, PIER_H_MM - NICHE_HEAD_MM)]),
    );
    let niche = t.extrude("Sink the lancet into the shaft", niche_s, 0.1 + NICHE_DEEP_MM, 0.0);
    let shaft = t.add("Cut the blind lancet", Operation::Boolean { a: shaft, b: niche, kind: Boolean::Subtract }, ComponentRole::Other);
    // The spire, its crockets and its finial.
    // Turned a quarter-square on the shaft, so its arrises stand in the silhouette and the crockets show in profile.
    let diamond = |reach: f64, r: f64| -> Vec<P2> {
        let half = (reach - r) * std::f64::consts::FRAC_1_SQRT_2 + r;
        rounded(2.0 * half, -half, half, r).into_iter().map(|[x, y]| [(x - y) * std::f64::consts::FRAC_1_SQRT_2, (x + y) * std::f64::consts::FRAC_1_SQRT_2]).collect()
    };
    let foot = t.sketch("Draw the spire's foot on the shaft's top, turned to stand on its arrises", plan_at("Spire foot", PIER_H_MM - 0.05, diamond(SPIRE_FOOT_MM / 2.0, r)));
    let tip = t.sketch("Draw the spire's top", plan_at("Spire top", PIER_H_MM + SPIRE_H_MM, diamond(SPIRE_TOP_MM / 2.0, 0.06)));
    let spire = t.add("Loft the spire", Operation::Loft { sections: vec![Profile::Feature { feature: foot }, Profile::Feature { feature: tip }] }, ComponentRole::Other);
    let buds: Vec<Vec<P2>> = CROCKET_AT.iter().map(|&k| crocket(arris(k), PIER_H_MM + SPIRE_H_MM * k)).collect();
    let crocket_s = t.sketch(
        "Draw the crockets up one arris",
        drawn("Crockets", [-MIN_SECTION_MM / 2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], &buds),
    );
    let crockets = t.extrude("Raise the crockets", crocket_s, MIN_SECTION_MM, 0.0);
    let crockets_all = t.add(
        "Turn the crockets onto all four arrises",
        Operation::Pattern { sources: Sources(vec![crockets]), kind: PatternKind::About { part: spire, count: 4, span_deg: 360.0 } },
        ComponentRole::Other,
    );
    let zt = PIER_H_MM + SPIRE_H_MM;
    let collar = t.add("Turn the finial's collar", Operation::Cylinder { radius_mm: COLLAR_MM, height_mm: 0.3 }, ComponentRole::Other);
    let collar = t.add("Seat the collar on the spire's top", Operation::Transform { source: collar, translation: [0.0, 0.0, zt], rotation_deg: [0.0; 3] }, ComponentRole::Other);
    let ring = |rad: f64| -> Vec<P2> { (0..24).map(|i| polar(15.0 * i as f64, rad)).collect() };
    let knop_sections: Vec<Id> = KNOP
        .iter()
        .enumerate()
        .map(|(i, (dz, rad))| t.sketch(&format!("Draw the knop's section {}", i + 1), plan_at("Knop section", zt + dz, ring(*rad))))
        .collect();
    let knop = t.add(
        "Loft the pointed knop over the collar",
        Operation::Loft { sections: knop_sections.iter().map(|f| Profile::Feature { feature: *f }).collect() },
        ComponentRole::Other,
    );
    // The flyer.
    let rib_s = t.sketch("Draw the flyer: a raking coping over a segmental arch, from the pier to the choir", elevation("Flyer", RIB_MM / 2.0, &[rib_outline(f.to_local(choir.upper), f.to_local(choir.lower))?]));
    let rib = t.extrude("Raise the flyer across the finger", rib_s, RIB_MM, 0.0);
    // Every part is seated at the pier's placement and joined; the band's join clusters them into one tool. Only the
    // plinth meets the band, so only it lays the seam bead.
    let parts = [plinth, shaft, spire, crockets, crockets_all, collar, knop, rib];
    for id in parts {
        let c = &mut t.doc.features.iter_mut().find(|g| g.id == id).unwrap().component;
        c.role = ComponentRole::Shank;
        // A pattern stands where its copies do.
        if id != crockets_all {
            c.placement = f.placement(theta);
        }
        c.attach = Attach::Join;
        c.blend_mm = if id == plinth { SEAM_MM } else { 0.0 };
    }
    Ok(parts.to_vec())
}

/// The nave's blind arcade: lancets cut into both side faces, from the right pier round through the palm to the left.
fn arcade(d: &RingDesign) -> Vec<Stamp> {
    let bay = Stamp {
        name: "Nave arcade".into(),
        theta_deg: 0.0,
        v_mm: 0.0,
        // Heads out from the finger, as the piers stand.
        rot_deg: 180.0,
        outline: lancet(ARCADE_W_MM, -ARCADE_H_MM / 2.0, ARCADE_H_MM / 2.0),
        height_mm: 0.1,
        sink_mm: ARCADE_DEEP_MM,
        draft_deg: 0.0,
        cut: true,
        bench: false,
        along_pull: true,
        tier: 0,
        top: StampTop::Flat,
        fine_cap: false,
    };
    let mut out = Vec::new();
    for high in [false, true] {
        let row = StampRow { stamp: bay.clone(), path: RowPath::SideFace { high, frac: 0.5 }, from_deg: ARCADE_FROM_DEG, to_deg: ARCADE_TO_DEG, count: ARCADE_BAYS, taper: 0.0, fold_clear_mm: 0.0, mirror_shoulders: false };
        out.extend(setting::stamp_row(d, &row));
    }
    out
}

fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe.name = "Arcus / investment / Gold 18k".into();
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).map_or(1.25, |m| m.shrink_pct);
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, investment and measured trials.".into();
    s.sample_pitch_mm = 0.10;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.5, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -30.0, 0.0], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Sprue at the palm. Set the sapphire in its basket and tip the claws. Polish the flyers, spires and finials bright; leave the arcade's lancets and the piers' blind lancets as cast, or satin them.".into();
    s
}

/// The ring: the band, the sapphire in its basket with the bur and azures under it, and each side's pinnacled pier
/// throwing its flyer to the choir, over the nave's blind arcade.
fn author() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    d.name = "Arcus — the flying buttress".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("size")?;
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 3.2;
    d.profile.thickness_mm = 2.3;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Cathedral;
    d.shank.amount = 0.8;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    let gem = sapphire();
    let mut t = Tree { doc: Document::default() };
    t.add("The band", Operation::Band, ComponentRole::Shank);
    let stone = t.next();
    let mut s = builders::stone_feature(stone, gem, Placement::ring(90.0, builders::stand_off_mm(builders::BASKET, gem) + CHOIR_LIFT_MM));
    s.name = "Stand the sapphire over the crown".into();
    t.push(s);
    let head = t.next();
    let mut f = builders::feature_on(head, "Raise the choir: a four-claw basket with two rails", builders::BASKET, stone, json!({ "prongs": 4, "wire_mm": 0.9, "rails": 2, "tip": "Point" }));
    f.component.stage = Stage::Cast;
    t.push(f);
    let id = t.next();
    t.push(builders::feature_on(id, "Bur the seat through to the bore", builders::BUR, stone, json!({ "through": true })));
    let id = t.next();
    t.push(builders::feature_on(id, "Cut the six crypt windows under the stone", builders::AZURE, stone, json!({ "shape": "Teardrop", "count": 6, "head": head })));
    let choir = {
        let mut probe = d.clone();
        probe.cad = Some(t.doc.clone());
        rails(&probe, &t.doc.features[head as usize - 1].name)?
    };
    let side = pinnacle(&mut t, &d, &choir, 90.0 + SPREAD_DEG)?;
    t.add(
        "Mirror the pier and its flyer across the crown",
        Operation::Pattern { sources: Sources(side), kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } },
        ComponentRole::Shank,
    );
    let c = &mut t.doc.features.last_mut().unwrap().component;
    c.attach = Attach::Join;
    c.blend_mm = SEAM_MM;
    d.stamps = arcade(&d);
    let mut s = setup();
    s.component = None;
    d.cad = Some(t.doc);
    d.manufacturing = Some(s);
    Ok(d)
}

// --- Measures --------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// Every CAD part's self-crossings as built.
fn made_parts(built: &mesh::BuildResult) -> Vec<(String, usize)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| !c.settings.reference)
        .map(|c| {
            let n = match &c.made {
                Some(m) => csg::self_crossings(m.solid()),
                None => csg::self_crossings(&csg::Solid { v: c.trace.positions.clone(), f: c.mesh.faces.clone() }),
            };
            (c.name.clone(), n)
        })
        .collect()
}

/// The nearest vertex to the finger's axis, and how many stand inside the bore.
fn bore_intrusion(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    let mut least = f64::MAX;
    let mut inside = 0;
    for v in &m.vertices {
        let r = (v.0 as f64).hypot(v.1 as f64);
        least = least.min(r);
        inside += usize::from(r < bore - 0.01);
    }
    (least, inside)
}

// --- Renders ---------------------------------------------------------------------------------------

/// Each named view as the direction from the ring toward the eye, world +y (the crown) kept up. The face is the
/// ring seen along the finger: the flyers' elevation, piers and spires in silhouette either side of the choir.
const VIEWS: [(&str, P3); 7] = [
    ("hero", [0.5, 0.55, 0.7]),
    ("face", [0.0, 0.12, 1.0]),
    ("top", [0.02, 1.0, 0.25]),
    ("palm", [0.35, -1.0, 0.45]),
    ("side", [1.0, 0.15, 0.05]),
    ("shoulder", [0.6, 0.55, 0.35]),
    ("reverse", [-0.7, 0.35, -0.6]),
];

fn rotated(m: &mesh::Mesh, f: impl Fn(P3) -> P3) -> mesh::Mesh {
    let map = |v: mesh::Vec3| {
        let p = f([v.0 as f64, v.1 as f64, v.2 as f64]);
        mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
    };
    mesh::Mesh { vertices: m.vertices.iter().map(|v| map(*v)).collect(), normals: m.normals.iter().map(|v| map(*v)).collect(), faces: m.faces.clone(), ..mesh::Mesh::default() }
}

/// The mesh in the frame of a camera looking back along `toward` with world +y kept up, so a plain front view renders it.
fn looked(m: &mesh::Mesh, toward: P3) -> mesh::Mesh {
    let l = (toward[0] * toward[0] + toward[1] * toward[1] + toward[2] * toward[2]).sqrt();
    let z = toward.map(|v| v / l);
    let up = [0.0, 1.0, 0.0];
    let x = [up[1] * z[2] - up[2] * z[1], up[2] * z[0] - up[0] * z[2], up[0] * z[1] - up[1] * z[0]];
    let xl = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let x = x.map(|v| v / xl);
    let y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut out = rotated(m, |p| [dot(p, x), dot(p, y), dot(p, z)]);
    for v in &mut out.vertices {
        v.2 *= 0.05;
    }
    out
}

/// The faces of `m` wholly above height `y`, to frame the crown on.
fn crop_above(m: &mesh::Mesh, y: f64) -> mesh::Mesh {
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in &m.faces {
        if !f.iter().all(|&i| m.vertices[i as usize].1 as f64 > y) {
            continue;
        }
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

fn side_by_side(path: &Path, images: &[Vec<u8>], edge: usize) -> Result<()> {
    let n = images.len();
    let mut out = vec![0u8; edge * n * edge * 3];
    for (k, img) in images.iter().enumerate() {
        for y in 0..edge {
            let dst = y * edge * n * 3 + k * edge * 3;
            out[dst..dst + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * n) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn shot(parts: &[(&mesh::Mesh, bool, [f32; 3])], toward: P3, edge: usize) -> Vec<u8> {
    let turned: Vec<(mesh::Mesh, bool, [f32; 3])> = parts.iter().map(|(m, gem, t)| (looked(m, toward), *gem, *t)).collect();
    let parts: Vec<render::Part> = turned.iter().map(|(m, gem, t)| if *gem { render::Part::tinted_stone(m, *t) } else { render::Part::metal(m, *t) }).collect();
    render::render_parts_ss(&parts, 0.0, 0.0, edge, edge, 3)
}

fn png(path: &Path, img: &[u8], edge: usize) -> Result<()> {
    image::save_buffer(path, img, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, fin: &render::Finished, bare: &mesh::Mesh, edge: usize) -> Result<()> {
    let mut parts: Vec<(&mesh::Mesh, bool, [f32; 3])> = vec![(&fin.metal, false, render::GOLD)];
    parts.extend(fin.stones.iter().map(|(m, t)| (m, true, *t)));
    for (name, toward) in VIEWS {
        png(&out.join(format!("{name}.png")), &shot(&parts, toward, edge), edge)?;
    }
    // The stone close-up: the crown alone, the choir and the flyers' heads.
    let crown = crop_above(&fin.metal, 9.0);
    let mut close: Vec<(&mesh::Mesh, bool, [f32; 3])> = vec![(&crown, false, render::GOLD)];
    close.extend(fin.stones.iter().map(|(m, t)| (m, true, *t)));
    png(&out.join("stones.png"), &shot(&close, [0.45, 0.6, 0.65], edge), edge)?;
    side_by_side(&out.join("bare-vs-finished.png"), &[shot(&[(bare, false, render::GOLD)], VIEWS[0].1, edge), shot(&parts, VIEWS[0].1, edge)], edge)?;
    png(&out.join("hero-300.png"), &shot(&parts, VIEWS[0].1, 300), 300)?;
    png(&out.join("face-300.png"), &shot(&parts, VIEWS[1].1, 300), 300)?;
    let sheet: Vec<Vec<u8>> = [VIEWS[0].1, VIEWS[1].1, VIEWS[4].1, VIEWS[2].1, VIEWS[3].1].iter().map(|t| shot(&parts, *t, 300)).collect();
    side_by_side(&out.join("contact-300.png"), &sheet, 300)?;
    Ok(())
}

/// The bare band alone, the stock every other feature works on.
fn plain_ring() -> Result<mesh::Mesh> {
    let mut d = author()?;
    let doc = d.cad.as_mut().unwrap();
    doc.features.truncate(1);
    doc.outputs = vec![1];
    Ok(mesh::try_build(&d, &AlphaLibrary::builtin(), draft_params())?.mesh)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/arcus"));
    std::fs::create_dir_all(&out)?;
    println!("Arcus");
    let started = std::time::Instant::now();
    let d = author()?;
    let lib = AlphaLibrary::builtin();
    let params = if draft { draft_params() } else { export_params() };
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    println!(
        "  {} triangles in {build_s:.1} s; watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; notes {:?} {:?}",
        built.mesh.faces.len(),
        built.solids.notes,
        built.parts.notes
    );
    let made = made_parts(&built);
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let statuses: Vec<(String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| (r.id.to_string(), format!("{:?}", r.status)))
        .collect();
    let features_ok = statuses.iter().all(|(_, s)| s == "Ok");
    // The thickness probe takes at most 250 000 faces, so the whole ring is measured on a coarse build of the same
    // design, and every made part on its own mesh.
    let coarse = mesh::try_build(&d, &lib, BuildParams { theta_steps: 256, profile_steps: 96, ..BuildParams::default() })?;
    let thick = cad::measure::thickness(&coarse.mesh, MIN_SECTION_MM);
    let part_thick: Vec<serde_json::Value> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| !c.settings.reference && c.attach == Attach::Join)
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "rays": t.rays, "sampled_min_mm": t.sampled_min_mm, "below_limit": t.below_limit, "unresolved": t.unresolved, "note": t.note})
        })
        .collect();
    let parts_thick_ok = part_thick.iter().all(|t| t["below_limit"] == 0 && t["rays"].as_u64().unwrap_or(0) > 0);
    let lands = dfm::cut_lands(&d, &built, MIN_SECTION_MM);
    let findings = dfm::findings_in(&d, &lib);
    let fin = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(&d, &lib, &built) };
    let stones = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let crowding = stones.as_ref().and_then(|s| s.crowding_note());
    let previewed = fin.stones.len();
    let prepared = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&prepared.mesh);
    library::save_design(out.join("design.ring.json"), &d)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let grams = built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams);
    let thick_ok = thick.rays > 0 && thick.below_limit == 0 && thick.unresolved == 0;
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings, every feature Ok", made.iter().all(|(_, n)| *n == 0) && features_ok),
        ("solids and parts notes empty", built.solids.notes.is_empty() && built.parts.notes.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("thickness clean at the 0.8 mm section, whole ring (coarse build)", thick_ok),
        ("thickness clean at the 0.8 mm section, every joined part", parts_thick_ok),
        ("cut lands clean at 0.8 mm", lands.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed && reported > 0),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let features: Vec<serde_json::Value> = d.cad.as_ref().unwrap().features.iter().map(|f| json!({"id": f.id, "name": f.name, "operation": f.operation.label()})).collect();
    let mut report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "alloy": ALLOY,
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s},
        "geometry": {"watertight": watertight, "boundary_edges": built.report.validation.boundary_edges, "non_manifold_edges": built.report.validation.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm},
        "made_parts": made,
        "feature_status": statuses,
        "solids": {"notes": built.solids.notes, "parts_notes": built.parts.notes},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "part_thickness": part_thick,
        "thickness": {"coarse_triangles": coarse.mesh.faces.len(), "limit_mm": thick.limit_mm, "sampled_min_mm": thick.sampled_min_mm, "rays": thick.rays, "unresolved": thick.unresolved, "below_limit": thick.below_limit, "point": thick.point, "note": thick.note},
        "cut_lands": lands.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed, "crowding": crowding},
        "grams_gold": grams,
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len()},
        "design": {"bytes": text.len(), "cad_features": features.len()},
        "features": features,
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let process = json!({"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm});
    let report_path = out.join("report.json");
    if draft {
        report["draft"] = process;
        report["draft_build"] = report.clone();
    } else {
        report["draft"] = process;
        if let Ok(old) = std::fs::read(&report_path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<serde_json::Value>(&b)?)) {
            report["draft_build"] = old.get("draft_build").cloned().unwrap_or(serde_json::Value::Null);
        }
    }
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Arcus / investment pattern")?;
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-sapphire.stl".to_string() } else { format!("reference-sapphire-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Arcus reference stone")?;
            materials.push(json!({"mesh": file, "name": "Sapphire", "tint": tint, "ior": 1.77, "dispersion": 0.018, "roughness": 0.05, "transmission": 0.7}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    renders(&out, &fin, &plain_ring()?, if draft { 900 } else { 1600 })?;
    println!(
        "  thickness min {:?} ({} below, {} unresolved); lands {}; dfm {}; stones {reported}/{previewed}; {grams:.2} g gold",
        thick.sampled_min_mm,
        thick.below_limit,
        thick.unresolved,
        lands.len(),
        findings.len()
    );
    for f in lands.iter().chain(&findings) {
        println!("    {}: {}", f.label, f.message);
    }
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Arcus failed a gate; see {}", report_path.display());
    Ok(())
}
