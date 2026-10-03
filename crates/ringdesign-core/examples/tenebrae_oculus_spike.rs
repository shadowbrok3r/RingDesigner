//! Tenebrae — Oculus concept spike: three rethinks of the wheel window, each blocked out for a read test.
//! `flange`: one raised flange on the high band edge carrying the whole wheel (hub, lancets on spokes, a ring of foils).
//! `head`: the wheel standing up off the crown as a vertical disc across the band, square to the finger, like a gable's oculus.
//! `stones`: the wheel's lights made of stained-glass stones in collets on the side face of a deep band.
//! cargo build --release -p ringdesign-core --example tenebrae_oculus_spike
//! target/release/examples/tenebrae_oculus_spike <flange|head|stones> [OUT_DIR] [--rev]
use anyhow::{Result, bail};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, PlaneBase, Profile, Stage, builders},
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch},
};
use serde_json::json;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::path::PathBuf;

const BORE_MM: f64 = 18.6;
const BORE_R_MM: f64 = BORE_MM * 0.5;
/// Least metal the investment fills, and the least a tracery bar or rail may be, mm.
const MIN_SECTION_MM: f64 = 0.8;
/// How far past a face a through-cut runs, mm.
const OVERSHOOT_MM: f64 = 0.3;

#[derive(Clone, Copy, PartialEq)]
enum Option_ {
    Flange,
    Head,
    Stones,
}
impl Option_ {
    fn slug(self) -> &'static str {
        match self {
            Self::Flange => "flange",
            Self::Head => "head",
            Self::Stones => "stones",
        }
    }
}

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

/// A flat band, squared side faces, uniform all the way round, lost wax.
fn band(width: f64, thickness: f64) -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Oculus".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = width;
    d.profile.thickness_mm = thickness;
    d.profile.flatten_sides();
    d.profile.shape_a = 1.7;
    d.profile.edge_round_mm = 0.25;
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Uniform;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.build = draft_params();
    d
}

// --- 2D outlines ------------------------------------------------------------------------------------------------------

type P2 = [f64; 2];
fn rot(p: P2, a: f64) -> P2 {
    let (s, c) = a.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}
fn sub2(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: P2) -> f64 {
    a[0].hypot(a[1])
}

/// The wheel's tracery, drawn about its own centre with the first lancet's axis on +y: `lights` lancets whose jambs run
/// parallel to straight spokes `spoke` wide, sills at `r_sill`, springing at `r_spring`, equilateral pointed heads; and
/// between each pair of heads a quatrefoil of overall width `foil` centred at `r_foil`.
#[derive(Clone, Copy)]
struct Wheel {
    lights: usize,
    spoke: f64,
    r_sill: f64,
    r_spring: f64,
    foil: f64,
    r_foil: f64,
}
impl Wheel {
    fn lancet(&self) -> Vec<P2> {
        let phi = PI / self.lights as f64;
        let s = self.spoke * 0.5;
        // Right jamb: offset s inside the spoke centreline at +phi from the axis.
        let d = [phi.sin(), phi.cos()];
        let n = [-phi.cos(), phi.sin()];
        let at = |r: f64| { let t = (r * r - s * s).max(0.0).sqrt(); [t * d[0] + s * n[0], t * d[1] + s * n[1]] };
        let (sill, spring) = (at(self.r_sill), at(self.r_spring));
        let w = 2.0 * spring[0];
        let left = [-spring[0], spring[1]];
        let steps = 10;
        let head = |k: usize| { let a = (60.0 * k as f64 / steps as f64).to_radians(); [left[0] + w * a.cos(), left[1] + w * a.sin()] };
        let mut pts = vec![[-sill[0], sill[1]], sill];
        pts.extend((0..steps).map(head));
        pts.push([0.0, spring[1] + w * 60f64.to_radians().sin()]);
        pts.extend((0..steps).rev().map(|k| { let p = head(k); [-p[0], p[1]] }));
        pts
    }
    fn apex_r(&self) -> f64 {
        self.lancet().iter().map(|p| len2(*p)).fold(0.0, f64::max)
    }
    /// Every loop of the wheel, placed: lancets then foils.
    fn loops(&self, turn: f64) -> Vec<Vec<P2>> {
        let mut out = Vec::new();
        let lancet = self.lancet();
        let foil = quatrefoil(self.foil);
        for k in 0..self.lights {
            let a = turn + TAU * k as f64 / self.lights as f64;
            out.push(lancet.iter().map(|p| rot(*p, a)).collect());
        }
        for k in 0..self.lights {
            let a = turn + TAU * (k as f64 + 0.5) / self.lights as f64;
            out.push(foil.iter().map(|p| rot([p[0], p[1] + self.r_foil], a)).collect());
        }
        out
    }
    fn lands(&self) -> serde_json::Value {
        let l = self.loops(0.0);
        let n = self.lights;
        let r_lo = l.iter().flatten().map(|p| len2(*p)).fold(f64::MAX, f64::min);
        let r_hi = l.iter().flatten().map(|p| len2(*p)).fold(0.0, f64::max);
        json!({
            "lights": n,
            "spoke_mm": self.spoke,
            "lancet_to_foil_mm": gap(&l[0], &l[n]),
            "foil_to_foil_mm": gap(&l[n], &l[n + 1]),
            "lancet_to_lancet_mm": gap(&l[0], &l[1]),
            "inner_radius_mm": r_lo,
            "outer_radius_mm": r_hi,
            "lancet_apex_r_mm": self.apex_r(),
            "lancet_width_at_spring_mm": 2.0 * self.lancet()[2][0].abs(),
        })
    }
}

/// A quatrefoil `width` across: four round lobes on the axes, cusps between with round tips. Counter-clockwise.
fn quatrefoil(width: f64) -> Vec<P2> {
    let k = width / 1.52;
    let (e, r, rc) = (0.40 * k, 0.36 * k, 0.42 * k);
    let c = |i: usize| { let a = i as f64 * FRAC_PI_2; [e * a.cos(), e * a.sin()] };
    let s45 = std::f64::consts::FRAC_1_SQRT_2;
    let t = e * s45 + ((r + rc).powi(2) - (e * s45).powi(2)).sqrt();
    let f = |i: usize| { let a = (i as f64 + 0.5) * FRAC_PI_2; [t * a.cos(), t * a.sin()] };
    let touch = |lobe: P2, tip: P2| { let d = sub2(tip, lobe); let l = len2(d); [lobe[0] + r * d[0] / l, lobe[1] + r * d[1] / l] };
    let arc = |c: P2, r: f64, a: P2, b: P2, via: P2, steps: usize| -> Vec<P2> {
        let ang = |p: P2| (p[1] - c[1]).atan2(p[0] - c[0]);
        let (a0, a1, av) = (ang(a), ang(b), ang(via));
        let wrap = |x: f64| x.rem_euclid(TAU);
        let ccw = wrap(a1 - a0);
        let sweep = if wrap(av - a0) <= ccw { ccw } else { ccw - TAU };
        (0..steps).map(|k| { let t = a0 + sweep * k as f64 / steps as f64; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect()
    };
    let mut pts = Vec::new();
    for i in 0..4 {
        let (ci, fin, fout, next) = (c(i), f((i + 3) % 4), f(i), c((i + 1) % 4));
        pts.extend(arc(ci, r, touch(ci, fin), touch(ci, fout), [ci[0] * 2.0, ci[1] * 2.0], 12));
        let inner = { let l = len2(fout); [fout[0] * (l - rc) / l, fout[1] * (l - rc) / l] };
        pts.extend(arc(fout, rc, touch(ci, fout), touch(next, fout), inner, 6));
    }
    pts.rotate_left(6);
    pts
}

/// Shortest distance between two closed polygons, mm.
fn gap(a: &[P2], b: &[P2]) -> f64 {
    let seg = |p: P2, a: P2, b: P2| {
        let (d, e) = (sub2(b, a), sub2(p, a));
        let t = ((e[0] * d[0] + e[1] * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-18)).clamp(0.0, 1.0);
        len2(sub2(p, [a[0] + d[0] * t, a[1] + d[1] * t]))
    };
    let one = |a: &[P2], b: &[P2]| a.iter().map(|p| (0..b.len()).map(|k| seg(*p, b[k], b[(k + 1) % b.len()])).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
    one(a, b).min(one(b, a))
}

/// Closed loops as one sketch on work plane `plane`, each moved by `offset`.
fn loops_sketch(name: &str, loops: &[Vec<P2>], offset: P2, plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    for pts in loops {
        let ids: Vec<Id> = pts.iter().map(|p| s.point([p[0] + offset[0], p[1] + offset[1]])).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}
/// Concentric circles about `c` as one sketch: an annulus from two, a disc from one.
fn circles_sketch(name: &str, c: P2, radii: &[f64], plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    for r in radii {
        let centre = s.point(c);
        let rim = s.point([c[0] + r, c[1]]);
        s.entity(Geometry::Circle { center: centre, rim });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}
fn cut() -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, stage: Stage::Cast, ..Default::default() }
}
fn join(blend: f64) -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Join, stage: Stage::Cast, blend_mm: blend, ..Default::default() }
}
fn plane(offset_mm: f64) -> Operation {
    Operation::Plane { base: PlaneBase::Parting, offset_mm }
}
fn extrude(sketch: Id, height_mm: f64, draft_deg: f64) -> Operation {
    Operation::Extrude { sketch: Profile::Feature { feature: sketch }, height_mm, draft_deg }
}
fn mirror(source: Id) -> Operation {
    Operation::Pattern { sources: cad::pattern::Sources(vec![source]), kind: PatternKind::Mirror { plane: cad::MirrorPlane::Band } }
}

/// What a block-out is, with the numbers its report carries.
struct Authored {
    design: RingDesign,
    summary: String,
    lands: serde_json::Value,
    /// Where the bench darkens the piercing walls: a centre in the finger's plane and the radii between which walls are antiqued.
    antique: Option<(P2, f64, f64)>,
}

// --- Option 1: the flange ---------------------------------------------------------------------------------------------

fn flange(rev: bool) -> Result<Authored> {
    let (width, thickness) = (6.0, 3.0);
    let half = width * 0.5;
    // The flange: an annulus on the high edge, 2.1 thick along the finger, standing 0.3 proud of the band's side face. The lancets
    // spring above the crown, so only their jambs cross it, square to it.
    let (flange_in, flange_out, flange_t, proud) = (9.5, BORE_R_MM + 6.9, 2.1, 0.3);
    let wheel = if rev {
        Wheel { lights: 28, spoke: 0.85, r_sill: 10.5, r_spring: 12.45, foil: 1.25, r_foil: 14.5 }
    } else {
        Wheel { lights: 24, spoke: 0.9, r_sill: 10.5, r_spring: 12.45, foil: 1.3, r_foil: 14.65 }
    };
    // Hub and rim mouldings: the field between them is sunk into the flange's face.
    let (field_in, field_out, field_depth) = (10.35, flange_out - 0.8, 0.4);
    let mut d = band(width, thickness);
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Component::default()))?;
    doc.append(feature(2, "The flange's outer face, proud of the high side face", plane(half + proud), Component::default()))?;
    doc.append(feature(3, "The flange: an annulus round the finger out to six and a half millimetres of run", Operation::Sketch { sketch: circles_sketch("Flange", [0.0, 0.0], &[flange_out, flange_in], 2) }, Component::default()))?;
    doc.append(feature(4, "Raise the flange along the band's high edge", extrude(3, -flange_t, 0.0), join(0.3)))?;
    doc.append(feature(5, "The tracery field between the hub and the rim", Operation::Sketch { sketch: circles_sketch("Field", [0.0, 0.0], &[field_out, field_in], 2) }, Component::default()))?;
    doc.append(feature(6, "Sink the field, leaving the hub and rim mouldings standing", extrude(5, -field_depth, 20.0), cut()))?;
    doc.append(feature(7, "The flange's face, lifted clear of the metal", plane(half + proud + OVERSHOOT_MM), Component::default()))?;
    let loops = wheel.loops(0.0);
    let bay = [loops[0].clone(), loops[wheel.lights].clone()];
    doc.append(feature(8, "One bay of the wheel: a lancet on its spokes and the quatrefoil beside its head", Operation::Sketch { sketch: loops_sketch("Bay", &bay, [0.0, 0.0], 7) }, Component::default()))?;
    doc.append(feature(9, "Pierce the bay through the flange and the band", extrude(8, -(width + proud + 2.0 * OVERSHOOT_MM), 0.0), cut()))?;
    doc.append(feature(
        10,
        "Wheel the bay round the finger",
        Operation::Pattern { sources: cad::pattern::Sources(vec![9]), kind: PatternKind::Ring { count: wheel.lights as u32, span_deg: 360.0 } },
        cut(),
    ))?;
    d.cad = Some(doc);
    let mut lands = wheel.lands();
    lands["bore_rail_mm"] = json!(wheel.r_sill - BORE_R_MM);
    lands["rim_rail_mm"] = json!(flange_out - lands["outer_radius_mm"].as_f64().unwrap());
    lands["hub_step_to_light_mm"] = json!(lands["inner_radius_mm"].as_f64().unwrap() - field_in);
    lands["rim_step_to_light_mm"] = json!(field_out - lands["outer_radius_mm"].as_f64().unwrap());
    lands["radial_run_mm"] = json!(flange_out - BORE_R_MM);
    lands["flange_proud_of_crown_mm"] = json!(flange_out - BORE_R_MM - thickness);
    lands["flange_thickness_mm"] = json!(flange_t + proud);
    let summary = format!(
        "{width} x {thickness} band with one {:.1} mm flange on its high edge running out to r {flange_out:.1} ({:.1} mm of radial run); the wheel is {} lancets on {:.1} mm spokes and {} quatrefoils between hub and rim mouldings, pierced through.",
        flange_t + proud, flange_out - BORE_R_MM, wheel.lights, wheel.spoke, wheel.lights
    );
    Ok(Authored { design: d, summary, lands, antique: Some(([0.0, 0.0], 10.0, flange_out - 0.4)) })
}

// --- Option 2: the head -----------------------------------------------------------------------------------------------

fn head(rev: bool) -> Result<Authored> {
    let (width, thickness) = (6.0, 2.6);
    let crown = BORE_R_MM + thickness;
    // The wheel stands across the band, square to the finger: a disc in the plane of the finger's turn.
    let (r_wheel, plate_t) = if rev { (6.5, 3.2) } else { (6.0, 3.0) };
    let wheel = if rev {
        Wheel { lights: 8, spoke: 0.9, r_sill: 1.95, r_spring: 3.7, foil: 1.3, r_foil: 4.95 }
    } else {
        Wheel { lights: 8, spoke: 0.85, r_sill: 2.0, r_spring: 3.0, foil: 1.0, r_foil: 4.5 }
    };
    let (field_in, field_out, field_depth) = (wheel.r_sill - 0.3, r_wheel - 0.85, 0.35);
    // A lancet points straight down; the lowest loops must clear the crown by a land so no cut runs into the band.
    let loops = wheel.loops(PI);
    let low = loops.iter().flatten().map(|p| p[1]).fold(f64::MAX, f64::min);
    let centre_y = crown + MIN_SECTION_MM + 0.1 - low;
    // Lost wax (Logan, 2026-10-03): no pull rule, so the wheel and plinth are straight extrusions through their thickness.
    let half_t = plate_t * 0.5;
    let mut d = band(width, thickness);
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Component::default()))?;
    doc.append(feature(2, "Back face of the wheel", plane(-half_t), Component::default()))?;
    doc.append(feature(3, "The wheel's disc, standing on the crown", Operation::Sketch { sketch: circles_sketch("Wheel disc", [0.0, centre_y], &[r_wheel], 2) }, Component::default()))?;
    doc.append(feature(4, "Stand the wheel up across the band", extrude(3, plate_t, 0.0), join(0.3)))?;
    // The plinth: a splayed foot from the wheel's lower rim into the band, so the wheel grows out of the crown.
    let foot_y = crown - 1.7;
    let plinth = vec![[-4.0, foot_y], [4.0, foot_y], [2.8, centre_y - r_wheel + 2.2], [-2.8, centre_y - r_wheel + 2.2]];
    doc.append(feature(6, "The plinth under the wheel", Operation::Sketch { sketch: loops_sketch("Plinth", &[plinth], [0.0, 0.0], 2) }, Component::default()))?;
    doc.append(feature(7, "Raise the plinth into the crown", extrude(6, plate_t, 0.0), join(0.3)))?;
    doc.append(feature(9, "Front face of the wheel", plane(half_t), Component::default()))?;
    doc.append(feature(10, "The tracery field between the hub and the rim", Operation::Sketch { sketch: circles_sketch("Field", [0.0, centre_y], &[field_out, field_in], 9) }, Component::default()))?;
    doc.append(feature(11, "Sink the field in the front face, leaving the hub and rim mouldings", extrude(10, -field_depth, 20.0), cut()))?;
    doc.append(feature(12, "Sink the same field in the back face", mirror(11), cut()))?;
    doc.append(feature(13, "Front face, lifted clear", plane(half_t + OVERSHOOT_MM), Component::default()))?;
    doc.append(feature(14, "The wheel: lancets on spokes and a ring of quatrefoils", Operation::Sketch { sketch: loops_sketch("Wheel", &loops, [0.0, centre_y], 13) }, Component::default()))?;
    doc.append(feature(15, "Pierce the wheel through", extrude(14, -(plate_t + 2.0 * OVERSHOOT_MM), 0.0), cut()))?;
    d.cad = Some(doc);
    let mut lands = wheel.lands();
    lands["rim_rail_mm"] = json!(r_wheel - lands["outer_radius_mm"].as_f64().unwrap());
    lands["hub_radius_mm"] = json!(lands["inner_radius_mm"]);
    lands["lowest_light_over_crown_mm"] = json!(centre_y + low - crown);
    lands["wheel_diameter_mm"] = json!(2.0 * r_wheel);
    lands["wheel_thickness_mm"] = json!(plate_t);
    lands["head_proud_of_crown_mm"] = json!(centre_y + r_wheel - crown);
    let summary = format!(
        "{width} x {thickness} band carrying a vertical wheel {:.1} mm across and {plate_t} thick, square to the finger and standing {:.1} mm off the crown on a splayed plinth; {} lancets on {:.2} mm spokes round a hub, {} quatrefoils, a rim moulding, pierced through.",
        2.0 * r_wheel, centre_y + r_wheel - crown, wheel.lights, wheel.spoke, wheel.lights
    );
    Ok(Authored { design: d, summary, lands, antique: Some(([0.0, centre_y], 0.5, r_wheel - 0.4)) })
}

// --- Option 3: the stones ---------------------------------------------------------------------------------------------

const SAPPHIRE: [f32; 3] = [0.02, 0.06, 0.45];
const RUBY: [f32; 3] = [0.45, 0.01, 0.04];

fn stones(rev: bool) -> Result<Authored> {
    let (width, thickness) = (7.0, 6.0);
    let half = width * 0.5;
    let crown = BORE_R_MM + thickness;
    let n = 12usize;
    // Lights: marquise sapphires pointing out along the spokes; foils: round rubies between their heads.
    let light = Gem { preview_tint: Some(SAPPHIRE), l_mm: if rev { 3.5 } else { 3.0 }, ..Gem::calibrated(GemCut::Marquise, if rev { 1.75 } else { 1.5 }) };
    let foil = Gem { preview_tint: Some(RUBY), ..Gem::calibrated(GemCut::Round, if rev { 1.5 } else { 1.25 }) };
    let (r_light, r_foil) = if rev { (11.95, 13.75) } else { (11.9, 13.6) };
    let (field_in, field_out, field_depth) = (10.1, crown - 0.95, 0.35);
    let mut d = band(width, thickness);
    let mut doc = Document::default();
    doc.append(feature(1, "Procedural shank", Operation::Band, Component::default()))?;
    doc.append(feature(2, "High side face", plane(half), Component::default()))?;
    doc.append(feature(3, "The wheel's field between the hub and the rim", Operation::Sketch { sketch: circles_sketch("Field", [0.0, 0.0], &[field_out, field_in], 2) }, Component::default()))?;
    doc.append(feature(4, "Sink the field, leaving the hub and rim mouldings", extrude(3, -field_depth, 20.0), cut()))?;
    // A stone on the side face: seated round the ring at its radius, turned to look along the finger, out of the high side face.
    let face_seat = |gem: Gem, r: f64, theta: f64, key_spin: f64| {
        let stand = builders::stand_off_mm("bezel", gem);
        Placement::Side { theta_deg: theta, radius_mm: r, face: ringdesign_core::field::SideFacePick::High, height_mm: stand - field_depth, spin_deg: key_spin, tilt_deg: 0.0 }
    };
    // Each stone placed by hand: a ring pattern drops its copies back onto the crown.
    let mut id = 5;
    for (label, gem, r, first, wall) in [("light", light, r_light, 90.0, 0.35), ("foil", foil, r_foil, 90.0 + 180.0 / n as f64, 0.3)] {
        for k in 0..n {
            let theta = first + 360.0 * k as f64 / n as f64;
            let mut stone = builders::stone_feature(id, gem, face_seat(gem, r, theta, 90.0));
            stone.name = format!("{} {} of the wheel", if label == "light" { "Light" } else { "Foil" }, k + 1);
            doc.append(stone)?;
            doc.append(builders::feature_on(id + 1, &format!("Collet of {label} {}", k + 1), builders::BEZEL, id, json!({"wall_mm": wall, "lip": 0.3})))?;
            doc.append(builders::feature_on(id + 2, &format!("Seat of {label} {}, through for the light", k + 1), builders::BUR, id, json!({"through": true})))?;
            id += 3;
        }
    }
    d.cad = Some(doc);
    // Lands between collets, measured on the girdle plans plus their walls.
    let half_len = |g: Gem, wall: f64| 0.5 * g.l_mm + wall;
    let half_w = |g: Gem, wall: f64| 0.5 * g.w_mm + wall;
    let step = TAU / n as f64;
    let light_light = 2.0 * r_light * (step * 0.5).sin() - 2.0 * half_w(light, 0.35);
    let foil_foil = 2.0 * r_foil * (step * 0.5).sin() - 2.0 * half_w(foil, 0.3);
    let lands = json!({
        "lights": n, "light": format!("{:?} {} x {}", light.cut, light.w_mm, light.l_mm), "foil": format!("{:?} {}", foil.cut, foil.w_mm),
        "light_radius_mm": r_light, "foil_radius_mm": r_foil,
        "light_collet_inner_r_mm": r_light - half_len(light, 0.35), "light_collet_outer_r_mm": r_light + half_len(light, 0.35),
        "foil_collet_outer_r_mm": r_foil + half_w(foil, 0.3),
        "light_to_light_mm": light_light, "foil_to_foil_mm": foil_foil,
        "side_face_run_end_r_mm": BORE_R_MM + thickness - d.profile.effective_crown_mm(),
    });
    let summary = format!(
        "{width} x {thickness} band whose high side face carries the wheel in stones: {n} marquise sapphires {} x {} pointing out along the spokes as the lights, {n} round rubies {} between their heads as the foils, each in a collet set through, inside sunk hub and rim mouldings.",
        light.w_mm, light.l_mm, foil.w_mm
    );
    Ok(Authored { design: d, summary, lands, antique: None })
}

// --- Build, check, render ---------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
}

/// The metal split for the antique finish: the walls the piercings leave, darkened, apart from the polished rest.
fn antiqued(m: &mesh::Mesh, (centre, r_lo, r_hi): (P2, f64, f64)) -> (mesh::Mesh, mesh::Mesh) {
    let wall = |f: &[u32; 3]| {
        let from_part = f.iter().all(|&i| m.origin.get(i as usize).is_some_and(|&o| o >= mesh::SOLID_VERTEX));
        let Some((a, b, c)) = m.triangle(f) else { return false };
        let (u, v) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        from_part && l > 1e-12 && (n[2] / l).abs() < 0.5 && {
            // A wall of a piercing or a sunk field inside the window, not the rim of a raised part.
            let r = len2([(a[0] + b[0] + c[0]) / 3.0 - centre[0], (a[1] + b[1] + c[1]) / 3.0 - centre[1]]);
            r > r_lo && r < r_hi
        }
    };
    let mut bright = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..mesh::Mesh::default() };
    let mut dark = bright.clone();
    let corners: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    for (i, f) in m.faces.iter().enumerate() {
        let target = if wall(f) { &mut dark } else { &mut bright };
        if let Some(c) = corners.get(&(i as u32)) {
            target.corner_normals.push((target.faces.len() as u32, *c));
        }
        target.faces.push(*f);
    }
    (bright, dark)
}
const ANTIQUE: [f32; 3] = [0.16, 0.12, 0.06];

/// Views: yaw about the finger axis, pitch from looking along the finger (0) to down onto the crown (pi/2).
const VIEWS: &[(&str, f64, f64)] = &[("hero", 0.5, 0.55), ("face", 0.0, 0.0), ("side", 0.0, FRAC_PI_2)];

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rev = args.iter().any(|a| a == "--rev");
    let mut free = args.iter().filter(|a| !a.starts_with("--"));
    let option = match free.next().map(String::as_str) {
        Some("flange") => Option_::Flange,
        Some("head") => Option_::Head,
        Some("stones") => Option_::Stones,
        other => bail!("usage: tenebrae_oculus_spike <flange|head|stones> [OUT_DIR] [--rev]; got {other:?}"),
    };
    let out = free
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/oculus-spike").join(option.slug()));
    std::fs::create_dir_all(&out)?;
    let a = match option {
        Option_::Flange => flange(rev)?,
        Option_::Head => head(rev)?,
        Option_::Stones => stones(rev)?,
    };
    let d = a.design.clone();
    println!("Oculus spike: {}{}\n  {}", option.slug(), if rev { " (revision)" } else { "" }, a.summary);
    println!("  lands {}", a.lands);
    if std::env::var("SPIKE_LANDS").is_ok() {
        return Ok(());
    }
    let lib = AlphaLibrary::builtin();
    let params = draft_params();
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let watertight = built.report.validation.watertight;
    let q = built.report.quality;
    println!("  {} triangles in {build_s:.1} s; watertight {}; degenerate {}", built.mesh.faces.len(), watertight, q.degenerate_faces);
    let features: Vec<(Id, String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| {
            let s = match &r.status {
                FeatureStatus::Ok => "Ok".to_string(),
                FeatureStatus::Suppressed => "Suppressed".into(),
                FeatureStatus::Failed(m) => format!("Failed: {m}"),
                FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
            };
            (r.id, r.name.clone(), s)
        })
        .collect();
    for (id, name, s) in &features {
        println!("    #{id} {name}: {s}");
    }
    for n in built.parts.notes.iter().chain(&built.solids.notes) {
        println!("    note: {n}");
    }
    let ring_crossings = csg::self_crossings(&solid_of(&built.mesh));
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let bore = d.inner_radius_mm();
    // Quick draft check, both ways: the sand field verdict on a Delft copy, and the investment's wall thickness.
    let mut sand = d.clone();
    sand.draft.process = CastProcess::SandTwoPart;
    castability::SandProcess::DelftClay.apply(&mut sand.draft);
    let mut field = castability::attributed_field_report(&sand, &lib, &sand.draft, 256, 128);
    castability::judge_parts(&mut field, &sand, &built);
    let coarse = BuildParams { theta_steps: 384, profile_steps: 160, ..params };
    let thin_mesh = mesh::try_build(&d, &lib, coarse)?.mesh;
    let thickness = cad::measure::thickness(&thin_mesh, MIN_SECTION_MM);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let notes: Vec<String> = built.parts.notes.iter().chain(&built.solids.notes).cloned().collect();
    let finished = render::finished_from(&d, &lib, built);
    let previewed: usize = finished.stone_faces();
    let stone_count = stones_report.as_ref().map_or(0, |s| s.stone_count as usize);
    println!(
        "  sand (Delft) field: {} (undercut {:.2}%, worst draft {:.1}, thinnest wall {:.2}); wax thickness(0.8) at {}x{}: {} rays, min {:?}, {} below, {} unresolved; dfm {}; stones {}; min r {:.3} vs bore {:.3}; crossings {}",
        field.verdict.label(),
        field.undercut_fraction() * 100.0,
        field.worst_draft_deg,
        field.thinnest_wall_mm,
        coarse.theta_steps,
        coarse.profile_steps,
        thickness.rays,
        thickness.sampled_min_mm,
        thickness.below_limit,
        thickness.unresolved,
        findings.len(),
        stone_count,
        min_r,
        bore,
        ring_crossings
    );
    if let Some(p) = thickness.point {
        println!("    thinnest at r {:.2}, theta {:.1}, z {:.2} (x {:.2}, y {:.2})", p[0].hypot(p[1]), p[1].atan2(p[0]).to_degrees(), p[2], p[0], p[1]);
    }
    for n in &field.notes {
        println!("    field: {n}");
    }
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    // Renders: studio gold, piercing walls antiqued, stones set.
    let (bright, dark) = match a.antique {
        Some(zone) => antiqued(&finished.metal, zone),
        None => (finished.metal.clone(), mesh::Mesh::default()),
    };
    let mut parts = vec![render::Part::metal(&bright, render::GOLD)];
    if !dark.faces.is_empty() {
        parts.push(render::Part::metal(&dark, ANTIQUE));
    }
    parts.extend(finished.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, *yaw, *pitch, 1000)?;
    }
    for name in ["hero", "face"] {
        let (_, yaw, pitch) = VIEWS.iter().find(|v| v.0 == name).unwrap();
        render::write_png_parts(out.join(format!("{name}-300.png")), &parts, *yaw, *pitch, 300)?;
    }
    let sand_ok = field.verdict == castability::Verdict::Castable;
    let wax_ok = thickness.rays > 0 && thickness.below_limit == 0 && thickness.unresolved == 0;
    let report = json!({
        "option": option.slug(),
        "revision": rev,
        "summary": a.summary,
        "lands": a.lands,
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "triangles": finished.metal.faces.len(),
        "watertight": watertight,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "features": features.iter().map(|(id, n, s)| json!({"id": id, "name": n, "status": s})).collect::<Vec<_>>(),
        "notes": notes,
        "bore_radius_mm": bore,
        "min_vertex_radius_mm": min_r,
        "sand_check": {"process": "Delft clay, 3.0 deg, 0.8 section, 0.30 detail", "field_verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "thinnest_wall_mm": field.thinnest_wall_mm, "notes": field.notes, "castable": sand_ok},
        "wax_check": {"build": [coarse.theta_steps, coarse.profile_steps], "faces": thin_mesh.faces.len(), "limit_mm": thickness.limit_mm, "rays": thickness.rays, "sampled_min_mm": thickness.sampled_min_mm, "below_limit": thickness.below_limit, "unresolved": thickness.unresolved, "note": thickness.note, "clean": wax_ok},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones_reported": stone_count,
        "stone_preview_triangles": previewed,
        "process": if sand_ok { "Delft sand" } else if wax_ok { "lost wax" } else { "neither clean at the draft check" },
    });
    let name = if rev { "spike-rev.json" } else { "spike.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&report)?)?;
    println!("  process: {}", report["process"]);
    Ok(())
}
