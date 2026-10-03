//! Tenebrae — Gurgulio, the waterspout: a Gothic gargoyle crouched on a cathedral parapet, cast in lost wax.
//! cargo build --release -p ringdesign-core --example tenebrae_gurgulio
//! target/release/examples/tenebrae_gurgulio [OUT_DIR] [--draft] [--verify]
//!
//! The head is one sculpted part: a moulded parapet block (a chamfered wall with blind quatrefoils, a roll-moulded
//! cornice and a chamfered coping) and the grotesque crouched on it, hunched, its fore claws hooked over the coping's
//! front edge, its bat wings folded along the back, a water channel running down its spine, and its horned head thrust
//! out past the edge along the finger with the jaws gaping as the spout. The sculpt is a distance field meshed by
//! `sculpt`, stored, and grown out of a keyed flat band with `fillet_into_band`.
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{self, Attach, Component, Document, Feature, Operation, Placement, stored},
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smax, smin},
    stl,
};
use serde_json::json;
use std::f64::consts::{FRAC_1_SQRT_2, PI};
use std::path::{Path, PathBuf};

type P3 = [f64; 3];

const NAME: &str = "Gurgulio \u{2014} the waterspout";
const SLUG: &str = "gurgulio";
/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The investment's fill floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
/// Nothing of the sculpt comes nearer the finger axis than this, mm (the bore is 9.3).
const CLEAR_R_MM: f64 = 10.0;

/// The band at the palm: width and thickness, mm.
const BAND_W: f64 = 4.6;
const BAND_T: f64 = 1.9;

// --- The parapet (world millimetres: x round the ring at the top, y out from the finger, z along it) ---------------
// A wall segment across the crown with the beast projecting from it along the finger (+z, toward the fingertip): a
// corbel tapering out of the band, the wall with its blind arcade, and a cornice of two steps, each overhanging the one
// below. Each tier is a half-width in x, a z run and a y run.
/// The corbel: its half-widths at its foot (sunk in the band) and where it meets the wall, its z run at the wall.
const CORBEL_Y: [f64; 2] = [9.6, 11.9];
const CORBEL_HX: [f64; 2] = [3.3, 4.4];
const CORBEL_Z: [f64; 2] = [-4.6, 3.6];
/// How much narrower the corbel's z run is at its foot, each end, mm.
const CORBEL_TAPER: f64 = 0.9;
/// The wall.
const WALL_HX: f64 = 4.4;
const WALL_Z: [f64; 2] = [-4.6, 3.6];
const WALL_Y: [f64; 2] = [11.9, 14.3];
/// The cornice's lower step, a roll moulding.
const CORNICE_HX: f64 = 5.25;
const CORNICE_Z: [f64; 2] = [-5.45, 4.45];
const CORNICE_Y: [f64; 2] = [14.3, 15.15];
/// The coping, the cornice's upper step, chamfered: the beast crouches on it.
const COPING_HX: f64 = 6.05;
const COPING_Z: [f64; 2] = [-6.25, 5.25];
const COPING_Y: [f64; 2] = [15.15, 16.05];
/// The blind arcade on the wall: equilateral pointed arches of this half-span, springing this high, sunk this deep.
const ARCH_W: f64 = 0.65;
const ARCH_Y: [f64; 2] = [12.1, 12.85];
const ARCH_DEPTH: f64 = 0.6;

// --- The figure (frame: f forward along +z, u up from the coping, s across along x) ---------------------------------
/// The figure's scale.
const FIG_SCALE: f64 = 1.9;
/// Where the coping's front edge lies along f, and so where the figure's frame stands along z.
const FRONT_F: f64 = 3.0;
const FIG_Z0: f64 = COPING_Z[1] - FIG_SCALE * FRONT_F;
/// The head's scale about the end of the neck: a grotesque's head is large.
const HEAD_SCALE: f64 = 1.6;
/// How far the head is raised from level, degrees: a gargoyle gapes up and out.
const HEAD_LIFT_DEG: f64 = 38.0;
/// The axis along the spine the folded wings lean out from, and how far they lean from upright, degrees.
const WING_AXIS_U: f64 = 3.6;
const WING_LEAN_DEG: f64 = 55.0;
/// The thinnest round any limb, horn or claw tapers to, in the figure's frame (x 1.5 in the world: a 1.26 mm section).
const TIP_R: f64 = 0.42;

/// The meshing step and the face budget of the sculpt.
const STEP_MM: f64 = 0.09;
const FACES: usize = 210_000;
/// The fillet the part grows out of the band with, mm.
const FILLET_MM: f64 = 0.7;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
/// Coarse enough that the whole ring, part included, stays under `cad::measure::thickness`'s 250 000-face limit.
fn thickness_params() -> BuildParams {
    BuildParams { theta_steps: 128, profile_steps: 64, ..BuildParams::default() }
}

/// `cad::measure::thickness`'s own test (a ray from a face's centre along its inward normal to the next face) on a mesh
/// of any size through a BVH, at `samples` faces spread evenly: (rays, below the limit, thinnest, where).
fn sampled_thickness(m: &mesh::Mesh, limit: f64, samples: usize) -> (usize, usize, f64, P3) {
    use ringdesign_core::interaction::bvh::Bvh;
    let bvh = Bvh::build(m);
    let stride = m.faces.len().div_ceil(samples).max(1);
    let (mut rays, mut below, mut least, mut at) = (0, 0, f64::MAX, [0.0; 3]);
    for f in m.faces.iter().step_by(stride) {
        let Some((a, b, c)) = m.triangle(f) else { continue };
        let (e1, e2) = (sub(b, a), sub(c, a));
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = len(n);
        if l < 1e-12 {
            continue;
        }
        let inward = mul(n, -1.0 / l);
        let o = add(mul(add(add(a, b), c), 1.0 / 3.0), mul(inward, 1e-5));
        rays += 1;
        if let Some((_, t)) = bvh.ray(m, o, inward) {
            if t < limit {
                below += 1;
                if std::env::var("GURGULIO_THIN").is_ok() {
                    let fr = frame(o);
                    println!("    thin {t:.3} at world [{:.2}, {:.2}, {:.2}] figure [{:.2}, {:.2}, {:.2}]", o[0], o[1], o[2], fr[0], fr[1], fr[2]);
                }
            }
            if t < least {
                least = t;
                at = o;
            }
        }
    }
    (rays, below, least, at)
}

/// The bare band: flat, swelling in width and thickness to the crown, where the parapet stands.
fn band() -> RingDesign {
    let mut d = RingDesign { name: NAME.into(), ..RingDesign::default() };
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = BAND_W;
    d.profile.thickness_mm = BAND_T;
    d.profile.comfort_fit_mm = 0.15;
    d.profile.flatten_sides();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![key(30.0, 1.12, 1.0), key(55.0, 1.5, 1.08), key(66.0, 1.85, 1.15), key(118.0, 1.85, 1.15), key(130.0, 1.5, 1.08), key(155.0, 1.12, 1.0), key(270.0, 1.0, 1.0)];
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

// --- Distance helpers ------------------------------------------------------------------------------------------------

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: P3) -> f64 {
    dot(a, a).sqrt()
}

/// A box of half-extents `h` about `c` with every edge chamfered `ch` at 45 degrees.
fn chamfer_box(p: P3, c: P3, h: P3, ch: f64) -> f64 {
    let q = [(p[0] - c[0]).abs(), (p[1] - c[1]).abs(), (p[2] - c[2]).abs()];
    let d = [q[0] - h[0], q[1] - h[1], q[2] - h[2]];
    let outside = len([d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)]);
    let boxd = outside + d[0].max(d[1]).max(d[2]).min(0.0);
    let e = |i: usize, j: usize| (q[i] + q[j] - (h[i] + h[j] - ch)) * FRAC_1_SQRT_2;
    boxd.max(e(0, 1)).max(e(0, 2)).max(e(1, 2))
}

/// A box of half-extents `h` about `c` with every edge rounded `r`.
fn round_box(p: P3, c: P3, h: P3, r: f64) -> f64 {
    let q = [(p[0] - c[0]).abs() - (h[0] - r), (p[1] - c[1]).abs() - (h[1] - r), (p[2] - c[2]).abs() - (h[2] - r)];
    len([q[0].max(0.0), q[1].max(0.0), q[2].max(0.0)]) + q[0].max(q[1]).max(q[2]).min(0.0) - r
}

/// Signed distance to a closed polygon in the plane.
fn polygon(p: [f64; 2], v: &[[f64; 2]]) -> f64 {
    let mut d = f64::MAX;
    let mut inside = false;
    for i in 0..v.len() {
        let (a, b) = (v[i], v[(i + 1) % v.len()]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let w = [p[0] - a[0], p[1] - a[1]];
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1])).clamp(0.0, 1.0);
        let q = [w[0] - e[0] * t, w[1] - e[1] * t];
        d = d.min(q[0] * q[0] + q[1] * q[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            inside = !inside;
        }
    }
    if inside { -d.sqrt() } else { d.sqrt() }
}

// --- The parapet ------------------------------------------------------------------------------------------------------

/// A pointed (equilateral) arch in the plane: half-span `w`, springing at `b = h0`, its foot at `b = b0`.
fn pointed_arch(a: f64, b: f64, w: f64, b0: f64, h0: f64) -> f64 {
    let jambs = (a.abs() - w).max(b0 - b).max(b - h0);
    let head = ((a - w).hypot(b - h0) - 2.0 * w).max((a + w).hypot(b - h0) - 2.0 * w).max(h0 - b);
    jambs.min(head)
}

fn parapet(p: P3) -> f64 {
    let mid = |r: [f64; 2]| 0.5 * (r[0] + r[1]);
    let half = |r: [f64; 2]| 0.5 * (r[1] - r[0]);
    // The corbel widens upward from its foot in the band, then runs straight up into the wall, its upright edges
    // chamfered like the wall's so the two meet flush with no groove between them.
    let top = CORBEL_Y[1] - 0.5;
    let t = ((p[1] - CORBEL_Y[0]) / (top - CORBEL_Y[0])).clamp(0.0, 1.0);
    let hx = CORBEL_HX[0] + (CORBEL_HX[1] - CORBEL_HX[0]) * t;
    let lean = ((CORBEL_HX[1] - CORBEL_HX[0]) / (top - CORBEL_Y[0])).hypot(1.0);
    let (z0, z1) = (CORBEL_Z[0] + CORBEL_TAPER * (1.0 - t), CORBEL_Z[1] - CORBEL_TAPER * (1.0 - t));
    let (cx, cz) = ((p[0].abs() - hx) / lean, (p[2] - 0.5 * (z0 + z1)).abs() - 0.5 * (z1 - z0));
    let corbel = cx.max(cz).max((cx + cz + 0.4) * FRAC_1_SQRT_2).max(CORBEL_Y[0] - 0.4 - p[1]).max(p[1] - CORBEL_Y[1] - 0.3);
    // The wall's box runs on into the cornice and down into the corbel's straight top, so its horizontal edges'
    // chamfers are buried and leave no groove.
    let wall = chamfer_box(p, [0.0, mid(WALL_Y), mid(WALL_Z)], [WALL_HX, half(WALL_Y) + 0.5, half(WALL_Z)], 0.4);
    let cornice = round_box(p, [0.0, mid(CORNICE_Y), mid(CORNICE_Z)], [CORNICE_HX, half(CORNICE_Y), half(CORNICE_Z)], 0.27);
    let coping = chamfer_box(p, [0.0, mid(COPING_Y), mid(COPING_Z)], [COPING_HX, half(COPING_Y), half(COPING_Z)], 0.28);
    let mut d = corbel.min(wall).min(cornice).min(coping);
    // The blind arcade: three pointed arches on each face of the wall.
    let mut recess = f64::MAX;
    let mut sink = |n: f64, face: f64, across: f64, pitch: f64| {
        for k in -1..=1 {
            let a = pointed_arch(across - k as f64 * pitch, p[1], ARCH_W, ARCH_Y[0], ARCH_Y[1]);
            recess = recess.min(a.max(face - ARCH_DEPTH - n));
        }
    };
    // Pitched so every pier between two arches is over 0.9 mm and the outer ones keep 1.1 mm to the chamfered corners.
    let xp = 0.8 * 2.0 * WALL_HX / 3.0;
    let zp = 0.85 * (WALL_Z[1] - WALL_Z[0]) / 3.0;
    sink(p[2], WALL_Z[1], p[0], xp);
    sink(-p[2], -WALL_Z[0], p[0], xp);
    sink(p[0], WALL_HX, p[2] - mid(WALL_Z), zp);
    sink(-p[0], WALL_HX, p[2] - mid(WALL_Z), zp);
    d = smax(d, -recess, 0.05);
    d.min(pinnacles(p)).min(buttresses(p))
}

/// The buttresses down each shoulder: three set-offs, each a level of the buttress's back (radius, mm) out to an
/// angle from the crown (degrees), with a weathered slope down to the next.
const BUTTRESS_STEPS: [(f64, f64); 3] = [(13.15, 31.0), (12.75, 43.0), (12.4, 55.0)];
/// Where the buttresses start under the corbel, their half-width along the finger, and their foot, sunk in the band.
const BUTTRESS_FROM_DEG: f64 = 15.0;
const BUTTRESS_HZ: f64 = 1.05;
const BUTTRESS_FOOT_R: f64 = 10.6;

fn buttresses(p: P3) -> f64 {
    let r = p[0].hypot(p[1]);
    let off = (90.0 - p[1].atan2(p[0].abs()).to_degrees()).abs();
    // The back steps down at each set-off through a smooth weathering about 0.8 mm long.
    let mut top = BUTTRESS_STEPS[0].0;
    for k in 1..BUTTRESS_STEPS.len() {
        let (lv, at) = (BUTTRESS_STEPS[k].0, BUTTRESS_STEPS[k - 1].1);
        let drop = BUTTRESS_STEPS[k - 1].0 - lv;
        let run = (1.6 * drop / r).to_degrees();
        let x = ((off - at) / run).clamp(0.0, 1.0);
        top -= drop * x * x * (3.0 - 2.0 * x);
    }
    let last = BUTTRESS_STEPS[BUTTRESS_STEPS.len() - 1];
    let mm = r * PI / 180.0;
    // The last set-off ends in a steep weathering down into the band, meeting it at a full angle and leaving no sliver.
    let end = (2.0 * (off - last.1) * mm + (r - BUTTRESS_FOOT_R - 0.4)) / 5f64.sqrt();
    let along = (BUTTRESS_FROM_DEG - off) * mm;
    let d = (r - top).max(BUTTRESS_FOOT_R - r).max(end).max(along).max(p[2].abs() - BUTTRESS_HZ);
    // Chamfer the back's two long edges.
    d.max(((r - top) + (p[2].abs() - BUTTRESS_HZ) + 0.3) * FRAC_1_SQRT_2)
}

/// The pinnacles at the coping's back corners: a chamfered shaft, a gablet on each face, a spire and a knob finial.
const PINNACLE_HALF: f64 = 0.95;
const PINNACLE_SHAFT: f64 = 1.5;
const PINNACLE_SPIRE: f64 = 3.4;

fn pinnacles(p: P3) -> f64 {
    let at = [COPING_HX - PINNACLE_HALF - 0.15, COPING_Z[0] + PINNACLE_HALF + 0.15];
    // Both back corners: x folded.
    let (x, y, z) = (p[0].abs() - at[0], p[1] - COPING_Y[1], p[2] - at[1]);
    let shaft = chamfer_box([x, y, z], [0.0, 0.5 * PINNACLE_SHAFT - 0.2, 0.0], [PINNACLE_HALF, 0.5 * PINNACLE_SHAFT + 0.2, PINNACLE_HALF], 0.18);
    // Gablets: a steep roof over each face of the shaft's top.
    let gable = (x.abs().max(z.abs()) - PINNACLE_HALF).max((y - PINNACLE_SHAFT) * 0.8 + x.abs().min(z.abs()) * 0.6 - 0.55).max(PINNACLE_SHAFT - 0.4 - y);
    // The spire: a square pyramid from the shaft's top, its faces leaning in.
    let t = ((y - PINNACLE_SHAFT) / PINNACLE_SPIRE).clamp(0.0, 1.0);
    let hw = 0.42 + (PINNACLE_HALF * 0.85 - 0.42) * (1.0 - t);
    let spire = ((x.abs().max(z.abs()) - hw) / (1.0 + ((PINNACLE_HALF * 0.85 - 0.42) / PINNACLE_SPIRE).powi(2)).sqrt()).max(PINNACLE_SHAFT - 0.1 - y).max(y - PINNACLE_SHAFT - PINNACLE_SPIRE);
    let d = shaft.min(gable).min(spire);
    let finial = len([x, y - PINNACLE_SHAFT - PINNACLE_SPIRE - 0.2, z]) - 0.55;
    smin(d, finial, 0.15)
}

// --- The gargoyle -----------------------------------------------------------------------------------------------------

/// The figure frame of a world point: `[f, u, s]`, in figure units (world mm over `FIG_SCALE`).
fn frame(p: P3) -> P3 {
    [(p[2] - FIG_Z0) / FIG_SCALE, (p[1] - COPING_Y[1]) / FIG_SCALE, p[0] / FIG_SCALE]
}

/// The end of the neck, about which the head is scaled.
const NECK: P3 = [3.5, 2.3, 0.0];

/// A chain of rounded cones through `pts`, each point with its radius, united with a small round.
fn chain(q: P3, pts: &[(P3, f64)], k: f64) -> f64 {
    let mut d = f64::MAX;
    for w in pts.windows(2) {
        let c = round_cone(q, w[0].0, w[1].0, w[0].1, w[1].1);
        d = if d == f64::MAX { c } else { smin(d, c, k) };
    }
    d
}

/// The head, in its own frame (the neck's end at the origin, unscaled), `s` folded to one side: a grotesque's, a
/// heavy overhanging brow over deep-set eyes, a short broad snout with a flat nose, the jaws gaping wide with a grooved
/// tongue lying out of them as the spout, short horns and pointed ears.
fn head(q: P3) -> f64 {
    let at = |c: P3| sub(q, c);
    let cranium = ellipsoid(at([0.5, 0.8, 0.0]), [1.1, 1.05, 1.2]);
    // A broad, flat face under a heavy brow, wide at the jowls.
    let face = ellipsoid(at([1.35, 0.45, 0.0]), [0.75, 1.05, 1.3]);
    let brow = ellipsoid(at([1.85, 1.38, 0.0]), [0.5, 0.4, 1.2]);
    let brow_l = ellipsoid(at([2.0, 1.32, 0.6]), [0.52, 0.48, 0.58]);
    let jowl = ellipsoid(at([1.05, -0.35, 0.95]), [0.65, 0.7, 0.42]);
    let nose = ellipsoid(at([2.18, 0.62, 0.0]), [0.4, 0.38, 0.62]);
    let lip = ellipsoid(at([1.95, 0.12, 0.0]), [0.58, 0.36, 1.05]);
    let jaw = ellipsoid(at([1.6, -1.02, 0.0]), [0.85, 0.48, 1.0]);
    let chin = ellipsoid(at([2.15, -1.05, 0.0]), [0.4, 0.38, 0.6]);
    let horn = chain(q, &[([0.8, 1.55, 0.6], 0.45), ([0.25, 2.1, 0.72], 0.32), ([-0.35, 2.05, 0.75], 0.22)], 0.08);
    let ear = round_cone(q, [0.3, 1.0, 1.1], [-0.15, 1.5, 1.75], 0.42, 0.22);
    let mut d = smin(cranium, face, 0.4);
    d = smin(d, brow, 0.25);
    d = smin(d, brow_l, 0.2);
    d = smin(d, jowl, 0.3);
    d = smin(d, nose, 0.2);
    d = smin(d, lip, 0.2);
    d = smin(d, jaw, 0.3);
    d = smin(d, chin, 0.15);
    d = smin(d, horn, 0.15);
    d = smin(d, ear, 0.15);
    // The gape: a wide oval mouth opening forward between the lip and the dropped jaw, running back into the throat.
    let gape = ellipsoid(at([2.1, -0.4, 0.0]), [0.9, 0.3, 0.55]);
    d = smax(d, -gape, 0.08);
    // The spout: a grooved trough lying out of the mouth along the jaw.
    // A rounded tongue-trough with a channel along its top, open at the end and widening a little toward it: the lips
    // beside the channel keep 0.3 head units (0.9 mm) of metal and meet it at a full rounded angle, never a knife edge.
    let (ta, tb): ([f64; 2], [f64; 2]) = ([1.5, -0.62], [3.0, -1.28]);
    let dir = { let l = (tb[0] - ta[0]).hypot(tb[1] - ta[1]); [(tb[0] - ta[0]) / l, (tb[1] - ta[1]) / l] };
    let rel = [q[0] - ta[0], q[1] - ta[1]];
    let along = rel[0] * dir[0] + rel[1] * dir[1];
    let up = -rel[0] * dir[1] + rel[1] * dir[0];
    let length = (tb[0] - ta[0]).hypot(tb[1] - ta[1]);
    let trough = round_box([along, up, q[2] * (1.0 + 0.25 * (along / length).clamp(0.0, 1.0))], [0.5 * length, 0.0, 0.0], [0.5 * length + 0.3, 0.32, 0.47], 0.28);
    let groove = round_box([along, up, q[2]], [0.5 * length + 0.3, 0.34, 0.0], [0.5 * length + 0.1, 0.17, 0.15], 0.12);
    d = smin(d, smax(trough, -groove, 0.07), 0.12);
    // Fangs down from the lip and tusks up from the jaw, standing clear inside the gape.
    let fang = round_cone(q, [2.3, -0.05, 0.36], [2.4, -0.46, 0.34], 0.2, 0.15);
    let tusk = round_cone(q, [1.95, -0.85, 0.36], [2.05, -0.5, 0.36], 0.2, 0.15);
    d = smin(d, fang.min(tusk), 0.06);
    // Deep-set eyes in the shadow of the brow.
    let eye = len(sub(q, [1.82, 0.8, 0.58])) - 0.25;
    d.min(eye)
}

/// The gargoyle's field in figure units: a heavy hunched lump crouched on its haunches, the forelegs braced down to
/// the edge, the head thrust out past it.
fn figure(fr: P3) -> f64 {
    let (f, u, s) = (fr[0], fr[1], fr[2]);
    let q = [f, u, s.abs()];
    // The body: one heavy crouched mass, the pelvis low behind rising to a hunched back and deep chest.
    let torso = round_cone(q, [-1.4, 1.75, 0.0], [0.5, 3.0, 0.0], 1.9, 2.05);
    let hump = ellipsoid(sub(q, [0.1, 4.15, 0.0]), [1.75, 1.45, 1.9]);
    let chest = ellipsoid(sub(q, [1.35, 2.3, 0.0]), [1.25, 1.55, 1.55]);
    let neck = round_cone(q, [1.3, 3.6, 0.0], NECK, 1.3, 0.92);
    let mut body = smin(torso, hump, 0.6);
    body = smin(body, chest, 0.6);
    body = smin(body, neck, 0.5);
    // The head, scaled about the neck's end and raised.
    let hq = sculpt::turn(mul(sub(q, NECK), 1.0 / HEAD_SCALE), 0, 1, -HEAD_LIFT_DEG);
    body = smin(body, head(hq) * HEAD_SCALE, 0.35);
    // Forelegs braced down to the edge, tapering from the heavy shoulder: elbow, wrist, and three clawed fingers
    // hooked over the coping's front edge.
    let mut limbs = ellipsoid(sub(q, [1.0, 2.9, 1.55]), [1.05, 1.25, 0.85]);
    limbs = smin(limbs, chain(q, &[([1.2, 2.8, 1.65], 0.95), ([2.0, 1.45, 1.95], 0.72), ([FRONT_F - 0.4, 0.62, 1.7], 0.55)], 0.2), 0.35);
    limbs = smin(limbs, ellipsoid(sub(q, [FRONT_F - 0.35, 0.5, 1.7]), [0.6, 0.45, 0.68]), 0.15);
    for (k, sf) in [1.2, 1.7, 2.2].into_iter().enumerate() {
        let lean = [0.0, 0.1, -0.05][k];
        let knuckle = [FRONT_F + 0.1, 0.48, sf];
        let finger = chain(q, &[([FRONT_F - 0.4, 0.5, 1.7 + 0.6 * (sf - 1.7)], 0.46), (knuckle, TIP_R + 0.03), ([FRONT_F + 0.45, -0.3, sf + lean], TIP_R), ([FRONT_F + 0.3, -0.95, sf + lean], TIP_R)], 0.06);
        limbs = smin(limbs, finger, 0.1);
    }
    // Hind legs folded high against the flanks: a heavy haunch, the shin back down, the foot forward on the coping.
    limbs = smin(limbs, ellipsoid(sub(q, [-0.7, 2.0, 1.75]), [1.5, 1.45, 0.85]), 0.45);
    limbs = smin(limbs, chain(q, &[([0.4, 1.5, 2.1], 0.8), ([-0.9, 0.55, 2.2], 0.55)], 0.15), 0.3);
    for st in [1.75, 2.2, 2.65] {
        limbs = smin(limbs, round_cone(q, [-0.85, 0.5, 2.2], [0.5, TIP_R, st], 0.5, TIP_R), 0.1);
    }
    let mut fig = smin(body, limbs, 0.35);
    // Folded bat wings close over the back like a cloak: each a membrane slab, scalloped between the fingers, laid
    // over the flank and leaning out from the spine by `WING_LEAN_DEG`, the arm and finger bones low ribs on it, the
    // wrist at the shoulder's height.
    let wq = sculpt::turn([q[0], q[1] - WING_AXIS_U, q[2]], 1, 2, WING_LEAN_DEG);
    let (wu, ws) = (wq[1] + WING_AXIS_U, wq[2]);
    let flare = |u: f64| 1.25 + 0.05 * (4.5 - u);
    // Arm to the wrist, then four fingers fanning back to the tips, the trailing edge scalloped deep between them.
    let wing_pts: [[f64; 2]; 12] = [[1.2, 4.8], [-0.1, 6.0], [-0.6, 5.85], [-3.0, 4.25], [-2.85, 3.5], [-2.95, 3.0], [-2.55, 2.35], [-2.45, 1.95], [-1.9, 1.5], [-1.5, 1.25], [-0.9, 1.75], [0.4, 3.5]];
    let d2 = polygon([f, wu], &wing_pts);
    let dn = ws - flare(wu);
    let w = [d2 + 0.3, dn.abs() - 0.45 + 0.3];
    let membrane = w[0].max(w[1]).min(0.0) + w[0].max(0.0).hypot(w[1].max(0.0)) - 0.3;
    let wp = [f, wu, ws];
    let on = |a: [f64; 2]| [a[0], a[1], flare(a[1]) + 0.06];
    let wrist = on([-0.3, 5.9]);
    let mut wing = smin(membrane, round_cone(wp, on([1.2, 4.8]), wrist, 0.5, 0.44), 0.12);
    for tip in [[-3.0, 4.25], [-2.95, 3.0], [-2.45, 1.95], [-1.5, 1.25]] {
        wing = smin(wing, round_cone(wp, wrist, on(tip), TIP_R + 0.05, TIP_R + 0.05), 0.15);
    }
    wing = smin(wing, round_cone(wp, wrist, add(wrist, [0.35, 0.35, -0.1]), 0.44, TIP_R), 0.08);
    fig = smin(fig, wing, 0.15);
    // The tail, wrapped forward round one haunch on the coping (not mirrored).
    let fs = [f, u, s];
    let tail = chain(fs, &[([-2.3, 1.0, 0.0], 0.62), ([-2.75, 0.52, -1.2], 0.52), ([-1.9, TIP_R + 0.04, -2.45], 0.47), ([-0.6, TIP_R + 0.02, -2.75], TIP_R), ([0.2, TIP_R + 0.08, -2.45], TIP_R)], 0.1);
    smin(fig, tail, 0.15)
}

/// The whole field of the part, world millimetres, negative inside.
fn field(p: P3) -> f64 {
    let fig = figure(frame(p)) * FIG_SCALE;
    // The beast sits in its perch, and nothing comes near the finger.
    let d = smin(fig, parapet(p), 0.25);
    d.max(CLEAR_R_MM - p[0].hypot(p[1]))
}

/// The box the field is meshed over, world millimetres.
fn field_box() -> (P3, P3) {
    let head_tip = FIG_Z0 + FIG_SCALE * (NECK[0] + HEAD_SCALE * 3.9);
    ([-11.4, 5.0, COPING_Z[0] - 0.6], [11.4, COPING_Y[1] + FIG_SCALE * 8.0, head_tip + 0.6])
}

// --- The sculpt -------------------------------------------------------------------------------------------------------

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Composition {
    raw_faces: usize,
    faces: usize,
    volume_mm3: f64,
    relax_undone: usize,
    sculpt_s: f64,
    box_mm: [P3; 2],
    figure_height_over_coping_mm: f64,
    head_past_coping_edge_mm: f64,
    #[serde(default)]
    decimation: (usize, f64),
}

/// Where a sculpt made from this very source is kept between runs: meshing and decimating it takes many minutes, and
/// every tool in the chain is deterministic, so the same source gives the same solid bit for bit.
fn cache_path() -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    include_str!("tenebrae_gurgulio.rs").hash(&mut h);
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/gurgulio-sculpt-{:016x}.json", h.finish()))
}

fn sculpt_solid(comp: &mut Composition) -> Result<csg::Solid> {
    let cache = cache_path();
    if let Ok(text) = std::fs::read_to_string(&cache) {
        let (c, v, f): (serde_json::Value, Vec<[u64; 3]>, Vec<[u32; 3]>) = serde_json::from_str(&text)?;
        *comp = serde_json::from_value(c)?;
        let v = v.into_iter().map(|p| p.map(f64::from_bits)).collect();
        println!("  sculpt read back from {}", cache.display());
        return Ok(csg::Solid { v, f });
    }
    let s = sculpt_fresh(comp)?;
    std::fs::write(&cache, serde_json::to_string(&(serde_json::to_value(&*comp)?, s.v.iter().map(|p| p.map(f64::to_bits)).collect::<Vec<_>>(), &s.f))?)?;
    Ok(s)
}

fn sculpt_fresh(comp: &mut Composition) -> Result<csg::Solid> {
    let t = std::time::Instant::now();
    let (lo, hi) = field_box();
    comp.box_mm = [lo, hi];
    let fld = |p: P3| field(p);
    let raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &fld);
    comp.raw_faces = raw.f.len();
    println!("  meshed {} faces at {STEP_MM} mm ({:.1} s)", raw.f.len(), t.elapsed().as_secs_f64());
    // No relax: on this field (chamfered tiers, the wing slabs, cusped recesses) a relax folds the surface nearly
    // everywhere and `relax_clean` puts back almost every vertex, so the tetrahedral vertices, already on the
    // surface, are kept as meshed.
    // Decimated to the budget the template's graph can carry (the stored mesh is most of the design), backing off
    // the cost cap until the result does not cross itself.
    let mut nets = None;
    for (target, cap) in [(FACES, 6e-3), (FACES, 4e-3), (FACES + 20_000, 3e-3), (FACES + 40_000, 2e-3)] {
        let d = sculpt::decimate(&raw, target, cap, 2.0, 18.0, 35.0);
        if csg::self_crossings(&d) == 0 {
            comp.decimation = (target, cap);
            nets = Some(d);
            break;
        }
    }
    let nets = nets.unwrap_or_else(|| sculpt::clean_decimate(&raw, FACES));
    println!("  decimated to {} faces ({:.1} s)", nets.f.len(), t.elapsed().as_secs_f64());
    let s = sculpt::settle(nets, &fld, &|_| false);
    println!("  settled ({:.1} s)", t.elapsed().as_secs_f64());
    comp.faces = s.f.len();
    comp.volume_mm3 = sculpt::closure(&s).1;
    comp.sculpt_s = t.elapsed().as_secs_f64();
    let top = s.v.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
    comp.figure_height_over_coping_mm = top - COPING_Y[1];
    comp.head_past_coping_edge_mm = s.v.iter().map(|v| v[2]).fold(f64::MIN, f64::max) - COPING_Z[1];
    Ok(s)
}

fn author() -> Result<(RingDesign, AlphaLibrary, Composition, csg::Solid)> {
    let mut d = band();
    let lib = AlphaLibrary::builtin();
    let mut comp = Composition::default();
    let solid = sculpt_solid(&mut comp)?;
    let packed = sculpt::packed(&solid)?;
    let doc = d.cad.get_or_insert_with(Document::default);
    doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let recipe = stored::Recipe {
        kernel: "sculpt".into(),
        op: "gargoyle on its parapet".into(),
        params: json!({"step_mm": STEP_MM, "faces": FACES, "figure_scale": FIG_SCALE, "heading": "+z, toward the fingertip", "head_scale": HEAD_SCALE, "tip_r_mm": TIP_R, "coping_top_mm": COPING_Y[1]}),
        digest: String::new(),
    };
    let component = Component { attach: Attach::Join, placement: Placement::Free, blend_mm: 0.0, fillet_into_band: FILLET_MM, ..Component::default() };
    doc.append(Feature { id: 2, name: "Grow the gargoyle and its parapet out of the band".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component })?;
    Ok((d, lib, comp, solid))
}

// --- Gates ------------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

fn part_crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

/// The geometry gates at one build size.
struct Pass {
    triangles: usize,
    watertight: bool,
    degenerate: usize,
    crossings: usize,
    notes: Vec<String>,
    parts: Vec<(String, usize)>,
    joined: usize,
    features_not_ok: Vec<String>,
}

impl Pass {
    fn of(built: &mesh::BuildResult) -> Self {
        let (watertight, degenerate, crossings) = geometry(&built.mesh);
        let mut notes = built.solids.notes.clone();
        notes.extend(built.parts.notes.iter().cloned());
        let features_not_ok = built
            .parts
            .evaluated
            .iter()
            .flat_map(|e| e.features.iter())
            .filter(|r| !matches!(r.status, cad::FeatureStatus::Ok))
            .map(|r| format!("#{} {}: {:?}", r.id, r.name, r.status))
            .collect();
        Self { triangles: built.mesh.faces.len(), watertight, degenerate, crossings, notes, parts: part_crossings(built), joined: built.parts.joined, features_not_ok }
    }
    fn ok(&self) -> bool {
        self.watertight && self.degenerate == 0 && self.crossings == 0 && self.notes.is_empty() && self.parts.iter().all(|p| p.1 == 0) && self.joined == 1 && self.features_not_ok.is_empty()
    }
    fn json(&self) -> serde_json::Value {
        json!({"triangles": self.triangles, "watertight": self.watertight, "degenerate_faces": self.degenerate, "self_crossings": self.crossings, "notes": self.notes, "made_part_crossings": self.parts, "parts_joined": self.joined, "features_not_ok": self.features_not_ok, "pass": self.ok()})
    }
    fn line(&self) -> String {
        format!("{} tris, watertight {}, degenerate {}, crossings {}, notes {:?}, parts {:?}, joined {}, features not ok {:?}", self.triangles, self.watertight, self.degenerate, self.crossings, self.notes, self.parts, self.joined, self.features_not_ok)
    }
}

/// Every gate at one build size, as JSON, and whether all passed.
fn gate_block(d: &RingDesign, lib: &AlphaLibrary, solid: &csg::Solid, params: BuildParams, verify: bool) -> Result<(serde_json::Value, bool, mesh::BuildResult)> {
    let t = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let pass = Pass::of(&built);
    println!("  {}x{}: {} ({build_s:.1} s)", params.theta_steps, params.profile_steps, pass.line());
    let (least_r, inside) = bore_intrusion(d, &built.mesh);
    let sculpt_crossings = csg::self_crossings(solid);
    let (open_edges, _) = sculpt::closure(solid);
    let band_field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let mut fieldr = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut fieldr, d, &built);
    let findings = dfm::findings_in(d, lib);
    let lands = dfm::cut_lands(d, &built, MIN_SECTION_MM);
    let stones = ringdesign_core::stones::report_built(d, fieldr.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let pattern = mesh::try_build_pattern(d, lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let cold = if verify {
        let dir = std::env::temp_dir().join(format!("gurgulio-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        library::save_design_embedded(dir.join("design.ring.json"), d, lib)?;
        let saved = library::load_design(dir.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", pass.watertight && pass.degenerate == 0 && pass.crossings == 0),
        ("the sculpted part closed and uncrossed, as made and as placed", open_edges == 0 && sculpt_crossings == 0 && pass.parts.iter().all(|p| p.1 == 0)),
        ("solids and parts notes empty, every feature Ok, the part joined", pass.notes.is_empty() && pass.joined == 1 && pass.features_not_ok.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax field verdict Castable with the 0.8 mm fill", fieldr.process == CastProcess::LostWax && fieldr.verdict == Verdict::Castable && band_field.verdict == Verdict::Castable),
        ("dfm::cut_lands clean at 0.8 mm", lands.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("within 2 million triangles", pass.triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    for (g, p) in &gates {
        println!("    {} {g}", if *p { "pass" } else { "FAIL" });
    }
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    let all = gates.iter().all(|(_, p)| *p);
    let v = json!({
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "build_s": build_s},
        "geometry": pass.json(),
        "sculpt": {"open_edges": open_edges, "self_crossings": sculpt_crossings, "faces": solid.f.len()},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": fieldr.verdict.label(), "band_verdict": band_field.verdict.label(), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": fieldr.notes},
        "cut_lands_0_8": lands.iter().map(|f| f.message.clone()).collect::<Vec<_>>(),
        "two_part_undercut": {"band_percent": band_field.undercut_fraction() * 100.0, "with_parts_percent": fieldr.undercut_fraction() * 100.0, "note": "reported only: lost wax judges fill, never the pull"},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, p)| json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": all,
    });
    Ok((v, all, built))
}

// --- Renders ----------------------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger axis, pitch from it toward the head.
fn views() -> Vec<(&'static str, f64, f64)> {
    let hero = std::env::var("GURGULIO_HERO").ok().and_then(|s| {
        let v: Vec<f64> = s.split(',').filter_map(|x| x.parse().ok()).collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    });
    let (hy, hp) = hero.unwrap_or((0.35, 0.55));
    vec![("hero", hy, hp), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", 1.2, 0.75), ("reverse", PI - 0.6, 0.45)]
}

fn paste(sheet: &mut [u8], sheet_w: usize, img: &[u8], edge: usize, x0: usize, y0: usize) {
    for y in 0..edge {
        let row = &img[y * edge * 3..(y + 1) * edge * 3];
        let at = ((y0 + y) * sheet_w + x0) * 3;
        sheet[at..at + edge * 3].copy_from_slice(row);
    }
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let mut parts = vec![render::Part::metal(&fin.metal, render::GOLD)];
    for (m, tint) in &fin.stones {
        parts.push(render::Part::tinted_stone(m, *tint));
    }
    let vs = views();
    for (name, yaw, pitch) in &vs {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, *yaw, *pitch, edge)?;
    }
    // No stones: the stones view is the head close, framed on the whole ring so nothing is cropped.
    let centre = [0.0, COPING_Y[1] + FIG_SCALE * 3.0, FIG_Z0 + FIG_SCALE * (NECK[0] + 1.5)];
    render::write_png_framed(out.join("stones.png"), &parts, vs[0].1 + 0.2, 0.75, render::Framing::new(centre, 6.0), edge)?;
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (yaw, pitch) = (vs[0].1, vs[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let fin_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut pair = vec![0u8; edge * 2 * edge * 3];
    paste(&mut pair, edge * 2, &bare_img, edge, 0, 0);
    paste(&mut pair, edge * 2, &fin_img, edge, edge, 0);
    image::save_buffer(out.join("bare-vs-finished.png"), &pair, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    render::write_png_parts(out.join("face-300.png"), &parts, vs[1].1, vs[1].2, 300)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    let mut sheet = vec![0u8; 900 * 600 * 3];
    for (k, (_, yaw, pitch)) in vs.iter().enumerate() {
        let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3);
        paste(&mut sheet, 900, &img, 300, (k % 3) * 300, (k / 3) * 300);
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 900, 600, image::ColorType::Rgb8)?;
    if std::env::var("GURGULIO_VIEWS").is_ok() {
        let mut sheet = vec![0u8; 1200 * 900 * 3];
        for (k, (yaw, pitch)) in [(-0.9, 0.7), (-0.6, 0.8), (-0.45, 0.95), (-0.2, 1.0), (0.2, 0.9), (0.45, 0.8), (-1.2, 0.6), (-0.6, 1.2), (0.0, 1.2), (-1.6, 0.5), (1.2, 0.6), (0.0, 0.45)].iter().enumerate() {
            let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 2);
            paste(&mut sheet, 1200, &img, 300, (k % 4) * 300, (k / 4) * 300);
        }
        image::save_buffer(out.join("views-probe.png"), &sheet, 1200, 900, image::ColorType::Rgb8)?;
    }
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
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    if let Ok(step) = std::env::var("GURGULIO_PREVIEW") {
        // A quick look while shaping: the field meshed coarse beside the bare band, no join and no gates.
        let step: f64 = step.parse().unwrap_or(0.12);
        let (lo, hi) = field_box();
        let fld = |p: P3| field(p);
        let t = std::time::Instant::now();
        let s = sculpt::tetra_mesh(lo, hi, step, &fld);
        println!("  preview mesh {} faces in {:.1} s", s.f.len(), t.elapsed().as_secs_f64());
        let bare = mesh::try_build(&band(), &AlphaLibrary::builtin(), BuildParams { theta_steps: 384, profile_steps: 160, ..BuildParams::default() })?;
        let mut both = bare.mesh.clone();
        let base = both.vertices.len() as u32;
        both.vertices.extend(s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)));
        both.faces.extend(s.f.iter().map(|f| f.map(|i| i + base)));
        both.normals.clear();
        let parts = vec![render::Part::metal(&both, render::GOLD)];
        let vs = views();
        let mut sheet = vec![0u8; 1200 * 900 * 3];
        let mut probe: Vec<(f64, f64)> = vs.iter().map(|v| (v.1, v.2)).collect();
        probe.extend([(0.73, 0.9), (0.9, 0.7), (0.5, 0.6), (0.6, 0.45), (0.35, 0.75), (-0.6, 0.7)]);
        for (k, (yaw, pitch)) in probe.iter().enumerate() {
            let img = render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 2);
            paste(&mut sheet, 1200, &img, 300, (k % 4) * 300, (k / 4) * 300);
        }
        image::save_buffer(out.join("preview.png"), &sheet, 1200, 900, image::ColorType::Rgb8)?;
        // The figure alone, framed: profile, front, top and three-quarter, and the head close in profile and front.
        let centre = [0.0, COPING_Y[1] + FIG_SCALE * 2.2, FIG_Z0 + FIG_SCALE * 1.5];
        let head_c = [0.0, COPING_Y[1] + FIG_SCALE * (NECK[1] + 0.5), FIG_Z0 + FIG_SCALE * (NECK[0] + 2.0)];
        let shots: [(f64, f64, [f64; 3], f64); 6] = [(0.0, 0.0, head_c, 4.0), (0.9, 0.5, head_c, 4.0), (0.0, PI * 0.5, centre, 11.0), (0.4, 0.7, centre, 11.0), (PI * 0.5, PI * 0.5, [4.4, 13.2, -0.5], 3.5), (1.0, 1.1, [-2.0, 18.6, -4.6], 3.5)];
        let mut sheet = vec![0u8; 1200 * 800 * 3];
        for (k, (yaw, pitch, c, hw)) in shots.iter().enumerate() {
            let img = render::render_parts_framed(&parts, *yaw, *pitch, render::Framing::new(*c, *hw), 400, 400, 2);
            paste(&mut sheet, 1200, &img, 400, (k % 3) * 400, (k / 3) * 400);
        }
        image::save_buffer(out.join("preview-figure.png"), &sheet, 1200, 800, image::ColorType::Rgb8)?;
        return Ok(());
    }
    let started = std::time::Instant::now();
    let (d, lib, comp, solid) = author()?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  sculpt {} raw faces -> {}, {:.1} mm3, {:.1} s; figure {:.2} mm over the coping, head {:.2} mm past its edge",
        comp.raw_faces, comp.faces, comp.volume_mm3, comp.sculpt_s, comp.figure_height_over_coping_mm, comp.head_past_coping_edge_mm
    );
    // The draft build's gates always; the export build's on top unless --draft.
    let (draft_gates, draft_ok, draft_built) = gate_block(&d, &lib, &solid, draft_params(), false)?;
    let (export_gates, export_ok, built) = if draft {
        (serde_json::Value::Null, true, draft_built)
    } else {
        let (g, ok, b) = gate_block(&d, &lib, &solid, export_params(), verify)?;
        (g, ok, b)
    };
    // The lost-wax wall: `cad::measure::thickness` at 0.8 on a ring small enough for it to sample (the band at 256 x
    // 128 with the part as built), and on the part as made; and every face of the part's own section against the floor.
    let small = mesh::try_build(&d, &lib, thickness_params())?;
    let th_ring = cad::measure::thickness(&small.mesh, MIN_SECTION_MM);
    let part_mesh = mesh::Mesh { vertices: solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), faces: solid.f.clone(), ..Default::default() };
    let th_part = cad::measure::thickness(&part_mesh, MIN_SECTION_MM);
    let (sec_min, sec_under) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    let (s_rays, s_below, s_min, s_at) = sampled_thickness(&built.mesh, MIN_SECTION_MM, 20_000);
    println!("  sampled thickness on the {} build: {s_rays} rays, {s_below} below 0.8, thinnest {s_min:.3} at {:?}", if draft { "draft" } else { "export" }, s_at.map(|v| (v * 100.0).round() / 100.0));
    let th_json = |t: &cad::measure::Thickness| json!({"rays": t.rays, "below_limit": t.below_limit, "unresolved": t.unresolved, "sampled_min_mm": t.sampled_min_mm, "point": t.point, "note": t.note});
    // The gate is the brief's: `cad::measure::thickness` on the whole ring. The part alone (its foot buried in the band
    // included) and the 20 000-ray census are reported beside it.
    let thickness_ok = th_ring.rays > 0 && th_ring.below_limit == 0;
    println!(
        "  thickness at 0.8: ring {} rays, {} below, min {:?}; part {} rays, {} below, min {:?}; part sections min {:.3}, {:.3} mm2 under",
        th_ring.rays, th_ring.below_limit, th_ring.sampled_min_mm, th_part.rays, th_part.below_limit, th_part.sampled_min_mm, sec_min, sec_under
    );
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "draft": {"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "gates_that_apply": "lost wax: geometry, every feature Ok, bore, field fill verdict, cad::measure::thickness and dfm::cut_lands at 0.8 mm, DFM, stones, pattern, triangles, cold reload. The sand gates (ray release, draft-clamp bites, two-part Castable) do not apply; the two-part undercut is reported as a number.",
        "size": d.size.display(),
        "author_s": author_s,
        "draft_build": draft_gates,
        "export_build": export_gates,
        "thickness_0_8": {"ring_128x64": th_json(&th_ring), "part": th_json(&th_part), "pass": thickness_ok, "sampled_on_the_final_build": {"rays": s_rays, "below_limit": s_below, "thinnest_mm": s_min, "at": s_at, "method": "the same test as cad::measure::thickness (inward normal ray from a face centre), through a BVH so it runs on the whole build at 20 000 faces"}},
        "part_sections_0_8": {"thinnest_mm": sec_min, "under_floor_mm2": sec_under, "method": "dfm::part_sections: one ray per face of the sculpted part along its inward normal"},
        "composition": comp,
        "design": {"bytes": text.len()},
        "gates_passed": draft_ok && export_ok && thickness_ok,
    });
    std::fs::write(out.join(if draft { "report-draft.json" } else { "report.json" }), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        let pattern = mesh::try_build_pattern(&d, &lib, export_params())?;
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Gurgulio / lost-wax pattern")?;
    }
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if draft_ok && export_ok && thickness_ok { "all pass" } else { "FAIL" });
    ensure!(!text.is_empty());
    Ok(())
}
