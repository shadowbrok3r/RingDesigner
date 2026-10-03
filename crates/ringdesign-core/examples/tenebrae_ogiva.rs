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
    fn sketch(&mut self, name: &str, plane: Id, loops: &[Vec<P2>]) -> Id {
        let mut s = Sketch::default();
        s.name = name.into();
        s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
        for l in loops {
            // A polyline takes at most 512 points: a long loop keeps every k-th.
            let k = l.len().div_ceil(500);
            let ids: Vec<Id> = l.iter().step_by(k).map(|p| s.point(*p)).collect();
            s.entity(Geometry::Polyline { points: ids, closed: true });
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
    fn slab(&mut self, what: &str, loops: &[Vec<P2>], z0: f64, z1: f64, draft: f64) -> Id {
        let plane = self.plane(&format!("Lay a plane at z {z0:+.3} for the {what}"), z0);
        let s = self.sketch(&format!("Draw the {what}"), plane, loops);
        let cope = self.extrude(&format!("Raise the cope half of the {what} along the pull"), s, z1 - z0, draft);
        let drag = self.mirror(&format!("Mirror the {what} into the drag half"), vec![cope]);
        self.boolean(&format!("Join the halves of the {what}"), cope, drag, Boolean::Union)
    }
    /// Loops raised from a hair under the parting line to `half` either side.
    fn both_halves(&mut self, what: &str, loops: &[Vec<P2>], half: f64, draft: f64) -> Id {
        self.slab(what, loops, -HAIR_MM, half, draft)
    }
    /// A pocket drawn on a plane `top` over the side face and sunk to `floor` along the pull in the cope with `draft`, mirrored into the drag.
    fn pocket(&mut self, what: &str, loops: &[Vec<P2>], top: f64, floor: f64, draft: f64) -> (Id, Id) {
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
        let (half_span, spring_y, sill_y, corner, width): (f64, f64, f64, f64, f64) = if rev { (11.0, -3.0, -11.0, 2.4, 6.0) } else { (11.0, -3.0, -11.0, 2.4, 6.0) };
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
    /// The whole outline offset `d` inward: the head, down the left pier, round the corner along the sill, and up the right pier.
    fn outline(&self, d: f64, round: f64) -> Vec<P2> {
        let mut pts = self.head(d, round);
        let x = self.half_span - d;
        let k = self.corner - d;
        let (cx, cy) = (self.half_span - self.corner, self.sill_y + self.corner);
        pts.push([-x, cy]);
        pts.extend(arc_n([-cx, cy], k, PI, 1.5 * PI, 24));
        pts.push([cx, self.sill_y + d]);
        pts.extend(arc_n([cx, cy], k, 1.5 * PI, 2.0 * PI, 24));
        pts.pop();
        pts
    }
}

/// Chords on each head arc.
const HEAD_STEPS: usize = 110;

/// `n` chords round an arc about `c`, the start left out.
fn arc_n(c: P2, r: f64, a0: f64, a1: f64, n: usize) -> Vec<P2> {
    (1..=n).map(|i| a0 + (a1 - a0) * i as f64 / n as f64).map(|t| [c[0] + r * t.cos(), c[1] + r * t.sin()]).collect()
}

fn circle(r: f64) -> Vec<P2> {
    let mut pts = arc_pts([0.0, 0.0], r, 0.0, 2.0 * PI);
    pts.pop();
    pts
}

// --- The ring's numbers ----------------------------------------------------------------------------

/// The keel: the outline on the parting plane falls KEEL_IN inward over KEEL_RISE along the pull, and its crease is rounded.
const KEEL_IN: f64 = 0.8;
const KEEL_RISE: f64 = 1.05;
const KEEL_ROUND_MM: f64 = 0.3;
/// The round is walked as tangent faces at these drafts, then the keel's own flank.
const KEEL_ROUND_DRAFTS: [f64; 4] = [3.0, 12.0, 21.0, 29.0];
/// The orders sunk into the head: how far inside the outline each starts, how deep its floor sits below the face, and its chamfer's draft.
const ORDERS: [(f64, f64, f64); 3] = [(1.4, 0.6, 35.0), (2.3, 1.2, 35.0), (3.2, 1.9, 12.0)];
/// The capitals at the springers: abacus, bell and astragal heights, how far each projects past the pier, how proud of the face.
const ABACUS: (f64, f64) = (0.55, 0.75);
const BELL: (f64, f64) = (0.95, 0.6);
const ASTRAGAL: (f64, f64) = (0.5, 0.35);
const CAPITAL_PROUD: (f64, f64) = (0.5, 0.3);
/// How far in from the pier's outer face the capitals reach (the bore trims them).
const CAPITAL_IN: f64 = 3.4;
/// Crockets up each slope of the extrados, as shares of the head arc, each one's growth, and the plate's half-width along the finger.
const CROCKETS: [(f64, f64); 6] = [(0.16, 0.85), (0.29, 0.92), (0.42, 1.0), (0.55, 1.07), (0.68, 1.14), (0.81, 1.2)];
const CROCKET_HALF: f64 = 0.9;
/// How deep each crocket and the finial sink below the outline.
const CROCKET_SINK: f64 = 1.2;
/// A blind lancet niche in each pier face: centre off the axis, width, sill and apex heights, floor depth below the face.
const PIER_NICHE: (f64, f64, f64, f64, f64) = (9.2, 1.0, -9.0, -5.2, 0.7);
/// The pierced trefoil in the mouth over the finger: centre height, lobe radius, lobe centres' distance from the centre.
const TREFOIL: (f64, f64, f64) = (11.0, 0.55, 0.5);

/// The four tangent faces that walk the keel's round and the flank: (draft, the inset each starts at on the parting plane, the height it stops at).
fn keel_layers() -> Vec<(f64, f64, f64, f64)> {
    let flank = (KEEL_IN / KEEL_RISE).atan();
    let c = KEEL_ROUND_MM / flank.cos();
    let mut drafts: Vec<f64> = KEEL_ROUND_DRAFTS.iter().map(|d| d.to_radians()).collect();
    drafts.push(flank);
    let e: Vec<f64> = drafts.iter().map(|a| c - KEEL_ROUND_MM / a.cos()).collect();
    let mut out = Vec::new();
    let mut z0 = -HAIR_MM;
    for k in 0..drafts.len() {
        let z1 = if k + 1 < drafts.len() { (e[k] - e[k + 1]) / (drafts[k + 1].tan() - drafts[k].tan()) } else { KEEL_RISE };
        // Each face runs a hair past the line where it meets the next, and the next starts a hair before it, so every
        // rim stands inside its neighbour and no edge lies on another face.
        let start = if k == 0 { z0 } else { z0 - 0.01 };
        let stop = if k + 1 < drafts.len() { z1 + 0.01 } else { z1 };
        out.push((drafts[k], e[k] + start.max(0.0) * drafts[k].tan(), start, stop));
        z0 = z1;
    }
    out
}

/// A closed outline through key points `(u, v, sharp)`, a cubic Hermite between each pair with Catmull–Rom tangents, its
/// corners cut and the loop walked at an even pitch, so every tip is rounder than a drafted inset.
fn spline(keys: &[(f64, f64, bool)]) -> Vec<P2> {
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
    smoothed(&out)
}

/// A loop walked at 0.3 mm, its corners cut twice and walked again at 0.28 mm.
fn smoothed(pts: &[P2]) -> Vec<P2> {
    let mut out = even(pts, 0.3);
    for _ in 0..2 {
        let n = out.len();
        out = (0..n)
            .flat_map(|i| {
                let (a, b) = (out[i], out[(i + 1) % n]);
                [[0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]], [0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]]
            })
            .collect();
    }
    even(&out, 0.28)
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
        (0.3, 1.9, false),
        (0.0, 2.7, true),
        (-0.3, 1.9, false),
        (-0.45, 1.25, false),
        (-1.0, 1.55, false),
        (-1.35, 0.9, false),
        (-0.6, 0.3, false),
    ]
}

/// Keys laid at `origin` with `t` along the slope and `n` out of it, `grow` times their size.
fn laid(keys: &Keys, origin: P2, t: P2, n: P2, grow: f64) -> Vec<P2> {
    spline(&keys.iter().map(|(a, b, s)| (a * grow, if *b > 0.0 { b * grow } else { *b }, *s)).collect::<Vec<_>>())
        .into_iter()
        .map(|[a, b]| [origin[0] + a * t[0] + b * n[0], origin[1] + a * t[1] + b * n[1]])
        .collect()
}

/// The trefoil: three lobes round a centre, walked as the outline a ray from the centre meets, its cusps eased.
fn trefoil(centre: P2, lobe: f64, out: f64) -> Vec<P2> {
    smoothed(&trefoil_raw(centre, lobe, out))
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

/// A capital at the right springer, mirrored for the left: the astragal, the bell flaring out under the abacus, or the abacus.
fn capital(a: &GreatArch, side: f64, part: &str) -> Vec<P2> {
    let (x_o, x_i, s) = (a.half_span, a.half_span - CAPITAL_IN, a.spring_y);
    let (bell_top, bell_bot) = (s - ABACUS.0 + 0.05, s - ABACUS.0 - BELL.0);
    let ast = bell_bot - ASTRAGAL.0;
    let pts: Vec<P2> = match part {
        "abacus" => vec![[x_i, s - ABACUS.0], [x_o + ABACUS.1, s - ABACUS.0], [x_o + ABACUS.1, s], [x_i, s]],
        "astragal" => vec![[x_i, ast], [x_o + ASTRAGAL.1, ast], [x_o + ASTRAGAL.1, bell_bot + 0.05], [x_i, bell_bot + 0.05]],
        // The bell flares from the astragal's reach out to under the abacus.
        _ => vec![[x_i, ast + 0.05], [x_o + ASTRAGAL.1, ast + 0.05], [x_o + ASTRAGAL.1, bell_bot], [x_o + BELL.1, bell_top], [x_i, bell_top]],
    };
    let pts: Vec<P2> = pts.into_iter().map(|[x, y]| [side * x, y]).collect();
    if side < 0.0 { pts.into_iter().rev().collect() } else { pts }
}

/// The plain arch: the keel walked through its round, and the face inside it.
fn arch_body(t: &mut Tree, a: &GreatArch) -> Id {
    let half = a.width / 2.0;
    let round0 = KEEL_IN + 0.05;
    let mut cur = None;
    for (k, (draft, off, z0, z1)) in keel_layers().into_iter().enumerate() {
        let what = if k + 1 == KEEL_ROUND_DRAFTS.len() + 1 { "keel's flank".to_string() } else { format!("keel's round, face {}", k + 1) };
        let id = t.slab(&what, &[a.outline(off, round0 - off)], z0, z1, draft.to_degrees());
        cur = Some(match cur {
            None => id,
            Some(c) => t.boolean(&format!("Walk the {what} onto the keel"), c, id, Boolean::Union),
        });
    }
    let face = t.both_halves("arch's face, inside the keel", &[a.outline(KEEL_IN, 0.25)], half, DRAFT_DEG);
    t.boolean("Set the face on the keel", cur.unwrap(), face, Boolean::Union)
}

fn bore(t: &mut Tree, a: &GreatArch) -> Id {
    let half = a.width / 2.0;
    // The bore, a double cone drafted from the parting line out to each face.
    let s_plane = t.plane("Lay a plane on the parting line for the bore", -HAIR_MM - 0.02);
    let s = t.sketch("Draw the bore", s_plane, &[circle(BORE_MM / 2.0)]);
    let cope = t.extrude("Raise the bore's cope half, widening to the face", s, half + 0.8, -DRAFT_DEG);
    let drag = t.mirror("Mirror the bore into the drag half", vec![cope]);
    t.boolean("Join the bore's halves", cope, drag, Boolean::Union)
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
        leaves.push(laid(&finial(), [0.0, a.apex_y], [1.0, 0.0], [0.0, 1.0], 1.0));
        let crockets = t.both_halves("crockets climbing the extrados to the finial", &leaves, CROCKET_HALF, DRAFT_DEG);
        cur = t.boolean("Set the crockets and the finial on the keel", cur, crockets, Boolean::Union);
        // The orders and the mouth, sunk into the head and run down onto the capitals.
        for (i, (d, depth, draft)) in ORDERS.iter().enumerate() {
            let what = ["outer order", "inner order", "mouth over the finger"][i];
            let top = half + 0.3;
            let floor = half - depth;
            let inset = (top - floor) * draft.to_radians().tan();
            let (cope, drag) = t.pocket(what, &[a.head_region(*d, inset + 0.3, (ABACUS.0 + 0.437).max(inset + 0.237))], top, floor, *draft);
            cur = t.boolean(&format!("Sink the cope's {what}"), cur, cope, Boolean::Subtract);
            cur = t.boolean(&format!("Sink the drag's {what}"), cur, drag, Boolean::Subtract);
        }
        // The capitals: astragal and bell, then the abacus standing prouder over them.
        let bells = [capital(&a, 1.0, "bell"), capital(&a, -1.0, "bell")];
        let abaci = [capital(&a, 1.0, "abacus"), capital(&a, -1.0, "abacus"), capital(&a, 1.0, "astragal"), capital(&a, -1.0, "astragal")];
        let b = t.both_halves("capitals' bells", &bells, half + CAPITAL_PROUD.1, DRAFT_DEG);
        cur = t.boolean("Set the bells under the springers", cur, b, Boolean::Union);
        let ab = t.both_halves("capitals' abaci and astragals", &abaci, half + CAPITAL_PROUD.0, DRAFT_DEG);
        cur = t.boolean("Lay the abaci over the bells and the astragals under them", cur, ab, Boolean::Union);
        // A blind lancet niche in each pier face.
        let (x, w, from, to, depth) = PIER_NICHE;
        let niche = |side: f64| -> Vec<P2> { lancet(w, from, to).into_iter().map(|[u, v]| [side * x + u, v]).collect() };
        let (cope, drag) = t.pocket("blind lancets in the pier faces", &[niche(1.0), niche(-1.0)], half + 0.3, half - depth, DRAFT_DEG);
        cur = t.boolean("Sink the cope's pier lancets", cur, cope, Boolean::Subtract);
        cur = t.boolean("Sink the drag's pier lancets", cur, drag, Boolean::Subtract);
        // The trefoil pierced through the mouth's web along the pull, each half narrowing to the parting line.
        let (ty, lobe, out) = TREFOIL;
        let (cope, drag) = t.pocket("trefoil through the mouth", &[trefoil([0.0, ty], lobe, out)], half + 0.3, -0.03, DRAFT_DEG);
        let light = t.boolean("Join the trefoil's halves", cope, drag, Boolean::Union);
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
    let mouth = ORDERS[2];
    let (x, w, _, to, depth) = PIER_NICHE;
    // The niche's inner jamb against the bore at its springing, and its outer jamb against the face's edge inside the keel.
    let niche_spring = to - w * 3f64.sqrt() / 2.0;
    let niche_to_bore = (x - w / 2.0) - (bore_r * bore_r - niche_spring * niche_spring).max(0.0).sqrt();
    let face_edge = a.half_span - KEEL_IN;
    let pierce_draft = (half + 0.3) * DRAFT_DEG.to_radians().tan();
    json!({
        "trefoil_to_bore_mm": ty - out / 2.0 - lobe - bore_r,
        "web_between_mouth_floors_mm": 2.0 * (half - mouth.1),
        "web_between_order_floors_mm": 2.0 * (half - ORDERS[1].1),
        "web_between_pier_niche_floors_mm": 2.0 * (half - depth),
        "pier_niche_to_bore_mm": niche_to_bore,
        "pier_niche_to_face_edge_mm": face_edge - (x + w / 2.0),
        "trefoil_lobe_width_at_parting_mm": 2.0 * (lobe - pierce_draft),
        "crocket_plate_at_root_mm": 2.0 * CROCKET_HALF,
        "crocket_plate_at_face_mm": 2.0 * CROCKET_HALF - 2.0 * CROCKET_HALF * DRAFT_DEG.to_radians().tan(),
        "floors": {"section_mm": MIN_SECTION_MM, "sand_web_mm": MIN_SAND_WEB_MM},
    })
}

fn lands_ok(a: &GreatArch) -> bool {
    let l = lands(a);
    let n = |k: &str| l[k].as_f64().unwrap_or(0.0);
    ["trefoil_to_bore_mm", "web_between_mouth_floors_mm", "web_between_order_floors_mm", "web_between_pier_niche_floors_mm", "pier_niche_to_bore_mm", "crocket_plate_at_face_mm"]
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
    let head = boxed(&fin.metal, [-40.0, 5.0, -40.0], [40.0, 40.0, 40.0])?;
    png(&out.join("stones.png"), &shot(&[(&head, false, render::GOLD)], [0.45, 0.3, 0.85], edge), edge)?;
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
        "keel": {"inset_mm": KEEL_IN, "rise_mm": KEEL_RISE, "round_mm": KEEL_ROUND_MM, "faces": keel_layers().iter().map(|(a, e, z0, z1)| json!({"draft_deg": a.to_degrees(), "inset_mm": e, "z0": z0, "z1": z1})).collect::<Vec<_>>()},
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
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Ogiva failed a gate; see {}", report_path.display());
    Ok(())
}
