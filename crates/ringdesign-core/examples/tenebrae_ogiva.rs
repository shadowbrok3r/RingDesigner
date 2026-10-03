//! Tenebrae — Ogiva, the arch: seen along the finger, the ring is one great equilateral pointed arch on two piers, every
//! form drawn on the parting plane and extruded along the pull, poured in Delft sand as a parts-only ring.
//! cargo build --release -p ringdesign-core --example tenebrae_ogiva
//! target/release/examples/tenebrae_ogiva [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Boolean, Component, ComponentRole, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, PlaneBase, Profile, pattern::Sources},
    castability::{self, CastProcess, SandProcess},
    csg, dfm, library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

type P2 = [f64; 2];
type P3 = [f64; 3];
type Keys = Vec<(f64, f64, bool)>;

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Delft clay draft, degrees.
const DRAFT_DEG: f64 = 3.0;
/// How far each drafted half starts past the parting line, so the halves overlap without a waist the release study reads as undercut, mm.
const HAIR_MM: f64 = 0.008;
/// Chord the outlines are walked in, mm.
const CHORD_MM: f64 = 0.12;
/// Delft section and sand web floors, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_SAND_WEB_MM: f64 = 0.6;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: P2) -> f64 {
    a[0].hypot(a[1])
}

/// Points round an arc about `c` of radius `r` from angle `a0` to `a1` (radians, either way), the start left out.
fn arc_pts(c: P2, r: f64, a0: f64, a1: f64) -> Vec<P2> {
    let n = ((a1 - a0).abs() * r / CHORD_MM).ceil().max(2.0) as usize;
    (1..=n).map(|i| a0 + (a1 - a0) * i as f64 / n as f64).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect()
}

struct Tree {
    doc: Document,
}

impl Tree {
    fn add(&mut self, name: &str, operation: Operation, role: ComponentRole) -> Id {
        let id = self.doc.features.len() as Id + 1;
        let component = Component { role, material: "Silver 925".into(), ..Component::default() };
        self.doc.append(Feature { id, name: name.into(), enabled: true, operation, component }).expect("feature appends");
        id
    }
    fn plane(&mut self, name: &str, offset_mm: f64) -> Id {
        self.add(name, Operation::Plane { base: PlaneBase::Parting, offset_mm }, ComponentRole::Other)
    }
    fn sketch(&mut self, name: &str, plane: Id, loops: &[Shape]) -> Id {
        let mut s = Sketch::default();
        s.name = name.into();
        s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
        for l in loops {
            match l {
                Shape::Poly(l) => {
                    // A polyline takes at most 512 points: a long loop keeps every k-th.
                    let k = l.len().div_ceil(500);
                    let ids: Vec<Id> = l.iter().step_by(k).map(|p| s.point(*p)).collect();
                    s.entity(Geometry::Polyline { points: ids, closed: true });
                }
                Shape::Circle { centre, radius } => {
                    let center = s.point(*centre);
                    let rim = s.point([centre[0] + radius, centre[1]]);
                    s.entity(Geometry::Circle { center, rim });
                }
                Shape::Arch { arch, kind, d, round } => {
                    // Walked in chords: the kernel's drafted true arcs tessellate with open seams at some resolutions.
                    let pts = match kind {
                        PathKind::Outline => arch.outline(*d, *round),
                        PathKind::Head { drop } => arch.head_region(*d, *round, *drop),
                    };
                    let ids: Vec<Id> = pts.iter().map(|p| s.point(*p)).collect();
                    s.entity(Geometry::Polyline { points: ids, closed: true });
                }
            }
        }
        self.add(name, Operation::Sketch { sketch: s }, ComponentRole::Other)
    }
    fn extrude(&mut self, name: &str, sketch: Id, height_mm: f64, draft_deg: f64) -> Id {
        self.add(name, Operation::Extrude { sketch: Profile::Feature { feature: sketch }, height_mm, draft_deg }, ComponentRole::Other)
    }
    fn mirror(&mut self, name: &str, sources: Vec<Id>) -> Id {
        self.add(name, Operation::Pattern { sources: Sources(sources), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, ComponentRole::Other)
    }
    fn boolean(&mut self, name: &str, a: Id, b: Id, kind: Boolean) -> Id {
        self.add(name, Operation::Boolean { a, b, kind }, ComponentRole::Other)
    }
    /// Loops drawn on a plane at `z0` and raised to `z1` along the pull in the cope with draft, mirrored into the drag, the halves joined.
    fn slab(&mut self, what: &str, loops: &[Shape], z0: f64, z1: f64, draft: f64) -> Id {
        let plane = self.plane(&format!("Lay a plane at z {z0:+.3} for the {what}"), z0);
        let s = self.sketch(&format!("Draw the {what}"), plane, loops);
        let cope = self.extrude(&format!("Raise the cope half of the {what} along the pull"), s, z1 - z0, draft);
        let drag = self.mirror(&format!("Mirror the {what} into the drag half"), vec![cope]);
        self.boolean(&format!("Join the halves of the {what}"), cope, drag, Boolean::Union)
    }
    /// Loops raised from a hair under the parting line to `half` either side.
    fn both_halves(&mut self, what: &str, loops: &[Shape], half: f64, draft: f64) -> Id {
        self.slab(what, loops, -HAIR_MM, half, draft)
    }
    /// Loops raised to `half` either side of the parting line: a straight belt across it, BELT_MM each way, and drafted
    /// halves from inside the belt out to the faces.
    fn belted(&mut self, what: &str, loops: &[Shape], half: f64, draft: f64) -> Id {
        // A part's belt stands a little past the keel's land, so no two belt tops share a plane where the part meets the keel.
        let belt_mm = BELT_MM + PART_BELT_EXTRA_MM;
        let plane = self.plane(&format!("Lay a plane under the parting line for the {what}'s belt"), -belt_mm);
        let s = self.sketch(&format!("Draw the {what}'s belt"), plane, loops);
        let belt = self.extrude(&format!("Run the {what}'s belt straight across the parting line"), s, 2.0 * belt_mm, 0.0);
        let inner: Vec<Shape> = loops.iter().map(|l| l.inset(HALF_IN_MM)).collect();
        let halves = self.slab(what, &inner, HALF_FROM_MM + PART_BELT_EXTRA_MM, half, draft);
        self.boolean(&format!("Join the {what}'s halves to its belt"), belt, halves, Boolean::Union)
    }
    /// A cut through the ring along the pull: drafted halves from each face to inside a straight belt at the parting
    /// line, the belt drawn at the halves' narrowest, so each opening narrows toward the parting line.
    fn belted_cut(&mut self, what: &str, loops: &[Shape], top: f64, draft: f64) -> Id {
        let floor = HALF_FROM_MM + 0.04;
        let (cope, drag) = self.pocket(what, loops, top, floor, draft);
        let narrow = (top - floor) * draft.to_radians().tan() + HALF_IN_MM;
        let inner: Vec<Shape> = loops.iter().map(|l| l.inset(narrow)).collect();
        let plane = self.plane(&format!("Lay a plane under the parting line for the {what}'s belt"), -(BELT_MM + 0.04));
        let s = self.sketch(&format!("Draw the {what}'s belt"), plane, &inner);
        let belt = self.extrude(&format!("Run the {what}'s belt straight across the parting line"), s, 2.0 * (BELT_MM + 0.04), 0.0);
        let both = self.boolean(&format!("Join the halves of the {what}"), cope, drag, Boolean::Union);
        self.boolean(&format!("Join the {what}'s belt"), both, belt, Boolean::Union)
    }

    /// A pocket drawn on a plane `top` over the side face and sunk to `floor` along the pull in the cope with `draft`, mirrored into the drag.
    fn pocket(&mut self, what: &str, loops: &[Shape], top: f64, floor: f64, draft: f64) -> (Id, Id) {
        let plane = self.plane(&format!("Lay a plane over the face for the {what}"), top);
        let s = self.sketch(&format!("Draw the {what}"), plane, loops);
        let cope = self.extrude(&format!("Sink the {what} along the pull"), s, -(top - floor), draft);
        let drag = self.mirror(&format!("Mirror the {what} into the drag side"), vec![cope]);
        (cope, drag)
    }
}

fn setup() -> mf::Setup {
    let mut s = probe::sand_setup(0.10);
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(1.5, |m| m.shrink_pct);
    s.recipe.process = CastProcess::SandTwoPart;
    s.recipe.name = "Ogiva / Delft clay / Silver 925".into();
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, sand and measured trials.".into();
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.9, 0.0], end: [0.0, -20.0, 0.0], diameter_mm: 3.2 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -20.0, 0.0], end: [0.0, -28.0, 0.0], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Pour in two-part Delft clay with the parting on the keel (z = 0), pulling along the finger. Gate at the sill under the palm. Break the flash along the keel's round with a fine file, keeping the crease's line; polish the face, the orders' chamfers and the capitals; leave the mouth and the niche floors as cast, or satin them.".into();
    s
}

// --- The arch ------------------------------------------------------------------------------------

/// The great arch seen along the finger: an equilateral pointed head (two arcs of radius `2 × half_span` about the
/// opposite springers) springing at `spring_y`, standing on two straight piers that turn into a flat sill under the palm.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct GreatArch {
    half_span: f64,
    spring_y: f64,
    radius: f64,
    apex_y: f64,
    sill_y: f64,
    corner: f64,
    width: f64,
}

impl GreatArch {
    fn of(rev: bool) -> Self {
        let (half_span, spring_y, sill_y, corner, width): (f64, f64, f64, f64, f64) = if rev { (12.15, -3.0, -11.0, 2.4, 6.0) } else { (12.15, -3.0, -11.0, 2.4, 6.0) };
        let radius = 2.0 * half_span;
        let c = radius - half_span;
        let apex_y = spring_y + (radius * radius - c * c).sqrt();
        Self { half_span, spring_y, radius, apex_y, sill_y, corner, width }
    }
    /// The right arc's centre (on the left springer for an equilateral arch).
    fn centre(&self, side: f64) -> P2 {
        [-side * (self.radius - self.half_span), self.spring_y]
    }
    /// The arch's head offset `d` inward, from the right springer up to the apex and down to the left: the two
    /// arcs at radius `radius - d` about their centres, meeting on the axis in a round of `round` mm.
    fn head(&self, d: f64, round: f64) -> Vec<P2> {
        let r = self.radius - d;
        let cr = self.centre(1.0);
        let cl = self.centre(-1.0);
        let c = cr[0].abs();
        // The round's centre stands on the axis, r − round from both arc centres; each arc ends where the round meets it.
        let f = [0.0, self.spring_y + ((r - round).powi(2) - c * c).sqrt()];
        let end = (f[1] - cr[1]).atan2(f[0] - cr[0]);
        let mut pts = vec![[cr[0] + r, self.spring_y]];
        pts.extend(arc_n(cr, r, 0.0, end, HEAD_STEPS));
        pts.extend(arc_n(f, round, end, PI - end, 4));
        pts.extend(arc_n(cl, r, PI - end, PI, HEAD_STEPS));
        pts
    }
    /// The head offset `d` inward, run straight down `drop` below the spring line on both legs and closed across.
    fn head_region(&self, d: f64, round: f64, drop: f64) -> Vec<P2> {
        let mut pts = self.head(d, round);
        let (right, left) = (pts[0], *pts.last().unwrap());
        pts.push([left[0], left[1] - drop]);
        pts.insert(0, [right[0], right[1] - drop]);
        pts
    }
    /// One of the arch's paths `d` in from the outline, as a shape a sketch draws.
    fn shape(&self, kind: PathKind, d: f64, round: f64) -> Shape {
        Shape::Arch { arch: *self, kind, d, round }
    }
    /// The whole outline offset `d` inward: the head, down the left pier, round the corner along the sill, and up the right pier.
    fn outline(&self, d: f64, round: f64) -> Vec<P2> {
        let mut pts = self.head(d, round);
        let x = self.half_span - d;
        let k = self.corner - d;
        let (cx, cy) = (self.half_span - self.corner, self.sill_y + self.corner);
        pts.push([-x, cy]);
        pts.extend(arc_n([-cx, cy], k, PI, 1.5 * PI, 10));
        pts.push([cx, self.sill_y + d]);
        pts.extend(arc_n([cx, cy], k, 1.5 * PI, 2.0 * PI, 10));
        pts.pop();
        pts
    }
}

/// Which of the arch's paths a shape draws.
#[derive(Clone, Copy, Debug)]
enum PathKind {
    Outline,
    Head { drop: f64 },
}

/// A closed loop a sketch draws: a polyline, one of the arch's paths `d` in with its apex round, or a circle.
#[derive(Clone, Debug)]
enum Shape {
    Poly(Vec<P2>),
    Arch { arch: GreatArch, kind: PathKind, d: f64, round: f64 },
    Circle { centre: P2, radius: f64 },
}

impl Shape {
    /// The same loop drawn `by` further in.
    fn inset(&self, by: f64) -> Shape {
        match self {
            Shape::Poly(l) => Shape::Poly(inset_loop(l, by)),
            Shape::Arch { arch, kind, d, round } => Shape::Arch { arch: *arch, kind: *kind, d: d + by, round: round - by },
            Shape::Circle { centre, radius } => Shape::Circle { centre: *centre, radius: radius - by },
        }
    }
}

/// Chords on each head arc.
const HEAD_STEPS: usize = 16;

/// `n` chords round an arc about `c`, the start left out.
fn arc_n(c: P2, r: f64, a0: f64, a1: f64, n: usize) -> Vec<P2> {
    (1..=n).map(|i| a0 + (a1 - a0) * i as f64 / n as f64).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect()
}

// --- The ring's numbers ----------------------------------------------------------------------------

/// Every form that straddles the parting line has a straight belt across it, BELT_MM either side, and its drafted halves
/// start inside the belt: the ledge where a half leaves the belt then stands 2 × BELT_MM of metal over its mirror, and no
/// hair-thin lip is left where two overlapped halves meet.
const BELT_MM: f64 = 0.41;
/// Where each drafted half starts inside the belt, and how far inside the belt's outline it is drawn, mm.
const HALF_FROM_MM: f64 = 0.38;
const HALF_IN_MM: f64 = 0.012;
/// How much further a part's belt runs than the keel's land.
const PART_BELT_EXTRA_MM: f64 = 0.023;
/// The keel's steep flanks start a finer hair inside its land: their drafted inset leaves the apex round little to spare.
const KEEL_HALF_IN_MM: f64 = 0.006;
/// The keel: its crease blunted to the belt's land, KEEL_LAND inside the outline, then a flank falling at KEEL_FLANK_DEG
/// to KEEL_IN at the face.
const KEEL_LAND: f64 = 0.15;
const KEEL_FLANK_DEG: f64 = 37.0;
const KEEL_IN: f64 = 0.6;
/// The order sunk into the head (how far inside the outline, its floor's depth, its chamfer's draft), and the mouth
/// (how far inside the outline, how far it keeps off the bore, its depth).
const ORDER: (f64, f64, f64) = (1.45, 0.6, 35.0);
const MOUTH: (f64, f64, f64) = (2.15, 0.9, 1.6);
/// The capitals at the springers: abacus and bell heights and how far each projects past the pier, and how proud of the face.
/// Each member is drawn tall enough that what both drafts leave of it at its proud face still holds the 0.8 mm section.
const ABACUS: (f64, f64) = (1.4, 0.95);
const BELL: (f64, f64) = (1.2, 0.55);
/// How far inside the capital's own outline the abacus's boss is drawn, clear of the capital's drafted face.
const ABACUS_BOSS_INSET: f64 = 0.2;
const CAPITAL_PROUD: (f64, f64) = (0.5, 0.3);
/// How far in from the pier's outer face the capitals reach: they stop short of the bore, whose drafted wall would
/// otherwise trim them to a wedge.
const CAPITAL_IN: f64 = 2.35;
/// Crockets up each slope of the extrados, as shares of the head arc, each one's growth, and the plate's half-width along the finger.
const CROCKETS: [(f64, f64); 6] = [(0.16, 0.85), (0.29, 0.92), (0.42, 1.0), (0.55, 1.07), (0.68, 1.14), (0.81, 1.2)];
const CROCKET_HALF: f64 = 0.9;
/// How deep each crocket and the finial sink below the outline.
const CROCKET_SINK: f64 = 1.2;
/// A blind lancet niche in each pier face: centre off the axis, width, sill and apex heights, floor depth below the face.
const PIER_NICHE: (f64, f64, f64, f64, f64) = (9.4, 1.0, -9.4, -6.1, 0.7);
/// The pierced trefoil in the mouth over the finger: centre height, lobe radius, lobe centres' distance from the centre.
const TREFOIL: (f64, f64, f64) = (12.6, 0.7, 0.65);

/// The keel's rise: from the land at the belt's top to KEEL_IN at the flank's draft.
fn keel_rise() -> f64 {
    HALF_FROM_MM + (KEEL_IN - KEEL_LAND - KEEL_HALF_IN_MM) / KEEL_FLANK_DEG.to_radians().tan()
}

/// A loop drawn `d` further in (out for a negative `d`), each point moved along its corner's bisecting normal: exact
/// enough for the hair of an inset it is used for, on loops whose turns are wider than it.
fn inset_loop(pts: &[P2], d: f64) -> Vec<P2> {
    let n = pts.len();
    let area: f64 = (0..n).map(|i| pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1]).sum::<f64>() / 2.0;
    let side = if area > 0.0 { 1.0 } else { -1.0 };
    (0..n)
        .map(|i| {
            let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let e = |p: P2, q: P2| {
                let v = sub2(q, p);
                let l = len2(v).max(1e-12);
                [-v[1] / l * side, v[0] / l * side]
            };
            let (n1, n2) = (e(a, b), e(b, c));
            let m = [n1[0] + n2[0], n1[1] + n2[1]];
            let l = len2(m).max(1e-12);
            let cos = (m[0] * n1[0] + m[1] * n1[1]) / l;
            let k = d / cos.max(0.2);
            [b[0] + m[0] / l * k, b[1] + m[1] / l * k]
        })
        .collect()
}

/// A closed outline through key points `(u, v, sharp)`, a cubic Hermite between each pair with Catmull–Rom tangents, its
/// corners cut and the loop walked at `pitch`, so every tip is rounder than a drafted inset.
fn spline_at(keys: &[(f64, f64, bool)], pitch: f64) -> Vec<P2> {
    let n = keys.len();
    let p = |i: usize| -> P2 { [keys[i % n].0, keys[i % n].1] };
    let tangent = |i: usize| -> P2 {
        let (a, b) = (p((i + n - 1) % n), p((i + 1) % n));
        let k = if keys[i % n].2 { 0.12 } else { 0.5 };
        [(b[0] - a[0]) * k, (b[1] - a[1]) * k]
    };
    let mut out = Vec::new();
    for i in 0..n {
        let (p0, p1, m0, m1) = (p(i), p(i + 1), tangent(i), tangent(i + 1));
        let steps = (len2(sub2(p1, p0)) / CHORD_MM).ceil().max(2.0) as usize;
        for s in 0..steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            let h = [2.0 * t3 - 3.0 * t2 + 1.0, t3 - 2.0 * t2 + t, -2.0 * t3 + 3.0 * t2, t3 - t2];
            out.push([0, 1].map(|k| h[0] * p0[k] + h[1] * m0[k] + h[2] * p1[k] + h[3] * m1[k]));
        }
    }
    smoothed_at(&out, pitch)
}

/// A loop walked at 0.3 mm, its corners cut twice and walked again at 0.28 mm.
fn smoothed_at(pts: &[P2], pitch: f64) -> Vec<P2> {
    let mut out = even(pts, 0.45);
    for _ in 0..2 {
        let n = out.len();
        out = (0..n)
            .flat_map(|i| {
                let (a, b) = (out[i], out[(i + 1) % n]);
                [[0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]], [0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]]
            })
            .collect();
    }
    even(&out, pitch)
}

/// A closed loop walked again at an even pitch along its length.
fn even(pts: &[P2], pitch: f64) -> Vec<P2> {
    let n = pts.len();
    let total: f64 = (0..n).map(|i| len2(sub2(pts[(i + 1) % n], pts[i]))).sum();
    let count = (total / pitch).round().max(3.0) as usize;
    let step = total / count as f64;
    let mut out = Vec::with_capacity(count);
    let (mut i, mut walked) = (0, 0.0);
    for k in 0..count {
        let at = k as f64 * step;
        while walked + len2(sub2(pts[(i + 1) % n], pts[i])) < at {
            walked += len2(sub2(pts[(i + 1) % n], pts[i]));
            i += 1;
        }
        let (a, b) = (pts[i % n], pts[(i + 1) % n]);
        let l = len2(sub2(b, a)).max(1e-12);
        let t = (at - walked) / l;
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
    out
}

/// A lancet `w` wide: a flat sill at `from`, straight jambs and an equilateral pointed head with its apex at `to`, x across it and y up it.
fn lancet(w: f64, from: f64, to: f64) -> Vec<P2> {
    let h = w / 2.0;
    let spring = to - w * 3f64.sqrt() / 2.0;
    let mut pts = vec![[-h, from], [h, from], [h, spring]];
    let steps = 6;
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

/// One crocket in its own frame, `t` along the slope toward the apex and `n` out of the extrados: a leaf rising from the
/// keel, leaning toward the apex and rolling its tip forward into a bud.
fn crocket() -> Keys {
    let k = -CROCKET_SINK;
    vec![(-0.75, k, true), (0.75, k, true), (0.8, 0.2, false), (1.3, 0.7, false), (1.1, 1.25, false), (0.45, 1.4, false), (-0.2, 1.1, false), (-0.65, 0.5, false)]
}

/// The finial on the apex, symmetric about the axis: a stem opening into three lobes, the middle one pointed.
fn finial() -> Keys {
    let k = -CROCKET_SINK;
    vec![
        (-0.9, k, true),
        (0.9, k, true),
        (0.6, 0.3, false),
        (1.35, 0.9, false),
        (1.0, 1.55, false),
        (0.45, 1.25, false),
        (0.45, 1.9, false),
        (0.3, 2.5, false),
        (0.0, 2.75, false),
        (-0.3, 2.5, false),
        (-0.45, 1.9, false),
        (-0.45, 1.25, false),
        (-1.0, 1.55, false),
        (-1.35, 0.9, false),
        (-0.6, 0.3, false),
    ]
}

/// Keys laid at `origin` with `t` along the slope and `n` out of it, `grow` times their size.
fn laid(keys: &Keys, origin: P2, t: P2, n: P2, grow: f64) -> Vec<P2> {
    // Walked coarser than other outlines: a crocket is drawn twice, belt and halves, and its curl reads at this pitch.
    spline_at(&keys.iter().map(|(a, b, s)| (a * grow, if *b > 0.0 { b * grow } else { *b }, *s)).collect::<Vec<_>>(), 0.55)
        .into_iter()
        .map(|[a, b]| [origin[0] + a * t[0] + b * n[0], origin[1] + a * t[1] + b * n[1]])
        .collect()
}

/// The trefoil: three lobes round a centre, walked as the outline a ray from the centre meets, its cusps eased.
fn trefoil(centre: P2, lobe: f64, out: f64) -> Vec<P2> {
    smoothed_at(&trefoil_raw(centre, lobe, out), 0.4)
}

fn trefoil_raw(centre: P2, lobe: f64, out: f64) -> Vec<P2> {
    let lobes: Vec<P2> = [90.0f64, 210.0, 330.0].iter().map(|a| [centre[0] + out * a.to_radians().cos(), centre[1] + out * a.to_radians().sin()]).collect();
    (0..144)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / 144.0;
            let d = [a.cos(), a.sin()];
            let reach = lobes
                .iter()
                .map(|c| {
                    let oc = sub2(*c, centre);
                    let b = oc[0] * d[0] + oc[1] * d[1];
                    let q = b * b - (oc[0] * oc[0] + oc[1] * oc[1] - lobe * lobe);
                    if q >= 0.0 { b + q.sqrt() } else { 0.0 }
                })
                .fold(0.0, f64::max);
            [centre[0] + reach * d[0], centre[1] + reach * d[1]]
        })
        .collect()
}

/// A capital at the right springer, mirrored for the left: the bell flaring from the pier's face out to the abacus
/// as one outline, or the abacus's boss standing on it, `inset` inside the abacus's own outline.
fn capital(a: &GreatArch, side: f64, part: &str) -> Vec<P2> {
    let (x_o, x_i, s) = (a.half_span, a.half_span - CAPITAL_IN, a.spring_y);
    let (abacus_bot, bell_bot) = (s - ABACUS.0, s - ABACUS.0 - BELL.0);
    let b = ABACUS_BOSS_INSET;
    let pts: Vec<P2> = match part {
        "abacus" => vec![[x_i + b, abacus_bot + b], [x_o + ABACUS.1 - b, abacus_bot + b], [x_o + ABACUS.1 - b, s - b], [x_i + b, s - b]],
        _ => vec![[x_i, bell_bot], [x_o, bell_bot], [x_o + BELL.1, abacus_bot], [x_o + ABACUS.1, abacus_bot], [x_o + ABACUS.1, s], [x_i, s]],
    };
    let pts: Vec<P2> = pts.into_iter().map(|[x, y]| [side * x, y]).collect();
    if side < 0.0 { pts.into_iter().rev().collect() } else { pts }
}

/// The plain arch: the keel's land straight across the parting line, its flanks falling to the face, and the face inside it.
fn arch_body(t: &mut Tree, a: &GreatArch) -> Id {
    let half = a.width / 2.0;
    let round = KEEL_IN + 0.1;
    let plane = t.plane("Lay a plane under the parting line for the keel's land", -BELT_MM);
    let s = t.sketch("Draw the keel's land", plane, &[a.shape(PathKind::Outline, KEEL_LAND, round - KEEL_LAND)]);
    let land = t.extrude("Run the keel's land straight across the parting line", s, 2.0 * BELT_MM, 0.0);
    let from = KEEL_LAND + KEEL_HALF_IN_MM;
    let flank = t.slab("keel's flanks", &[a.shape(PathKind::Outline, from, round - from)], HALF_FROM_MM, keel_rise(), KEEL_FLANK_DEG);
    let keel = t.boolean("Raise the keel's flanks from its land", land, flank, Boolean::Union);
    let face = t.both_halves("arch's face, inside the keel", &[a.shape(PathKind::Outline, KEEL_IN, 0.25)], half, DRAFT_DEG);
    t.boolean("Set the face on the keel", keel, face, Boolean::Union)
}

fn bore(t: &mut Tree, a: &GreatArch) -> Id {
    // The bore, a cone widening from a straight belt at the parting line out to each face.
    let r = BORE_MM / 2.0 + (a.width / 2.0 + 0.8 - HALF_FROM_MM - 0.04) * DRAFT_DEG.to_radians().tan() + HALF_IN_MM;
    t.belted_cut("bore", &[Shape::Circle { centre: [0.0, 0.0], radius: r }], a.width / 2.0 + 0.8, DRAFT_DEG)
}

/// The mouth over the finger: inside the head `d` in from the outline and outside a circle `keep` off the bore, a pointed
/// crescent standing on the bore's crown, its two ends squared off where it is still 0.7 mm wide.
fn mouth(a: &GreatArch, d: f64, keep: f64) -> Vec<P2> {
    let r = a.radius - d;
    let rc = BORE_MM / 2.0 + keep;
    let cr = a.centre(1.0);
    let head_x = |y: f64| cr[0] + (r * r - (y - cr[1]).powi(2)).max(0.0).sqrt();
    let circle_x = |y: f64| (rc * rc - y * y).max(0.0).sqrt();
    // Up from the springers to where the head arc stands 0.7 mm clear of the circle, above the bore's side.
    let mut y = 0.0;
    while head_x(y) - circle_x(y) < 0.7 && y < rc {
        y += 0.02;
    }
    let (xh, xc) = (head_x(y), circle_x(y));
    let mut pts = vec![[xc, y], [xh, y]];
    pts.extend(a.head(d, 0.3).into_iter().filter(|q| q[1] > y + 0.05));
    pts.push([-xh, y]);
    pts.push([-xc, y]);
    // Back along the circle over the crown, from the left end to the right.
    let mut back = arc_pts([0.0, 0.0], rc, y.atan2(-xc), y.atan2(xc));
    back.pop();
    pts.extend(back);
    // Its four corners eased, so the drafted floor keeps every segment.
    smoothed_at(&pts, 0.55)
}

/// The finished arch: the keel and face, the crockets climbing to the finial, the orders and the mouth sunk into the head,
/// the capitals at the springers, a blind lancet in each pier face, the trefoil pierced through the mouth, and the bore.
fn author(bare: bool) -> Result<(RingDesign, GreatArch)> {
    let a = GreatArch::of(false);
    let half = a.width / 2.0;
    let mut t = Tree { doc: Document::default() };
    let mut cur = arch_body(&mut t, &a);
    if !bare {
        // Crockets up both slopes of the extrados, leaning toward the apex, and the finial on it.
        let mut leaves = Vec::new();
        let r = a.radius;
        let cr = a.centre(1.0);
        let top = (cr[0].abs() / r).acos();
        for (share, grow) in CROCKETS {
            let phi = top * share;
            let n = [phi.cos(), phi.sin()];
            let p = [cr[0] + r * n[0], cr[1] + r * n[1]];
            leaves.push(laid(&crocket(), p, [-n[1], n[0]], n, grow));
            leaves.push(laid(&crocket(), [-p[0], p[1]], [-n[1] * -1.0, n[0]], [-n[0], n[1]], grow).into_iter().rev().collect());
        }
        leaves.push(laid(&finial(), [0.0, a.apex_y], [1.0, 0.0], [0.0, 1.0], 1.3));
        let leaves: Vec<Shape> = leaves.into_iter().map(Shape::Poly).collect();
        let crockets = t.belted("crockets climbing the extrados to the finial", &leaves, CROCKET_HALF, DRAFT_DEG);
        cur = t.boolean("Set the crockets and the finial on the keel", cur, crockets, Boolean::Union);
        // The order, sunk into the head and run down onto the capitals, and the mouth sunk deeper over the finger.
        let top = half + 0.3;
        let (d, depth, draft) = ORDER;
        let inset = (top - (half - depth)) * draft.to_radians().tan();
        let drop = (ABACUS.0 * 0.6).max(inset + 0.237);
        let (cope, drag) = t.pocket("order", &[a.shape(PathKind::Head { drop }, d, inset + 0.3)], top, half - depth, draft);
        cur = t.boolean("Sink the cope's order", cur, cope, Boolean::Subtract);
        cur = t.boolean("Sink the drag's order", cur, drag, Boolean::Subtract);
        let (d, keep, depth) = MOUTH;
        let (cope, drag) = t.pocket("mouth over the finger", &[Shape::Poly(mouth(&a, d, keep))], top, half - depth, DRAFT_DEG);
        cur = t.boolean("Sink the cope's mouth", cur, cope, Boolean::Subtract);
        cur = t.boolean("Sink the drag's mouth", cur, drag, Boolean::Subtract);
        // The capitals: the bell, then the abacus standing prouder over it.
        let bells = [Shape::Poly(capital(&a, 1.0, "bell")), Shape::Poly(capital(&a, -1.0, "bell"))];
        let abaci = [Shape::Poly(capital(&a, 1.0, "abacus")), Shape::Poly(capital(&a, -1.0, "abacus"))];
        let b = t.belted("capitals", &bells, half + CAPITAL_PROUD.1, DRAFT_DEG);
        cur = t.boolean("Set the capitals under the springers", cur, b, Boolean::Union);
        // The abacus stands prouder than the bell as a boss on the capital's face, drawn from just inside that face.
        let ab = t.slab("abaci", &abaci, half + CAPITAL_PROUD.1 - 0.05, half + CAPITAL_PROUD.0, DRAFT_DEG);
        cur = t.boolean("Lay the abaci on the capitals", cur, ab, Boolean::Union);
        // A blind lancet niche in each pier face.
        let (x, w, from, to, depth) = PIER_NICHE;
        let niche = |side: f64| -> Vec<P2> { lancet(w, from, to).into_iter().map(|[u, v]| [side * x + u, v]).collect() };
        let (cope, drag) = t.pocket("blind lancets in the pier faces", &[Shape::Poly(niche(1.0)), Shape::Poly(niche(-1.0))], half + 0.3, half - depth, DRAFT_DEG);
        cur = t.boolean("Sink the cope's pier lancets", cur, cope, Boolean::Subtract);
        cur = t.boolean("Sink the drag's pier lancets", cur, drag, Boolean::Subtract);
        // The trefoil pierced through the mouth's web along the pull, each half narrowing to the parting line.
        let (ty, lobe, out) = TREFOIL;
        let light = t.belted_cut("trefoil through the mouth", &[Shape::Poly(trefoil([0.0, ty], lobe, out))], half + 0.3, DRAFT_DEG);
        cur = t.boolean("Pierce the trefoil through the mouth", cur, light, Boolean::Subtract);
    }
    let b = bore(&mut t, &a);
    t.boolean("Pierce the bore through the arch", cur, b, Boolean::Subtract);
    t.doc.features.last_mut().unwrap().component.role = ComponentRole::Shank;
    let mut d = RingDesign::default();
    d.name = "Ogiva — the arch".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("size")?;
    SandProcess::DelftClay.apply(&mut d.draft);
    CastProcess::SandTwoPart.apply(&mut d.draft);
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    let mut s = setup();
    s.component = None;
    t.doc.features.last_mut().unwrap().component.manufacturing = Some(s.clone());
    d.cad = Some(t.doc);
    d.manufacturing = Some(s);
    Ok((d, a))
}

/// The lands the sketch numbers leave, mm.
fn lands(a: &GreatArch) -> serde_json::Value {
    let half = a.width / 2.0;
    let bore_r = BORE_MM / 2.0;
    let (ty, lobe, out) = TREFOIL;
    let (x, w, _, to, depth) = PIER_NICHE;
    // The niche's inner jamb against the bore at its springing, and its outer jamb against the face's edge inside the keel.
    let niche_spring = to - w * 3f64.sqrt() / 2.0;
    let niche_to_bore = (x - w / 2.0) - (bore_r * bore_r - niche_spring * niche_spring).max(0.0).sqrt();
    let face_edge = a.half_span - KEEL_IN;
    let pierce_draft = (half + 0.3 - HALF_FROM_MM - 0.04) * DRAFT_DEG.to_radians().tan();
    // The side wall at the axis, outline to bore, and what the order leaves of it over the bore.
    let side = a.centre(1.0)[0] + (a.radius * a.radius - a.spring_y * a.spring_y).sqrt();
    json!({
        "side_wall_at_axis_mm": side - bore_r,
        "order_to_bore_at_axis_mm": side - ORDER.0 - bore_r,
        "mouth_kept_off_bore_mm": MOUTH.1,
        "trefoil_to_bore_mm": ty - out / 2.0 - lobe - bore_r,
        "trefoil_to_mouth_step_mm": ty - out / 2.0 - lobe - (bore_r + MOUTH.1),
        "web_between_mouth_floors_mm": 2.0 * (half - MOUTH.2),
        "web_between_order_floors_mm": 2.0 * (half - ORDER.1),
        "web_between_pier_niche_floors_mm": 2.0 * (half - depth),
        "pier_niche_to_bore_mm": niche_to_bore,
        "capital_to_bore_mm": a.half_span - CAPITAL_IN - (bore_r + (half + 0.8 - HALF_FROM_MM) * DRAFT_DEG.to_radians().tan()) * (1.0 - (a.spring_y / bore_r).powi(2)).max(0.0).sqrt(),
        "pier_niche_to_face_edge_mm": face_edge - (x + w / 2.0),
        "trefoil_lobe_width_at_parting_mm": 2.0 * (lobe - pierce_draft),
        "crocket_plate_mm": 2.0 * CROCKET_HALF,
        "belt_ledge_over_its_mirror_mm": 2.0 * BELT_MM,
        "floors": {"section_mm": MIN_SECTION_MM, "sand_web_mm": MIN_SAND_WEB_MM},
    })
}

fn lands_ok(a: &GreatArch) -> bool {
    let l = lands(a);
    let n = |k: &str| l[k].as_f64().unwrap_or(0.0);
    ["order_to_bore_at_axis_mm", "mouth_kept_off_bore_mm", "trefoil_to_bore_mm", "trefoil_to_mouth_step_mm", "web_between_mouth_floors_mm", "web_between_order_floors_mm", "web_between_pier_niche_floors_mm", "pier_niche_to_bore_mm", "capital_to_bore_mm", "pier_niche_to_face_edge_mm", "crocket_plate_mm", "belt_ledge_over_its_mirror_mm"]
        .iter()
        .all(|k| n(k) >= MIN_SECTION_MM)
        && n("trefoil_lobe_width_at_parting_mm") >= MIN_SAND_WEB_MM
}

// --- Mesh helpers ----------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn mesh_of(s: &csg::Solid) -> mesh::Mesh {
    mesh::Mesh { vertices: s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: s.f.clone(), ..mesh::Mesh::default() }
}

/// Loose triangles with each corner's normal averaged only over faces within 35° of its own, so a cut face shades flat.
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

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

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

fn rotated(m: &mesh::Mesh, f: impl Fn(P3) -> P3) -> mesh::Mesh {
    let map = |v: mesh::Vec3| {
        let p = f([v.0 as f64, v.1 as f64, v.2 as f64]);
        mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)
    };
    mesh::Mesh { vertices: m.vertices.iter().map(|v| map(*v)).collect(), normals: m.normals.iter().map(|v| map(*v)).collect(), faces: m.faces.clone(), ..mesh::Mesh::default() }
}

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

/// `m` inside the box `lo`..`hi`, closed by csg.
fn boxed(m: &mesh::Mesh, lo: P3, hi: P3) -> Result<mesh::Mesh> {
    let mut bx = csg::Solid::default();
    bx.v = (0..8).map(|i| [if i & 1 == 0 { lo[0] } else { hi[0] }, if i & 2 == 0 { lo[1] } else { hi[1] }, if i & 4 == 0 { lo[2] } else { hi[2] }]).collect();
    bx.f = vec![[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4], [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
    let cut = csg::combine(&solid_of(m), &bx, csg::Op::Intersect).map_err(|e| anyhow::anyhow!("box cut: {e:?}"))?;
    Ok(creased(&mesh_of(&cut)))
}

fn shot(parts: &[(&mesh::Mesh, bool, [f32; 3])], toward: P3, edge: usize) -> Vec<u8> {
    let turned: Vec<(mesh::Mesh, bool, [f32; 3])> = parts.iter().map(|(m, gem, t)| (looked(m, toward), *gem, *t)).collect();
    let parts: Vec<render::Part> = turned.iter().map(|(m, gem, t)| if *gem { render::Part::tinted_stone(m, *t) } else { render::Part::metal(m, *t) }).collect();
    render::render_parts_ss(&parts, 0.0, 0.0, edge, edge, 3)
}

/// The whole ring seen from `toward`, framed on `centre` (world) with `half` mm either side: a close-up that crops nothing.
fn shot_framed(parts: &[(&mesh::Mesh, bool, [f32; 3])], toward: P3, centre: P3, half: f64, edge: usize) -> Vec<u8> {
    let probe = mesh::Mesh { vertices: vec![mesh::Vec3(centre[0] as f32, centre[1] as f32, centre[2] as f32)], ..mesh::Mesh::default() };
    let c = looked(&probe, toward).vertices[0];
    let turned: Vec<(mesh::Mesh, bool, [f32; 3])> = parts.iter().map(|(m, gem, t)| (looked(m, toward), *gem, *t)).collect();
    let parts: Vec<render::Part> = turned.iter().map(|(m, gem, t)| if *gem { render::Part::tinted_stone(m, *t) } else { render::Part::metal(m, *t) }).collect();
    render::render_parts_framed(&parts, 0.0, 0.0, render::Framing::new([c.0 as f64, c.1 as f64, c.2 as f64], half), edge, edge, 3)
}

fn png(path: &Path, img: &[u8], edge: usize) -> Result<()> {
    image::save_buffer(path, img, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

// --- Measures --------------------------------------------------------------------------------------

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

// --- Renders ---------------------------------------------------------------------------------------

/// Each named view as the direction from the ring toward the eye, world +y (the apex) kept up. The face is the arch seen
/// along the finger, where the ring carries its subject; the side looks across the finger at the keel.
const VIEWS: [(&str, P3); 7] = [
    ("hero", [0.42, 0.28, 0.86]),
    ("face", [0.0, 0.0, 1.0]),
    ("palm", [0.35, -1.0, 0.45]),
    ("side", [1.0, 0.1, 0.1]),
    ("shoulder", [1.0, 0.75, 0.25]),
    ("reverse", [-0.8, 0.3, -0.55]),
    ("top", [0.02, 1.0, 0.12]),
];

fn renders(out: &Path, fin: &render::Finished, bare: &mesh::Mesh, edge: usize) -> Result<()> {
    let metal = creased(&fin.metal);
    let parts: Vec<(&mesh::Mesh, bool, [f32; 3])> = vec![(&metal, false, render::GOLD)];
    for (name, toward) in VIEWS {
        png(&out.join(format!("{name}.png")), &shot(&parts, toward, edge), edge)?;
    }
    // No stones: the close-up is the head, the crockets climbing to the finial over the orders and the trefoil.
    png(&out.join("stones.png"), &shot_framed(&parts, [0.45, 0.3, 0.85], [0.0, 13.0, 0.0], 7.5, edge), edge)?;
    // The section through the apex, the cut toward +x: the keel's round, the orders and the mouth.
    let section = boxed(&fin.metal, [-0.3, 2.0, -40.0], [0.0, 40.0, 40.0])?;
    let sec = [(&section, false, render::GOLD)];
    png(&out.join("section.png"), &shot(&sec, [1.0, 0.0, 0.0001], edge), edge)?;
    let bare = creased(bare);
    side_by_side(&out.join("bare-vs-finished.png"), &[shot(&[(&bare, false, render::GOLD)], VIEWS[0].1, edge), shot(&parts, VIEWS[0].1, edge)], edge)?;
    png(&out.join("hero-300.png"), &shot(&parts, VIEWS[0].1, 300), 300)?;
    png(&out.join("face-300.png"), &shot(&parts, VIEWS[1].1, 300), 300)?;
    let sheet: Vec<Vec<u8>> = [VIEWS[0].1, VIEWS[1].1, VIEWS[3].1, VIEWS[2].1].iter().map(|t| shot(&parts, *t, 300)).chain(std::iter::once(shot(&sec, [1.0, 0.0, 0.0001], 300))).collect();
    side_by_side(&out.join("contact-300.png"), &sheet, 300)?;
    Ok(())
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
    let (d, arch) = author(false)?;
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
    let statuses: Vec<(String, String)> = built.parts.evaluated.iter().flat_map(|e| e.features.iter()).map(|r| (r.id.to_string(), format!("{:?}", r.status))).collect();
    let features_ok = statuses.iter().all(|(_, s)| s == "Ok");
    // The sand release study at both pitches, the ring judged as its parts alone.
    let mut s100 = d.manufacturing.clone().unwrap();
    s100.sample_pitch_mm = 0.100;
    let i100 = mf::inspect(&d, &lib, &s100, params)?;
    let mut s075 = s100.clone();
    s075.sample_pitch_mm = 0.075;
    let i075 = mf::inspect(&d, &lib, &s075, params)?;
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let wall = cad::measure::thickness(&built.mesh, MIN_SECTION_MM);
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
        json!({"pitch_mm": i.release.cell_mm, "status": i.release.status, "obstructions": i.release.obstructions.len(), "unresolved_rays": i.release.unresolved_rays, "fits_flask": i.release.fits_flask, "low_draft_area_mm2": i.release.low_draft_area_mm2, "sand_findings": i.release.sand_findings.iter().map(|f| f.message.clone()).collect::<Vec<_>>(), "notes": i.release.notes, "local_wall": i.local_wall, "details": i.details})
    };
    let clean = |i: &mf::Inspection| i.release.obstructions.is_empty() && i.release.unresolved_rays == 0 && i.release.fits_flask;
    let wall_ok = wall.below_limit == 0 && wall.rays > 0;
    let grams = built.report.volume_mm3 * 10.36 / 1000.0;
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings, every feature Ok", made.iter().all(|(_, n)| *n == 0) && features_ok),
        ("solids and parts notes empty", built.solids.notes.is_empty() && built.parts.notes.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("ray release clean at 0.100 mm", clean(&i100)),
        ("ray release clean at 0.075 mm", clean(&i075)),
        ("local wall at or above the 0.8 mm section (cad::measure::thickness)", wall_ok),
        ("sketch lands at or above the floors", lands_ok(&arch)),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let features: Vec<serde_json::Value> = d.cad.as_ref().unwrap().features.iter().map(|f| json!({"id": f.id, "name": f.name, "operation": f.operation.label()})).collect();
    let mut report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "sand": "Delft clay",
        "alloy": "Silver 925",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s},
        "arch": arch,
        "keel": {"inset_mm": KEEL_IN, "rise_mm": keel_rise(), "land_inset_mm": KEEL_LAND, "land_mm": 2.0 * BELT_MM, "flank_draft_deg": KEEL_FLANK_DEG, "keel_angle_deg": 180.0 - 2.0 * KEEL_FLANK_DEG},
        "geometry": {"watertight": watertight, "boundary_edges": built.report.validation.boundary_edges, "non_manifold_edges": built.report.validation.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "shells": shells(&built.mesh), "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm},
        "made_parts": made,
        "feature_status": statuses,
        "solids": {"notes": built.solids.notes, "parts_notes": built.parts.notes, "stamps": 0},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "applies": false, "why": "A parts-only ring has no band chart: the field reports 'Castable with care' with the note that CAD solids need mesh-space inspection, so the ring is judged by the ray release at both pitches and the local wall instead (brief, Ogiva).", "thinnest_wall_mm": field.thinnest_wall_mm, "undercut_percent": field.undercut_fraction() * 100.0, "notes": field.notes, "min_section_mm": d.draft.min_section_mm, "min_draft_deg": d.draft.min_draft_deg},
        "release_0_100": release(&i100),
        "release_0_075": release(&i075),
        "local_wall": {"limit_mm": wall.limit_mm, "sampled_min_mm": wall.sampled_min_mm, "below_limit": wall.below_limit, "rays": wall.rays, "unresolved": wall.unresolved, "worst_point": wall.point, "note": wall.note},
        "land_widths": lands(&arch),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "grams_silver": grams,
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len()},
        "design": {"bytes": text.len(), "cad_features": features.len()},
        "features": features,
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    // The draft build writes its own block; the export keeps it beside its own numbers.
    let report_path = out.join("report.json");
    if draft {
        report["draft"] = json!({"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "parting_z_mm": d.draft.parting_z_mm, "auto_parting": d.draft.auto_parting, "build": report.clone()});
    } else if let Ok(old) = std::fs::read(&report_path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<serde_json::Value>(&b)?)) {
        report["draft"] = old.get("draft").cloned().unwrap_or(serde_json::Value::Null);
    }
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Ogiva / sand pattern")?;
    }
    let bare = mesh::try_build(&author(true)?.0, &lib, params)?.mesh;
    renders(&out, &fin, &bare, if draft { 900 } else { 1600 })?;
    println!(
        "  field {} (does not apply); release 0.100: {} obstructions, {} unresolved; 0.075: {} obstructions, {} unresolved; wall min {:?} ({} of {} below 0.8 at {:?}); dfm {}; {grams:.2} g silver",
        field.verdict.label(),
        i100.release.obstructions.len(),
        i100.release.unresolved_rays,
        i075.release.obstructions.len(),
        i075.release.unresolved_rays,
        wall.sampled_min_mm,
        wall.below_limit,
        wall.rays,
        wall.point,
        findings.len(),
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    if std::env::var("OGIVA_WHERE").is_ok() {
        for (name, i) in [("0.100", &i100), ("0.075", &i075)] {
            for o in i.release.obstructions.iter().take(8) {
                println!("    obstruction {name}: {:?} {:.3} mm deep, {:.4} mm²", o.world.map(|v| (v * 100.0).round() / 100.0), o.depth_mm, o.projected_area_mm2);
            }
        }
        let m = &built.mesh;
        let tris: Vec<[[f64; 3]; 3]> = m.faces.iter().map(|f| f.map(|i| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] })).collect();
        let stride = tris.len().div_ceil(384).max(1);
        let sub3 = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let cr = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let dt = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (k, t) in tris.iter().enumerate().step_by(stride) {
            let n = cr(sub3(t[1], t[0]), sub3(t[2], t[0]));
            let l = dt(n, n).sqrt();
            if l < 1e-12 { continue; }
            let c = [0, 1, 2].map(|i| (t[0][i] + t[1][i] + t[2][i]) / 3.0);
            let d = n.map(|v| -v / l);
            let mut best = f64::MAX;
            for (j, u) in tris.iter().enumerate() {
                if j == k { continue; }
                let (e1, e2) = (sub3(u[1], u[0]), sub3(u[2], u[0]));
                let h = cr(d, e2);
                let det = dt(e1, h);
                if det.abs() < 1e-12 { continue; }
                let sv = sub3(c, u[0]);
                let uu = dt(sv, h) / det;
                if !(-1e-8..=1.0 + 1e-8).contains(&uu) { continue; }
                let q = cr(sv, e1);
                let vv = dt(d, q) / det;
                if vv < -1e-8 || uu + vv > 1.0 + 1e-8 { continue; }
                let tt = dt(e2, q) / det;
                if tt > 1e-5 { best = best.min(tt); }
            }
            if best < 0.8 {
                println!("    thin {best:.3} at ({:.2}, {:.2}, {:.3}) normal ({:.2}, {:.2}, {:.2})", c[0], c[1], c[2], -d[0], -d[1], -d[2]);
            }
        }
        for f in &m.faces {
            let p = f.map(|i| m.vertices[i as usize]);
            let e = |a: mesh::Vec3, b: mesh::Vec3| (((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)) as f64).sqrt();
            let (a, b, c) = (e(p[0], p[1]), e(p[1], p[2]), e(p[2], p[0]));
            let s = (a + b + c) / 2.0;
            let area = (s * (s - a) * (s - b) * (s - c)).max(0.0).sqrt();
            if area < 1e-10 || a.max(b).max(c) < 1e-9 {
                println!("    degenerate near ({:.2}, {:.2}, {:.3}), edges {a:.2e} {b:.2e} {c:.2e}", p[0].0, p[0].1, p[0].2);
            }
        }
    }
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Ogiva failed a gate; see {}", report_path.display());
    Ok(())
}
