//! Officina — Aile, wings clasping a bezel: an oval 7 x 5 in a collet, clasped by two three-feather wings
//! that lift off the band toward their tips. A lesson-sized homage to Logan's Hypnos, cast in lost wax.
//! cargo build --release -p ringdesign-core --example officina_aile
//! target/release/examples/officina_aile [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{
        self, Attach, Component, ComponentRole, Document, Feature, MirrorPlane, Operation, PatternKind, Placement, Stage,
        SurfaceKind, builders, stored,
    },
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    library,
    manufacturing::{self as mf, Setup},
    mesh,
    profile::MAX_PROFILE_STEPS,
    render,
    sketch::Id,
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const SLUG: &str = "aile";
/// Bore diameter, mm: Officina's 18.2.
const BORE_MM: f64 = 18.2;
/// The lost-wax fill floor and detail floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;
/// The collet's wall, mm, and the share of the crown its lip climbs.
const COLLET_WALL_MM: f64 = 0.8;
/// How far the stone stands over the bezel's own height, mm, so its culet clears the crest and no seat bur is cut: on this
/// 1.6 mm crown a full bur leaves 0.54 mm under the culet, and the culet relief it cuts for a raised stone skims the
/// dome to a 0.01 to 0.2 mm skin. The collet's own bearing ledge seats the oval.
const SEAT_RISE_MM: f64 = 0.9;
/// The head joins the band without a fillet bead. A collet wider than a 2.4 mm D band grazes its domed crown,
/// and the fillet could not be laid there at any radius from 0.12 to 0.5 mm (with or without a seat under the
/// collet); its seams are the clasp lapped over the collet and the roots sunk into the crown.
const HEAD_BLEND_MM: f64 = 0.0;
/// The rachis's rise along a feather's top, mm.
const RACHIS_MM: f64 = 0.2;
/// Stations along a feather and points round its section.
/// Kept lean: the stored meshes ride inline in the design and its template, whose budget is 300 KB.
const PRIMARY_RES: Res = Res { pointed_root: false, along: 72, around: 40, root: 5, tip: 14 };
const CLASP_RES: Res = Res { pointed_root: true, along: 44, around: 32, root: 14, tip: 14 };
/// Stations along a feather, points round its section, and rings in its root and tip caps.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Res {
    /// A pointed root, for a part whose root shows; otherwise a rounded one buried in the collet.
    pointed_root: bool,
    along: usize,
    around: usize,
    root: usize,
    tip: usize,
}

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn timeline_params() -> BuildParams {
    BuildParams { theta_steps: 512, profile_steps: 192, ..BuildParams::default() }
}

/// The investment recipe Officina's wax rings share: `workshop_collection.rs`'s flask and channels.
fn setup() -> Setup {
    let mut s = Setup::default();
    s.recipe.alloy = "Gold 18k".into();
    s.recipe.shrink_pct = ringdesign_core::metal::find("Gold 18k").map_or(1.3, |m| m.shrink_pct);
    s.recipe.process = CastProcess::LostWax;
    s.recipe.name = "Investment casting starting recipe / Gold 18k".into();
    s.recipe.sand = None;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.min_detail_mm = MIN_DETAIL_MM;
    s.recipe.min_draft_deg = 0.0;
    s.sample_pitch_mm = 0.10;
    s.recipe.calibration_note =
        "Uncalibrated starting allowance. Confirm with the caster's alloy, pattern material, mold, and measured trials.".into();
    s.flask.width_mm = 70.;
    s.flask.length_mm = 70.;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0., -11., 0.], end: [0., -22., 0.], diameter_mm: 3.2 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0., -22., 0.], end: [0., -30., 0.], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Invest the ring whole: band, collet and both wings. Set the oval and burnish the collet's lip.".into();
    s
}

/// The bare band: DShape 2.4 x 1.8, narrowing to the top by a reverse taper of 0.4, bore 18.2.
fn band() -> RingDesign {
    let mut d = RingDesign { name: "Aile \u{2014} wings clasping a bezel".into(), ..RingDesign::default() };
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::DShape);
    d.profile.width_mm = 2.4;
    d.profile.thickness_mm = 1.8;
    d.profile.comfort_fit_mm = 0.15;
    d.shank.kind = ShankKind::ReverseTaper;
    d.shank.amount = 0.4;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.manufacturing = Some(setup());
    d
}

fn oval() -> Gem {
    Gem { l_mm: 7.0, preview_tint: Some([0.03, 0.09, 0.42]), ..Gem::calibrated(GemCut::Oval, 5.0) }
}

// --- Small vector help ---------------------------------------------------------

fn add(a: P3, b: P3, k: f64) -> P3 {
    std::array::from_fn(|i| a[i] + b[i] * k)
}
fn sub(a: P3, b: P3) -> P3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: P3) -> P3 {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}
fn er(theta: f64) -> P3 {
    let t = theta.to_radians();
    [t.cos(), t.sin(), 0.0]
}
fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The crest radius of the bare band round its top, tabled every quarter degree from 0 to 180.
struct Crest(Vec<f64>);
impl Crest {
    fn of(d: &RingDesign) -> Self {
        let reference = d.reference_loop();
        Self(
            (0..=720)
                .map(|k| {
                    let theta = k as f64 * 0.25;
                    let l = d.section_at(theta, MAX_PROFILE_STEPS, None, Some(&reference));
                    l.pts.iter().filter(|p| p.surface).map(|p| p.r).fold(0.0, f64::max)
                })
                .collect(),
        )
    }
    fn at(&self, theta: f64) -> f64 {
        let x = (theta / 0.25).clamp(0.0, 719.999);
        let (k, f) = (x.floor() as usize, x.fract());
        self.0[k] * (1.0 - f) + self.0[k + 1] * f
    }
}

// --- The feathers --------------------------------------------------------------

/// One primary of the east wing, in the band's own terms: `s` is arc length round the crest from the stone's
/// centre line (east, toward the palm), `z` runs along the finger, `h` stands over the crest.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Primary {
    name: &'static str,
    /// Where the root leaves the collet's flank, along the finger, mm.
    root_z: f64,
    /// The spine's heading at the root, degrees from round-the-ring toward +z, and how far it sweeps back by the tip.
    heading_deg: f64,
    sweep_deg: f64,
    /// Spine length from root to where the tip starts to round, mm.
    length: f64,
    /// Height of the spine over the crest at root and tip, mm.
    h_root: f64,
    h_tip: f64,
    /// Full width across the vane at root and where the tip starts to round, mm.
    w_root: f64,
    w_tip: f64,
    /// Thickness through the vane at root and tip, mm.
    t_root: f64,
    t_tip: f64,
    /// How far the vane arches across its width, mm.
    camber: f64,
    /// The vane's roll about the spine, degrees; positive lifts the leading (+z) edge.
    roll_deg: f64,
}

/// The clasp: a crescent of coverts that hooks round the collet's flank, lying low over the band at its foot and
/// climbing the collet toward its tip, so the wing holds the stone rather than butting into it.
const CLASP: Primary = Primary { name: "Clasp", root_z: -2.3, heading_deg: 0.0, sweep_deg: 0.0, length: 0.0, h_root: 0.4, h_tip: 1.35, w_root: 2.0, w_tip: 1.5, t_root: 1.3, t_tip: 1.12, camber: 0.3, roll_deg: 0.0 };
/// Where the clasp's spine runs, (s, z): from its foot under the wing, out round the collet's flank, to its tip high on the collet's far side.
fn clasp_spine() -> [[f64; 2]; 3] {
    let (z0, z2) = (-2.3, 3.3);
    let p0 = [flank_s(z0) + 0.45, z0];
    let p2 = [flank_s(z2) + 0.25, z2];
    let mid_s = flank_s(0.5) + 0.6;
    [p0, [2.0 * mid_s - 0.5 * (p0[0] + p2[0]), 0.5], p2]
}

const PRIMARIES: [Primary; 3] = [
    Primary { name: "Primary I", root_z: 1.6, heading_deg: 38.0, sweep_deg: 12.0, length: 7.8, h_root: 0.45, h_tip: 1.6, w_root: 3.4, w_tip: 2.3, t_root: 1.05, t_tip: 1.06, camber: 0.35, roll_deg: 12.0 },
    Primary { name: "Primary II", root_z: 0.3, heading_deg: 18.0, sweep_deg: 8.0, length: 6.8, h_root: 0.4, h_tip: 1.35, w_root: 3.4, w_tip: 2.2, t_root: 1.05, t_tip: 1.06, camber: 0.33, roll_deg: 12.0 },
    Primary { name: "Primary III", root_z: -0.9, heading_deg: 0.0, sweep_deg: 4.0, length: 5.6, h_root: 0.35, h_tip: 1.1, w_root: 3.0, w_tip: 2.0, t_root: 1.05, t_tip: 1.06, camber: 0.3, roll_deg: 12.0 },
];

/// A primary's spine, (s, z): from its root in the collet's flank, out along its heading, sweeping back by the tip.
fn primary_spine(p: &Primary) -> [[f64; 2]; 3] {
    let root_len = 0.25 * p.w_root;
    let p0 = [flank_s(p.root_z) - 0.6 + root_len, p.root_z];
    let (h0, h1) = (p.heading_deg.to_radians(), (p.heading_deg - p.sweep_deg).to_radians());
    let p1 = [p0[0] + 0.5 * p.length * h0.cos(), p0[1] + 0.5 * p.length * h0.sin()];
    let p2 = [p1[0] + 0.5 * p.length * h1.cos(), p1[1] + 0.5 * p.length * h1.sin()];
    [p0, p1, p2]
}

/// The collet's outer flank round the ring at `z` along the finger, mm from the stone's centre line.
fn flank_s(z: f64) -> f64 {
    let (a, b) = (2.5 + COLLET_WALL_MM, 3.5 + COLLET_WALL_MM);
    a * (1.0 - (z / b).powi(2)).max(0.0).sqrt()
}

fn bezier(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], u: f64) -> [f64; 2] {
    let w = [(1.0 - u) * (1.0 - u), 2.0 * u * (1.0 - u), u * u];
    [w[0] * p0[0] + w[1] * p1[0] + w[2] * p2[0], w[0] * p0[1] + w[1] * p1[1] + w[2] * p2[1]]
}

/// The radius every feather's point ends in, mm: the vane narrows to it and closes in a round, so no wall
/// at a point falls under the 0.8 mm fill floor.
const END_MM: f64 = 0.5;

/// An ogive end `d` mm into a cap `len` long from a section of half-width `a` and half-thickness `b`: the plan
/// narrows to `END_MM` and closes in a round, the thickness held to the round. Returns half-width,
/// half-thickness and the share of camber and rachis kept.
fn ogive(d: f64, len: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let narrow = len - END_MM;
    if d <= narrow {
        let q = d / narrow;
        let f = (q * PI / 2.0).cos().max(0.0).powf(0.9);
        let k = q.powi(3);
        (END_MM + (a - END_MM) * f, b + (END_MM.min(b) - b) * k, f)
    } else {
        let w = ((d - narrow) / END_MM).min(1.0);
        let g = (1.0 - w * w).max(0.0).sqrt();
        (END_MM * g, END_MM.min(b) * g, 0.0)
    }
}

/// The barb lines of a vane: how many rows of the mesh make one groove, their pitch along the rachis, depth,
/// and how far a barb runs toward the tip for each mm it runs out from the rachis on the leading and trailing vanes.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Barbs {
    rows: usize,
    pitch_mm: f64,
    depth_mm: f64,
    lead_slope: f64,
    trail_slope: f64,
}
/// The clasp's coverts are short and broad: their barbs stand steeper and closer.
const CLASP_BARBS: Barbs = Barbs { rows: 5, pitch_mm: 0.42, depth_mm: 0.07, lead_slope: 1.1, trail_slope: 1.1 };
const BARBS: Barbs = Barbs { rows: 5, pitch_mm: 0.46, depth_mm: 0.085, lead_slope: 1.6, trail_slope: 1.25 };

/// A feather as a closed solid: a cambered vane with a rounded rim and a raised rachis, tapering from a buried
/// rounded root to an ogive tip, its spine following the band round and lifting off it toward the tip. Barbs
/// are cut as true V-grooves; each ring of the mesh is bent into a chevron along the barbs on the vane's top,
/// so a groove is a row of the mesh and stays crisp with few triangles.
fn feather(p: &Primary, ctrl: [[f64; 2]; 3], barbs: Option<Barbs>, res: Res, crest: &Crest, top_r: f64) -> csg::Solid {
    let world = |s: f64, z: f64, h: f64| -> P3 {
        let theta = 90.0 - (s / top_r).to_degrees();
        let r = crest.at(theta) + h;
        let e = er(theta);
        // The wings rise toward -z, which the face camera shows as up.
        [e[0] * r, e[1] * r, -z]
    };
    let [p0, p1, p2] = ctrl;
    // The spine tabled finely by arc length: length so far, point, tangent, up, across.
    const TABLE: usize = 2000;
    let point = |u: f64| -> (P3, P3) {
        let [s, z] = bezier(p0, p1, p2, u);
        let h = p.h_root + (p.h_tip - p.h_root) * smooth(0.05, 0.7, u);
        (world(s, z, h), er(90.0 - (s / top_r).to_degrees()))
    };
    let mut table: Vec<(f64, P3, P3, P3, P3)> = Vec::with_capacity(TABLE + 1);
    let mut length = 0.0;
    for k in 0..=TABLE {
        let u = k as f64 / TABLE as f64;
        let (at, radial) = point(u);
        let (a, _) = point((u - 1e-4).max(0.0));
        let (b, _) = point((u + 1e-4).min(1.0));
        let tangent = unit(sub(b, a));
        let across = unit(cross(tangent, radial));
        let up = cross(across, tangent);
        if let Some(last) = table.last() {
            length += dot(sub(at, last.1), sub(at, last.1)).sqrt();
        }
        table.push((length, at, tangent, up, across));
    }
    // The frame at arc length `l`, carried straight on past either end for the caps.
    let frame = |l: f64| -> (P3, P3, P3) {
        if l <= 0.0 {
            let t = table[0];
            return (add(t.1, t.2, l), t.3, t.4);
        }
        if l >= length {
            let t = table[TABLE];
            return (add(t.1, t.2, l - length), t.3, t.4);
        }
        let k = table.partition_point(|e| e.0 < l).clamp(1, TABLE);
        let (e0, e1) = (table[k - 1], table[k]);
        let f = ((l - e0.0) / (e1.0 - e0.0).max(1e-12)).clamp(0.0, 1.0);
        let lerp = |a: P3, b: P3| -> P3 { std::array::from_fn(|i| a[i] + (b[i] - a[i]) * f) };
        (lerp(e0.1, e1.1), unit(lerp(e0.3, e1.3)), unit(lerp(e0.4, e1.4)))
    };
    let root_len = if res.pointed_root { 1.3 * p.w_root + END_MM } else { 0.25 * p.w_root };
    let tip_len = 1.3 * p.w_tip + END_MM;
    // The section at arc length `l`: half-width, half-thickness, camber, rachis.
    let section = |l: f64| -> (f64, f64, f64, f64) {
        let (a0, b0) = (0.5 * p.w_root, 0.5 * p.t_root);
        let (a1, b1) = (0.5 * p.w_tip, 0.5 * p.t_tip);
        if l < 0.0 && res.pointed_root {
            let (a, b, c) = ogive(-l, root_len, a0, b0);
            return (a, b, p.camber * c, 0.0);
        }
        if l < 0.0 {
            let q = (-l / root_len).min(1.0);
            let f = (1.0 - q * q).max(0.0).sqrt();
            return (a0 * f, b0 * f.sqrt(), p.camber * f, 0.0);
        }
        if l > length {
            let (a, b, c) = ogive(l - length, tip_len, a1, b1);
            return (a, b, p.camber * c, RACHIS_MM * c);
        }
        let u = l / length;
        let taper = u.powf(1.2);
        (a0 + (a1 - a0) * taper, b0 + (b1 - b0) * taper, p.camber * (1.0 - 0.45 * u), RACHIS_MM * smooth(0.0, 0.15, u))
    };
    // Stations: the root cap, the body in groove rows, the tip cap.
    let mut stations: Vec<(f64, Option<usize>)> = Vec::new();
    // Cap rings close up toward the end, so the closing round is drawn smooth.
    let ease = |q: f64| 1.0 - (1.0 - q) * (1.0 - q);
    for k in (1..res.root).rev() {
        stations.push((-root_len * ease(k as f64 / res.root as f64), None));
    }
    let body = match barbs {
        Some(b) => (length / b.pitch_mm).round().max(1.0) as usize * b.rows,
        None => res.along,
    };
    for k in 0..=body {
        stations.push((length * k as f64 / body as f64, Some(k)));
    }
    for k in 1..res.tip {
        stations.push((length + tip_len * ease(k as f64 / res.tip as f64), None));
    }
    let roll = p.roll_deg.to_radians();
    // How far the barbs lean toward the tip, faded in from the root and out toward the tip so no rows cross.
    let lean = |l: f64| smooth(0.0, 0.8, l) * (1.0 - smooth(length - 4.5, length, l));
    let mut v: Vec<P3> = vec![frame(-root_len).0];
    for &(l, row) in &stations {
        for j in 0..res.around {
            let phi = 2.0 * PI * j as f64 / res.around as f64;
            let (c, sn) = (phi.cos(), phi.sin());
            let top = smooth(0.0, 0.55, sn);
            // Where this point sits along the spine: bent toward the tip along the barb on the vane's top.
            let (a_nom, ..) = section(l);
            let out = a_nom * c.abs().sqrt();
            let slope = match barbs {
                Some(b) if c >= 0.0 => b.lead_slope,
                Some(b) => b.trail_slope,
                None => 0.0,
            };
            let bend = (slope * out - 0.12 * out * out).max(0.0);
            let lp = if row.is_some() { l + top * lean(l) * bend } else { l };
            let (a, b, camber, rachis) = section(lp);
            // The leading vane narrower than the trailing one, as on a flight feather.
            let x = a * c.signum() * c.abs().powf(0.5) - 0.12 * a;
            let mut y = b * sn.signum() * sn.abs().powf(0.5);
            let xn = ((x + 0.12 * a) / a.max(1e-9)).clamp(-1.0, 1.0);
            let topw = smooth(-0.6, 0.6, sn);
            y += camber * (1.0 - xn * xn) * (topw - 0.3 * (1.0 - topw));
            y += rachis * (-(x / 0.3).powi(2)).exp() * smooth(-0.3, 0.6, sn);
            if let (Some(bb), Some(k)) = (barbs, row) {
                // A V across the groove's rows, steep on the side toward the root, as a graver leaves it.
                let profile = match k % bb.rows {
                    0 => 1.0,
                    1 => 0.3,
                    r if r == bb.rows - 1 => 0.55,
                    _ => 0.0,
                };
                let keep = smooth(0.3, 0.48, x.abs()) * (1.0 - smooth(0.72, 0.9, xn.abs())) * lean(l).min(1.0);
                y -= bb.depth_mm * profile * keep * smooth(0.3, 0.8, sn);
            }
            let (xr, yr) = (x * roll.cos() - y * roll.sin(), x * roll.sin() + y * roll.cos());
            let (at, up, across) = frame(lp);
            v.push(add(add(at, across, xr), up, yr));
        }
    }
    v.push(frame(length + tip_len).0);
    let n = stations.len();
    let around = res.around;
    let ring = |k: usize, j: usize| (1 + k * around + j % around) as u32;
    let mut f: Vec<[u32; 3]> = Vec::new();
    for j in 0..around {
        f.push([0, ring(0, j + 1), ring(0, j)]);
    }
    for k in 0..n - 1 {
        for j in 0..around {
            f.push([ring(k, j), ring(k, j + 1), ring(k + 1, j + 1)]);
            f.push([ring(k, j), ring(k + 1, j + 1), ring(k + 1, j)]);
        }
    }
    let last = (v.len() - 1) as u32;
    for j in 0..around {
        f.push([ring(n - 1, j), ring(n - 1, j + 1), last]);
    }
    let mut solid = csg::Solid { v, f };
    if signed_volume(&solid) < 0.0 {
        solid.f.iter_mut().for_each(|t| t.swap(1, 2));
    }
    solid
}

fn signed_volume(s: &csg::Solid) -> f64 {
    s.f.iter()
        .map(|t| {
            let (a, b, c) = (s.v[t[0] as usize], s.v[t[1] as usize], s.v[t[2] as usize]);
            dot(a, cross(b, c)) / 6.0
        })
        .sum()
}

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "officina_aile".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}

fn joined() -> Component {
    Component {
        attach: Attach::Join,
        stage: Stage::Cast,
        placement: Placement::Free,
        blend_mm: HEAD_BLEND_MM,
        material: "Gold 18k".into(),
        ..Component::default()
    }
}

/// The feature history: band, stone, collet, seat, the east wing's three primaries, and their mirror.
fn author() -> Result<(RingDesign, AlphaLibrary, serde_json::Value)> {
    let mut d = band();
    let lib = AlphaLibrary::builtin();
    let gem = oval();
    let stand = builders::stand_off_mm(builders::BEZEL, gem);
    let crest = Crest::of(&d);
    let top_r = crest.at(90.0);
    let mut doc = Document::default();
    doc.append(feature(1, "Band", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() }))?;
    let mut stone = builders::stone_feature(2, gem, Placement::Ring { theta_deg: 90.0, across_mm: 0.0, height_mm: stand + SEAT_RISE_MM, spin_deg: 90.0, tilt_deg: 0.0, cant_deg: 0.0 });
    stone.name = "Oval 7 x 5".into();
    doc.append(stone)?;
    doc.append(builders::feature_on(3, "Collet", builders::BEZEL, 2, json!({"wall_mm": COLLET_WALL_MM})))?;
    let mut feathers = Vec::new();
    let loose = || Component { material: "Gold 18k".into(), ..Component::default() };
    let parts: Vec<(&Primary, [[f64; 2]; 3], Option<Barbs>, Res)> = std::iter::once((&CLASP, clasp_spine(), Some(CLASP_BARBS), CLASP_RES))
        .chain(PRIMARIES.iter().map(|p| (p, primary_spine(p), Some(BARBS), PRIMARY_RES)))
        .collect();
    for (k, (p, ctrl, barbs, res)) in parts.into_iter().enumerate() {
        let solid = feather(p, ctrl, barbs, res, &crest, top_r);
        let check = solid.check(true);
        ensure!(check.self_crossings == Some(0) && check.open_edges == 0, "{} is not a clean solid: {check:?}", p.name);
        let params = json!({"spec": p, "spine_s_z": ctrl, "barbs": barbs, "resolution": res});
        feathers.push(json!({"name": p.name, "params": params, "triangles": solid.f.len(), "volume_mm3": signed_volume(&solid)}));
        doc.append(feature(5 + k as Id, p.name, stored_op(&solid, "feather", params)?, loose()))?;
    }
    doc.append(feature(9, "Wing: clasp and I", Operation::Boolean { a: 5, b: 6, kind: cad::Boolean::Union }, loose()))?;
    doc.append(feature(10, "Wing: and II", Operation::Boolean { a: 9, b: 7, kind: cad::Boolean::Union }, loose()))?;
    doc.append(feature(11, "East wing", Operation::Boolean { a: 10, b: 8, kind: cad::Boolean::Union }, loose()))?;
    doc.append(feature(
        12,
        "West wing (mirror)",
        Operation::Pattern { sources: cad::pattern::Sources(vec![11]), kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } },
        loose(),
    ))?;
    // The collet and both wings become one head, which joins the band.
    doc.append(feature(13, "Head: collet and east wing", Operation::Boolean { a: 3, b: 11, kind: cad::Boolean::Union }, loose()))?;
    doc.append(feature(14, "Head", Operation::Boolean { a: 13, b: 12, kind: cad::Boolean::Union }, joined()))?;
    d.cad = Some(doc);
    let comp = json!({"stand_off_mm": stand, "crest_top_r_mm": top_r, "feathers": feathers});
    Ok((d, lib, comp))
}

// --- Checks --------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}
fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}
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

/// The sampler `cad::measure::thickness` runs, every sample kept: each face centre's depth along its inward normal.
fn census(m: &mesh::Mesh, samples: usize) -> Vec<(P3, P3, f64)> {
    let tri: Vec<[P3; 3]> = m
        .faces
        .iter()
        .map(|f| f.map(|i| { let p = m.vertices[i as usize]; [p.0 as f64, p.1 as f64, p.2 as f64] }))
        .collect();
    let stride = tri.len().div_ceil(samples).max(1);
    let hit = |o: P3, d: P3, t: &[P3; 3]| -> Option<f64> {
        let (e1, e2) = (sub(t[1], t[0]), sub(t[2], t[0]));
        let p = cross(d, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-12 { return None; }
        let s = sub(o, t[0]);
        let u = dot(s, p) / det;
        if !(-1e-8..=1.0 + 1e-8).contains(&u) { return None; }
        let q = cross(s, e1);
        let v = dot(d, q) / det;
        if v < -1e-8 || u + v > 1.0 + 1e-8 { return None; }
        let t = dot(e2, q) / det;
        (t > 1e-5).then_some(t)
    };
    tri.iter()
        .enumerate()
        .step_by(stride)
        .filter_map(|(k, t)| {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            if dot(n, n).sqrt() < 1e-12 { return None; }
            let inward = unit(n).map(|v| -v);
            let c: P3 = std::array::from_fn(|i| (t[0][i] + t[1][i] + t[2][i]) / 3.0);
            let depth = tri.iter().enumerate().filter(|(j, _)| *j != k).filter_map(|(_, u)| hit(c, inward, u)).fold(f64::MAX, f64::min);
            Some((c, unit(n), depth))
        })
        .collect()
}

// --- Pictures ------------------------------------------------------------------

/// The timeline looks from the east, where the wing is built before the mirror.
const TIMELINE_YAW: f64 = 0.6;
/// The camera for each named view: yaw about the head's axis, pitch toward the finger's.
const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", -0.55, 0.85),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.6),
    ("reverse", PI - 0.5, 0.35),
];

/// A label in EB Garamond, dark on the light strip of a sheet.
fn label(img: &mut [u8], w: usize, x0: usize, y0: usize, text: &str, px: f32) {
    static FONT: std::sync::OnceLock<fontdue::Font> = std::sync::OnceLock::new();
    let font = FONT.get_or_init(|| {
        fontdue::Font::from_bytes(include_bytes!("../../../assets/fonts/EBGaramond.ttf").as_slice(), fontdue::FontSettings::default())
            .expect("bundled font parses")
    });
    let h = img.len() / (3 * w);
    let mut pen = x0 as f32;
    for ch in text.chars() {
        let (m, cov) = font.rasterize(ch, px);
        let top = y0 as i64 + (px * 0.8) as i64 - (m.ymin as i64 + m.height as i64);
        for gy in 0..m.height {
            for gx in 0..m.width {
                let (x, y) = (pen as i64 + m.xmin as i64 + gx as i64, top + gy as i64);
                if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                    continue;
                }
                let a = cov[gy * m.width + gx] as f32 / 255.0;
                let i = (y as usize * w + x as usize) * 3;
                for c in 0..3 {
                    img[i + c] = (img[i + c] as f32 * (1.0 - a) + 30.0 * a) as u8;
                }
            }
        }
        pen += m.advance_width;
    }
}

/// Labelled cells on one sheet, `cols` across, each `edge` square with a strip under it.
fn sheet(path: &Path, cells: &[(String, Vec<u8>)], edge: usize, cols: usize) -> Result<()> {
    let strip = 34;
    let rows = cells.len().div_ceil(cols);
    let (w, h) = (cols * edge, rows * (edge + strip));
    let mut img = vec![236u8; w * h * 3];
    for (k, (name, px)) in cells.iter().enumerate() {
        let (cx, cy) = ((k % cols) * edge, (k / cols) * (edge + strip));
        for y in 0..edge {
            let row = &px[y * edge * 3..(y + 1) * edge * 3];
            let at = ((cy + y) * w + cx) * 3;
            img[at..at + edge * 3].copy_from_slice(row);
        }
        label(&mut img, w, cx + 8, cy + edge + 4, name, 22.0);
    }
    image::save_buffer(path, &img, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, lib: &AlphaLibrary, fin: &render::Finished, d: &RingDesign, edge: usize) -> Result<()> {
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    render::write_png_parts(out.join("stones.png"), &parts, -0.3, 1.1, edge)?;
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&bare_img[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&finished_img[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(out.join("bare-vs-finished.png"), &both, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, 0.0, PI * 0.5, 300)?;
    let cells: Vec<(String, Vec<u8>)> =
        VIEWS.iter().map(|(name, yaw, pitch)| (name.to_string(), render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3))).collect();
    sheet(&out.join("contact-300.png"), &cells, 300, 3)?;
    // The timeline: the ring after each feature, as the rollback marker builds it.
    let doc = d.cad.as_ref().expect("a CAD document");
    let mut steps = Vec::new();
    for (k, f) in doc.features.iter().enumerate() {
        let mut at = d.clone();
        at.cad.as_mut().unwrap().through = Some(f.id);
        let fin = render::finished(&at, lib, timeline_params())?;
        steps.push((format!("{}. {}", k + 1, f.name), render::render_parts_ss(&fin.parts(render::GOLD), TIMELINE_YAW, pitch, 300, 300, 3)));
    }
    sheet(&out.join("timeline.png"), &steps, 300, 4)?;
    Ok(())
}

// --- The run -------------------------------------------------------------------

/// Every gate number of one build, at `params`.
fn gates_at(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(mesh::BuildResult, serde_json::Value, bool)> {
    let t = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let made = made_parts(&built);
    let e = built.parts.evaluated.as_ref();
    let statuses: Vec<_> = e.map(|e| e.features.iter().map(|r| json!({"id": r.id, "name": r.name, "status": format!("{:?}", r.status)})).collect()).unwrap_or_default();
    let all_ok = e.is_some_and(|e| e.features.iter().all(|r| r.status.is_ok()));
    let (least_r, inside) = bore_intrusion(d, &built.mesh);
    let field = castability::judged_field_report(d, lib, &d.draft, 256, 128, Some(&built));
    // Every sample under the floor must be the collet's burnished lip, the bezel pushed over the crown at the bench,
    // which stands above the girdle.
    let girdle_r = Crest::of(d).at(90.0) + builders::stand_off_mm(builders::BEZEL, oval()) + SEAT_RISE_MM;
    let on_lip = |p: P3| p[0].hypot(p[1]) >= girdle_r - 0.2 && (p[1].atan2(p[0]).to_degrees() - 90.0).abs() < 20.0;
    let walls: Vec<_> = e
        .map(|e| {
            e.components
                .iter()
                .filter(|c| c.attach == Attach::Join && c.settings.role != ComponentRole::Stone && !c.name.starts_with("Collet"))
                .map(|c| {
                    let w = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
                    let thin: Vec<_> = census(&c.mesh, 384).into_iter().filter(|t| t.2 < MIN_SECTION_MM).map(|t| json!({"mm": t.2, "theta_deg": t.0[1].atan2(t.0[0]).to_degrees(), "r_mm": t.0[0].hypot(t.0[1]), "z_mm": t.0[2], "lip": on_lip(t.0)})).collect();
                    let off_lip = thin.iter().filter(|t| t["lip"] == false).count();
                    json!({"part": c.name, "census_below_floor": thin, "census_below_floor_off_lip": off_lip, "sampled_min_mm": w.sampled_min_mm, "rays": w.rays, "unresolved": w.unresolved, "below_limit": w.below_limit, "note": w.note})
                })
                .collect()
        })
        .unwrap_or_default();
    // The whole ring as cast, at a build the sampler takes (it declines over 250 000 triangles).
    let coarse = mesh::try_build(d, lib, BuildParams { theta_steps: 320, profile_steps: 112, ..BuildParams::default() })?;
    let ring_wall = cad::measure::thickness(&coarse.mesh, MIN_SECTION_MM);
    let samples = census(&coarse.mesh, 384);
    let thin: Vec<_> = samples.iter().filter(|c| c.2 < MIN_SECTION_MM).collect();
    let in_lip = |c: &&(P3, P3, f64)| on_lip(c.0);
    let thin_off_lip = thin.iter().filter(|c| !in_lip(c)).count();
    let ring_wall = json!({
        "part": "whole ring at 320 x 112", "triangles": coarse.mesh.faces.len(),
        "sampled_min_mm": ring_wall.sampled_min_mm, "point": ring_wall.point, "rays": ring_wall.rays, "unresolved": ring_wall.unresolved, "below_limit": ring_wall.below_limit, "note": ring_wall.note,
        "census": {"samples": samples.len(), "below_floor": thin.len(), "below_floor_on_the_burnished_lip": thin.len() - thin_off_lip, "below_floor_elsewhere": thin_off_lip, "girdle_r_mm": girdle_r,
            "below_floor_points": thin.iter().map(|c| json!({"mm": c.2, "theta_deg": c.0[1].atan2(c.0[0]).to_degrees(), "r_mm": c.0[0].hypot(c.0[1]), "z_mm": c.0[2], "lip": in_lip(c)})).collect::<Vec<_>>()},
        "exception": "the collet's lip: a bezel thinned to its edge so it can be burnished over the crown; every other sample clears 0.8 mm",
    });
    let ring_ok = ring_wall["unresolved"] == 0 && thin_off_lip == 0;
    let walls_ok = ring_ok && walls.iter().all(|w| w["census_below_floor_off_lip"] == 0 && w["unresolved"] == 0);
    let mut walls = walls;
    walls.push(ring_wall);
    let lands = dfm::cut_lands(d, &built, MIN_SECTION_MM);
    let findings = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let gems = ringdesign_core::gems::built_meshes(d, lib, &built);
    let previewed: usize = gems.iter().map(|(m, _)| shell_count_loose(m)).sum();
    // The culet over the band: the stone's lowest point against the crest under it, mm.
    let crest = Crest::of(d);
    let culet_clear = gems
        .iter()
        .flat_map(|(m, _)| m.vertices.iter())
        .map(|v| {
            let (x, y) = (v.0 as f64, v.1 as f64);
            x.hypot(y) - crest.at(y.atan2(x).to_degrees().clamp(0.0, 180.0))
        })
        .fold(f64::MAX, f64::min);
    let warnings: Vec<String> =
        stones.iter().flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter().map(|w| format!("{}: {w}", s.label)))).collect();
    let crowding: Vec<String> =
        stones.iter().flat_map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm))).collect();
    let pattern = mesh::try_build_pattern(d, lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("0 self-crossings on every made part", made.iter().all(|(_, n)| *n == 0)),
        ("solids and parts notes empty", built.solids.notes.is_empty() && built.parts.notes.is_empty()),
        ("every CAD feature Ok", all_ok),
        ("nothing enters the finger hole", inside == 0),
        ("lost wax: thickness clean at 0.8 mm on the head and the whole ring, but for the burnished lip", walls_ok),
        ("lost wax: dfm::cut_lands clean at 0.8 mm", lands.is_empty()),
        ("lost wax: field verdict Castable", field.process == CastProcess::LostWax && field.verdict == castability::Verdict::Castable),
        ("0 DFM findings", findings.is_empty()),
        ("stone record equals the gem preview, no warnings, crowding clean", reported == previewed && reported == 1 && warnings.is_empty() && crowding.is_empty()),
        ("the culet stands clear of the band", culet_clear > 0.05),
        ("casting pattern watertight, 0 degenerate faces, 0 crossings", pw && pd == 0 && px == 0),
        ("within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
    ];
    let pass = gates.iter().all(|(_, p)| *p);
    let report = json!({
        "params": [params.theta_steps, params.profile_steps],
        "triangles": built.mesh.faces.len(),
        "build_s": build_s,
        "watertight": watertight,
        "boundary_edges": built.report.validation.boundary_edges,
        "non_manifold_edges": built.report.validation.non_manifold_edges,
        "degenerate_faces": degenerate,
        "mesh_self_crossings": crossings,
        "shells": shells(&built.mesh),
        "made_parts": made,
        "feature_status": statuses,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "parts_joined": built.parts.joined,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"process": field.process.label(), "verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "notes": field.notes, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "thickness": walls,
        "cut_lands": lands.iter().map(|f| f.message.clone()).collect::<Vec<_>>(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"culet_over_crest_mm": culet_clear, "reported": reported, "previewed": previewed, "carats": stones.as_ref().map_or(0.0, |s| s.total_carats), "warnings": warnings, "crowding": crowding},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len(), "shells": shells(&pattern.mesh)},
        "volume_mm3": built.report.volume_mm3,
        "grams_18k": built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams),
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": pass,
    });
    for (g, p) in &gates {
        println!("  {} {g}", if *p { "pass" } else { "FAIL" });
    }
    if !pass {
        println!("  {}", serde_json::to_string(&json!({"made": report["made_parts"], "status": report["feature_status"], "thickness": report["thickness"], "field": report["field"], "dfm": report["dfm_findings"], "stones": report["stones"], "notes": [report["solids_notes"], report["parts_notes"]]}))?);
    }
    Ok((built, report, pass))
}

/// Stones in a loose-triangle preview mesh: welded, then counted by shell.
fn shell_count_loose(m: &mesh::Mesh) -> usize {
    let mut out = mesh::Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in &m.faces {
        let face = f.map(|i| {
            let p = m.vertices[i as usize];
            let key = [p.0, p.1, p.2].map(|x| (x * 1e5).round() as i64);
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(p);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(face);
    }
    shells(&out)
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/officina").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("Aile");
    let started = std::time::Instant::now();
    let (mut d, lib, comp) = author()?;
    if let Ok(spec) = std::env::var("AILE_LOOK") {
        // theta,z,radius,yaw,pitch: a close-up of the draft build round one point.
        let v: Vec<f64> = spec.split(',').map(|x| x.parse().unwrap()).collect();
        let built = mesh::try_build(&d, &lib, draft_params())?;
        let c = add([0.0, 0.0, v[1]], er(v[0]), 10.8);
        let fin = render::finished_from(&d, &lib, built);
        render::write_png_framed(out.join("look.png"), &fin.parts(render::GOLD), render::yaw_facing(v[0]) + v[3], v[4], render::Framing::new(c, v[2]), 1000)?;
        return Ok(());
    }
    if let Ok(t) = std::env::var("AILE_PROBE") {
        d.cad.as_mut().unwrap().through = t.parse().ok();
        let coarse = mesh::try_build(&d, &lib, BuildParams { theta_steps: 320, profile_steps: 112, ..BuildParams::default() })?;
        let w = cad::measure::thickness(&coarse.mesh, MIN_SECTION_MM);
        println!("probe through {t}: {w:?}; notes {:?} {:?}", coarse.solids.notes, coarse.parts.notes);
        for (at, n, t) in census(&coarse.mesh, 1536).iter().filter(|c| c.2 < MIN_SECTION_MM) {
            let r = at[0].hypot(at[1]);
            println!("  thin {t:.3} at theta {:.1} r {r:.3} z {:.3} normal {:?}", at[1].atan2(at[0]).to_degrees(), at[2], n.map(|v| (v * 100.0).round() / 100.0));
        }
        return Ok(());
    }
    let author_s = started.elapsed().as_secs_f64();
    println!("  draft build");
    let (draft_built, draft_report, draft_pass) = gates_at(&d, &lib, draft_params())?;
    let (built, export_report, export_pass) = if draft {
        (draft_built, draft_report.clone(), draft_pass)
    } else {
        println!("  export build");
        drop(draft_built);
        gates_at(&d, &lib, export_params())?
    };
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let params = if draft { draft_params() } else { export_params() };
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let fin = render::finished_from(&d, &lib, built);
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &fin.metal, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, export_params())?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Aile / casting pattern")?;
        let mut entries = Vec::new();
        for (m, tint) in &fin.stones {
            stl::write_stl(out.join("reference-sapphire.stl"), m, "Aile reference sapphire")?;
            entries.push(json!({"mesh": "reference-sapphire.stl", "name": "Blue sapphire, oval 7 x 5", "tint": tint, "ior": 1.77, "dispersion": 0.018, "roughness": 0.065, "transmission": 0.72}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": entries}))?)?;
    }
    renders(&out, &lib, &fin, &d, if draft { 1000 } else { 1600 })?;
    let doc = d.cad.as_ref().unwrap();
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "alloy_for_weight": "Gold 18k",
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "band": {"style": "DShape", "width_mm": d.profile.width_mm, "thickness_mm": d.profile.thickness_mm, "shank": "ReverseTaper", "amount": d.shank.amount},
        "stone": {"cut": "Oval", "l_mm": 7.0, "w_mm": 5.0, "setting": "head.bezel collet", "wall_mm": COLLET_WALL_MM},
        "features": doc.features.iter().map(|f| json!({"id": f.id, "name": f.name, "op": f.operation.label()})).collect::<Vec<_>>(),
        "composition": comp,
        "author_s": author_s,
        "export": if draft { serde_json::Value::Null } else { export_report.clone() },
        "draft": draft_report,
        "cold_reload_identical": cold,
        "design": {"bytes": text.len(), "cad_features": doc.features.len()},
        "gates_passed": draft_pass && export_pass && cold != Some(false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("  done in {:.1} s; gates {}", started.elapsed().as_secs_f64(), if report["gates_passed"] == true { "green" } else { "RED" });
    ensure!(report["gates_passed"] == true, "Aile failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
