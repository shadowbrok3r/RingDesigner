//! Tenebrae — Ogiva, the keel: a ring whose cross-section is a pointed arch, revolved round the finger and poured in Delft sand.
//! cargo build --release -p ringdesign-core --example tenebrae_ogiva
//! target/release/examples/tenebrae_ogiva [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{Boolean, Component, ComponentRole, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, PlaneBase, Profile, pattern::Sources},
    castability::{self, CastProcess, SandProcess},
    csg, dfm, library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch, Workplane},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Width of the arch at its springers, along the finger, mm.
const WIDTH_MM: f64 = 4.2;
/// Straight jambs rising off the bore before the arch springs, mm.
const JAMB_MM: f64 = 0.5;
/// Radius of each head arc as a share of the width: 1.0 is the equilateral arch, above it a lancet.
const ARCH_RADIUS: f64 = 1.5;
/// Radius of the rounds where the jambs meet the bore, mm.
const FOOT_ROUND_MM: f64 = 0.3;
/// Sagitta of the comfort arc across the bore, mm.
const COMFORT_MM: f64 = 0.12;
/// Chord the section's arcs are walked in, mm.
const ARC_STEP_MM: f64 = 0.1;

/// The gable over the top of the hand: where its ogive leaves the band either side of the crown, and how high its apex stands over the keel, mm.
const GABLE_FROM_DEG: f64 = 30.0;
const GABLE_RISE_MM: f64 = 4.5;
/// Where the gable's sections stand inside the band, radially, and how many sections it is lofted through.
const GABLE_BASE_MM: f64 = 11.5;
const GABLE_SECTIONS: usize = 31;
/// The gable's sections sit this share inside the band's flanks, so no face of it lies in the band's.
const GABLE_INSET: f64 = 0.97;

/// Crockets climbing the gable's right slope toward the finial: arc length from where the ogive leaves the band, and how tall each grows.
const CROCKETS: [(f64, f64); 4] = [(1.5, 1.0), (5.0, 1.2), (8.5, 1.4), (12.0, 1.6)];
/// How far each crocket's plate runs either side of the parting plane, mm.
const CROCKET_HALF_MM: f64 = 0.8;
/// The crest moulding along the gable's keel: how proud it stands and how far either side of the parting plane it runs, mm.
const CREST_MM: f64 = 0.8;
const CREST_HALF_MM: f64 = 0.6;
/// The finial's height over the apex, mm.
const FINIAL_MM: f64 = 4.5;
/// How far each half of the finial and crest runs past the crown's centre line, so the mirrored half overlaps it, mm.
const FINIAL_OVERLAP_MM: f64 = 0.1;

/// Three lancet lights pierced through the gable: centre offset, width, sill and head apex (world x, width, y from, y to), mm.
const LIGHTS: [(f64, f64, f64, f64); 2] = [(0.0, 1.6, 10.6, 17.6), (2.7, 1.3, 10.0, 16.1)];
/// The plane the lights are drawn on, over the whole cope half.
const LIGHT_PLANE_MM: f64 = 3.0;

/// The step moulding on each flank: how far below the keel, and how far it steps out, mm.
const STEP_BELOW_MM: f64 = 0.9;
const STEP_MM: f64 = 0.25;
/// Run of the drafted riser at the step, mm.
const STEP_RUN_MM: f64 = 0.06;

/// Lancet niches round each foot, their width, where they start and end radially, and the floor they are cut to, mm.
const NICHES: usize = 15;
const NICHE_W_MM: f64 = 1.4;
const NICHE_FROM_MM: f64 = 10.2;
const NICHE_TO_MM: f64 = 13.0;
const NICHE_FLOOR_MM: f64 = 0.45;
/// Where the first niche stands round the ring, past the gable, and the span the rest follow over, degrees.
const NICHE_PHASE_DEG: f64 = 130.0;
const NICHE_SPAN_DEG: f64 = 280.0;
/// The plane the niches are drawn on, above the foot.
const FOOT_PLANE_MM: f64 = 2.5;

/// The section is cut this far round from the crown, at the side of the band clear of the gable and between two niches, degrees.
const SECTION_OFF_DEG: f64 = 90.0;

/// Delft clay draft, degrees.
const DRAFT_DEG: f64 = 3.0;
/// Where each drafted half starts across the parting plane, so the halves overlap.
const PARTING_OVERLAP_MM: f64 = 0.03;
/// Delft section and sand web floors, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_SAND_WEB_MM: f64 = 0.6;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The pointed arch's numbers: springer radius on the bore, the spring line over the jambs, half width,
/// the head arcs' radius and their centres' offset across the axis, the head's rise, and the keel.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Arch {
    springer_r: f64,
    spring_line_r: f64,
    half_w: f64,
    radius: f64,
    centre_off: f64,
    rise: f64,
    apex_r: f64,
    step_r: f64,
    thickness: f64,
    keel_draft_deg: f64,
    keel_angle_deg: f64,
}

impl Arch {
    fn of(bore_r: f64, width: f64, jamb: f64, radius_share: f64, comfort: f64) -> Self {
        let h = width / 2.0;
        let r = radius_share * width;
        let c = r - h;
        let rise = (r * r - c * c).sqrt();
        // The gable over the step is the head drawn STEP_MM narrower on each side.
        let cap = (r * r - (c + STEP_MM).powi(2)).sqrt();
        let draft = ((c + STEP_MM) / cap).atan().to_degrees();
        let springer_r = bore_r + comfort;
        let apex_r = springer_r + jamb + cap;
        Self {
            springer_r,
            spring_line_r: springer_r + jamb,
            half_w: h,
            radius: r,
            centre_off: c,
            rise,
            apex_r,
            step_r: apex_r - STEP_BELOW_MM,
            thickness: jamb + cap,
            keel_draft_deg: draft,
            keel_angle_deg: 180.0 - 2.0 * draft,
        }
    }
    /// The surface's height above the parting plane at radius `r`, mm.
    fn flank_z(&self, r: f64) -> f64 {
        let d = (r - self.spring_line_r).max(0.0);
        let z = (self.radius * self.radius - d * d).max(0.0).sqrt() - self.centre_off;
        if r >= self.step_r { z - STEP_MM } else { z }
    }
}

fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: P2) -> f64 {
    a[0].hypot(a[1])
}
fn along2(from: P2, to: P2, r: f64) -> P2 {
    let d = sub2(to, from);
    let l = len2(d);
    [from[0] + d[0] / l * r, from[1] + d[1] / l * r]
}

/// The ring's section on the section plane (x out from the finger's axis, y along the finger): two head
/// arcs meeting at the keel, straight jambs down to the bore, a comfort arc across it, and a round at each foot.
fn pointed_arch(bore_r: f64) -> (Sketch, Arch) {
    let a = Arch::of(bore_r, WIDTH_MM, JAMB_MM, ARCH_RADIUS, COMFORT_MM);
    let (rj, h, c, f) = ( a.spring_line_r, a.half_w, a.centre_off, FOOT_ROUND_MM);
    let bore_radius = (COMFORT_MM * COMFORT_MM + h * h) / (2.0 * COMFORT_MM);
    let bore_c = [bore_r + bore_radius, 0.0];
    let mut s = Sketch::default();
    s.name = "Pointed-arch section".into();
    s.plane = Workplane::section();
    let keel = [a.apex_r, 0.0];
    // The upper head arc turns about the centre below the axis, the lower about the one above.
    let (upper_c, lower_c) = ([rj, -c], [rj, c]);
    // Each foot round sits f inside the jamb and f inside the bore arc.
    let round = |side: f64| -> (P2, P2, P2) {
        let y = side * (h - f);
        let dy = y - bore_c[1];
        let x = bore_c[0] - ((bore_radius - f).powi(2) - dy * dy).sqrt();
        let centre = [x, y];
        (centre, [x, side * h], along2(bore_c, centre, bore_radius))
    };
    let (fu, fu_jamb, fu_bore) = round(1.0);
    let (fl, fl_jamb, fl_bore) = round(-1.0);
    // The kernel tessellates revolved circular pieces with seams that do not meet, so each arc is walked in short chords.
    let mut pts: Vec<P2> = Vec::new();
    let arc = |pts: &mut Vec<P2>, centre: P2, from: P2, to: P2, step: f64, first: bool| {
        let r = len2(sub2(from, centre));
        let a0 = (from[1] - centre[1]).atan2(from[0] - centre[0]);
        let mut a1 = (to[1] - centre[1]).atan2(to[0] - centre[0]);
        while a1 <= a0 {
            a1 += 2.0 * PI;
        }
        let n = ((a1 - a0) * r / step).ceil().max(2.0) as usize;
        for i in usize::from(!first)..n {
            let t = a0 + (a1 - a0) * i as f64 / n as f64;
            pts.push([centre[0] + r * t.cos(), centre[1] + r * t.sin()]);
        }
    };
    // From the keel down the gable to the step, out over the drafted riser, and on down the head.
    let (cap_u, cap_l) = ([rj, -c - STEP_MM], [rj, c + STEP_MM]);
    let zs = |r: f64| (a.radius * a.radius - (r - rj).powi(2)).sqrt() - c;
    let step_in = [a.step_r, zs(a.step_r) - STEP_MM];
    let step_out = [a.step_r - STEP_RUN_MM, zs(a.step_r - STEP_RUN_MM)];
    arc(&mut pts, cap_u, keel, step_in, ARC_STEP_MM * 0.5, true);
    pts.push(step_in);
    arc(&mut pts, upper_c, step_out, [rj, h], ARC_STEP_MM, true);
    pts.push([rj, h]);
    arc(&mut pts, fu, fu_jamb, fu_bore, ARC_STEP_MM * 0.5, true);
    arc(&mut pts, bore_c, fu_bore, fl_bore, ARC_STEP_MM * 2.0, true);
    arc(&mut pts, fl, fl_bore, fl_jamb, ARC_STEP_MM * 0.5, true);
    pts.push(fl_jamb);
    pts.push([rj, -h]);
    let step_out_l = [step_out[0], -step_out[1]];
    let step_in_l = [step_in[0], -step_in[1]];
    arc(&mut pts, lower_c, [rj, -h], step_out_l, ARC_STEP_MM, false);
    pts.push(step_out_l);
    arc(&mut pts, cap_l, step_in_l, keel, ARC_STEP_MM * 0.5, true);
    let ids: Vec<Id> = pts.into_iter().map(|p| s.point(p)).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    (s, a)
}

/// A cubic Bézier's points at `n` steps, its start left out.
fn bez(p: [P2; 4], n: usize) -> Vec<P2> {
    (1..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            [0, 1].map(|k| w[0] * p[0][k] + w[1] * p[1][k] + w[2] * p[2][k] + w[3] * p[3][k])
        })
        .collect()
}

/// The ogive over the top of the hand: its right arc's centre and radius, where it leaves the band, and its apex.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Ogive {
    centre: P2,
    radius: f64,
    from_deg: f64,
    to_deg: f64,
    apex: P2,
    apex_angle_deg: f64,
}

impl Ogive {
    /// The arc tangent to the band's keel circle `k` at `from_deg` round the ring and through the apex `rise` over it.
    fn of(k: f64, from_deg: f64, rise: f64) -> Self {
        let (s, c) = from_deg.to_radians().sin_cos();
        let top = k + rise;
        // The centre m along the ray to the tangent point: (m c)^2 + (top - m s)^2 = (k - m)^2.
        let m = (k * k - top * top) / (2.0 * k - 2.0 * top * s);
        let centre = [m * c, m * s];
        let radius = k - m;
        let apex = [0.0, top];
        let at_apex = (apex[1] - centre[1]).atan2(apex[0] - centre[0]);
        let from = s.atan2(c);
        let slope = (PI / 2.0 - at_apex).abs().to_degrees();
        Self { centre, radius, from_deg: from.to_degrees(), to_deg: at_apex.to_degrees(), apex, apex_angle_deg: 180.0 - 2.0 * (90.0 - slope) }
    }
    /// The point, outward normal and tangent toward the apex `s` mm along the right arc from where it leaves the band.
    fn at(&self, s: f64) -> (P2, P2, P2) {
        let a = self.from_deg.to_radians() + s / self.radius;
        let n = [a.cos(), a.sin()];
        ([self.centre[0] + self.radius * n[0], self.centre[1] + self.radius * n[1]], n, [-n[1], n[0]])
    }
    fn length(&self) -> f64 {
        (self.to_deg - self.from_deg).to_radians() * self.radius
    }
    /// How far from the finger's axis the ogive stands at `theta_deg`, mirrored about the crown; the keel circle outside it.
    fn reach(&self, theta_deg: f64, k: f64) -> f64 {
        let t = if theta_deg > 90.0 { 180.0 - theta_deg } else { theta_deg };
        if t <= self.from_deg {
            return k;
        }
        let d = [t.to_radians().cos(), t.to_radians().sin()];
        let dc = d[0] * self.centre[0] + d[1] * self.centre[1];
        let cc = self.centre[0] * self.centre[0] + self.centre[1] * self.centre[1];
        dc + (dc * dc - cc + self.radius * self.radius).sqrt()
    }
}

/// The band's flank sampled from the gable's base over the keel and back: (r, z) round the section, anticlockwise.
fn flank_loop(a: &Arch) -> Vec<P2> {
    let run = a.step_r - STEP_RUN_MM;
    let n1 = 36;
    let n2 = 14;
    let mut up: Vec<P2> = (0..=n1).map(|i| GABLE_BASE_MM + (run - GABLE_BASE_MM) * i as f64 / n1 as f64).map(|r| [r, a.flank_z(r.min(run))]).collect();
    up.extend((0..n2).map(|i| a.step_r + (a.apex_r - a.step_r) * i as f64 / n2 as f64).map(|r| [r, a.flank_z(r)]));
    // Up the lower flank to the keel, then down the upper one to the base.
    let mut lower: Vec<P2> = up.iter().map(|p| [p[0], -p[1]]).collect();
    lower.push([a.apex_r, 0.0]);
    lower.extend(up.iter().rev().cloned());
    lower
}

/// One of the gable's sections at `theta_deg`: the band's flank stretched out to the ogive and drawn a hair inside it.
fn gable_section(a: &Arch, og: &Ogive, theta_deg: f64) -> Sketch {
    let reach = og.reach(theta_deg, a.apex_r).max(a.apex_r - 0.05);
    let stretch = (reach - GABLE_BASE_MM) / (a.apex_r - GABLE_BASE_MM);
    let mut s = Sketch::default();
    s.name = format!("Gable section at {theta_deg:.0}°");
    let (st, ct) = theta_deg.to_radians().sin_cos();
    s.plane = Workplane { origin: [0.0; 3], x: [ct, st, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
    let pts: Vec<Id> = flank_loop(a)
        .into_iter()
        .map(|[r, z]| s.point([GABLE_BASE_MM + (r - GABLE_BASE_MM) * stretch, z * GABLE_INSET]))
        .collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    s
}

/// How deep below the gable's keel a plate `half` either side of the parting plane has to reach before the gable holds it, at stretch `k`.
fn sink_for(a: &Arch, k: f64, half: f64) -> f64 {
    let mut d = 0.2;
    while d < 4.0 {
        let r = a.apex_r - d / k;
        if a.flank_z(r) * GABLE_INSET >= half + 0.15 {
            return d + 0.1;
        }
        d += 0.05;
    }
    d
}

/// One crocket in its own frame, `v` along the slope toward the finial and `u` out of the keel: a leaf of three
/// notched lobes that rises from the crest, leans toward the finial and curls its tip forward into a bud.
/// `grow` stretches it out of the keel; `sink` is how deep its foot reaches. Its outline runs as a chain of cubic Béziers.
fn crocket_outline(grow: f64, sink: f64) -> Vec<[P2; 4]> {
    let k = -sink;
    let g = |p: [P2; 4]| p.map(|[v, u]| [v, if u > 0.0 { u * grow } else { u }]);
    [
        [[-0.8, k], [-0.3, k], [0.3, k], [0.8, k]],
        // The front of the stem, up under the bud.
        [[0.8, k], [0.8, k * 0.5], [0.85, 0.7], [1.25, 0.85]],
        // The bud curling forward and down onto the crest.
        [[1.25, 0.85], [1.7, 0.7], [2.2, 0.85], [2.25, 1.3]],
        [[2.25, 1.3], [2.3, 1.7], [2.0, 1.95], [1.65, 1.9]],
        // The two upper lobes, notched apart.
        [[1.65, 1.9], [1.6, 2.3], [1.2, 2.55], [0.8, 2.45]],
        [[0.8, 2.45], [0.6, 2.7], [0.1, 2.75], [-0.15, 2.4]],
        // The back lobe, and down the back to the foot.
        [[-0.15, 2.4], [-0.5, 2.2], [-0.7, 1.9], [-0.55, 1.6]],
        [[-0.55, 1.6], [-0.8, 1.4], [-0.85, 0.8], [-0.8, k]],
    ]
    .into_iter()
    .map(g)
    .collect()
}

/// Half the finial at the apex, in the same frame (`v` round the ring past the crown): a stem that opens into a fleur of
/// three lobes, the middle one pointed; it runs FINIAL_OVERLAP_MM past the centre line so the mirrored half overlaps it.
fn finial_outline(sink: f64) -> Vec<[P2; 4]> {
    let k = -sink;
    let f = FINIAL_MM / 3.5;
    let o = -FINIAL_OVERLAP_MM;
    vec![
        [[o, k], [0.1, k], [0.4, k], [0.6, k]],
        [[0.6, k], [0.6, k * 0.5], [0.6, 0.8], [0.6, 1.2 * f]],
        [[0.6, 1.2 * f], [1.3, 1.0 * f], [1.85, 1.8 * f], [1.3, 2.3 * f]],
        [[1.3, 2.3 * f], [1.0, 2.5 * f], [0.65, 2.35 * f], [0.5, 2.2 * f]],
        [[0.5, 2.2 * f], [0.35, 2.35 * f], [0.55, 3.1 * f], [0.12, 3.45 * f]],
        [[0.12, 3.45 * f], [0.08, 3.48 * f], [0.04, 3.5 * f], [0.0, 3.5 * f]],
        [[0.0, 3.5 * f], [o * 0.33, 3.5 * f], [o * 0.67, 3.5 * f], [o, 3.5 * f]],
        [[o, 3.5 * f], [o, 2.0], [o, 0.5], [o, k]],
    ]
}

/// A cubic Bézier's reach along its control polygon.
fn reach(seg: &[P2; 4]) -> f64 {
    (0..3).map(|i| len2(sub2(seg[i + 1], seg[i]))).sum()
}

/// A closed outline of Béziers laid at `origin` with `v` along `t` and `u` along `n`, walked in short chords: a drafted extrusion takes lines and arcs.
fn laid_loop(s: &mut Sketch, segs: &[[P2; 4]], origin: P2, n: P2, t: P2) {
    let world = |[v, u]: P2| -> P2 { [origin[0] + v * t[0] + u * n[0], origin[1] + v * t[1] + u * n[1]] };
    let mut ring: Vec<P2> = Vec::new();
    for seg in segs {
        ring.extend(bez(*seg, (reach(seg) / 0.12).ceil().clamp(1.0, 12.0) as usize));
    }
    let points: Vec<Id> = ring.into_iter().map(|p| s.point(world(p))).collect();
    s.entity(Geometry::Polyline { points, closed: true });
}

fn on_plane(name: &str, plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
    s
}

/// The stretch of the gable's section where the ogive stands `reach` from the finger's axis.
fn stretch_at(a: &Arch, reach: f64) -> f64 {
    (reach - GABLE_BASE_MM) / (a.apex_r - GABLE_BASE_MM)
}

/// The crockets up the gable's right slope, leaning toward the apex, and the finial's half past the crown, on the parting plane.
fn crockets_sketch(a: &Arch, og: &Ogive, plane: Id) -> Sketch {
    let mut s = on_plane("Crockets and half the finial", plane);
    for (at, grow) in CROCKETS {
        let (p, n, t) = og.at(at);
        let sink = sink_for(a, stretch_at(a, len2(p)), CROCKET_HALF_MM);
        laid_loop(&mut s, &crocket_outline(grow, sink), p, n, t);
    }
    let sink = sink_for(a, stretch_at(a, og.apex[1]), CROCKET_HALF_MM);
    laid_loop(&mut s, &finial_outline(sink), og.apex, [0.0, 1.0], [-1.0, 0.0]);
    s
}

/// The crest moulding along the gable's right slope, from where it leaves the band to just past the crown.
fn crest_sketch(a: &Arch, og: &Ogive, plane: Id) -> Sketch {
    let mut s = on_plane("Crest moulding", plane);
    let len = og.length();
    let n = 60;
    let mut outer = Vec::new();
    let mut inner = Vec::new();
    for i in 0..=n {
        let at = 0.4 + (len - 0.4) * i as f64 / n as f64;
        let (p, nn, _) = og.at(at);
        // It rises out of the keel over its first 2 mm.
        let proud = CREST_MM * (at / 2.0).min(1.0);
        let sink = sink_for(a, stretch_at(a, len2(p)), CREST_HALF_MM);
        outer.push([p[0] + nn[0] * proud, p[1] + nn[1] * proud]);
        inner.push([p[0] - nn[0] * sink, p[1] - nn[1] * sink]);
    }
    // Past the centre line, so the mirrored half overlaps this one.
    let top = outer[n];
    let low = inner[n];
    outer.push([top[0] - FINIAL_OVERLAP_MM, top[1]]);
    inner.push([low[0] - FINIAL_OVERLAP_MM, low[1]]);
    let mut ring = outer;
    ring.extend(inner.into_iter().rev());
    let pts: Vec<Id> = ring.into_iter().map(|p| s.point(p)).collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    s
}

/// A lancet's outline: a flat sill `w` wide at `from`, straight jambs and an equilateral pointed head with its apex at `to`,
/// walked in short chords, in a frame with `x` across it and `y` up it.
fn lancet(w: f64, from: f64, to: f64) -> Vec<P2> {
    let h = w / 2.0;
    let spring = to - w * (3.0f64).sqrt() / 2.0;
    let mut pts = vec![[-h, from], [h, from], [h, spring]];
    // The right half of the head turns about the left springer, the left half about the right one.
    let steps = 8;
    for i in 1..steps {
        let a = PI / 3.0 * i as f64 / steps as f64;
        pts.push([-h + w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([0.0, to]);
    for i in 1..steps {
        let a = PI / 3.0 * (steps - i) as f64 / steps as f64;
        pts.push([h - w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([-h, spring]);
    pts
}

/// The three lancet lights through the gable, on a plane over the cope half: straight jambs, a flat sill and an equilateral pointed head.
fn lights_sketch(plane: Id) -> Sketch {
    let mut s = on_plane("Lancet lights", plane);
    for (x, w, from, to) in LIGHTS {
        for side in if x > 0.0 { vec![1.0, -1.0] } else { vec![1.0] } {
            let pts: Vec<Id> = lancet(w, from, to).into_iter().map(|p| s.point([p[0] + side * x, p[1]])).collect();
            s.entity(Geometry::Polyline { points: pts, closed: true });
        }
    }
    s
}

/// One lancet niche drawn on the foot plane at θ = NICHE_PHASE_DEG: parallel jambs from the inner end, a pointed equilateral head outward.
fn niche_sketch(plane: Id) -> Sketch {
    let mut s = on_plane("Lancet niche", plane);
    let (st, ct) = NICHE_PHASE_DEG.to_radians().sin_cos();
    // The lancet stands out from the bore: its y runs out along the radius, its x round the ring.
    let pts: Vec<Id> = lancet(NICHE_W_MM, NICHE_FROM_MM, NICHE_TO_MM)
        .into_iter()
        .map(|[x, y]| s.point([y * ct - x * st, y * st + x * ct]))
        .collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    s
}

struct Tree {
    doc: Document,
}

impl Tree {
    fn add(&mut self, name: &str, operation: Operation, role: ComponentRole) -> Id {
        let id = self.doc.features.len() as Id + 1;
        self.doc
            .append(Feature {
                id,
                name: name.into(),
                enabled: true,
                operation,
                component: Component { role, material: "Silver 925".into(), ..Component::default() },
            })
            .expect("feature appends");
        id
    }
}

fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(1.5, |m| m.shrink_pct);
    s.recipe.process = CastProcess::SandTwoPart;
    s.recipe.name = "Ogiva / Delft clay / Silver 925".into();
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, sand and measured trials.".into();
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.sample_pitch_mm = 0.10;
    s.flask.width_mm = 70.0;
    s.flask.length_mm = 70.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -13.2, 0.0], end: [0.0, -22.0, 0.0], diameter_mm: 3.2 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -22.0, 0.0], end: [0.0, -30.0, 0.0], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Pour in two-part Delft clay with the parting on the keel (z = 0), pulling along the finger. Gate at the palm's keel, where no crocket stands. Break the flash along the keel with a fine file, keeping the crease sharp; polish the flanks and the crockets' faces; leave the niche floors as cast, or satin them.".into();
    s
}

/// The ring: its section revolved, the crockets raised on the keel in both mould halves, and the niches cut into both feet.
fn author() -> Result<(RingDesign, Arch)> {
    let mut d = RingDesign::default();
    d.name = "Ogiva — the keel".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("size")?;
    SandProcess::DelftClay.apply(&mut d.draft);
    CastProcess::SandTwoPart.apply(&mut d.draft);
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    let bore_r = d.inner_radius_mm();
    let (section, arch) = pointed_arch(bore_r);
    let mut t = Tree { doc: Document::default() };
    let sec = t.add("Draw the pointed-arch section on the plane through the finger", Operation::Sketch { sketch: section }, ComponentRole::Other);
    let ring = t.add(
        "Revolve the arch round the finger",
        Operation::Revolve { sketch: Profile::Feature { feature: sec }, pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false },
        ComponentRole::Shank,
    );
    let og = Ogive::of(arch.apex_r, GABLE_FROM_DEG, GABLE_RISE_MM);
    // The kernel lofts along each section plane's normal, which points back round the ring: the sections run from 150° down to 30°.
    let sections: Vec<Profile> = (0..GABLE_SECTIONS)
        .map(|i| 180.0 - GABLE_FROM_DEG - (180.0 - 2.0 * GABLE_FROM_DEG) * i as f64 / (GABLE_SECTIONS - 1) as f64)
        .map(|theta| Profile::Inline(gable_section(&arch, &og, theta)))
        .collect();
    let gable = t.add("Loft the gable over the hand through its pointed sections, rising to the ogive", Operation::Loft { sections }, ComponentRole::Shank);
    let parting = t.add("Lay the parting plane just under the keel", Operation::Plane { base: PlaneBase::Parting, offset_mm: -PARTING_OVERLAP_MM }, ComponentRole::Other);
    let halves = |t: &mut Tree, sketch: Id, half: f64, what: &str| -> Id {
        let cope = t.add(
            &format!("Raise the cope half of the {what} along the pull"),
            Operation::Extrude { sketch: Profile::Feature { feature: sketch }, height_mm: half + PARTING_OVERLAP_MM, draft_deg: DRAFT_DEG },
            ComponentRole::Other,
        );
        let drag = t.add(
            &format!("Mirror the {what} into the drag half"),
            Operation::Pattern { sources: Sources(vec![cope]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } },
            ComponentRole::Other,
        );
        t.add(&format!("Join the halves of the {what}"), Operation::Boolean { a: cope, b: drag, kind: Boolean::Union }, ComponentRole::Other)
    };
    let crest_s = t.add("Draw the crest moulding up the gable's right slope on the parting plane", Operation::Sketch { sketch: crest_sketch(&arch, &og, parting) }, ComponentRole::Other);
    let crest = halves(&mut t, crest_s, CREST_HALF_MM, "crest");
    let leaf = t.add(
        "Draw the crockets climbing the right slope, and half the finial, on the parting plane",
        Operation::Sketch { sketch: crockets_sketch(&arch, &og, parting) },
        ComponentRole::Other,
    );
    let leaves = halves(&mut t, leaf, CROCKET_HALF_MM, "crockets");
    let right = t.add("Set the crockets on the crest", Operation::Boolean { a: crest, b: leaves, kind: Boolean::Union }, ComponentRole::Other);
    let left = t.add(
        "Mirror the climb across the crown onto the left slope",
        Operation::Pattern { sources: Sources(vec![right]), kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } },
        ComponentRole::Other,
    );
    let crockets = t.add("Join both climbs and close the finial", Operation::Boolean { a: right, b: left, kind: Boolean::Union }, ComponentRole::Other);
    let foot = t.add("Lay a plane over the cope foot", Operation::Plane { base: PlaneBase::Parting, offset_mm: FOOT_PLANE_MM }, ComponentRole::Other);
    let niche_s = t.add("Draw one lancet niche on the foot plane", Operation::Sketch { sketch: niche_sketch(foot) }, ComponentRole::Other);
    let niche = t.add(
        "Sink the niche along the pull",
        Operation::Extrude { sketch: Profile::Feature { feature: niche_s }, height_mm: -(FOOT_PLANE_MM - NICHE_FLOOR_MM), draft_deg: DRAFT_DEG },
        ComponentRole::Other,
    );
    let niches = t.add(
        "Array the niches round the cope foot, clear of the gable",
        Operation::Pattern { sources: Sources(vec![niche]), kind: PatternKind::Ring { count: NICHES as u32, span_deg: NICHE_SPAN_DEG } },
        ComponentRole::Other,
    );
    let drag_niches = t.add(
        "Mirror the niches into the drag foot",
        Operation::Pattern { sources: Sources(vec![niche, niches]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } },
        ComponentRole::Other,
    );
    let over = t.add("Lay a plane over the whole cope half", Operation::Plane { base: PlaneBase::Parting, offset_mm: LIGHT_PLANE_MM }, ComponentRole::Other);
    let lights_s = t.add("Draw the three lancet lights in the gable", Operation::Sketch { sketch: lights_sketch(over) }, ComponentRole::Other);
    let lights = t.add(
        "Pierce the lights down to the parting plane",
        Operation::Extrude { sketch: Profile::Feature { feature: lights_s }, height_mm: -(LIGHT_PLANE_MM + PARTING_OVERLAP_MM + 0.02), draft_deg: DRAFT_DEG },
        ComponentRole::Other,
    );
    let drag_lights = t.add(
        "Mirror the lights up from the drag side",
        Operation::Pattern { sources: Sources(vec![lights]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } },
        ComponentRole::Other,
    );
    // The array is a mesh, so this first cut makes the band one, and every later boolean runs through csg.
    let cut1 = t.add("Cut the cope foot's niches", Operation::Boolean { a: ring, b: niches, kind: Boolean::Subtract }, ComponentRole::Shank);
    let cut2 = t.add("Cut the first niche", Operation::Boolean { a: cut1, b: niche, kind: Boolean::Subtract }, ComponentRole::Shank);
    let cut3 = t.add("Cut the drag foot's niches", Operation::Boolean { a: cut2, b: drag_niches, kind: Boolean::Subtract }, ComponentRole::Shank);
    let gabled = t.add("Raise the gable on the band", Operation::Boolean { a: cut3, b: gable, kind: Boolean::Union }, ComponentRole::Shank);
    let crowned = t.add("Join the crest, the crockets and the finial to the gable", Operation::Boolean { a: gabled, b: crockets, kind: Boolean::Union }, ComponentRole::Shank);
    let lit = t.add("Pierce the cope half of the lights", Operation::Boolean { a: crowned, b: lights, kind: Boolean::Subtract }, ComponentRole::Shank);
    let _done = t.add("Pierce the drag half of the lights", Operation::Boolean { a: lit, b: drag_lights, kind: Boolean::Subtract }, ComponentRole::Shank);
    let mut s = setup();
    s.component = None;
    t.doc.features.last_mut().unwrap().component.manufacturing = Some(s.clone());
    d.cad = Some(t.doc);
    d.manufacturing = Some(s);
    Ok((d, arch))
}

// --- Measures --------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn mesh_of(s: &csg::Solid) -> mesh::Mesh {
    let mut m = mesh::Mesh {
        vertices: s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: s.f.clone(),
        ..mesh::Mesh::default()
    };
    m.normals = vertex_normals(&m);
    m
}

fn vertex_normals(m: &mesh::Mesh) -> Vec<mesh::Vec3> {
    let mut n = vec![[0.0f64; 3]; m.vertices.len()];
    for f in &m.faces {
        let p = f.map(|i| {
            let v = m.vertices[i as usize];
            [v.0 as f64, v.1 as f64, v.2 as f64]
        });
        let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        for i in f {
            for k in 0..3 {
                n[*i as usize][k] += c[k];
            }
        }
    }
    n.into_iter()
        .map(|v| {
            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
            mesh::Vec3((v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32)
        })
        .collect()
}

/// Loose triangles with each corner's normal averaged only over faces within 35° of its own, so a cut face shades flat and the flanks smooth.
fn creased(m: &mesh::Mesh) -> mesh::Mesh {
    let normal = |f: &[u32; 3]| {
        let p = f.map(|i| {
            let v = m.vertices[i as usize];
            [v.0 as f64, v.1 as f64, v.2 as f64]
        });
        let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let c = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let l = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt().max(1e-12);
        (c.map(|v| v / l), l)
    };
    let normals: Vec<(P3, f64)> = m.faces.iter().map(normal).collect();
    let mut incident = vec![Vec::new(); m.vertices.len()];
    for (k, f) in m.faces.iter().enumerate() {
        for i in f {
            incident[*i as usize].push(k);
        }
    }
    let mut out = mesh::Mesh::default();
    for (k, f) in m.faces.iter().enumerate() {
        let n = normals[k].0;
        let base = out.vertices.len() as u32;
        for i in f {
            let mut s = [0.0; 3];
            for j in &incident[*i as usize] {
                let (o, w) = normals[*j];
                if o[0] * n[0] + o[1] * n[1] + o[2] * n[2] > 0.82 {
                    for c in 0..3 {
                        s[c] += o[c] * w;
                    }
                }
            }
            let l = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt().max(1e-12);
            out.vertices.push(m.vertices[*i as usize]);
            out.normals.push(mesh::Vec3((s[0] / l) as f32, (s[1] / l) as f32, (s[2] / l) as f32));
        }
        out.faces.push([base, base + 1, base + 2]);
    }
    out
}

fn shells(m: &mesh::Mesh) -> usize {
    let mut parent: Vec<usize> = (0..m.vertices.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for f in &m.faces {
        let a = root(&mut parent, f[0] as usize);
        for &j in &f[1..] {
            let b = root(&mut parent, j as usize);
            parent[b] = a;
        }
    }
    let mut roots = std::collections::BTreeSet::new();
    for f in &m.faces {
        roots.insert(root(&mut parent, f[0] as usize));
    }
    roots.len()
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

/// The lands the sketch numbers leave, mm: between niches, niche to bore, niche floor to the far foot's floor, crocket plate and strokes.
fn lands(arch: &Arch) -> serde_json::Value {
    let pitch_in = 2.0 * PI * NICHE_FROM_MM / NICHES as f64;
    let bar = pitch_in - NICHE_W_MM;
    let rail = NICHE_FROM_MM - arch.springer_r;
    let web = 2.0 * NICHE_FLOOR_MM;
    let crocket_pitch = CROCKETS[1].0 - CROCKETS[0].0;
    let plate = 2.0 * CROCKET_HALF_MM - 2.0 * (CROCKET_HALF_MM * DRAFT_DEG.to_radians().tan());
    json!({
        "niche_bar_mm": bar,
        "niche_to_bore_rail_mm": rail,
        "axial_web_between_floors_mm": web,
        "niche_depth_inner_mm": arch.flank_z(NICHE_FROM_MM) - NICHE_FLOOR_MM,
        "niche_depth_head_mm": arch.flank_z(NICHE_TO_MM) - NICHE_FLOOR_MM,
        "niche_width_at_floor_mm": NICHE_W_MM - 2.0 * (FOOT_PLANE_MM - NICHE_FLOOR_MM) * DRAFT_DEG.to_radians().tan(),
        "crocket_plate_thickness_at_face_mm": plate,
        "crocket_pitch_along_gable_mm": crocket_pitch,
        "crocket_stem_mm": 1.6,
        "crocket_lobe_notch_mm": 0.3,
        "floors": {"section_mm": MIN_SECTION_MM, "sand_web_mm": MIN_SAND_WEB_MM},
    })
}

fn lands_ok(arch: &Arch) -> bool {
    let l = lands(arch);
    let n = |k: &str| l[k].as_f64().unwrap_or(0.0);
    n("niche_bar_mm") >= MIN_SECTION_MM
        && n("niche_to_bore_rail_mm") >= MIN_SECTION_MM
        && n("axial_web_between_floors_mm") >= MIN_SECTION_MM
        && n("crocket_plate_thickness_at_face_mm") >= MIN_SECTION_MM
        && n("niche_width_at_floor_mm") >= MIN_SAND_WEB_MM
        && n("niche_depth_head_mm") > 0.2
}

// --- Renders ---------------------------------------------------------------------------------------

/// Each named view as the direction from the ring toward the eye, world +y (the crown) kept up. The face is the
/// ring's façade, seen along the finger; the side is the keel end-on, its pointed-arch silhouette.
const VIEWS: [(&str, P3); 8] = [
    ("hero", [0.6, 0.32, 0.75]),
    ("face", [0.0, 0.0, 1.0]),
    ("top", [0.02, 1.0, 0.12]),
    ("palm", [0.35, -1.0, 0.45]),
    ("side", [1.0, 0.1, 0.1]),
    ("shoulder", [1.0, 0.75, 0.25]),
    ("reverse", [-0.8, 0.3, -0.55]),
    ("keel", [0.35, 0.25, 1.0]),
];

fn rotated(m: &mesh::Mesh, f: impl Fn(P3) -> P3) -> mesh::Mesh {
    let map = |v: mesh::Vec3| {
        let p = f([v.0 as f64, v.1 as f64, v.2 as f64]);
        mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
    };
    mesh::Mesh { vertices: m.vertices.iter().map(|v| map(*v)).collect(), normals: m.normals.iter().map(|v| map(*v)).collect(), faces: m.faces.clone(), ..mesh::Mesh::default() }
}

/// The mesh in the frame of a camera looking back along `toward` (from the ring to the eye) with world +y kept up, so a plain front view renders it.
fn looked(m: &mesh::Mesh, toward: P3) -> mesh::Mesh {
    let l = (toward[0] * toward[0] + toward[1] * toward[1] + toward[2] * toward[2]).sqrt();
    let z = toward.map(|v| v / l);
    let up = [0.0, 1.0, 0.0];
    let x = [up[1] * z[2] - up[2] * z[1], up[2] * z[0] - up[0] * z[2], up[0] * z[1] - up[1] * z[0]];
    let xl = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let x = x.map(|v| v / xl);
    let y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    // Depth is flattened so the frame fits what the camera sees; the normals keep the true shading.
    let mut out = rotated(m, |p| [dot(p, x), dot(p, y), dot(p, z)]);
    for v in &mut out.vertices {
        v.2 *= 0.05;
    }
    out
}

/// The faces of `m` wholly above height `y`, as a mesh of their own, to frame the crown on.
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

/// The ring cut through the finger's axis midway between two crockets, turned to θ = 90°, the cut face toward +x: the pointed arch in gold.
fn section_mesh(m: &mesh::Mesh) -> Result<mesh::Mesh> {
    let turn = -SECTION_OFF_DEG.to_radians();
    let (st, ct) = turn.sin_cos();
    let m = &rotated(m, |p| [p[0] * ct - p[1] * st, p[0] * st + p[1] * ct, p[2]]);
    let r = 40.0;
    let mut bx = csg::Solid::default();
    // A slab 0.3 mm deep behind the cut, so nothing further round the ring stands over the section.
    let corners: Vec<P3> = (0..8).map(|i| [if i & 1 == 0 { -0.3 } else { 0.0 }, if i & 2 == 0 { -r } else { r }, if i & 4 == 0 { -r } else { r }]).collect();
    bx.v = corners;
    bx.f = vec![[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
    let cut = csg::combine(&solid_of(m), &bx, csg::Op::Intersect).map_err(|e| anyhow::anyhow!("section cut: {e:?}"))?;
    Ok(creased(&mesh_of(&cut)))
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

fn renders(out: &Path, fin: &render::Finished, edge: usize) -> Result<()> {
    let mut parts: Vec<(&mesh::Mesh, bool, [f32; 3])> = vec![(&fin.metal, false, render::GOLD)];
    parts.extend(fin.stones.iter().map(|(m, t)| (m, true, *t)));
    for (name, toward) in VIEWS {
        png(&out.join(format!("{name}.png")), &shot(&parts, toward, edge), edge)?;
    }
    // No stones: the close-up is the crown's crockets and the head of the niches.
    let crown = crop_above(&fin.metal, 6.0);
    png(&out.join("stones.png"), &shot(&[(&crown, false, render::GOLD)], [0.7, 0.45, 0.55], edge), edge)?;
    let section = crop_above(&section_mesh(&fin.metal)?, 7.5);
    let sec = [(&section, false, render::GOLD)];
    png(&out.join("section.png"), &shot(&sec, [1.0, 0.0, 0.0001], edge), edge)?;
    let bare = plain_ring()?;
    side_by_side(&out.join("bare-vs-finished.png"), &[shot(&[(&bare, false, render::GOLD)], VIEWS[0].1, edge), shot(&parts, VIEWS[0].1, edge)], edge)?;
    png(&out.join("hero-300.png"), &shot(&parts, VIEWS[0].1, 300), 300)?;
    png(&out.join("face-300.png"), &shot(&parts, VIEWS[1].1, 300), 300)?;
    let sheet: Vec<Vec<u8>> = [VIEWS[0].1, VIEWS[1].1, VIEWS[4].1, VIEWS[2].1].iter().map(|t| shot(&parts, *t, 300)).chain(std::iter::once(shot(&sec, [1.0, 0.0, 0.0001], 300))).collect();
    side_by_side(&out.join("contact-300.png"), &sheet, 300)?;
    Ok(())
}

/// The revolved section alone, the stock every other feature works on.
fn plain_ring() -> Result<mesh::Mesh> {
    let (mut d, _) = author()?;
    let doc = d.cad.as_mut().unwrap();
    doc.features.truncate(2);
    doc.outputs = vec![2];
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
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/ogiva"));
    std::fs::create_dir_all(&out)?;
    println!("Ogiva");
    let started = std::time::Instant::now();
    let (d, arch) = author()?;
    let lib = AlphaLibrary::builtin();

    println!("  arch: {arch:?}");
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
    // The sand release study at both pitches, the ring judged as its parts alone.
    let mut s100 = d.manufacturing.clone().unwrap();
    s100.sample_pitch_mm = 0.100;
    let i100 = mf::inspect(&d, &lib, &s100, params)?;
    let mut s075 = s100.clone();
    s075.sample_pitch_mm = 0.075;
    let i075 = mf::inspect(&d, &lib, &s075, params)?;
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let findings = dfm::findings_in(&d, &lib);
    let fin = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(&d, &lib, &built) };
    let stones = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
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
    let release = |i: &mf::Inspection| {
        json!({"pitch_mm": i.release.cell_mm, "status": i.release.status, "obstructions": i.release.obstructions.len(), "unresolved_rays": i.release.unresolved_rays, "fits_flask": i.release.fits_flask, "low_draft_area_mm2": i.release.low_draft_area_mm2, "local_wall": i.local_wall, "details": i.details})
    };
    let clean = |i: &mf::Inspection| i.release.obstructions.is_empty() && i.release.unresolved_rays == 0 && i.release.fits_flask;
    let wall_ok = |i: &mf::Inspection| i.local_wall.as_ref().is_some_and(|w| w.below_limit == 0);
    let field_ok = field.verdict == castability::Verdict::Castable;
    let grams = built.report.metals.iter().find(|m| m.metal == "Silver 925").map_or(0.0, |m| m.grams);
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings, every feature Ok", made.iter().all(|(_, n)| *n == 0) && features_ok),
        ("solids and parts notes empty", built.solids.notes.is_empty() && built.parts.notes.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("sand field verdict Castable", field_ok),
        ("ray release clean at 0.100 mm", clean(&i100)),
        ("ray release clean at 0.075 mm", clean(&i075)),
        ("local wall at or above the 0.8 mm section", wall_ok(&i100)),
        ("sketch lands at or above the floors", lands_ok(&arch)),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let features: Vec<serde_json::Value> = d.cad.as_ref().unwrap().features.iter().map(|f| json!({"id": f.id, "name": f.name, "operation": f.operation.label()})).collect();
    let report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "sand": "Delft clay",
        "alloy": "Silver 925",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s},
        "arch": arch,
        "geometry": {"watertight": watertight, "boundary_edges": built.report.validation.boundary_edges, "non_manifold_edges": built.report.validation.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "shells": shells(&built.mesh), "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm},
        "made_parts": made,
        "feature_status": statuses,
        "solids": {"notes": built.solids.notes, "parts_notes": built.parts.notes, "stamps": 0},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "undercut_percent": field.undercut_fraction() * 100.0, "notes": field.notes, "min_section_mm": d.draft.min_section_mm, "min_draft_deg": d.draft.min_draft_deg},
        "release_0_100": release(&i100),
        "release_0_075": release(&i075),
        "land_widths": lands(&arch),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "grams_silver": grams,
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len()},
        "design": {"bytes": text.len(), "cad_features": features.len()},
        "features": features,
        "cold_reload_identical": cold,
        "draft": if draft { json!({"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "parting_z_mm": d.draft.parting_z_mm, "auto_parting": d.draft.auto_parting}) } else { serde_json::Value::Null },
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let report_path = out.join("report.json");
    // The draft build writes its own block; the export keeps it beside its own numbers.
    let mut report = report;
    if !draft {
        if let Ok(old) = std::fs::read(&report_path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<serde_json::Value>(&b)?)) {
            report["draft"] = old.get("draft_build").cloned().unwrap_or(serde_json::Value::Null);
        }
    }
    if draft {
        report["draft_build"] = report.clone();
    }
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Ogiva / sand pattern")?;
    }
    renders(&out, &fin, if draft { 900 } else { 1600 })?;
    println!(
        "  field {} ; release 0.100: {} obstructions, {} unresolved; 0.075: {} obstructions, {} unresolved; wall {:?}; dfm {}; {:.2} g silver",
        field.verdict.label(),
        i100.release.obstructions.len(),
        i100.release.unresolved_rays,
        i075.release.obstructions.len(),
        i075.release.unresolved_rays,
        i100.local_wall.as_ref().and_then(|w| w.sampled_min_mm),
        findings.len(),
        grams
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Ogiva failed a gate; see {}", report_path.display());
    Ok(())
}
