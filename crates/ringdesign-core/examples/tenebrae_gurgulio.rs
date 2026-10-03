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
    field::{Blend, Layer, LayerEntry},
    library, manufacturing as mf, mesh,
    profile::ShankKey,
    render,
    sculpt::{self, ellipsoid, round_cone, smax, smin},
    stl,
    svg::SvgAlpha,
    tiling::TilingLayer,
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
const BAND_T: f64 = 2.4;

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
const ARCH_W: f64 = 0.72;
const ARCH_Y: [f64; 2] = [12.2, 12.8];
const ARCH_DEPTH: f64 = 0.6;
/// The quatrefoil roundels on the wall's sides: lobe radius, lobe offset, centre height.
const QUATRE_R: f64 = 0.5;
const QUATRE_O: f64 = 0.55;
const QUATRE_Y: f64 = 13.2;

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
/// The spout pipe: its angle in the head's frame (with the head raised 38 degrees, 12.5 above the finger's line), and
/// its length, head units.
const SPOUT_DEG: f64 = -25.5;
const SPOUT_LEN: f64 = 1.9;
/// The thinnest round any limb, horn or claw tapers to, in the figure's frame (x 1.5 in the world: a 1.26 mm section).
const TIP_R: f64 = 0.42;

/// The meshing step and the face budget of the sculpt.
const STEP_MM: f64 = 0.065;
const FACES: usize = 245_000;
/// The fillet the part grows out of the band with, mm.
const FILLET_MM: f64 = 0.7;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
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
    d.shank.keys = vec![key(30.0, 1.12, 1.0), key(55.0, 1.5, 1.03), key(66.0, 1.85, 1.05), key(118.0, 1.85, 1.05), key(130.0, 1.5, 1.03), key(155.0, 1.12, 1.0), key(270.0, 1.0, 1.0)];
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

// --- The band: the lead gutter and the nave's arcade -------------------------------------------------------------

/// The gutter along the crown's centre line: its width and depth, mm.
const GUTTER_W: f64 = 0.9;
const GUTTER_MM: f64 = 0.4;
/// The blind arcade down both side walls: bays round the ring, and how far its ribs stand, mm.
const ARCADE_BAYS: u32 = 33;
const ARCADE_MM: f64 = 0.3;
/// Every rib of the arcade is this wide as drawn, mm (the tile is laid at the crown's circumference, so on the side wall
/// a rib narrows a little with the radius).
const RIB_MM: f64 = 0.95;

/// One bay of the arcade as an SVG tile `w` by `h` mm: the ribs stand (ink) and a pointed lancet opening is left
/// between them. The mullion between two openings and the head over each are each at
/// least `RIB_MM` wide.
fn arcade_svg(w: f64, h: f64) -> String {
    // The tile's top edge lies on the bore side of the wall, so each opening runs off it and its head points out
    // toward the crown, with a rib of `RIB_MM` left over the apex.
    let (cx, half) = (0.5 * w, 0.5 * (w - RIB_MM));
    let apex = h - RIB_MM;
    // A lancet head: each side an arc of 1.5 times the half-span, centred inside the opposite half, over short jambs.
    let r = 1.5 * half;
    let spring = apex - (r * r - (r - half) * (r - half)).sqrt();
    let path = format!(
        "M{:.4} -0.5 L{:.4} {spring:.4} A{r:.4} {r:.4} 0 0 0 {cx:.4} {apex:.4} A{r:.4} {r:.4} 0 0 0 {:.4} {spring:.4} L{:.4} -0.5 Z",
        cx - half, cx - half, cx + half, cx + half
    );
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><rect x="0" y="0" width="{w:.4}" height="{h:.4}" fill="#000"/><path d="{path}" fill="#fff"/></svg>"##
    )
}

/// The band's own ornament: the lead gutter sunk along the crown's centre line all the way round, so the water the
/// spout throws has its source, and the nave's blind arcade standing on both side walls from the parapet to the palm.
fn band_ornament(d: &mut RingDesign, lib: &mut AlphaLibrary) -> Result<serde_json::Value> {
    let ctx = d.field_context();
    let faces = ctx.side_faces_std().ok_or_else(|| anyhow::anyhow!("the band has no side faces"))?;
    let (lo, hi) = (faces.low.ok_or_else(|| anyhow::anyhow!("no low side face"))?, faces.high.ok_or_else(|| anyhow::anyhow!("no high side face"))?);
    // The gutter: a plain stripe sunk along the middle of the crown.
    let mut g = TilingLayer::default_for("Lead gutter", &ctx);
    g.v_center_mm = 0.5 * (lo.1 + hi.0);
    g.v_span_mm = GUTTER_W;
    g.repeats_around = 24;
    g.rows = 1;
    g.height_mm = GUTTER_MM;
    g.feather_mm = 0.0;
    let (gw, gh) = g.cell_size(&ctx);
    let stripe = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{gw:.4}" height="{gh:.4}" viewBox="0 0 {gw:.4} {gh:.4}"><rect x="-1" y="0" width="{:.4}" height="{gh:.4}" fill="#000"/></svg>"##, gw + 2.0);
    d.svgs.push(SvgAlpha { name: "Lead gutter".into(), svg: stripe, invert: false });
    let mut e = LayerEntry::new("Lead gutter", Layer::Tiling(g));
    e.blend = Blend::Subtract;
    d.layers.layers.push(e);
    // The arcade: one bay a tile on the low side wall, mirrored onto the high one.
    let mut t = TilingLayer::default_for("Nave arcade", &ctx);
    t.v_center_mm = 0.5 * (lo.0 + lo.1);
    t.v_span_mm = (lo.1 - lo.0) - 0.1;
    t.repeats_around = ARCADE_BAYS;
    t.rows = 1;
    t.height_mm = ARCADE_MM;
    t.feather_mm = 0.0;
    t.mirror_v = true;
    let (cw, ch) = t.cell_size(&ctx);
    d.svgs.push(SvgAlpha { name: "Nave arcade".into(), svg: arcade_svg(cw, ch), invert: false });
    let mut e = LayerEntry::new("Nave arcade", Layer::Tiling(t));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    d.crisp_relief = true;
    d.bake_all(lib);
    Ok(json!({"gutter": {"width_mm": GUTTER_W, "depth_mm": GUTTER_MM, "crown_v_mm": [lo.1, hi.0]}, "arcade": {"bays": ARCADE_BAYS, "cell_mm": [cw, ch], "rib_mm": RIB_MM, "stands_mm": ARCADE_MM, "low_face_v_mm": [lo.0, lo.1]}}))
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
    // The blind arcade: three pointed lancets on the front and back of the wall, and on each side one large blind
    // quatrefoil, its lobes run together with a 0.16 round so no cusp is a knife wedge of wall.
    let mut recess = f64::MAX;
    let mut sink = |n: f64, face: f64, across: f64, pitch: f64, foil: bool| {
        for k in -1..=1 {
            if foil && k != 0 {
                continue;
            }
            let a = across - k as f64 * pitch;
            let shape = if foil {
                let lobes = [[QUATRE_O, 0.0], [-QUATRE_O, 0.0], [0.0, QUATRE_O], [0.0, -QUATRE_O]].map(|c| (a - c[0]).hypot(p[1] - QUATRE_Y - c[1]) - QUATRE_R);
                smin(smin(lobes[0], lobes[1], 0.16), smin(lobes[2], lobes[3], 0.16), 0.16)
            } else {
                pointed_arch(a, p[1], ARCH_W, ARCH_Y[0], ARCH_Y[1])
            };
            recess = recess.min(shape.max(face - ARCH_DEPTH - n));
        }
    };
    // Pitched so every pier between two bays, and from the outer ones to the chamfered corners, is over 0.85 mm.
    let xp = 0.8 * 2.0 * WALL_HX / 3.0;
    let zp = 0.88 * (WALL_Z[1] - WALL_Z[0]) / 3.0;
    sink(p[2], WALL_Z[1], p[0], xp, false);
    sink(-p[2], -WALL_Z[0], p[0], xp, false);
    sink(p[0], WALL_HX, p[2] - mid(WALL_Z), zp, true);
    sink(-p[0], WALL_HX, p[2] - mid(WALL_Z), zp, true);
    d = smax(d, -recess, 0.05);
    d.min(pinnacles(p))
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
    let hw = 0.5 + (PINNACLE_HALF * 0.85 - 0.5) * (1.0 - t);
    let spire = ((x.abs().max(z.abs()) - hw) / (1.0 + ((PINNACLE_HALF * 0.85 - 0.5) / PINNACLE_SPIRE).powi(2)).sqrt()).max(PINNACLE_SHAFT - 0.1 - y).max(y - PINNACLE_SHAFT - PINNACLE_SPIRE);
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
const NECK: P3 = [3.5, 2.6, 0.0];

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
/// hard brow ridge over deep sockets, cheekbones, a short broad snout creased with snarl furrows, the jaws gaping wide
/// over a bored lead spout, two fangs, short horns and pointed ears.
fn head(q: P3) -> f64 {
    let at = |c: P3| sub(q, c);
    let cranium = ellipsoid(at([0.5, 0.8, 0.0]), [1.1, 1.05, 1.2]);
    // A broad, flat face under a heavy, hard brow ridge, wide at the jowls, with cheekbones.
    let face = ellipsoid(at([1.35, 0.45, 0.0]), [0.75, 1.05, 1.3]);
    let brow = ellipsoid(at([1.9, 1.4, 0.0]), [0.52, 0.36, 1.25]);
    let brow_l = ellipsoid(at([2.08, 1.3, 0.6]), [0.55, 0.42, 0.6]);
    let cheekbone = ellipsoid(at([1.75, 0.55, 0.95]), [0.5, 0.35, 0.4]);
    let jowl = ellipsoid(at([1.05, -0.35, 0.95]), [0.65, 0.7, 0.42]);
    let nose = ellipsoid(at([2.18, 0.62, 0.0]), [0.42, 0.38, 0.6]);
    let lip = ellipsoid(at([1.95, 0.12, 0.0]), [0.58, 0.36, 1.05]);
    let jaw = ellipsoid(at([1.6, -1.02, 0.0]), [0.85, 0.48, 1.0]);
    let chin = ellipsoid(at([2.15, -1.05, 0.0]), [0.4, 0.38, 0.6]);
    let horn = chain(q, &[([0.8, 1.55, 0.6], 0.45), ([0.25, 2.1, 0.72], 0.32), ([-0.35, 2.05, 0.75], 0.22)], 0.08);
    let ear = round_cone(q, [0.35, 1.1, 1.05], [-0.05, 1.8, 1.6], 0.36, 0.22);
    let mut d = smin(cranium, face, 0.4);
    d = smin(d, brow, 0.15);
    d = smin(d, brow_l, 0.12);
    d = smin(d, cheekbone, 0.15);
    d = smin(d, jowl, 0.3);
    d = smin(d, nose, 0.2);
    d = smin(d, lip, 0.2);
    d = smin(d, jaw, 0.3);
    d = smin(d, chin, 0.15);
    d = smin(d, horn, 0.15);
    d = smin(d, ear, 0.15);
    // Deep sockets under the brow, each centred on the face's surface so its rim stands square, and the eye set back in
    // it.
    let socket = len(sub(q, [2.2, 0.8, 0.58])) - 0.23;
    d = smax(d, -socket, 0.05);
    // Furrows: a snarl crease across the bridge of the snout and two down each cheek, each a round groove sunk about
    // 0.1 units (0.3 mm) with its centre on the surface.
    let furrows = [
        round_cone(q, [2.62, 0.95, -0.15], [2.62, 0.95, 0.15], 0.08, 0.08),
        round_cone(q, [1.95, 0.45, 1.2], [2.25, -0.1, 0.92], 0.1, 0.09),
        round_cone(q, [1.7, 0.1, 1.28], [1.95, -0.45, 1.08], 0.1, 0.09),
    ];
    for g in furrows {
        d = smax(d, -g, 0.04);
    }
    // The gape: a wide oval mouth opening forward between the lip and the dropped jaw, running back into the throat.
    let gape = ellipsoid(at([2.2, -0.4, 0.0]), [0.8, 0.3, 0.62]);
    d = smax(d, -gape, 0.08);
    // The spout: a lead pipe lying out of the jaws a little above the finger's line, its end cut square and bored.
    let dir = [(SPOUT_DEG.to_radians()).cos(), (SPOUT_DEG.to_radians()).sin(), 0.0];
    let root = [1.55, -0.72, 0.0];
    let tip = add(root, mul(dir, SPOUT_LEN));
    let pipe = round_cone(q, root, tip, 0.4, 0.4).max(dot(sub(q, tip), dir));
    d = smin(d, pipe, 0.1);
    let into = dot(sub(q, tip), dir);
    let radial = len(sub(sub(q, tip), mul(dir, into)));
    let bore = (radial - 0.12).max(-0.55 - into);
    d = smax(d, -bore, 0.02);
    // Two fangs down from the lip, tapering to points, standing clear in the gape over the pipe.
    let fang = round_cone(q, [2.3, -0.05, 0.42], [2.42, -0.42, 0.4], 0.22, 0.14);
    d = smin(d, fang, 0.06);
    let eye = len(sub(q, [2.06, 0.78, 0.56])) - 0.18;
    d.min(eye)
}

/// The height of the beast's back along the spine, figure units: the folded wings lie on it.
fn back_top(f: f64) -> f64 {
    const KNOTS: [(f64, f64); 7] = [(-3.3, 2.9), (-2.6, 3.65), (-2.0, 4.35), (-1.4, 4.95), (-0.9, 5.3), (0.1, 5.6), (1.4, 5.0)];
    if f <= KNOTS[0].0 {
        return KNOTS[0].1;
    }
    for w in KNOTS.windows(2) {
        if f <= w[1].0 {
            let t = (f - w[0].0) / (w[1].0 - w[0].0);
            return w[0].1 + (w[1].1 - w[0].1) * t * t * (3.0 - 2.0 * t);
        }
    }
    KNOTS[KNOTS.len() - 1].1
}

/// The wings' slope from the spine down the flank, degrees, where they leave the spine, and how high they lie on it.
const WING_SLOPE_DEG: f64 = 35.0;
const WING_S0: f64 = 0.5;
const WING_LIFT: f64 = 0.05;
/// A folded wing's outline on its slope: `f` along the body and `b` down the slope from the spine. The arm runs along
/// the spine to the wrist at the shoulder; four fingers lie back from it to tips down the flank, and the trailing
/// edge is scalloped between them.
const WING_OUTLINE: [[f64; 2]; 11] = [[1.5, 0.0], [-3.1, 0.05], [-3.05, 0.85], [-2.45, 1.0], [-2.55, 1.7], [-1.75, 1.6], [-1.65, 2.25], [-0.8, 1.95], [-0.4, 2.35], [0.45, 1.55], [1.5, 0.75]];
const WING_WRIST: [f64; 2] = [1.3, 0.35];
const WING_TIPS: [[f64; 2]; 4] = [[-3.05, 0.85], [-2.55, 1.7], [-1.65, 2.25], [-0.4, 2.35]];
/// Half the membrane's thickness, and the bones' round: the membranes lie 0.17 units (0.32 mm) below the bones.
const MEMBRANE_HALF: f64 = 0.36;
const BONE_R: f64 = 0.53;

/// One folded wing (the right; the field folds `s`): a membrane slab on the slope over the back, its bones low ribs on
/// it and a claw at the wrist.
fn wing(q: P3) -> f64 {
    let (sn, cs) = WING_SLOPE_DEG.to_radians().sin_cos();
    let (f, u, s) = (q[0], q[1], q[2]);
    // The slope's height over (f, s), and a point's place on it and off it.
    let plane_u = |f: f64, s: f64| back_top(f) + WING_LIFT - (s - WING_S0) * sn / cs;
    let b = (s - WING_S0) / cs;
    // The slab's true normal distance: the slope falls along the body as well as across it.
    let gf = (back_top(f + 0.01) - back_top(f - 0.01)) / 0.02;
    let off = (u - plane_u(f, s)) / (1.0 + gf * gf + (sn / cs).powi(2)).sqrt();
    let d2 = polygon([f, b], &WING_OUTLINE);
    let w = [d2 + 0.2, off.abs() - MEMBRANE_HALF + 0.2];
    let membrane = w[0].max(w[1]).min(0.0) + w[0].max(0.0).hypot(w[1].max(0.0)) - 0.2;
    let on = |a: [f64; 2]| {
        let s = WING_S0 + a[1] * cs;
        [a[0], plane_u(a[0], s), s]
    };
    let wrist = on(WING_WRIST);
    let mut d = smin(membrane, round_cone(q, on([1.5, 0.15]), wrist, BONE_R + 0.06, BONE_R + 0.04), 0.1);
    for tip in WING_TIPS {
        d = smin(d, round_cone(q, wrist, on(tip), BONE_R, BONE_R - 0.04), 0.1);
    }
    // The wrist's claw, hooked up over the shoulder.
    smin(d, chain(q, &[(wrist, BONE_R), (add(wrist, [0.25, 0.55, 0.05]), 0.32), (add(wrist, [0.5, 0.75, 0.0]), 0.24)], 0.06), 0.08)
}

/// The beast's skin on the back and shoulders: a shallow chiselled pebbling, figure units.
fn chisel(q: P3) -> f64 {
    let k = 8.5;
    0.05 * (k * q[0]).sin() * (k * q[1] + 0.7).sin() * (k * q[2] + 1.3).sin()
}

/// The gargoyle's field in figure units: a heavy hunched lump crouched on its haunches, the forelegs braced down to
/// the edge, the head thrust out past it.
fn figure(fr: P3) -> f64 {
    let (f, u, s) = (fr[0], fr[1], fr[2]);
    let q = [f, u, s.abs()];
    // The body: one heavy crouched mass, the pelvis low behind rising to a hunched back and deep chest, chiselled.
    let torso = round_cone(q, [-1.4, 1.75, 0.0], [0.5, 3.0, 0.0], 1.9, 2.05);
    let hump = ellipsoid(sub(q, [0.1, 4.15, 0.0]), [1.75, 1.45, 1.9]);
    let chest = ellipsoid(sub(q, [1.35, 2.3, 0.0]), [1.25, 1.55, 1.55]);
    let neck = round_cone(q, [1.3, 3.6, 0.0], NECK, 1.3, 0.92);
    let mut body = smin(torso, hump, 0.6);
    body = smin(body, chest, 0.6);
    body = smin(body, neck, 0.5);
    body += chisel(q);
    // The head, scaled about the neck's end and raised.
    let hq = sculpt::turn(mul(sub(q, NECK), 1.0 / HEAD_SCALE), 0, 1, -HEAD_LIFT_DEG);
    body = smin(body, head(hq) * HEAD_SCALE, 0.35);
    // Forelegs braced down to the edge, tapering from the heavy shoulder to the elbow, a knobbed wrist, the paw, and
    // three hooked claws over the coping's front edge, gripping it (sunk a little into its face, so no film of metal
    // is left between claw and stone).
    let mut limbs = ellipsoid(sub(q, [1.0, 2.9, 1.55]), [1.05, 1.25, 0.85]);
    limbs = smin(limbs, chain(q, &[([1.2, 2.8, 1.65], 0.95), ([2.0, 1.45, 1.95], 0.72), ([FRONT_F - 0.75, 0.8, 1.75], 0.5)], 0.2), 0.35);
    limbs = smin(limbs, ellipsoid(sub(q, [FRONT_F - 0.75, 0.82, 1.75]), [0.42, 0.42, 0.55]), 0.1);
    limbs = smin(limbs, ellipsoid(sub(q, [FRONT_F - 0.3, 0.5, 1.7]), [0.55, 0.4, 0.75]), 0.15);
    let mut claws = f64::MAX;
    for sk in [1.15, 1.7, 2.25] {
        let knuckle = [FRONT_F + 0.02, 0.56, sk];
        let claw = chain(q, &[([FRONT_F - 0.45, 0.5, 1.7 + 0.8 * (sk - 1.7)], 0.36), (knuckle, 0.36), ([FRONT_F + 0.06, -0.02, sk], 0.31), ([FRONT_F + 0.1, -0.4, sk], 0.27)], 0.05);
        claws = claws.min(claw);
    }
    limbs = smin(limbs, claws, 0.1);
    // Hind legs folded high against the flanks: a heavy haunch, the shin back down, the foot forward on the coping.
    limbs = smin(limbs, ellipsoid(sub(q, [-0.7, 2.0, 1.75]), [1.5, 1.45, 0.85]), 0.45);
    limbs = smin(limbs, chain(q, &[([0.4, 1.5, 2.2], 0.8), ([-0.9, 0.55, 2.3], 0.6)], 0.15), 0.5);
    let mut toes = f64::MAX;
    for st in [1.7, 2.2, 2.7] {
        toes = toes.min(round_cone(q, [-0.85, 0.5, 2.3], [0.5, TIP_R, st], 0.5, TIP_R));
    }
    limbs = smin(limbs, toes, 0.1);
    let mut fig = smin(body, limbs, 0.45);
    // The folded wings along the back, one each side of the spine.
    fig = smin(fig, wing(q), 0.12);
    // The tail, wrapped forward round one haunch into the foot, lying a little sunk in the coping so no film of metal
    // is left under it (not mirrored).
    let fs = [f, u, s];
    let tail = chain(fs, &[([-2.3, 1.0, 0.0], 0.62), ([-2.85, 0.4, -1.3], 0.52), ([-2.3, TIP_R - 0.08, -2.6], 0.47), ([-1.3, TIP_R - 0.12, -2.8], TIP_R)], 0.1);
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
    ([-COPING_HX - 0.6, 9.0, COPING_Z[0] - 0.6], [COPING_HX + 0.6, COPING_Y[1] + FIG_SCALE * 8.0, head_tip + 0.6])
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
    #[serde(default)]
    band: serde_json::Value,
    #[serde(default)]
    inner_shells_dropped: Vec<(usize, f64)>,
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

/// The meshing's largest connected shell, and the others dropped (their face counts and enclosed volumes, mm³). The
/// beast and its parapet are one solid; `tetra_mesh` skips blocks far from the surface, and deep in this heavy body it
/// can close a few sealed shells round skipped blocks, voids the field does not have (it reads solid there).
fn largest_shell(m: &csg::Solid) -> (csg::Solid, Vec<(usize, f64)>) {
    let n = m.v.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for t in &m.f {
        for k in 0..2 {
            let (a, b) = (find(&mut parent, t[k] as usize), find(&mut parent, t[k + 1] as usize));
            if a != b {
                parent[a] = b;
            }
        }
    }
    let root: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut faces: std::collections::BTreeMap<usize, (usize, f64)> = Default::default();
    for t in &m.f {
        let [a, b, c] = t.map(|i| m.v[i as usize]);
        let vol = (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
        let e = faces.entry(root[t[0] as usize]).or_default();
        e.0 += 1;
        e.1 += vol;
    }
    let keep = faces.iter().max_by_key(|(_, (f, _))| *f).map(|(r, _)| *r).unwrap_or(0);
    let dropped = faces.iter().filter(|(r, _)| **r != keep).map(|(_, v)| *v).collect();
    let mut index = vec![u32::MAX; n];
    let mut v = Vec::new();
    for i in 0..n {
        if root[i] == keep {
            index[i] = v.len() as u32;
            v.push(m.v[i]);
        }
    }
    let f = m.f.iter().filter(|t| root[t[0] as usize] == keep).map(|t| t.map(|i| index[i as usize])).collect();
    (csg::Solid { v, f }, dropped)
}

fn sculpt_fresh(comp: &mut Composition) -> Result<csg::Solid> {
    let t = std::time::Instant::now();
    let (lo, hi) = field_box();
    comp.box_mm = [lo, hi];
    let fld = |p: P3| field(p);
    let raw = sculpt::tetra_mesh(lo, hi, STEP_MM, &fld);
    let (raw, dropped) = largest_shell(&raw);
    comp.inner_shells_dropped = dropped;
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
    let mut lib = AlphaLibrary::builtin();
    let mut comp = Composition::default();
    let solid = sculpt_solid(&mut comp)?;
    comp.band = band_ornament(&mut d, &mut lib)?;
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
    // The lost-wax wall gate: the whole-mesh census of the finished ring at the 0.8 mm floor, every sub-floor reading
    // classed edge (a tip or lip closing at a free edge) or wall (a web, fin or sheet that stays thin).
    let t = std::time::Instant::now();
    let wall = cad::measure::thickness(&built.mesh, MIN_SECTION_MM);
    println!(
        "  wall census on {}x{}: assessed {}, {} samples, {} unresolved, {} wall ({:.3} mm2), {} edge ({:.3} mm2), thinnest {:?} ({:.1} s)",
        params.theta_steps, params.profile_steps, wall.assessed, wall.rays, wall.unresolved, wall.below_limit, wall.wall_area_mm2, wall.edge_below_limit, wall.edge_area_mm2, wall.sampled_min_mm, t.elapsed().as_secs_f64()
    );
    // Until the census's sliver readings are fixed (lead, 2026-10-03): a wall zone 0.05-0.8 mm thick is a real section
    // to fix; one under 0.05 mm is listed as a suspected census artifact, with its point and area, and not reshaped.
    let real_walls: Vec<&cad::measure::ThinZone> = wall.walls.iter().filter(|z| z.thinnest_mm >= 0.05).collect();
    let artifacts: Vec<serde_json::Value> = wall.walls.iter().filter(|z| z.thinnest_mm < 0.05).map(|z| json!({"point": z.point, "area_mm2": z.area_mm2, "thinnest_mm": z.thinnest_mm})).collect();
    for z in wall.walls.iter().take(12) {
        println!("    wall {:.3} mm, {:.3} mm2 at {:?}", z.thinnest_mm, z.area_mm2, z.point.map(|v| (v * 100.0).round() / 100.0));
    }
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
        ("cold reload identical", cold == Some(true)),
        ("wall census at 0.8 mm (the lead's interim gate): assessed, 0 unresolved, no wall zone a real section of 0.05-0.8 mm", wall.assessed && wall.unresolved == 0 && real_walls.is_empty()),
    ];
    for (g, p) in &gates {
        println!("    {} {g}", if *p { "pass" } else { "FAIL" });
    }
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    let all = gates.iter().all(|(_, p)| *p);
    // The band alone would pull from sand, and the field says so; with the part on it, it would not. Keep the notes
    // about the ring as built.
    let notes: Vec<&String> = fieldr.notes.iter().filter(|n| !(fieldr.undercut_fraction() > 0.0 && n.contains("would also pull"))).collect();
    let v = json!({
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "build_s": build_s},
        "geometry": pass.json(),
        "sculpt": {"open_edges": open_edges, "self_crossings": sculpt_crossings, "faces": solid.f.len()},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": fieldr.verdict.label(), "band_verdict": band_field.verdict.label(), "thinnest_wall_mm": band_field.thinnest_wall_mm, "notes": notes},
        "thickness_0_8": wall,
        "thickness_0_8_clean": wall.clean(),
        "suspected_census_artifacts": artifacts,
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

/// The finished mesh as the renders shade it. The beast is a decimated distance field: its crease normals draw the
/// meshing's sub-tenth folds as flecks. Faces of the figure itself, standing clear above the coping (the pinnacles
/// aside), are shaded with the mesh's smooth vertex normals; the parapet, the pinnacles and the band keep their crease
/// normals, so their arrises stay hard. Geometry is untouched: only the shading normals differ.
fn shading_mesh(m: &mesh::Mesh) -> mesh::Mesh {
    let mut out = m.clone();
    let pinnacle_x = COPING_HX - 2.0 * PINNACLE_HALF - 0.4;
    let pinnacle_z = COPING_Z[0] + 2.0 * PINNACLE_HALF + 0.4;
    let free = |i: u32| {
        let v = m.vertices[i as usize];
        let (x, y, z) = (v.0 as f64, v.1 as f64, v.2 as f64);
        y > COPING_Y[1] + 0.05 && !(x.abs() > pinnacle_x && z < pinnacle_z)
    };
    out.corner_normals.retain(|(fi, _)| !m.faces[*fi as usize].iter().all(|&i| free(i)));
    out
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let shaded = shading_mesh(&fin.metal);
    let mut parts = vec![render::Part::metal(&shaded, render::GOLD)];
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
    if let Ok(at) = std::env::var("GURGULIO_PROBE") {
        let v: Vec<f64> = at.split(',').filter_map(|x| x.parse().ok()).collect();
        let p = [v[0], v[1], v[2]];
        let fr = frame(p);
        let q = [fr[0], fr[1], fr[2].abs()];
        let hq = sculpt::turn(mul(sub(q, NECK), 1.0 / HEAD_SCALE), 0, 1, -HEAD_LIFT_DEG);
        println!("  field {:.3}, figure {:.3}, parapet {:.3}, wing {:.3}, head {:.3}, chisel {:.3}, frame {:?}", field(p), figure(fr), parapet(p), wing(q), head(hq) * HEAD_SCALE, chisel(q), fr);
        return Ok(());
    }
    if let Ok(step) = std::env::var("GURGULIO_PREVIEW") {
        // A quick look while shaping: the field meshed coarse beside the bare band, no join and no gates.
        let step: f64 = step.parse().unwrap_or(0.12);
        let (lo, hi) = field_box();
        let fld = |p: P3| field(p);
        let t = std::time::Instant::now();
        let s = sculpt::tetra_mesh(lo, hi, step, &fld);
        println!("  preview mesh {} faces in {:.1} s", s.f.len(), t.elapsed().as_secs_f64());
        let (mut bd, mut bl) = (band(), AlphaLibrary::builtin());
        band_ornament(&mut bd, &mut bl)?;
        let bare = mesh::try_build(&bd, &bl, BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() })?;
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
        let shots: [(f64, f64, [f64; 3], f64); 6] = [(0.0, 0.0, head_c, 4.0), (0.9, 0.5, head_c, 4.0), (0.0, PI * 0.5, centre, 11.0), (0.4, 0.7, centre, 11.0), (PI * 0.5, PI * 0.5, [4.4, 13.2, -0.5], 3.5), (0.0, 0.0, [0.0, -10.8, 2.5], 2.5)];
        let mut sheet = vec![0u8; 1200 * 800 * 3];
        for (k, (yaw, pitch, c, hw)) in shots.iter().enumerate() {
            let img = render::render_parts_framed(&parts, *yaw, *pitch, render::Framing::new(*c, *hw), 400, 400, 2);
            paste(&mut sheet, 1200, &img, 400, (k % 3) * 400, (k / 3) * 400);
        }
        image::save_buffer(out.join("preview-figure.png"), &sheet, 1200, 800, image::ColorType::Rgb8)?;
        if let Ok(look) = std::env::var("GURGULIO_LOOK") {
            // A close look anywhere: "x,y,z,half_width,yaw,pitch".
            let v: Vec<f64> = look.split(',').filter_map(|x| x.parse().ok()).collect();
            render::write_png_framed(out.join("look.png"), &parts, v[4], v[5], render::Framing::new([v[0], v[1], v[2]], v[3]), 800)?;
        }
        return Ok(());
    }
    let started = std::time::Instant::now();
    let (d, lib, comp, solid) = author()?;
    let author_s = started.elapsed().as_secs_f64();
    println!(
        "  sculpt {} raw faces -> {}, {:.1} mm3, {:.1} s; figure {:.2} mm over the coping, head {:.2} mm past its edge",
        comp.raw_faces, comp.faces, comp.volume_mm3, comp.sculpt_s, comp.figure_height_over_coping_mm, comp.head_past_coping_edge_mm
    );
    // The sculpted part's own sections, as `dfm::part_sections` reads them: reported only, since the wall gate is the
    // census on the finished ring (master PR #261), which tells a tip or a lip from a wall.
    let (sec_min, sec_under) = dfm::part_sections(&solid, None, MIN_SECTION_MM);
    // The draft build's gates always, its cold reload included; the export build's on top unless --draft.
    let (draft_gates, draft_ok, draft_built) = gate_block(&d, &lib, &solid, draft_params(), true)?;
    let (export_gates, export_ok, built) = if draft {
        (serde_json::Value::Null, true, draft_built)
    } else {
        let (g, ok, b) = gate_block(&d, &lib, &solid, export_params(), true)?;
        (g, ok, b)
    };
    let _ = verify;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let report = json!({
        "name": d.name,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "draft": {"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm},
        "gates_that_apply": "lost wax: geometry, every feature Ok, bore, field fill verdict, the wall census cad::measure::thickness(&built.mesh, 0.8).clean() on each build (its edge zones reported as read), dfm::cut_lands at 0.8 mm, DFM, stones, pattern, triangles, cold reload at both builds. The sand gates (ray release, draft-clamp bites, two-part Castable) do not apply; the two-part undercut is reported as a number.",
        "size": d.size.display(),
        "author_s": author_s,
        "draft_build": draft_gates,
        "export_build": export_gates,
        "part_sections_0_8": {"thinnest_mm": sec_min, "under_floor_mm2": sec_under, "gate": false, "method": "dfm::part_sections: one ray per face of the sculpted part alone (its foot buried in the band included) along its inward normal, every reading counted, tips and lips too. Reported only: the wall gate is cad::measure::thickness on the finished ring (master PR #261), each build's thickness_0_8."},
        "composition": comp,
        "design": {"bytes": text.len()},
        "gates_passed": draft_ok && export_ok,
    });
    std::fs::write(out.join(if draft { "report-draft.json" } else { "report.json" }), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        let pattern = mesh::try_build_pattern(&d, &lib, export_params())?;
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Gurgulio / lost-wax pattern")?;
    }
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if draft_ok && export_ok { "all pass" } else { "FAIL" });
    ensure!(!text.is_empty());
    Ok(())
}
