//! Tenebrae — Ogiva concept spike: three rethinks of the keel blocked out for a read test, nothing detailed.
//! cargo build --release -p ringdesign-core --example tenebrae_ogiva_spike
//! target/release/examples/tenebrae_ogiva_spike <arch|keel-section|gargoyle> [OUT_DIR] [--rev]
//!
//! - `arch`: the ring seen along the finger is one great equilateral pointed arch: a keeled extrados, two stepped
//!   orders and a recessed mouth over the finger, all drawn on the parting plane and extruded along the pull.
//! - `keel-section`: the original keel, a lancet section revolved round the finger, judged on its section and end-on.
//! - `gargoyle`: a crouched gargoyle drawn on the parting plane and carved in layers along the pull, on factory 015 Octagon's sand master.
//!
//! `--rev` builds each option's one revision, after its first read test.
use anyhow::{Context, Result, bail};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, Boolean, Component, ComponentRole, Document, FaceRef, Feature, MirrorPlane, Operation, PatternKind, PlaneBase, Profile, Stage, pattern::Sources},
    castability::{self, CastProcess, SandProcess},
    csg, dfm, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch, Workplane},
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Delft clay draft, degrees, and where each drafted half starts across the parting plane so the halves overlap, mm.
const DRAFT_DEG: f64 = 3.0;
const PARTING_OVERLAP_MM: f64 = 0.03;
/// How far each drafted half starts past the parting line, mm.
const HAIR_MM: f64 = 0.008;
/// Chord the outlines are walked in, mm.
const CHORD_MM: f64 = 0.12;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
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
        self.add_as(name, operation, Component { role, material: "Silver 925".into(), ..Component::default() })
    }
    fn add_as(&mut self, name: &str, operation: Operation, component: Component) -> Id {
        let id = self.doc.features.len() as Id + 1;
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
    /// A loop drawn on the parting plane raised `half` along the pull in the cope with draft, mirrored into the drag, and the halves joined.
    fn both_halves(&mut self, what: &str, parting: Id, loops: &[Vec<P2>], half: f64, draft: f64) -> Id {
        // Overlapped drafted halves leave a waist on the parting line, which the release study reads as an undercut,
        // so each half starts from a plane only a hair under the parting line.
        let _ = parting;
        let (plane, overlap) = (self.plane(&format!("Lay a plane a hair under the parting line for the {what}"), -HAIR_MM), HAIR_MM);
        let s = self.sketch(&format!("Draw the {what} on the parting plane"), plane, loops);
        let cope = self.extrude(&format!("Raise the cope half of the {what} along the pull"), s, half + overlap, draft);
        let drag = self.mirror(&format!("Mirror the {what} into the drag half"), vec![cope]);
        self.boolean(&format!("Join the halves of the {what}"), cope, drag, Boolean::Union)
    }
    /// A pocket drawn on a plane `top` over the side face and sunk to `floor` along the pull in the cope, mirrored into the drag.
    fn pocket(&mut self, what: &str, loops: &[Vec<P2>], top: f64, floor: f64) -> (Id, Id) {
        let plane = self.plane(&format!("Lay a plane over the side face for the {what}"), top);
        let s = self.sketch(&format!("Draw the {what}"), plane, loops);
        let cope = self.extrude(&format!("Sink the {what} along the pull"), s, -(top - floor), DRAFT_DEG);
        let drag = self.mirror(&format!("Mirror the {what} into the drag side"), vec![cope]);
        (cope, drag)
    }
}

fn sand_setup(name: &str) -> mf::Setup {
    let mut s = probe::sand_setup(0.10);
    s.recipe.alloy = "Silver 925".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").map_or(1.5, |m| m.shrink_pct);
    s.recipe.process = CastProcess::SandTwoPart;
    s.recipe.name = format!("{name} / Delft clay / Silver 925");
    s
}

/// A CAD-only ring in Delft sand, parting on z = 0.
fn cad_only(name: &str, doc: Document) -> Result<RingDesign> {
    let mut d = RingDesign::default();
    d.name = name.into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("size")?;
    SandProcess::DelftClay.apply(&mut d.draft);
    CastProcess::SandTwoPart.apply(&mut d.draft);
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    let mut doc = doc;
    let s = sand_setup(name);
    doc.features.last_mut().unwrap().component.manufacturing = Some(s.clone());
    d.cad = Some(doc);
    d.manufacturing = Some(s);
    Ok(d)
}

// --- Option 1: the arch ----------------------------------------------------------------------------

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

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<P2> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}

/// The arch ring: the keeled extrados, two chamfered orders sunk into the head, a deep mouth over the finger, an impost
/// projecting at each springer, and the bore.
fn author_arch(rev: bool) -> Result<(RingDesign, serde_json::Value)> {
    let a = GreatArch::of(rev);
    let half = a.width / 2.0;
    let bore_r = BORE_MM / 2.0;
    // The keel: the outline on the parting plane, falling keel_in inward over keel_rise along the pull.
    let (keel_in, keel_rise): (f64, f64) = (0.8, 1.05);
    let keel_draft = (keel_in / keel_rise).atan().to_degrees();
    // Each order: how far inside the outline it starts, how deep its floor sits below the face, and its chamfer's draft.
    let orders: [(f64, f64, f64); 3] = [(keel_in + 0.6, 0.55, 35.0), (keel_in + 1.4, 1.0, 35.0), (keel_in + 2.2, 1.5, 12.0)];
    // The imposts: how tall, how far they project past the piers, and how proud of the face.
    let (impost_h, impost_out, impost_proud) = (1.1, 0.6, 0.35);
    let mut t = Tree { doc: Document::default() };
    let parting = t.plane("Lay the parting plane through the arch", -PARTING_OVERLAP_MM);
    // The keel's flanks fall from the crease at a steep draft; its apex is rounded just past the inset so the drafted top keeps every segment.
    let keel = t.both_halves("keeled extrados of the great arch", parting, &[a.outline(0.0, keel_in + 0.05)], keel_rise, keel_draft);
    let body = t.both_halves("arch's face, inside the keel", parting, &[a.outline(keel_in, 0.25)], half, DRAFT_DEG);
    let mut cur = t.boolean("Set the face on the keel", keel, body, Boolean::Union);
    // The orders and the mouth, sunk into the head and run down onto the imposts.
    let mut sunk = Vec::new();
    for (i, (d, depth, draft)) in orders.iter().enumerate() {
        let what = ["outer order", "inner order", "mouth over the finger"][i];
        let top = half + 0.3;
        let floor = half - depth;
        let inset = (top - floor) * draft.to_radians().tan();
        let plane = t.plane(&format!("Lay a plane over the face for the {what}"), top);
        let s = t.sketch(&format!("Draw the {what} in the head"), plane, &[a.head_region(*d, inset + 0.3, impost_h)]);
        let cope = t.extrude(&format!("Sink the {what} along the pull, chamfered"), s, -(top - floor), *draft);
        let drag = t.mirror(&format!("Mirror the {what} into the drag side"), vec![cope]);
        cur = t.boolean(&format!("Sink the cope's {what}"), cur, cope, Boolean::Subtract);
        cur = t.boolean(&format!("Sink the drag's {what}"), cur, drag, Boolean::Subtract);
        sunk.push(json!({"what": what, "inside_outline_mm": d, "floor_z": floor, "chamfer_draft_deg": draft}));
    }
    let x0 = a.half_span - 3.4;
    let imposts = [rect(x0, a.spring_y - impost_h, a.half_span + impost_out, a.spring_y), rect(-a.half_span - impost_out, a.spring_y - impost_h, -x0, a.spring_y)];
    let im = t.both_halves("imposts at the springers", parting, &imposts, half + impost_proud, DRAFT_DEG);
    cur = t.boolean("Set the imposts on the piers, under the orders", cur, im, Boolean::Union);
    // The bore, a double cone drafted from the parting plane out to each face.
    let bore_plane = t.plane("Lay a plane on the parting plane for the bore", -PARTING_OVERLAP_MM - 0.02);
    let bore_s = t.sketch("Draw the bore", bore_plane, &[circle(bore_r)]);
    let bore_cope = t.extrude("Raise the bore's cope half, widening to the face", bore_s, half + 0.8, -DRAFT_DEG);
    let bore_drag = t.mirror("Mirror the bore into the drag half", vec![bore_cope]);
    let bore = t.boolean("Join the bore's halves", bore_cope, bore_drag, Boolean::Union);
    t.boolean("Pierce the bore through the arch", cur, bore, Boolean::Subtract);
    t.doc.features.last_mut().unwrap().component.role = ComponentRole::Shank;
    let d = cad_only("Ogiva spike — the arch", t.doc)?;
    let info = json!({
        "arch": a,
        "keel": {"inset_mm": keel_in, "rise_mm": keel_rise, "flank_draft_deg": keel_draft, "keel_angle_deg": 180.0 - 2.0 * keel_draft},
        "orders": sunk,
        "imposts": {"height_mm": impost_h, "project_mm": impost_out, "proud_mm": impost_proud},
        "apex_over_bore_mm": a.apex_y - bore_r,
        "sill_under_bore_mm": -a.sill_y - bore_r,
        "side_wall_at_axis_mm": a.centre(1.0)[0] + (a.radius * a.radius - a.spring_y * a.spring_y).sqrt() - bore_r,
    });
    Ok((d, info))
}

// --- Option 2: the keel section --------------------------------------------------------------------

/// The keel's numbers: width at the springers, straight jambs, head radius as a share of the width, the step moulding below the keel and how far it steps.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Keel {
    width: f64,
    jamb: f64,
    share: f64,
    step_below: f64,
    step: f64,
    comfort: f64,
    foot: f64,
}

impl Keel {
    fn of(rev: bool) -> Self {
        if rev {
            // The revision drops the step, so the section is one clean lancet.
            Self { width: 4.2, jamb: 0.5, share: 1.5, step_below: 0.0, step: 0.0, comfort: 0.12, foot: 0.3 }
        } else {
            Self { width: 4.2, jamb: 0.5, share: 1.5, step_below: 0.9, step: 0.25, comfort: 0.12, foot: 0.3 }
        }
    }
}

/// The keel's section on the plane through the finger (x out from the axis, y along it): two head arcs meeting at the
/// keel with a step moulding below it, straight jambs down to the bore, a comfort arc across it, and a round at each foot.
fn keel_section(bore_r: f64, k: Keel) -> (Sketch, serde_json::Value) {
    let h = k.width / 2.0;
    let r = k.share * k.width;
    let c = r - h;
    let springer = bore_r + k.comfort;
    let rj = springer + k.jamb;
    let cap = (r * r - (c + k.step).powi(2)).sqrt();
    let apex = rj + cap;
    let step_r = apex - k.step_below;
    let run = 0.06;
    let bore_radius = (k.comfort * k.comfort + h * h) / (2.0 * k.comfort);
    let bore_c = [bore_r + bore_radius, 0.0];
    let f = k.foot;
    let round = |side: f64| -> (P2, P2, P2) {
        let y = side * (h - f);
        let x = bore_c[0] - ((bore_radius - f).powi(2) - y * y).sqrt();
        let centre = [x, y];
        let d = sub2(centre, bore_c);
        let l = len2(d);
        (centre, [x, side * h], [bore_c[0] + d[0] / l * bore_radius, bore_c[1] + d[1] / l * bore_radius])
    };
    let (fu, fu_jamb, fu_bore) = round(1.0);
    let (fl, fl_jamb, fl_bore) = round(-1.0);
    let arc = |pts: &mut Vec<P2>, centre: P2, from: P2, to: P2, step: f64, first: bool| {
        let rr = len2(sub2(from, centre));
        let a0 = (from[1] - centre[1]).atan2(from[0] - centre[0]);
        let mut a1 = (to[1] - centre[1]).atan2(to[0] - centre[0]);
        while a1 <= a0 {
            a1 += 2.0 * PI;
        }
        let n = ((a1 - a0) * rr / step).ceil().max(2.0) as usize;
        for i in usize::from(!first)..n {
            let t = a0 + (a1 - a0) * i as f64 / n as f64;
            pts.push([centre[0] + rr * t.cos(), centre[1] + rr * t.sin()]);
        }
    };
    let zs = |x: f64| (r * r - (x - rj).powi(2)).sqrt() - c;
    let (cap_u, cap_l) = ([rj, -c - k.step], [rj, c + k.step]);
    let (upper_c, lower_c) = ([rj, -c], [rj, c]);
    let step_in = [step_r, zs(step_r) - k.step];
    let step_out = [step_r - run, zs(step_r - run)];
    let keel = [apex, 0.0];
    let mut pts: Vec<P2> = Vec::new();
    let stepped = k.step > 0.0;
    if stepped {
        arc(&mut pts, cap_u, keel, step_in, 0.05, true);
        pts.push(step_in);
        arc(&mut pts, upper_c, step_out, [rj, h], 0.1, true);
    } else {
        arc(&mut pts, upper_c, keel, [rj, h], 0.05, true);
    }
    pts.push([rj, h]);
    arc(&mut pts, fu, fu_jamb, fu_bore, 0.05, true);
    arc(&mut pts, bore_c, fu_bore, fl_bore, 0.2, true);
    arc(&mut pts, fl, fl_bore, fl_jamb, 0.05, true);
    pts.push(fl_jamb);
    pts.push([rj, -h]);
    let step_out_l = [step_out[0], -step_out[1]];
    let step_in_l = [step_in[0], -step_in[1]];
    if stepped {
        arc(&mut pts, lower_c, [rj, -h], step_out_l, 0.1, false);
        pts.push(step_out_l);
        arc(&mut pts, cap_l, step_in_l, keel, 0.05, true);
    } else {
        arc(&mut pts, lower_c, [rj, -h], keel, 0.05, false);
    }
    let mut s = Sketch::default();
    s.name = "Keel section".into();
    s.plane = Workplane::section();
    let ids: Vec<Id> = pts.into_iter().map(|p| s.point(p)).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    let draft = ((c + k.step) / cap).atan().to_degrees();
    let info = json!({"keel": k, "head_radius_mm": r, "keel_r_mm": apex, "thickness_mm": apex - springer, "keel_angle_deg": 180.0 - 2.0 * draft, "keel_flank_draft_deg": draft});
    (s, info)
}

fn author_keel(rev: bool) -> Result<(RingDesign, serde_json::Value)> {
    let k = Keel::of(rev);
    let (section, info) = keel_section(BORE_MM / 2.0, k);
    let mut t = Tree { doc: Document::default() };
    let sec = t.add("Draw the keel's lancet section", Operation::Sketch { sketch: section }, ComponentRole::Other);
    let ring = t.add(
        "Revolve the keel round the finger",
        Operation::Revolve { sketch: Profile::Feature { feature: sec }, pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false },
        ComponentRole::Shank,
    );
    if !rev {
        return Ok((cad_only("Ogiva spike — the keel", t.doc)?, json!({"section": info})));
    }
    // The revision: thirteen crockets climbing the keel over the top of the hand to a finial, and a lancet arcade sunk into each flank.
    let keel_r = info["keel_r_mm"].as_f64().unwrap();
    let parting = t.plane("Lay the parting plane on the keel", -PARTING_OVERLAP_MM);
    let mut leaves = Vec::new();
    for i in 0..13 {
        let theta = 18.0 + 12.0 * i as f64;
        let (st, ct) = (theta as f64).to_radians().sin_cos();
        let (p, n) = ([keel_r * ct, keel_r * st], [ct, st]);
        // Round the ring toward the apex: a crocket leans and curls toward the finial.
        let lean = if theta < 90.0 { 1.0 } else { -1.0 };
        let tangent = [-st * lean, ct * lean];
        let keys: Keys = if i == 6 {
            vec![(-0.8, -1.0, true), (0.8, -1.0, true), (0.6, 0.4, false), (1.1, 1.0, false), (0.45, 1.25, false), (0.0, 2.1, false), (-0.45, 1.25, false), (-1.1, 1.0, false), (-0.6, 0.4, false)]
        } else {
            vec![(-0.7, -1.0, true), (0.7, -1.0, true), (0.75, 0.3, false), (1.15, 0.85, false), (0.7, 1.3, false), (-0.1, 1.15, false), (-0.65, 0.6, false)]
        };
        leaves.push(spline(&keys).into_iter().map(|[a, b]| [p[0] + a * tangent[0] + b * n[0], p[1] + a * tangent[1] + b * n[1]]).collect::<Vec<P2>>());
    }
    let crockets = t.both_halves("crockets climbing the keel to the finial", parting, &leaves, 0.8, DRAFT_DEG);
    let mut cur = t.boolean("Set the crockets on the keel", ring, crockets, Boolean::Union);
    // Twenty-four lancet niches round each flank, drawn on a plane over the foot and sunk along the pull.
    let niches: Vec<Vec<P2>> = (0..24)
        .map(|i| {
            let a = (7.5 + 15.0 * i as f64).to_radians();
            let (sa, ca) = a.sin_cos();
            lancet(1.1, 10.0, 12.4).into_iter().map(|[x, y]| [y * ca - x * sa, y * sa + x * ca]).collect()
        })
        .collect();
    let (n_cope, n_drag) = t.pocket("lancet niches round the flank", &niches, 2.6, 1.3);
    cur = t.boolean("Sink the cope flank's niches", cur, n_cope, Boolean::Subtract);
    t.boolean("Sink the drag flank's niches", cur, n_drag, Boolean::Subtract);
    t.doc.features.last_mut().unwrap().component.role = ComponentRole::Shank;
    Ok((cad_only("Ogiva spike — the keel", t.doc)?, json!({"section": info, "crockets": {"count": 13, "from_deg": 18, "to_deg": 162, "proud_mm": 1.25, "finial_proud_mm": 2.1, "half_mm": 0.8}, "niches": {"per_flank": 24, "width_mm": 1.1, "from_r_mm": 10.0, "to_r_mm": 12.4, "floor_z": 1.3}})))
}

/// A lancet `w` wide: a flat sill at `from`, straight jambs and an equilateral pointed head with its apex at `to`, in a frame with x across it and y up it.
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

// --- Option 3: the gargoyle ------------------------------------------------------------------------

/// A closed outline through key points `(u, v, sharp)`, a cubic Hermite between each pair with Catmull–Rom tangents,
/// a sharp point's tangent shortened so the outline turns hard there without a cusp a drafted inset would drop.
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
    // Cut the corners three times over, then walk the loop at an even pitch: every tip is rounded wider than the
    // draft's inset, so the drafted top keeps every segment.
    out = even(&out, 0.3);
    for _ in 0..2 {
        let n = out.len();
        out = (0..n).flat_map(|i| {
            let (a, b) = (out[i], out[(i + 1) % n]);
            [[0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]], [0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]]
        }).collect();
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

type Keys = Vec<(f64, f64, bool)>;

/// The gargoyle in its own frame: `u` forward round the ring toward the snout, `v` up from the table, mm. A crouched
/// grotesque in profile: folded haunches at the back, a bat wing raised off its shoulders, a hunched neck, a wide horned
/// head thrust out past the table's edge with its jaws open like a spout, and forelegs gripping the edge.
struct Gargoyle {
    /// The trunk on its haunch, the neck and head, the foreleg and the tail, without the wing.
    body: Vec<Keys>,
    /// The wing, a membrane standing thinner than the body.
    wing: Keys,
    /// The wing's finger bones, raised on the membrane from the wrist.
    ribs: Vec<Keys>,
    /// The near haunch and the near foreleg, standing proud of the body.
    thigh: Keys,
    foreleg: Keys,
    /// The head's mass, wider than the neck, and the brow over the eye.
    head: Keys,
    brow: Keys,
    eye: Keys,
}

/// A rib from `a` to `b`, `w` wide, its ends squared.
fn strip(a: P2, b: P2, w: f64) -> Keys {
    let d = sub2(b, a);
    let l = len2(d);
    let n = [-d[1] / l * w / 2.0, d[0] / l * w / 2.0];
    vec![(a[0] + n[0], a[1] + n[1], true), (b[0] + n[0], b[1] + n[1], true), (b[0] - n[0], b[1] - n[1], true), (a[0] - n[0], a[1] - n[1], true)]
}

impl Gargoyle {
    fn of(rev: bool) -> Self {
        // The body in four overlapping pieces, each drawn and raised on its own: the trunk on its folded haunch, the
        // neck and head, the foreleg, and the tail.
        let body = vec![
            vec![
                (-2.8, -0.6, true),
                (-6.0, -0.6, true),
                (-6.6, 1.6, false),
                (-6.3, 3.0, false),
                (-5.4, 4.4, false),
                (-4.4, 6.0, false),
                (-3.2, 7.6, false),
                (-1.8, 8.6, false),
                (-0.4, 8.4, false),
                (1.0, 6.6, false),
                (2.4, 4.6, false),
                (2.8, 2.2, false),
                (1.0, 2.0, false),
                (-1.4, 1.9, false),
                (-2.6, 1.0, false),
            ],
            vec![
                (-2.2, 7.4, false),
                (-1.6, 8.6, false),
                (-0.6, 9.1, false),
                // The horn sweeping back.
                (-1.4, 10.2, false),
                (-2.2, 11.2, false),
                (-1.6, 11.8, false),
                (-0.2, 11.2, false),
                (0.9, 10.0, false),
                // The brow, the snout and the nose.
                (1.9, 9.8, true),
                (3.6, 9.4, false),
                (5.4, 9.1, false),
                (6.4, 8.9, true),
                (6.7, 8.1, false),
                (6.5, 7.4, false),
                // The upper fang, back along the upper jaw into the gape.
                (6.3, 6.6, true),
                (5.6, 7.2, false),
                (3.9, 7.4, false),
                (2.2, 7.0, false),
                // The lower jaw out to its tusk, the chin and the throat.
                (3.9, 6.3, false),
                (5.3, 6.5, true),
                (5.6, 5.6, false),
                (4.6, 5.0, false),
                (2.8, 4.8, false),
                (1.0, 5.4, false),
                (-0.8, 6.4, false),
            ],
            vec![
                (1.4, 5.8, false),
                (2.8, 3.8, false),
                (3.8, 2.4, false),
                (4.6, 1.1, false),
                (5.7, 0.4, false),
                (6.6, -0.2, false),
                (6.5, -1.3, false),
                (5.9, -0.6, true),
                (3.2, -0.6, true),
                (2.6, 1.2, false),
                (1.4, 3.0, false),
                (0.4, 5.0, false),
            ],
            vec![(-4.0, -0.6, true), (-11.0, -0.6, true), (-12.6, 0.2, false), (-11.6, 1.0, true), (-10.6, 0.5, false), (-8.0, 0.4, false), (-5.0, 0.7, false)],
        ];
        let wing = vec![
            (-4.6, 4.6, false),
            (-6.0, 5.4, false),
            // The trailing edge, scalloped between the fingers, out to the tip.
            (-7.0, 7.2, true),
            (-8.0, 7.2, true),
            (-8.2, 8.8, true),
            (-9.4, 9.0, true),
            (-9.8, 11.4, true),
            // The leading edge back to the wrist's hooked claw, and down to the shoulder.
            (-8.0, 12.0, false),
            (-6.0, 12.6, false),
            (-4.6, 13.6, true),
            (-4.0, 12.4, false),
            (-3.0, 10.2, false),
            (-2.4, 8.0, false),
        ];
        let wrist = [-4.8, 12.4];
        let ribs = vec![strip(wrist, [-9.4, 11.1], 0.55), strip(wrist, [-8.8, 8.9], 0.55), strip(wrist, [-7.6, 7.3], 0.55), strip([-2.9, 8.6], [-4.6, 12.0], 0.6)];
        let thigh = vec![(-5.4, -0.6, true), (-6.2, 0.8, false), (-6.0, 2.8, false), (-4.8, 4.2, false), (-3.2, 4.0, false), (-2.4, 2.6, false), (-2.6, 0.8, false), (-3.0, -0.6, true)];
        let foreleg = vec![(1.2, 6.0, false), (2.4, 4.4, false), (3.6, 2.6, false), (4.6, 1.2, false), (5.8, 0.3, false), (6.1, -0.6, true), (3.6, -0.6, true), (3.0, 0.8, false), (1.8, 2.6, false), (0.4, 4.6, false)];
        let head = vec![(0.0, 8.6, false), (1.0, 9.6, false), (2.2, 9.6, false), (4.2, 9.2, false), (6.2, 8.7, false), (6.3, 7.6, false), (4.0, 7.3, false), (2.4, 7.1, false), (4.0, 6.2, false), (5.2, 5.4, false), (3.6, 5.1, false), (1.4, 5.8, false), (0.0, 7.0, false)];
        let brow = vec![(0.8, 9.6, false), (2.0, 10.0, true), (3.8, 9.5, false), (4.6, 8.8, false), (3.4, 8.5, false), (1.8, 8.6, false), (0.8, 9.0, false)];
        let eye = vec![(1.7, 9.1, false), (2.9, 9.45, false), (4.0, 9.0, false), (2.8, 8.6, false)];
        let mut g = Self { body, wing, ribs, thigh, foreleg, head, brow, eye };
        if rev {
            // The revision thrusts the head out past the table's edge and down, like a spout, and drops the tail for the pinnacle behind.
            let out = |k: &mut Keys| k.iter_mut().for_each(|p| {
                p.0 += 2.0;
                p.1 -= 0.8;
            });
            out(&mut g.body[1]);
            out(&mut g.head);
            out(&mut g.brow);
            out(&mut g.eye);
            g.body.truncate(3);
        }
        g
    }
}

/// Factory 015 Octagon's sand master: the table's height over the axis across the head, from the bare stock's outline at the parting plane.
fn table_top(d: &RingDesign) -> Result<(f64, f64, Vec<(f64, f64)>)> {
    let mut bare = d.clone();
    bare.cad = None;
    let m = mesh::try_build(&bare, &AlphaLibrary::builtin(), draft_params())?.mesh;
    let mut top = vec![f64::MIN; 41];
    for v in &m.vertices {
        if (v.2 as f64).abs() < 0.5 && v.1 > 0.0 {
            let i = ((v.0 as f64 + 10.0) * 2.0).round();
            if (0.0..=40.0).contains(&i) {
                top[i as usize] = top[i as usize].max(v.1 as f64);
            }
        }
    }
    let row: Vec<(f64, f64)> = top.iter().enumerate().map(|(i, y)| (i as f64 * 0.5 - 10.0, *y)).collect();
    let flat: Vec<f64> = row.iter().filter(|(x, _)| x.abs() <= 5.0).map(|(_, y)| *y).collect();
    let low = flat.iter().cloned().fold(f64::MAX, f64::min);
    let high = flat.iter().cloned().fold(f64::MIN, f64::max);
    Ok((low, high, row))
}

/// Keys drawn `k` times their size about their own centre.
fn shrunk(keys: &Keys, k: f64) -> Keys {
    let n = keys.len() as f64;
    let c = keys.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0 / n, a.1 + p.1 / n));
    keys.iter().map(|p| (c.0 + (p.0 - c.0) * k, c.1 + (p.1 - c.1) * k, p.2)).collect()
}

fn author_gargoyle(rev: bool) -> Result<(RingDesign, serde_json::Value)> {
    let mut d = probe::stock("015", true, None)?;
    d.name = "Ogiva spike — the gargoyle".into();
    let (low, high, row) = table_top(&d)?;
    // The figure stands on the table's lowest point across its middle, set forward so the claws hook the edge and the head clears it.
    let scale = if rev { 1.0 } else { 0.95 };
    let (lift, shift) = (low, 9.4 - 6.6 * scale);
    let place = |keys: &Keys| -> Vec<P2> { spline(&keys.iter().map(|(u, v, k)| (u * scale, v * scale, *k)).collect::<Vec<_>>()).into_iter().map(|[u, v]| [u + shift, v + lift]).collect() };
    let g = Gargoyle::of(rev);
    // Half-widths along the finger: the wing's membrane and ribs, the body, the head, the near limbs and brow standing proud, the eye sunk.
    let (membrane, rib, body, head, limbs, brow, eye_floor) = if rev { (1.1, 1.6, 2.0, 3.3, 3.5, 3.6, 2.9) } else { (0.8, 1.2, 2.0, 2.6, 2.9, 3.0, 2.3) };
    let mut t = Tree { doc: Document::default() };
    t.add_as("The factory 015 Octagon sand master", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() });
    let parting = t.plane("Lay the parting plane through the head", -PARTING_OVERLAP_MM);
    // A part raised in nested layers along the pull, each smaller about its centre and standing further out, so it rounds toward its sides.
    let layered = |t: &mut Tree, what: &str, keys: &Keys, layers: &[(f64, f64)]| -> Id {
        let mut cur = t.both_halves(what, parting, &[place(&shrunk(keys, layers[0].0))], layers[0].1, DRAFT_DEG);
        for (i, (k, half)) in layers.iter().enumerate().skip(1) {
            let next = t.both_halves(&format!("{what}, rounded {i}"), parting, &[place(&shrunk(keys, *k))], *half, DRAFT_DEG);
            cur = t.boolean(&format!("Round the {what} ({i})"), cur, next, Boolean::Union);
        }
        cur
    };
    let pieces = ["trunk on its folded haunch", "neck and horned head", "foreleg", "tail laid along the table"];
    let trunk: Vec<(f64, f64)> = if rev { vec![(1.0, 1.3), (0.9, 2.1), (0.78, 2.8)] } else { vec![(1.0, body)] };
    let mut fig = layered(&mut t, pieces[0], &g.body[0], &trunk);
    for (i, k) in g.body.iter().enumerate().skip(1) {
        let next = t.both_halves(pieces[i], parting, &[place(k)], body, DRAFT_DEG);
        fig = t.boolean(&format!("Join the {}", pieces[i]), fig, next, Boolean::Union);
    }
    let wing = t.both_halves("raised wing's membrane", parting, &[place(&g.wing)], membrane, DRAFT_DEG);
    // Each finger bone stands alone, since they meet at the wrist and one sketch takes no touching loops.
    let mut ribs = t.both_halves("wing's first finger bone", parting, &[place(&g.ribs[0])], rib, DRAFT_DEG);
    for (i, r) in g.ribs.iter().enumerate().skip(1) {
        let next = t.both_halves(&format!("wing's finger bone {}", i + 1), parting, &[place(r)], rib, DRAFT_DEG);
        ribs = t.boolean(&format!("Join finger bone {}", i + 1), ribs, next, Boolean::Union);
    }
    let (thigh_l, fore_l, head_l): (Vec<(f64, f64)>, Vec<(f64, f64)>, Vec<(f64, f64)>) = if rev {
        (vec![(1.0, 2.3), (0.85, 3.0), (0.7, limbs)], vec![(1.0, 2.2), (0.85, limbs)], vec![(1.0, 2.2), (0.88, 2.8), (0.74, head)])
    } else {
        (vec![(1.0, limbs)], vec![(1.0, limbs)], vec![(1.0, head)])
    };
    let thigh = layered(&mut t, "near haunch", &g.thigh, &thigh_l);
    let fore = layered(&mut t, "near foreleg", &g.foreleg, &fore_l);
    let head_id = layered(&mut t, "head's mass", &g.head, &head_l);
    let brow_id = t.both_halves("brow over the eye", parting, &[place(&g.brow)], brow, DRAFT_DEG);
    let (eye_cope, eye_drag) = t.pocket("eye socket", &[place(&g.eye)], brow + 0.3, eye_floor);
    let mut cur = t.boolean("Raise the wing off the shoulders", fig, wing, Boolean::Union);
    cur = t.boolean("Raise the finger bones on the wing", cur, ribs, Boolean::Union);
    cur = t.boolean("Set the haunches on the body", cur, thigh, Boolean::Union);
    cur = t.boolean("Set the forelegs on the body", cur, fore, Boolean::Union);
    cur = t.boolean("Widen the head", cur, head_id, Boolean::Union);
    cur = t.boolean("Set the brows on the head", cur, brow_id, Boolean::Union);
    let mut extra = json!(null);
    if rev {
        // A masonry pinnacle at the back of the table for the wing to fold against: a pier under a gabled top, a blind lancet in its face.
        let (x0, x1) = (-8.9, -6.3);
        let pier = vec![[x0, lift - 0.6], [x1, lift - 0.6], [x1, lift + 7.0], [(x0 + x1) / 2.0, lift + 9.6], [x0, lift + 7.0]];
        let pin = t.both_halves("pinnacle behind the wing", parting, &[pier], 2.6, DRAFT_DEG);
        let light: Vec<P2> = lancet(1.1, 1.4, 5.8).into_iter().map(|[x, y]| [(x0 + x1) / 2.0 + x, lift + y]).collect();
        let (l_cope, l_drag) = t.pocket("pinnacle's blind lancet", &[light], 2.9, 2.0);
        let pin = t.boolean("Sink the pinnacle's cope lancet", pin, l_cope, Boolean::Subtract);
        let pin = t.boolean("Sink the pinnacle's drag lancet", pin, l_drag, Boolean::Subtract);
        cur = t.boolean("Fold the wing against the pinnacle", cur, pin, Boolean::Union);
        extra = json!({"pinnacle": {"x_mm": [x0, x1], "height_over_table_mm": 9.6, "half_mm": 2.6}, "head_thrust_mm": [2.0, -0.8], "arcade": {"lancets_per_wall": 7, "width_mm": 1.2, "y_mm": [9.9, 12.6], "wall_z_mm": 8.07, "floor_z_mm": 7.5}});
    }
    cur = t.boolean("Sink the cope side's eye", cur, eye_cope, Boolean::Subtract);
    t.add_as(
        "Sink the drag side's eye and seat the gargoyle on the table",
        Operation::Boolean { a: cur, b: eye_drag, kind: Boolean::Subtract },
        Component { role: ComponentRole::Other, material: "Silver 925".into(), attach: Attach::Join, stage: Stage::Cast, blend_mm: 0.0, ..Component::default() },
    );
    if rev {
        // A blind lancet arcade along each head wall under the table, sunk along the pull into the stock.
        let arcade: Vec<Vec<P2>> = (0..7).map(|i| lancet(1.2, 9.9, 12.6).into_iter().map(|[x, y]| [x - 6.0 + 2.0 * i as f64, y]).collect()).collect();
        let (a_cope, a_drag) = t.pocket("lancet arcade on the head wall", &arcade, 8.4, 7.5);
        t.add_as(
            "Cut both head walls' arcades into the stock",
            Operation::Boolean { a: a_cope, b: a_drag, kind: Boolean::Union },
            Component { role: ComponentRole::Other, material: "Silver 925".into(), attach: Attach::Cut, stage: Stage::Cast, blend_mm: 0.0, ..Component::default() },
        );
    }
    d.cad = Some(t.doc);
    let info = json!({
        "stock": "015 Octagon, Delft sand master",
        "table_top_mm": {"low": low, "high": high, "profile": row},
        "figure_scale": scale,
        "figure_shift_mm": shift,
        "half_widths_mm": {"membrane": membrane, "ribs": rib, "body": body, "head": head, "limbs": limbs, "brow": brow, "eye_floor": eye_floor},
        "revision": extra,
    });
    Ok((d, info))
}

// --- Measures --------------------------------------------------------------------------------------

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

/// Factory stock keeps the smooth normals it was built with; the figure standing on it above `y` shades flat at its creases.
fn figure_creased(m: &mesh::Mesh, y: f32) -> mesh::Mesh {
    let flat = creased(m);
    let mut out = mesh::Mesh::default();
    for (k, f) in m.faces.iter().enumerate() {
        let base = out.vertices.len() as u32;
        let above = f.iter().all(|&i| m.vertices[i as usize].1 > y);
        for (c, &i) in f.iter().enumerate() {
            out.vertices.push(m.vertices[i as usize]);
            out.normals.push(if above { flat.normals[3 * k + c] } else { m.normals[i as usize] });
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

// --- Renders ---------------------------------------------------------------------------------------

/// Each named view as the direction from the ring toward the eye, world +y (the crown) kept up. The face is
/// the ring seen along the finger, where every option draws its subject; the side looks across the finger.
const HERO: P3 = [0.6, 0.32, 0.75];
const FACE: P3 = [0.0, 0.0, 1.0];
const SIDE: P3 = [1.0, 0.1, 0.1];
const GARGOYLE_HERO: P3 = [0.38, 0.36, 0.85];

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

fn renders(out: &Path, option: &str, rev: bool, fin: &render::Finished, edge: usize) -> Result<Vec<String>> {
    // A CAD-only ring's csg faces shade flat at their creases; factory stock keeps the normals it was built with.
    let metal = if option == "gargoyle" { figure_creased(&fin.metal, 13.6) } else { creased(&fin.metal) };
    let mut parts: Vec<(&mesh::Mesh, bool, [f32; 3])> = vec![(&metal, false, render::GOLD)];
    parts.extend(fin.stones.iter().map(|(m, t)| (m, true, *t)));
    let mut written = Vec::new();
    let mut save = |name: &str, img: Vec<u8>, e: usize| -> Result<()> {
        png(&out.join(name), &img, e)?;
        written.push(name.to_string());
        Ok(())
    };
    // The gargoyle faces round the ring, so its hero turns further along the finger to keep the figure in three-quarter profile.
    let hero = match option {
        "gargoyle" => GARGOYLE_HERO,
        // The keel's revision turns the hero toward the side, so the near edge profiles as a lancet against the ground.
        "keel-section" if rev => [0.85, 0.35, 0.4],
        _ => HERO,
    };
    save("hero.png", shot(&parts, hero, edge), edge)?;
    save("face.png", shot(&parts, FACE, edge), edge)?;
    save("hero-300.png", shot(&parts, hero, 300), 300)?;
    save("face-300.png", shot(&parts, FACE, 300), 300)?;
    if option == "keel-section" {
        // End-on: looking along the keel at the crown, a short run of it only, so the lancet stands in the frame.
        // The crown above y = 8, seen exactly along the keel: its cut faces down, out of sight, and the lancet stands on top.
        let crown = boxed(&fin.metal, [-40.0, 8.0, -40.0], [40.0, 40.0, 40.0])?;
        save("side.png", shot(&[(&crown, false, render::GOLD)], [1.0, 0.0, 0.0001], edge), edge)?;
        // The ring cut through the finger's axis on the plane x = 0 and kept above the axis: the section at the crown, its cut face toward +x.
        let section = boxed(&fin.metal, [-0.3, 2.0, -40.0], [0.0, 40.0, 40.0])?;
        save("section.png", shot(&[(&section, false, render::GOLD)], [1.0, 0.0, 0.0001], edge), edge)?;
    } else {
        save("side.png", shot(&parts, SIDE, edge), edge)?;
    }
    Ok(written)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rev = args.iter().any(|a| a == "--rev");
    let mut plain = args.iter().filter(|a| !a.starts_with("--"));
    let option = plain.next().context("name an option: arch, keel-section or gargoyle")?.clone();
    let out = plain
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/ogiva-spike").join(&option));
    std::fs::create_dir_all(&out)?;
    let started = std::time::Instant::now();
    let (d, info) = match option.as_str() {
        "arch" => author_arch(rev)?,
        "keel-section" => author_keel(rev)?,
        "gargoyle" => author_gargoyle(rev)?,
        other => bail!("unknown option {other}"),
    };
    println!("Ogiva spike: {option}{}", if rev { " (revision)" } else { "" });
    println!("  {info}");
    let lib = AlphaLibrary::builtin();
    let params = draft_params();
    let built = mesh::try_build(&d, &lib, params)?;
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let statuses: Vec<String> = built.parts.evaluated.iter().flat_map(|e| e.features.iter()).filter(|r| !r.status.is_ok()).map(|r| format!("#{} {:?}", r.id, r.status)).collect();
    println!(
        "  {} triangles in {:.1} s; watertight {watertight}; degenerate {degenerate}; crossings {crossings}; notes {:?} {:?}; features not Ok {:?}",
        built.mesh.faces.len(),
        started.elapsed().as_secs_f64(),
        built.solids.notes,
        built.parts.notes,
        statuses
    );
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    // Silver 925 at 10.36 g/cm³.
    let grams = built.report.volume_mm3 * 10.36 / 1000.0;
    // The quick draft check: the sand field verdict and the ray release at 0.100 mm, and the 0.8 mm local wall the wax route would need.
    let field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    let setup = d.manufacturing.clone().context("set-up")?;
    let inspection = mf::inspect(&d, &lib, &setup, params)?;
    let wall = cad::measure::thickness(&built.mesh, 0.8);
    let findings = dfm::findings_in(&d, &lib);
    let mut worst: Vec<_> = inspection.release.obstructions.iter().collect();
    worst.sort_by(|a, b| b.depth_mm.total_cmp(&a.depth_mm));
    let worst: Vec<serde_json::Value> = worst.iter().take(6).map(|o| json!({"world": o.world.map(|v| (v * 100.0).round() / 100.0), "depth_mm": o.depth_mm, "area_mm2": o.projected_area_mm2})).collect();
    let check = json!({
        "option": option,
        "revision": rev,
        "info": info,
        "geometry": {"triangles": built.mesh.faces.len(), "watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings, "solids_notes": built.solids.notes, "parts_notes": built.parts.notes, "features_not_ok": statuses, "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm, "grams": grams},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "sand": {
            "field_verdict": field.verdict.label(),
            "field_notes": field.notes,
            "undercut_percent": field.undercut_fraction() * 100.0,
            "release_0_100": {"status": format!("{:?}", inspection.release.status), "obstructions": inspection.release.obstructions.len(), "unresolved_rays": inspection.release.unresolved_rays, "deepest_mm": inspection.release.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max), "area_mm2": inspection.release.obstructions.iter().map(|o| o.projected_area_mm2).sum::<f64>(), "worst": worst},
        },
        "wax": {"thickness_limit_mm": wall.limit_mm, "sampled_min_mm": wall.sampled_min_mm, "below_limit": wall.below_limit, "rays": wall.rays, "worst_point": wall.point},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
    });
    println!(
        "  field {}; release {:?} {} obstructions {} unresolved; wall min {:?} ({} below 0.8); dfm {}; {:.1} g",
        field.verdict.label(),
        inspection.release.status,
        inspection.release.obstructions.len(),
        inspection.release.unresolved_rays,
        wall.sampled_min_mm,
        wall.below_limit,
        findings.len(),
        grams
    );
    let fin = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(&d, &lib, &built) };
    let written = renders(&out, &option, rev, &fin, 900)?;
    let name = if rev { "check-2.json" } else { "check.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&json!({"check": check, "renders": written}))?)?;
    println!("  wrote {} in {:.1} s", out.display(), started.elapsed().as_secs_f64());
    Ok(())
}
