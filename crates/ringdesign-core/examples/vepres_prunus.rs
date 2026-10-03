//! Vepres — Prunus, *Straif, the blackthorn*: a black sloe in the cushion's face, a thorned twig down both
//! shoulders on the parting line, blossom on the head's walls, poured in Delft sand on the 012 Cushion master.
//! cargo build --release -p ringdesign-core --example vepres_prunus
//! target/release/examples/vepres_prunus [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{self, Attach, Component, Document, Feature, Operation, Placement, Stage, SurfaceKind, stored},
    castability, csg, dfm,
    field::{Blend, BorderLayer, BorderProfile, Layer, LayerEntry, SeatPadLayer, SeatStyle, Window},
    gem::{Gem, GemCut, GemForm},
    library, manufacturing as mf, mesh, outline, render,
    setting::{SolidKind, Stamp, StampTop},
    sketch::{Geometry, Sketch, Workplane},
    skin::{self, Atlas, Hide, Sample, draft_clamp},
    stl,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

type P2 = [f64; 2];
type P3 = [f64; 3];

const AW: usize = 2048;
const AH: usize = 768;

/// The sloe: a round onyx cabochon filling the cushion's face.
const SLOE_MM: f64 = 7.0;
/// The sloe's low boss, and its calyx: its hole round the stone, its collar, each sepal's reach past it and half width
/// in radians, where it starts down the boss's blend, how the five are turned, and its thickness at most.
const SLOE_BOSS_MM: f64 = 0.15;
const CALYX_HOLE_MM: f64 = 3.42;
const CALYX_COLLAR_MM: f64 = 3.75;
const CALYX_SEPAL_MM: f64 = 1.8;
const CALYX_SEPAL_HALF_RAD: f64 = 0.3;
const CALYX_DRAPE_MM: f64 = 4.0;
const CALYX_TURN_DEG: f64 = 18.0;
const CALYX_THICK_MM: f64 = 0.36;
/// Blue-black, a sloe's bloom over black.
const SLOE_TINT: [f32; 3] = [0.03, 0.034, 0.055];
/// The studio's antiquing: how far the surface is relaxed, the depths past which a facet is shaded half dark, then dark, and those two tints.
const ANTIQUE_PASSES: usize = 60;
const ANTIQUE_SHALLOW_MM: f64 = 0.012;
const ANTIQUE_DEEP_MM: f64 = 0.035;
const ANTIQUE_MID: [f32; 3] = [0.30, 0.21, 0.09];
const ANTIQUE_DARK: [f32; 3] = [0.08, 0.055, 0.03];
/// The house's oxidised finish on the wood: the twig and the spurs' stubs, the bark's plates, and its furrows.
const TWIG_TINT: [f32; 3] = [0.30, 0.21, 0.09];
const BARK_PLATE_TINT: [f32; 3] = [0.17, 0.12, 0.055];
const BARK_FURROW_TINT: [f32; 3] = [0.045, 0.035, 0.02];
/// The twig on the parting line: its radius at the head, its taper, how much of it stands proud.
const TWIG_R_MM: f64 = 0.85;
const TWIG_END_SCALE: f64 = 0.42;
const TWIG_PROUD: f64 = 0.6;
/// The twig runs from beside the sloe to here along the parting line, mm, and dives into the band over its last stretch.
const TWIG_FROM_MM: f64 = 3.75;
/// On the stalk's side the twig rises out of the head's end wall, and the stalk joins it.
const TWIG_FROM_STALK_SIDE_MM: f64 = 5.2;
/// Under the sloe's boss the twig lies low, its crown under the boss's own; it rises proud over this run.
const TWIG_RISE_MM: f64 = 2.2;
const TWIG_LOW: f64 = -0.35;
const TWIG_TO_MM: f64 = 27.0;
const TWIG_DIVE_MM: f64 = 2.5;
/// The fold where the parting line turns over the head's end wall: no spur stands within a millimetre of it.
const FOLD_TURN_DEG_PER_MM: f64 = 12.0;
/// How far a spur's foot reaches under the surface, its flare and the run it flares over, and its tip's radius.
const SPUR_SINK_MM: f64 = 0.8;
const SPUR_FLARE_MM: f64 = 0.22;
const SPUR_FOOT_MM: f64 = 0.9;
const SPUR_TIP_R_MM: f64 = 0.2;
/// The blossoms soldered on the twig: shoulder (+1 toward larger ring angles), how far along the parting line, their
/// turn; and their size.
const TWIG_BLOSSOMS: [(f64, f64, f64); 5] = [(1.0, 10.6, 10.0), (1.0, 17.0, 40.0), (-1.0, 6.9, 25.0), (-1.0, 13.6, 0.0), (-1.0, 20.0, 50.0)];
const TWIG_BLOSSOM_MM: f64 = 2.9;
/// A soldered blossom's margin, the dome of its petals over that, and its heart's rise over the petals.
const BLOSSOM_MARGIN_MM: f64 = 0.14;
const BLOSSOM_DOME_MM: f64 = 0.38;
const BLOSSOM_HEART_MM: f64 = 0.16;
/// The flowers: the head walls' blossom at most, the shank's smaller one, a stamen, a bud's length and width.
const BLOSSOM_MM: f64 = 3.0;
const SMALL_BLOSSOM_MM: f64 = 1.9;
const STAMEN_MM: f64 = 0.38;
/// Stamens stand this share of the blossom's diameter out from its heart.
const STAMEN_RING: f64 = 0.335;
const BUD_MM: [f64; 2] = [1.6, 1.05];
/// The bark: its relief, the pitch of its plates across the wall and the furrow between them, and a plate's run round the ring.
const BARK_MM: f64 = 0.42;
const BARK_PITCH_MM: f64 = 1.0;
/// The bark's ground under its plates, as a share of its relief: over half, so the furrows stay bark and never read as gaps.
const BARK_GROUND: f64 = 0.56;
const FURROW_HALF_MM: f64 = 0.12;
const PLATE_RUN_MM: f64 = 2.6;
/// The crest's terraces: where they start out from the parting line, how many step down to the rim, their share of
/// the bark's relief at the twig, and the run between the cracks across them.
const CREST_FROM_MM: f64 = 0.0;
const CREST_TERRACES: usize = 3;
const CREST_RELIEF: f64 = 0.85;
const CREST_CRACK_MM: f64 = 1.9;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit2(a: P2) -> P2 {
    let l = a[0].hypot(a[1]).max(1e-12);
    [a[0] / l, a[1] / l]
}

/// One spur: where it stands along the parting line (signed by shoulder), how long, its base radius and its lean toward the twig's tip.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Spur {
    along_mm: f64,
    theta_deg: f64,
    length_mm: f64,
    base_r_mm: f64,
    stub_r_mm: f64,
    stub_len_mm: f64,
    lean_deg: f64,
    tip_diameter_mm: f64,
}

/// The five spurs of one shoulder, from the fold outward: long and short in turn, each on a knuckled stub, pitches,
/// lengths and leans varied so the row never reads as a comb.
#[derive(Clone, Copy)]
struct SpurRow {
    offset: f64,
    length: f64,
    base_r: f64,
    lean: f64,
    stub_r: f64,
    stub_len: f64,
}

fn spur_plan() -> [SpurRow; 5] {
    let row = |offset, length, base_r, lean, stub_r, stub_len| SpurRow { offset, length, base_r, lean, stub_r, stub_len };
    [
        row(0.0, 3.0, 0.48, 38.0, 0.78, 1.0),
        row(2.9, 3.0, 0.46, 52.0, 0.66, 0.8),
        row(6.1, 4.2, 0.50, 31.0, 0.80, 1.1),
        row(9.3, 2.6, 0.44, 47.0, 0.62, 0.7),
        row(12.6, 3.3, 0.46, 40.0, 0.70, 0.9),
    ]
}

/// A spur turned about its own axis, as blackthorn grows one: a knuckled side-shoot stub `stub_r` thick and `stub_len`
/// long, necking to `base_r`, then a straight taper to a blunted tip `tip_r` across at `length`, the whole flaring by
/// `flare` into a concave foot over its first `foot` mm so it grows out of the twig with its own fillet. A body of
/// revolution whose axis lies in the parting plane faces away from that plane everywhere, so the foot pulls where a seam
/// bead laid round a leaning spur does not.
fn spur_op(r: SpurRow, tip_r: f64, flare: f64, foot: f64) -> Operation {
    let mut s = Sketch { plane: Workplane::section(), ..Sketch::default() };
    s.name = "Spur section".into();
    let mut pts: Vec<P2> = vec![[0.0, -SPUR_SINK_MM], [r.stub_r + flare, -SPUR_SINK_MM]];
    let n = 40;
    for k in 0..=n {
        let h = r.length * k as f64 / n as f64;
        let run = (h - r.stub_len).max(0.0) / (r.length - r.stub_len);
        let core = r.base_r - (r.base_r - tip_r) * run;
        // The stub: a slight swelling along it, necking to the spur over half a millimetre.
        let neck = 1.0 - smooth01((h - r.stub_len + 0.25) / 0.5);
        let swell = 0.05 * (PI * (h / r.stub_len).min(1.0)).sin();
        let stub = (r.stub_r - r.base_r + swell) * neck;
        let u = (h / foot).min(1.0);
        pts.push([core + stub + flare * (1.0 - u).powi(2), h]);
    }
    // The tip is blunted with a quarter round, then closed flat on the axis.
    for k in 1..6 {
        let a = (k as f64 / 6.0) * std::f64::consts::FRAC_PI_2 * 0.92;
        pts.push([tip_r * a.cos(), r.length + tip_r * 0.9 * a.sin()]);
    }
    let last = *pts.last().unwrap();
    pts.push([0.0, last[1]]);
    let ids: Vec<_> = pts.iter().map(|p| s.point(*p)).collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    Operation::Revolve { sketch: s.into(), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false }
}

/// The sloe's calyx, made at the bench and soldered on round the fruit: a collar under the girdle spreading into five
/// pointed sepals that lie out over the boss and down onto the face, each domed along its midrib, sculpted as a closed
/// mesh in the seat's own frame (z out of the face, the boss's top at z = 0).
fn calyx_op() -> Result<Operation> {
    const AROUND: usize = 360;
    const RINGS: usize = 20;
    let outline = |phi: f64| -> (f64, f64) {
        let lobe = (0..5)
            .map(|k| {
                let c = (CALYX_TURN_DEG + 72.0 * k as f64).to_radians();
                let off = ((phi - c + PI).rem_euclid(2.0 * PI) - PI).abs();
                // A sepal: widest a third of the way out, its sides curving in to a point.
                (1.0 - (off / CALYX_SEPAL_HALF_RAD).powf(1.3)).max(0.0).powf(1.6)
            })
            .fold(0.0, f64::max);
        (CALYX_COLLAR_MM + CALYX_SEPAL_MM * lobe, lobe)
    };
    let mut positions: Vec<P3> = Vec::new();
    for layer in 0..2 {
        for j in 0..=RINGS {
            let u = j as f64 / RINGS as f64;
            for i in 0..AROUND {
                let phi = 2.0 * PI * i as f64 / AROUND as f64;
                let (outer, lobe) = outline(phi);
                let rho = CALYX_HOLE_MM + u * (outer - CALYX_HOLE_MM);
                // It lies on the boss, then drapes down its blend onto the face.
                let drape = -SLOE_BOSS_MM * smooth01((rho - CALYX_DRAPE_MM) / 0.6);
                let thick = CALYX_THICK_MM * (0.45 + 0.55 * lobe) * (1.0 - 0.6 * u * u) * (0.75 + 0.25 * (1.0 - u));
                let z = if layer == 0 { drape - 0.04 + thick } else { drape - 0.04 };
                positions.push([rho * phi.cos(), rho * phi.sin(), z]);
            }
        }
    }
    let at = |layer: usize, j: usize, i: usize| (layer * (RINGS + 1) * AROUND + j * AROUND + i % AROUND) as u32;
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for i in 0..AROUND {
        for j in 0..RINGS {
            triangles.push([at(0, j, i), at(0, j + 1, i), at(0, j + 1, i + 1)]);
            triangles.push([at(0, j, i), at(0, j + 1, i + 1), at(0, j, i + 1)]);
            triangles.push([at(1, j, i), at(1, j + 1, i + 1), at(1, j + 1, i)]);
            triangles.push([at(1, j, i), at(1, j, i + 1), at(1, j + 1, i + 1)]);
        }
        let (a, b, c, d) = (at(0, RINGS, i), at(0, RINGS, i + 1), at(1, RINGS, i), at(1, RINGS, i + 1));
        triangles.push([a, c, d]);
        triangles.push([a, d, b]);
        let (a, b, c, d) = (at(0, 0, i), at(0, 0, i + 1), at(1, 0, i), at(1, 0, i + 1));
        triangles.push([a, b, d]);
        triangles.push([a, d, c]);
    }
    let mesh = stored::Packed::encode(&positions, &triangles, &vec![0; triangles.len()], &[SurfaceKind::Freeform])?;
    let recipe = stored::Recipe { kernel: "prunus".into(), op: "calyx".into(), params: serde_json::json!({}), digest: String::new() };
    Ok(Operation::Stored { recipe, sources: Vec::new(), mesh })
}

/// An open blossom made at the bench and soldered onto the twig, sculpted as a closed mesh: five petals, each domed along
/// its midline and falling to a thin rounded margin, parted by deep notches, round a raised heart of stamens. The top is
/// a height field over a polar grid within the petal outline; a short wall and a flat foot close it.
fn blossom_op(diameter: f64) -> Result<Operation> {
    const AROUND: usize = 180;
    const RINGS: usize = 28;
    let r = 0.5 * diameter;
    let outline = |phi: f64| {
        let c = (2.5 * phi).cos();
        let lo = 0.04_f64.sqrt();
        let k = ((c * c + 0.04).sqrt() - lo) / (1.04_f64.sqrt() - lo);
        (r * (0.42 + 0.58 * k.powf(0.8)), k)
    };
    // Height over the foot at a share `u` of the way out to the outline, at angle `phi`.
    let top = |u: f64, phi: f64| {
        let (_, petal) = outline(phi);
        let dome = (1.0 - u * u).max(0.0).sqrt();
        let midline = 0.55 + 0.45 * petal;
        let heart = BLOSSOM_HEART_MM * (1.0 - smooth01(u / 0.3));
        let stamens = 0.07 * (1.0 - smooth01((u - 0.36).abs() / 0.07)) * (0.5 + 0.5 * (10.0 * phi).cos()).powi(4);
        BLOSSOM_MARGIN_MM + BLOSSOM_DOME_MM * dome * midline + heart + stamens
    };
    let mut positions: Vec<P3> = vec![[0.0, 0.0, top(0.0, 0.0)]];
    for j in 1..=RINGS {
        let u = j as f64 / RINGS as f64;
        for i in 0..AROUND {
            let phi = 2.0 * PI * i as f64 / AROUND as f64;
            let (rr, _) = outline(phi);
            positions.push([u * rr * phi.cos(), u * rr * phi.sin(), top(u, phi)]);
        }
    }
    let foot = positions.len() as u32;
    for i in 0..AROUND {
        let phi = 2.0 * PI * i as f64 / AROUND as f64;
        let (rr, _) = outline(phi);
        positions.push([rr * phi.cos(), rr * phi.sin(), 0.0]);
    }
    positions.push([0.0, 0.0, 0.0]);
    let bottom = foot + AROUND as u32;
    let ring = |j: usize, i: usize| (1 + (j - 1) * AROUND + i % AROUND) as u32;
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    for i in 0..AROUND {
        triangles.push([0, ring(1, i), ring(1, i + 1)]);
        for j in 1..RINGS {
            triangles.push([ring(j, i), ring(j + 1, i), ring(j + 1, i + 1)]);
            triangles.push([ring(j, i), ring(j + 1, i + 1), ring(j, i + 1)]);
        }
        let (a, b) = (ring(RINGS, i), ring(RINGS, i + 1));
        let (c, d) = (foot + i as u32, foot + ((i + 1) % AROUND) as u32);
        triangles.push([a, c, d]);
        triangles.push([a, d, b]);
        triangles.push([bottom, d, c]);
    }
    let mesh = stored::Packed::encode(&positions, &triangles, &vec![0; triangles.len()], &[SurfaceKind::Freeform])?;
    let recipe = stored::Recipe { kernel: "prunus".into(), op: "blossom".into(), params: serde_json::json!({ "diameter_mm": diameter }), digest: String::new() };
    Ok(Operation::Stored { recipe, sources: Vec::new(), mesh })
}

/// Five rounded petals `diameter` across their tips, one along `+x`, parted by rounded notches at `notch` of the radius.
fn petals(diameter: f64, notch: f64) -> Vec<P2> {
    let r = 0.5 * diameter;
    let lo = (0.0_f64 + 0.04).sqrt();
    let hi = (1.0_f64 + 0.04).sqrt();
    (0..200)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / 200.0;
            let c = (2.5 * a).cos();
            let k = ((c * c + 0.04).sqrt() - lo) / (hi - lo);
            let rr = r * (notch + (1.0 - notch) * k.powf(0.8));
            [rr * a.cos(), rr * a.sin()]
        })
        .collect()
}

/// A closed bud: an egg `length` by `width`, its blunt end at the origin's far side along `+x`.
fn bud(length: f64, width: f64) -> Vec<P2> {
    (0..96)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / 96.0;
            let x = 0.5 * length * a.cos();
            let y = 0.5 * width * a.sin() * (1.0 - 0.18 * a.cos());
            [x, y]
        })
        .collect()
}

/// Catmull-Rom through `pts`, as one chain of cubic Béziers in the sketch's plane.
fn bezier_chain(s: &mut Sketch, pts: &[P2]) {
    let n = pts.len();
    let mut prev = s.point(pts[0]);
    for i in 0..n - 1 {
        let p0 = pts[i.saturating_sub(1)];
        let (p1, p2) = (pts[i], pts[i + 1]);
        let p3 = pts[(i + 2).min(n - 1)];
        let c1 = if i == 0 { [p1[0] + (p2[0] - p1[0]) / 3.0, p1[1] + (p2[1] - p1[1]) / 3.0] } else { [p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0] };
        let c2 = if i + 2 >= n { [p2[0] - (p2[0] - p1[0]) / 3.0, p2[1] - (p2[1] - p1[1]) / 3.0] } else { [p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0] };
        let (a, b) = (s.point(c1), s.point(c2));
        let next = s.point(p2);
        s.entity(Geometry::Bezier { points: [prev, a, b, next] });
        prev = next;
    }
}

/// The twig of one shoulder (`sign` +1 toward the larger ring angles): its centreline in the parting plane,
/// riding the parting line `proud` of its own radius, tapering, and diving into the band at its end.
/// A round shoot on the parting line: where it runs along it, its radius at the start and its taper, how long it takes
/// to rise from `low` of its radius under the surface to `proud` over it, and the run it dives back in over at its end.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Shoot {
    from: f64,
    to: f64,
    radius: f64,
    end_scale: f64,
    rise: f64,
    low: f64,
    proud: f64,
    dive: f64,
}

/// The twig of one shoulder (`sign` +1 toward the larger ring angles).
fn twig_spec(sign: f64) -> Shoot {
    let from = if sign > 0.0 { TWIG_FROM_MM } else { TWIG_FROM_STALK_SIDE_MM };
    Shoot { from, to: TWIG_TO_MM, radius: TWIG_R_MM, end_scale: TWIG_END_SCALE, rise: TWIG_RISE_MM, low: TWIG_LOW, proud: TWIG_PROUD, dive: TWIG_DIVE_MM }
}

/// The sloe's stalk: from under the fruit's girdle out along the parting line, thickening into the twig it hangs from.
fn stalk_spec() -> Shoot {
    Shoot { from: 3.95, to: 7.6, radius: 0.36, end_scale: 1.7, rise: 1.1, low: -0.6, proud: 0.45, dive: 0.0 }
}

fn twig(a: &Atlas, hide: &Hide, sign: f64, spec: Shoot) -> Result<Operation> {
    let Shoot { from, to, radius, end_scale, rise: rise_mm, low, proud, dive: dive_mm } = spec;
    let step = 0.25;
    let n = ((to - from) / step).round() as usize;
    let crest = |l: f64| -> P2 {
        let p = hide.crest_point(a, sign * l);
        [p[0], p[1]]
    };
    // Raw crest points, offset out along the in-plane normal by the twig's proud share of its local radius.
    let mut raw: Vec<P2> = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let l = from + k as f64 * step;
        let share = (l - from) / (to - from);
        let r = radius * (1.0 + (end_scale - 1.0) * share);
        // Rise out of the face at the start and dive back into the band at the end.
        let rise = smooth01((l - from) / rise_mm);
        let dive = if dive_mm > 0.0 { smooth01((to - l) / dive_mm) } else { 1.0 };
        let off = r * (low + (proud - low) * rise) * dive + (1.0 - dive) * (-1.1 * r);
        let (p, q) = (crest(l - 0.6), crest(l + 0.6));
        let t = unit2([sign * (q[0] - p[0]), sign * (q[1] - p[1])]);
        // Outward: the tangent turned clockwise for the shoulder walking toward larger angles.
        let nrm = [t[1], -t[0]];
        let c = crest(l);
        raw.push([c[0] + nrm[0] * off, c[1] + nrm[1] * off]);
    }
    // Ease the corners the head's end wall makes: three passes of a running mean over a millimetre and a half either
    // side, the window narrowing evenly toward the ends so they stay put.
    let w = (1.5 / step) as usize;
    let mut smooth = raw.clone();
    for _ in 0..3 {
        smooth = (0..smooth.len())
            .map(|i| {
                let h = w.min(i).min(smooth.len() - 1 - i);
                let k = (2 * h + 1) as f64;
                let s = smooth[i - h..=i + h].iter().fold([0.0, 0.0], |m, p| [m[0] + p[0], m[1] + p[1]]);
                [s[0] / k, s[1] / k]
            })
            .collect();
    }
    // Knots every millimetre for the Bézier chain.
    let knots: Vec<P2> = smooth.iter().step_by(3).copied().chain(std::iter::once(*smooth.last().unwrap())).collect();
    let mut knots = knots;
    knots.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 0.2);
    let mut path = Sketch { plane: Workplane { origin: [0.0; 3], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], on_face: None }, ..Sketch::default() };
    path.name = "Twig path".into();
    bezier_chain(&mut path, &knots);
    let tightest = knots.windows(3).map(|k| {
        let (a, b, c) = ((k[1][0] - k[0][0]).hypot(k[1][1] - k[0][1]), (k[2][0] - k[1][0]).hypot(k[2][1] - k[1][1]), (k[2][0] - k[0][0]).hypot(k[2][1] - k[0][1]));
        let area = ((k[1][0] - k[0][0]) * (k[2][1] - k[0][1]) - (k[2][0] - k[0][0]) * (k[1][1] - k[0][1])).abs() * 0.5;
        a * b * c / (4.0 * area).max(1e-12)
    }).fold(f64::MAX, f64::min);
    // The chain's own tightest bend, sampled along every piece.
    let mut fine = f64::MAX;
    let mut fine_at = 0.0;
    let pts: Vec<P2> = path.points.iter().map(|p| p.xy).collect();
    for (i, e) in path.entities.iter().enumerate() {
        let Geometry::Bezier { points } = &e.geometry else { continue };
        let c: Vec<P2> = points.iter().map(|id| pts[path.points.iter().position(|p| p.id == *id).unwrap()]).collect();
        for k in 0..=40 {
            let t = k as f64 / 40.0;
            let d1: P2 = std::array::from_fn(|j| 3.0 * (1.0 - t).powi(2) * (c[1][j] - c[0][j]) + 6.0 * (1.0 - t) * t * (c[2][j] - c[1][j]) + 3.0 * t * t * (c[3][j] - c[2][j]));
            let d2: P2 = std::array::from_fn(|j| 6.0 * (1.0 - t) * (c[2][j] - 2.0 * c[1][j] + c[0][j]) + 6.0 * t * (c[3][j] - 2.0 * c[2][j] + c[1][j]));
            let cr = (d1[0] * d2[1] - d1[1] * d2[0]).abs();
            let r = d1[0].hypot(d1[1]).powi(3) / cr.max(1e-12);
            if r < fine { fine = r; fine_at = i as f64 + t; }
        }
    }
    println!("  twig {sign:+}: {} knots, tightest bend radius {tightest:.2} mm at the knots, {fine:.2} mm along the chain at piece {fine_at:.2}", knots.len());
    // The section stands square to the path at its start.
    let t0 = unit2([knots[1][0] - knots[0][0], knots[1][1] - knots[0][1]]);
    let mut section = Sketch::circle(radius);
    section.plane = Workplane { origin: [knots[0][0], knots[0][1], 0.0], x: [0.0, 0.0, 1.0], y: [t0[1], -t0[0], 0.0], on_face: None };
    Ok(Operation::Twist { sketch: section.into(), path, degrees: 0.0, end_scale: end_scale })
}

fn smooth01(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn joined(blend: f64) -> Component {
    Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: blend, ..Default::default() }
}

/// Where the head's walls (facing the pull) stand: the band of radius from the finger they cover near the head's centre.
fn wall_span(a: &Atlas, side: f64) -> (f64, f64) {
    let rho: Vec<f64> = a
        .samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && s.n[2].abs() > 0.9 && s.p[0].abs() < 1.5 && s.p[1] > 0.0)
        .map(|s| s.p[0].hypot(s.p[1]))
        .collect();
    (rho.iter().copied().fold(f64::MAX, f64::min), rho.iter().copied().fold(0.0, f64::max))
}

/// The chart point on the head's wall nearest a place given about the finger: `rho` from its axis, `phi` radians round from the head.
fn on_wall(a: &Atlas, rho: f64, phi: f64, side: f64) -> Option<(f64, f64)> {
    let (x, y) = (rho * phi.sin(), rho * phi.cos());
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && s.n[2].abs() > 0.85)
        .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
        .filter(|(d, _)| *d < 0.12)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, s)| (s.theta, s.v))
}

/// The chart point `dist` mm from a stamp's centre at `t` radians in its own plane, on the wall it stands on.
fn stamen_at(a: &Atlas, frame: &csg::Frame, t: f64, dist: f64, side: f64) -> (f64, f64) {
    let p: P3 = std::array::from_fn(|k| frame.origin[k] + frame.x[k] * dist * t.cos() + frame.y[k] * dist * t.sin());
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && s.n[2].abs() > 0.6)
        .min_by(|q, r| (q.p[0] - p[0]).hypot(q.p[1] - p[1]).total_cmp(&(r.p[0] - p[0]).hypot(r.p[1] - p[1])))
        .map(|s| (s.theta, s.v))
        .unwrap()
}

/// The bark's relief at a sample, 0 to 1. The walls stand on a bark ground at `BARK_GROUND` of the relief; plates rise
/// from it between furrows running round the ring. Walking out from the parting line a wall runs toward the bore, so
/// each plate rises gently that way and drops steeply into the furrow below it: every rise the sand's draft rule would
/// cut is laid where the wall leans enough to hold it.
fn bark_at(a: &Atlas, hide: &Hide, s: &Sample, motifs: &[(P3, f64)]) -> f64 {
    let rho = s.p[0].hypot(s.p[1]);
    // Walls that face the pull: from a little past where each side turns to face it, measured over the surface so the
    // bark comes on smoothly however the stock's facets break, to clear of the bore's edge; never on the face.
    let h = hide.at(s);
    let crest = crest_bark(h);
    let wall = smooth01((h.across.abs() - h.rim - 0.3) / 0.6) * smooth01((rho - a.bore - 0.3) / 0.35) * (1.0 - a.face(s));
    if wall <= 0.0 {
        return crest;
    }
    // A narrow polished halo round every flower and bud.
    let halo = motifs.iter().map(|(c, r)| smooth01(((s.p[0] - c[0]).hypot(s.p[1] - c[1]).hypot(s.p[2] - c[2]) - r - 0.35) / 0.3)).fold(1.0, f64::min);
    let arc = s.p[0].atan2(s.p[1]) * rho;
    // Furrows running round the ring; each wavers on its own phase, the phase running on smoothly across the wall.
    let base = (rho - a.bore) / BARK_PITCH_MM;
    let waver = 0.10 * (arc / 1.9 + 2.1 * base).sin() + 0.06 * (arc / 0.83 + 3.7 * base + 1.3).sin();
    let q = base + waver / BARK_PITCH_MM;
    let row = q.floor() as i64;
    let t = q - q.floor();
    let (below, above) = (t * BARK_PITCH_MM - FURROW_HALF_MM, (1.0 - t) * BARK_PITCH_MM - FURROW_HALF_MM);
    let plate = smooth01(below / 0.08) * smooth01(above / 0.32);
    // Plates broken round the ring at scattered cracks.
    let along = arc + 7.3 * skin::hash(row, 11) * PLATE_RUN_MM;
    let j = (along / PLATE_RUN_MM).floor() as i64;
    let boundary = |j: i64| (j as f64 + 0.35 * (skin::hash(row, j) - 0.5)) * PLATE_RUN_MM;
    let to_crack = (along - boundary(j)).abs().min((boundary(j + 1) - along).abs());
    let crack = 0.3 + 0.7 * smooth01((to_crack - 0.16) / 0.2);
    let height = 0.75 + 0.25 * skin::hash(row * 31 + 7, j);
    (wall * halo * (BARK_GROUND + (1.0 - BARK_GROUND) * plate * crack * height)).max(crest)
}

/// The bark on the crest beside the twig, off the head: terraces running round the ring that step down away from the
/// parting line, as the sand demands there, each falling a little across its width and dropping at its outer edge, cut
/// by cracks straight across them. Nothing rises walking out from the parting line, so the draft rule takes nothing.
fn crest_bark(h: skin::HidePoint) -> f64 {
    let d = h.across.abs();
    let reach = (h.rim - 0.2).max(CREST_FROM_MM + 0.3);
    if d > reach {
        return 0.0;
    }
    let f = ((d - CREST_FROM_MM) / (reach - CREST_FROM_MM)).clamp(0.0, 1.0);
    let n = CREST_TERRACES as f64;
    let k = (f * n).floor().min(n - 1.0);
    let u = f * n - k;
    // Within a terrace the level falls 35% of a step; at its edge it drops to the next over the last 12% of it.
    let level = 1.0 - (k + 0.35 * u.min(0.88) / 0.88 + 0.65 * smooth01((u - 0.88) / 0.12)) / n;
    // Off the head only, past its folds: a column's share is set by its own station, never by where it is across.
    let off_head = smooth01((h.along.abs() - 8.6) / 1.4);
    // Fissures across the terraces: each opens from where it starts, somewhere across the crest, wider toward the rim,
    // so a column that is in one stays in it walking out and the draft rule still finds nothing rising.
    let l = h.along.abs();
    let j = (l / CREST_CRACK_MM).round();
    let mut crack: f64 = 1.0;
    for k in [j - 1.0, j, j + 1.0] {
        let centre = (k + 0.4 * (skin::hash(k as i64, 77) - 0.5)) * CREST_CRACK_MM;
        let start = reach * (0.05 + 0.6 * skin::hash(k as i64, 91));
        if d < start {
            continue;
        }
        let half = 0.10 + 0.22 * ((d - start) / reach);
        crack = crack.min(0.6 + 0.4 * smooth01(((l - centre).abs() - half) / 0.12));
    }
    CREST_RELIEF * level.max(0.0) * off_head * crack
}

fn on_face(a: &Atlas, x: f64, z: f64) -> (f64, f64) {
    let s = a
        .samples
        .iter()
        .filter(|s| s.p[1] > a.top - 1.2)
        .min_by(|p, q| ((p.p[0] - x).powi(2) + (p.p[2] - z).powi(2)).total_cmp(&((q.p[0] - x).powi(2) + (q.p[2] - z).powi(2))))
        .unwrap();
    (s.theta, s.v)
}

#[derive(Default, serde::Serialize)]
struct Authored {
    hide_reach_mm: f64,
    folds_mm: Vec<f64>,
    fold_clear_from_mm: f64,
    wall_rho_mm: [f64; 2],
    blossom_diameter_mm: f64,
    blossoms_per_wall: usize,
    spurs: Vec<Spur>,
    twig: serde_json::Value,
    bark: serde_json::Value,
}

fn author() -> Result<(RingDesign, AlphaLibrary, Authored)> {
    let mut d = probe::stock("012", true, None)?;
    d.name = "Prunus".into();
    let mut lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let hide = Hide::of(&a);
    let mut info = Authored { hide_reach_mm: hide.reach(), ..Default::default() };
    if std::env::var("PRUNUS_DEBUG").is_ok() {
        for theta in (0..=180).step_by(10) {
            let x = ((theta as f64 / 360.0 * a.width as f64).round() as usize) % a.width;
            let rows: Vec<f64> = (0..a.height).map(|y| a.at(x, y)).filter(|s| s.p[2] > 0.0 && s.n[2].abs() > 0.85).map(|s| s.p[0].hypot(s.p[1])).collect();
            let crest = (0..a.height).map(|y| a.at(x, y)).map(|s| s.p[0].hypot(s.p[1])).fold(0.0, f64::max);
            println!("    wall at {theta}: rho {:.2}..{:.2}, crest r {crest:.2}", rows.iter().copied().fold(f64::MAX, f64::min), rows.iter().copied().fold(0.0, f64::max));
        }
    }
    let folds = hide.folds(&a, FOLD_TURN_DEG_PER_MM);
    let last_fold = folds.iter().map(|f| f.abs()).fold(0.0, f64::max);
    info.folds_mm = folds;
    info.fold_clear_from_mm = last_fold + 1.0;
    {
        let setup = d.manufacturing.as_mut().unwrap();
        setup.recipe.alloy = "Gold 18k".into();
        setup.bench_notes = "Delft sand, Z=0 parting, opposed Z withdrawal. The twig and its spurs lie in the parting plane and pull both ways; the blossoms stand on the head's walls along the pull. At the bench: drill on the raised mark at the face's centre and cut the sloe's seat to the measured cabochon; drill and bead-set each blossom's diamond.".into();
    }
    // The sloe fills the face: Caiman's flush boss, cut to the cabochon, kept low so the face stays the fruit's ground.
    let (theta, v) = on_face(&a, 0.0, 0.0);
    let mut seat = SeatPadLayer {
        theta_deg: theta,
        v_mm: v,
        style: SeatStyle::Boss,
        crown: 0.15,
        blend_mm: 0.4,
        metal_true: true,
        solid: SolidKind::Flush,
        through: true,
        ..Default::default()
    };
    let mut sloe = Gem::cabochon(GemCut::Round, SLOE_MM);
    sloe.preview_tint = Some(SLOE_TINT);
    seat.fit_stone(sloe);
    seat.height_mm = SLOE_BOSS_MM;
    seat.mark_mm = 1.0;
    let mut e = LayerEntry::new("Sloe", Layer::SeatPad(seat));
    e.blend = Blend::Max;
    d.layers.layers.push(e);

    // The 012 master's shoulders dip a few microns at the parting line (122–162° and its mirror), which the field
    // reads as a 3° undercut; a low round rail on the parting line over each shoulder lifts it, falling away both sides.
    let ctx = d.field_context();
    for (centre, side) in [(140.0, "left"), (40.0, "right")] {
        let rail = BorderLayer { v_mm: ctx.band_v_len_mm * 0.5, width_mm: 1.2, height_mm: 0.08, profile: BorderProfile::Round, mirror: false, rope_twists: 0 };
        let mut e = LayerEntry::new(format!("Parting rail, {side} shoulder"), Layer::Border(rail));
        e.blend = Blend::Max;
        let mut w = Window::around(centre, 50.0);
        w.fade_deg = 6.0;
        e.window = w;
        d.layers.layers.push(e);
    }
    // The twig and its spurs, both shoulders.
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Cushion".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    let mut id = 2;
    // The sloe's calyx: soldered on at the bench after the stone's seat is cut, so the sand never sees it.
    let mut c = Component { attach: Attach::Join, stage: Stage::Bench, blend_mm: 0.0, ..Default::default() };
    c.placement = Placement::Ring { theta_deg: theta, across_mm: 0.0, height_mm: 0.0, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
    doc.append(Feature { id, name: "Calyx".into(), enabled: true, operation: calyx_op()?, component: c })?;
    id += 1;
    for (sign, side) in [(1.0, "left"), (-1.0, "right")] {
        doc.append(Feature { id, name: format!("Twig, {side} shoulder"), enabled: true, operation: twig(&a, &hide, sign, twig_spec(sign))?, component: joined(0.0) })?;
        id += 1;
        if sign < 0.0 {
            doc.append(Feature { id, name: "Twig, the sloe's stalk".into(), enabled: true, operation: twig(&a, &hide, sign, stalk_spec())?, component: joined(0.0) })?;
            id += 1;
        }
    }
    for (sign, side) in [(1.0, "left"), (-1.0, "right")] {
        for (k, row) in spur_plan().into_iter().enumerate() {
            let l = info.fold_clear_from_mm + row.offset;
            let (theta, _) = hide.crest_at(&a, sign * l);
            // Positive tilt leans toward smaller ring angles; every spur leans toward its twig's tip.
            let tilt = -sign * row.lean;
            let mut c = joined(0.0);
            c.placement = Placement::Ring { theta_deg: theta, across_mm: 0.0, height_mm: 0.0, spin_deg: 0.0, tilt_deg: tilt, cant_deg: 0.0 };
            doc.append(Feature {
                id,
                name: format!("Spur {} {side}", k + 1),
                enabled: true,
                operation: spur_op(row, SPUR_TIP_R_MM, SPUR_FLARE_MM, SPUR_FOOT_MM),
                component: c,
            })?;
            id += 1;
            info.spurs.push(Spur { along_mm: sign * l, theta_deg: theta, length_mm: row.length, base_r_mm: row.base_r, stub_r_mm: row.stub_r, stub_len_mm: row.stub_len, lean_deg: row.lean, tip_diameter_mm: 2.0 * SPUR_TIP_R_MM });
        }
    }
    // Blossom on the twig itself, between the spurs, as blackthorn flowers on bare wood: made at the bench and soldered
    // on, so they stand free of the sand's rules.
    for (sign, l, rot) in TWIG_BLOSSOMS {
        let spec = twig_spec(sign);
        let share = ((l - spec.from) / (spec.to - spec.from)).clamp(0.0, 1.0);
        let r = spec.radius * (1.0 + (spec.end_scale - 1.0) * share);
        let (theta, _) = hide.crest_at(&a, sign * l);
        let mut c = Component { attach: Attach::Join, stage: Stage::Bench, blend_mm: 0.0, ..Default::default() };
        c.placement = Placement::Ring { theta_deg: theta, across_mm: 0.0, height_mm: r * (spec.proud + 1.0) - 0.28, spin_deg: rot, tilt_deg: 0.0, cant_deg: 0.0 };
        doc.append(Feature { id, name: format!("Blossom on the twig, {:+.1} mm", sign * l), enabled: true, operation: blossom_op(TWIG_BLOSSOM_MM)?, component: c })?;
        id += 1;
    }
    d.cad = Some(doc);
    info.twig = serde_json::json!({"left": twig_spec(1.0), "right": twig_spec(-1.0), "stalk": stalk_spec()});

    // Blossom on bare wood: one open flower on each of the head's walls, set off its centre to opposite sides, a
    // diamond at its heart in a ring of stamens; a smaller flower and a bud on each shank wall. All stand along the pull.
    let (lo, hi) = wall_span(&a, 1.0);
    info.wall_rho_mm = [lo, hi];
    let rho = 0.5 * (lo + hi);
    let dia = (hi - lo - 0.05).clamp(2.4, BLOSSOM_MM);
    info.blossom_diameter_mm = dia;
    info.blossoms_per_wall = 1;
    let mut stamps = Vec::new();
    let mut motifs: Vec<(P3, f64)> = Vec::new();
    let raised = |name: String, (theta, v): (f64, f64), rot: f64, outline: Vec<P2>, height: f64, top: StampTop, tier: u8| Stamp {
        name,
        theta_deg: theta,
        v_mm: v,
        rot_deg: rot,
        outline,
        height_mm: height,
        sink_mm: 0.3,
        draft_deg: 0.0,
        cut: false,
        bench: false,
        along_pull: true,
        tier,
        top,
        fine_cap: true,
    };
    for (side, name, phi, rot) in [(1.0, "near", -0.26, 18.0), (-1.0, "far", 0.26, 54.0)] {
        let Some(at) = on_wall(&a, rho, phi, side) else { continue };
        motifs.push((a.point(at.0, at.1), 0.5 * dia));
        let blossom = raised(format!("Blossom on the {name} wall"), at, rot, petals(dia, 0.46), 0.42, StampTop::Dome { crown_mm: 0.22 }, 0);
        let frame = blossom.frame(&d, &ctx);
        stamps.push(blossom);
        // Five stamens on the petals' bases, round the diamond.
        for j in 0..5 {
            let t = (72.0 * j as f64).to_radians();
            let at_frame = stamen_at(&a, &frame, t, STAMEN_RING * dia, side);
            let mut dot = raised(format!("Stamen {} of the {name} blossom", j + 1), at_frame, 0.0, outline::circle(STAMEN_MM), 0.14, StampTop::Dome { crown_mm: 0.08 }, 1);
            dot.sink_mm = 0.2;
            stamps.push(dot);
        }
        let mut heart = SeatPadLayer {
            theta_deg: at.0,
            v_mm: at.1,
            style: SeatStyle::GypsyMound,
            blend_mm: 0.4,
            metal_true: true,
            solid: SolidKind::Bead,
            through: false,
            ..Default::default()
        };
        heart.fit_stone(Gem::calibrated(GemCut::Round, 1.3));
        // A heart, not a mound: the pad held inside the stamens' ring.
        heart.diameter_mm = 1.7;
        heart.elong = 1.0;
        heart.height_mm = 0.95;
        heart.mark_mm = 0.6;
        let mut e = LayerEntry::new(format!("Blossom heart, {name} wall"), Layer::SeatPad(heart));
        e.blend = Blend::Max;
        d.layers.layers.push(e);
        // Beside it on the same wall, toward the other shoulder: a smaller flower and a closed bud, a spray of three.
        for (k, (share, drho, small)) in [(-0.3, -0.35, true), (-0.9, 0.45, false)].into_iter().enumerate() {
            let Some(at) = on_wall(&a, rho + drho, phi * share, side) else { continue };
            let p = a.point(at.0, at.1);
            if small {
                motifs.push((p, 0.5 * SMALL_BLOSSOM_MM));
                stamps.push(raised(format!("Small blossom on the {name} wall"), at, rot + 25.0, petals(SMALL_BLOSSOM_MM, 0.5), 0.36, StampTop::Dome { crown_mm: 0.18 }, 0));
            } else {
                motifs.push((p, 0.5 * BUD_MM[0]));
                stamps.push(raised(format!("Bud on the {name} wall"), at, if phi > 0.0 { 160.0 } else { 20.0 } + 15.0 * k as f64, bud(BUD_MM[0], BUD_MM[1]), 0.30, StampTop::Dome { crown_mm: 0.28 }, 0));
            }
        }
    }
    // The bark: black, fissured blackthorn bark on every wall that faces the pull, plates in furrows running round the
    // ring, wavering and broken, a narrow polished halo round each flower and bud.
    let mut bark = a.paint("Blackthorn bark", |s| bark_at(&a, &hide, s, &motifs));
    let before = bark.data.clone();
    let bite = draft_clamp(&a, &mut bark, BARK_MM)?;
    if std::env::var("PRUNUS_DEBUG").is_ok() {
        let mut worst: Vec<(f32, usize)> = before.iter().zip(&bark.data).enumerate().map(|(i, (b, c))| (b - c, i)).filter(|(d, _)| *d > 0.12).collect();
        worst.sort_by(|p, q| q.0.total_cmp(&p.0));
        for (dv, i) in worst.iter().step_by((worst.len() / 12).max(1)).take(12) {
            let s = a.samples[*i];
            println!("    bark bite {:.3} mm at theta {:.1} p ({:.2}, {:.2}, {:.2}) n ({:.2}, {:.2}, {:.2})", *dv as f64 * BARK_MM, s.theta, s.p[0], s.p[1], s.p[2], s.n[0], s.n[1], s.n[2]);
        }
    }
    info.bark = serde_json::json!({"relief_mm": BARK_MM, "clamp_texels_cut": bite.texels_cut, "clamp_worst_mm": bite.worst_mm});
    // Kept as the file keeps it, a 16-bit PNG, so a cold reload builds the same metal.
    lib.insert(ringdesign_core::Alpha::from_png16(bark.name.clone(), &bark.to_png16()?)?);
    d.layers.layers.push(skin::hide_layer(&d, "Blackthorn bark", BARK_MM, Window::around(90.0, 360.0)));
    d.stamps = stamps;
    Ok((d, lib, info))
}

/// Every made part's self-crossings.
fn crossings(built: &mesh::BuildResult) -> Vec<(String, usize)> {
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

fn mesh_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() })
}

/// Metal vertices inside each stone's body.
fn metal_in_stones(d: &RingDesign, m: &mesh::Mesh) -> Vec<(String, usize, f64)> {
    // A vertex counts when it stands more than the mesh's own chord inside the stone's crown: a seat's wall cut to the
    // stone lies on its surface, and a facet's sag puts it a few hundredths either side.
    const TOLERANCE_MM: f64 = 0.05;
    ringdesign_core::stones::stone_frames(d)
        .into_iter()
        .map(|(st, f)| {
            let (a, b, h) = (st.gem.l_mm * 0.5, st.gem.w_mm * 0.5, st.gem.crown_mm());
            let faceted = st.gem.form == GemForm::Faceted;
            let (mut n, mut worst) = (0, 0.0_f64);
            for p in &m.vertices {
                let q = sub([p.0 as f64, p.1 as f64, p.2 as f64], f.girdle);
                let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                if !(z > TOLERANCE_MM && z < h - TOLERANCE_MM) {
                    continue;
                }
                let t = z / h;
                let k = if faceted { 1.0 - 0.45 * t } else { (1.0 - t * t).sqrt() };
                // How far inside the crown's outline at this height, along its plan's radius.
                let r = ((x / a).powi(2) + (y / b).powi(2)).sqrt();
                let depth = (k - r) * a.min(b);
                if depth > TOLERANCE_MM {
                    n += 1;
                    worst = worst.max(depth);
                    if std::env::var("PRUNUS_DEBUG").is_ok() {
                        println!("    {} intruded {depth:.3} at ({x:.3}, {y:.3}, {z:.3})", st.label);
                    }
                }
            }
            (st.label, n, worst)
        })
        .collect()
}

/// The release, field and gate numbers of one build.
fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams, built: &mesh::BuildResult) -> Result<serde_json::Value> {
    let v = &built.report.validation;
    let q = built.report.quality;
    let made = crossings(built);
    let ring_crossings = mesh_crossings(&built.mesh);
    let bore = d.inner_radius_mm();
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, built);
    let (inspection, fine) = probe::pull(d, lib, params, 0.075)?;
    let statuses: Vec<(String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|f| (f.name.clone(), format!("{:?}", f.status)))
        .collect();
    let features_ok = built.parts.evaluated.iter().flat_map(|e| e.features.iter()).all(|f| f.status.is_ok());
    let monotone: Vec<(String, bool)> = d.stamps.iter().map(|s| (s.name.clone(), s.parting_monotone(d).is_ok())).collect();
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = ringdesign_core::stones::stone_frames(d).len();
    let in_stones = metal_in_stones(d, &built.mesh);
    let crowding: Vec<String> = stones.as_ref().map_or(Vec::new(), |s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} mm", p.a, p.b, p.gap_mm)).collect());
    let tight = stones.as_ref().map_or(0, |s| s.tight_pairs);
    let release = |r: &mf::release::ReleaseReport| serde_json::json!({"status": format!("{:?}", r.status), "obstructions": r.obstructions.len(), "unresolved": r.unresolved_rays, "deepest_mm": r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max), "where": r.obstructions.iter().take(6).map(|o| o.world.map(|v| (v * 100.0).round() / 100.0)).collect::<Vec<_>>()});
    let stamped = built.solids.stamped;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && ring_crossings == 0
        && made.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && features_ok
        && stamped == d.stamps.len()
        && min_r >= bore - 0.01
        && field.verdict == castability::Verdict::Castable
        && inspection.release.obstructions.is_empty()
        && inspection.release.unresolved_rays == 0
        && fine.obstructions.is_empty()
        && fine.unresolved_rays == 0
        && monotone.iter().all(|(_, ok)| *ok)
        && findings.is_empty()
        && stones.as_ref().map_or(0, |s| s.stone_count) as usize == previewed
        && in_stones.iter().all(|(_, n, _)| *n == 0)
        && tight == 0
        && built.mesh.faces.len() <= 2_000_000;
    Ok(serde_json::json!({
        "build": [params.theta_steps, params.profile_steps],
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "made_part_crossings": made,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "feature_status": statuses,
        "features_all_ok": features_ok,
        "stamps": d.stamps.len(),
        "stamps_resolved": stamped,
        "bore_radius_mm": bore,
        "closest_vertex_to_axis_mm": min_r,
        "bore_clear": min_r >= bore - 0.01,
        "field_verdict": field.verdict.label(),
        "field_undercut_percent": field.undercut_fraction() * 100.0,
        "field_notes": field.notes,
        "parts_undercut_mm2": field.parts.iter().map(|p| p.undercut_area_mm2).sum::<f64>(),
        "parts_worst_draft_deg": field.parts.iter().map(|p| p.worst_draft_deg).fold(f64::MAX, f64::min),
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0_100": release(&inspection.release),
        "release_0_075": release(&fine),
        "parting_monotone": monotone,
        "dfm_findings": findings,
        "stones_reported": stones.as_ref().map_or(0, |s| s.stone_count),
        "stones_previewed": previewed,
        "metal_inside_stones": in_stones,
        "stone_carats": stones.as_ref().map_or(0.0, |s| s.total_carats),
        "crowding": crowding,
        "tight_pairs": tight,
        "pass": pass,
    }))
}

fn side_by_side(path: &Path, imgs: &[Vec<u8>], edge: usize) -> Result<()> {
    let n = imgs.len();
    let mut out = vec![0u8; edge * n * edge * 3];
    for y in 0..edge {
        for (k, img) in imgs.iter().enumerate() {
            let row = (y * n * edge + k * edge) * 3;
            out[row..row + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * n) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", 1.25, 1.05),
    ("reverse", PI, 0.8),
];

/// A stone as the renders draw it; the sloe is a cabochon under its bloom, shaded smooth and soft.
fn stone(m: &mesh::Mesh, t: [f32; 3]) -> render::Part<'_> {
    let mut p = render::Part::tinted_stone(m, t);
    if t == SLOE_TINT {
        p.smooth = true;
        p.roughness = 0.45;
    }
    p
}

/// The metal split for the studio as the house finishes it: recesses darkened, the high points polished. A vertex's
/// depth is how far under a smoothed copy of the surface it lies, along its normal: the copy is the mesh relaxed
/// `ANTIQUE_PASSES` times, so a flat or convex surface reads nothing and a furrow, a crease or a seat reads its depth.
fn antiqued(m: &mesh::Mesh, class: &[u8]) -> Vec<(mesh::Mesh, [f32; 3])> {
    let n = m.vertices.len();
    let mut neighbours: Vec<Vec<u32>> = vec![Vec::new(); n];
    for f in &m.faces {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            neighbours[a as usize].push(b);
            neighbours[b as usize].push(a);
        }
    }
    for l in &mut neighbours {
        l.sort_unstable();
        l.dedup();
    }
    let start: Vec<P3> = m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
    let mut p = start.clone();
    for _ in 0..ANTIQUE_PASSES {
        p = (0..n)
            .map(|i| {
                let l = &neighbours[i];
                if l.is_empty() {
                    return p[i];
                }
                let mean: P3 = std::array::from_fn(|c| l.iter().map(|&j| p[j as usize][c]).sum::<f64>() / l.len() as f64);
                std::array::from_fn(|c| 0.5 * p[i][c] + 0.5 * mean[c])
            })
            .collect();
    }
    let depth: Vec<f64> = (0..n)
        .map(|i| {
            let nn = m.normals.get(i).map_or([0.0; 3], |v| [v.0 as f64, v.1 as f64, v.2 as f64]);
            dot(sub(p[i], start[i]), nn)
        })
        .collect();
    // Gold in three depths; the twig's dark wood in two; the bark's plates and its furrows.
    let tints: [[f32; 3]; 7] = [render::GOLD, ANTIQUE_MID, ANTIQUE_DARK, TWIG_TINT, ANTIQUE_DARK, BARK_PLATE_TINT, BARK_FURROW_TINT];
    let mut out: Vec<(mesh::Mesh, [f32; 3])> = tints
        .iter()
        .map(|t| (mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..Default::default() }, *t))
        .collect();
    for f in &m.faces {
        let h = f.iter().map(|&i| depth[i as usize]).sum::<f64>() / 3.0;
        let c = f.iter().map(|&i| class.get(i as usize).copied().unwrap_or(0)).max().unwrap_or(0);
        let bin = match c {
            0 => if h > ANTIQUE_DEEP_MM { 2 } else if h > ANTIQUE_SHALLOW_MM { 1 } else { 0 },
            1 => if h > ANTIQUE_DEEP_MM { 4 } else { 3 },
            2 => if h > ANTIQUE_SHALLOW_MM { 6 } else { 5 },
            _ => 6,
        };
        out[bin].0.faces.push(*f);
    }
    out.retain(|(m, _)| !m.faces.is_empty());
    out
}

/// What finish each vertex takes: 0 polished gold, 1 the twig's dark wood (the twigs, the stalk, the calyx and the spurs' stubs),
/// 2 the bark's plates, 3 its furrows. Band vertices read the bark's own relief where they stand.
fn finish_classes(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> Result<Vec<u8>> {
    let m = &built.mesh;
    let a = Atlas::of(d, AW, AH)?;
    let bark = lib.get("Blackthorn bark");
    let doc = d.cad.as_ref();
    let frames: std::collections::HashMap<ringdesign_core::sketch::Id, (P3, P3)> = built.parts.evaluated.iter().flat_map(|e| e.components.iter()).map(|c| (c.id, (c.frame.origin, c.frame.z_axis))).collect();
    let stubs: std::collections::HashMap<String, f64> = spur_plan().iter().enumerate().flat_map(|(k, r)| ["left", "right"].map(|side| (format!("Spur {} {side}", k + 1), r.stub_len + 0.25))).collect();
    Ok((0..m.vertices.len())
        .map(|i| {
            let p = [m.vertices[i].0 as f64, m.vertices[i].1 as f64, m.vertices[i].2 as f64];
            let origin = m.origin.get(i).copied().unwrap_or(0);
            if origin >= mesh::SOLID_VERTEX {
                let Some(id) = built.parts.feature_of(origin) else { return 0 };
                let Some(f) = doc.and_then(|doc| doc.feature(id)) else { return 0 };
                if f.name.starts_with("Twig") || f.name == "Calyx" {
                    return 1;
                }
                if let (Some(stub), Some((o, z))) = (stubs.get(&f.name), frames.get(&id)) {
                    return u8::from(dot(sub(p, *o), *z) < *stub);
                }
                return 0;
            }
            // The head's top round the sloe's boss is the wood the fruit hangs on: oxidised like the bark's plates.
            if p[1] > a.top - 0.9 && p[0].hypot(p[2]) > SLOE_MM * 0.5 + 0.75 {
                return 2;
            }
            let Some(bark) = bark else { return 0 };
            let theta = p[1].atan2(p[0]).to_degrees().rem_euclid(360.0);
            let x = ((theta / 360.0 * a.width as f64).round() as usize) % a.width;
            let y = (0..a.height).min_by(|&u, &v| {
                let (su, sv) = (a.at(x, u).p, a.at(x, v).p);
                dot(sub(su, p), sub(su, p)).total_cmp(&dot(sub(sv, p), sub(sv, p)))
            });
            match y.map(|y| bark.data[y * a.width + x]) {
                Some(v) if v > 0.82 => 2,
                Some(v) if v > 0.03 => 3,
                _ => 0,
            }
        })
        .collect())
}

/// A loose-triangle stone mesh welded, with smooth normals, so a cabochon shades as a polished dome.
fn welded(m: &mesh::Mesh) -> mesh::Mesh {
    let mut out = mesh::Mesh::default();
    let mut index = std::collections::HashMap::new();
    for f in &m.faces {
        let g = f.map(|i| {
            let v = m.vertices[i as usize];
            let key = [v.0, v.1, v.2].map(|c| (c * 1e4).round() as i64);
            *index.entry(key).or_insert_with(|| {
                out.vertices.push(v);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    let mut normals = vec![[0.0_f64; 3]; out.vertices.len()];
    for f in &out.faces {
        let [a, b, c] = f.map(|i| { let v = out.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] });
        let (e1, e2) = (sub(b, a), sub(c, a));
        let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        for &i in f {
            for k in 0..3 {
                normals[i as usize][k] += nrm[k];
            }
        }
    }
    out.normals = normals.into_iter().map(|v| { let l = dot(v, v).sqrt().max(1e-12); mesh::Vec3((v[0] / l) as f32, (v[1] / l) as f32, (v[2] / l) as f32) }).collect();
    out
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let stones: Vec<(mesh::Mesh, [f32; 3])> = ringdesign_core::gems::built_meshes(d, lib, built).into_iter().map(|(m, t)| if t == SLOE_TINT { (welded(&m), t) } else { (m, t) }).collect();
    let metal = &built.mesh;
    let finish = antiqued(metal, &finish_classes(d, lib, built)?);
    let parts: Vec<render::Part> = finish.iter().map(|(m, t)| render::Part::metal(m, *t)).chain(stones.iter().map(|(m, t)| stone(m, *t))).collect();
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let small: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3], VIEWS[2]].iter().map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 4)).collect();
    image::save_buffer(out.join("hero-300.png"), &small[0], 300, 300, image::ColorType::Rgb8)?;
    image::save_buffer(out.join("face-300.png"), &small[1], 300, 300, image::ColorType::Rgb8)?;
    side_by_side(&out.join("contact-300.png"), &small, 300)?;
    // The stones close: the sloe and the near wall's blossom, framed on the head.
    let mut crop = mesh::Mesh::default();
    let mut map = std::collections::HashMap::new();
    for f in &metal.faces {
        if f.iter().all(|&i| metal.vertices[i as usize].1 > 6.0) {
            let g = f.map(|i| {
                *map.entry(i).or_insert_with(|| {
                    crop.vertices.push(metal.vertices[i as usize]);
                    crop.normals.push(metal.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 1.0, 0.0)));
                    (crop.vertices.len() - 1) as u32
                })
            });
            crop.faces.push(g);
        }
    }
    let mut close = vec![render::Part::metal(&crop, render::GOLD)];
    close.extend(stones.iter().map(|(m, t)| stone(m, *t)));
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 0.75, edge)?;
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.cad = None;
    bare.stamps.clear();
    bare.layers.layers.clear();
    let b = mesh::try_build(&bare, lib, built_params(built))?;
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &[left, right], edge)?;
    Ok(())
}

fn built_params(built: &mesh::BuildResult) -> BuildParams {
    let _ = built;
    draft_params()
}

/// The raw surface normal under each spur's seat: its part along the finger, which C-V1 `level` would zero.
fn normals_at_spurs(built: &mesh::BuildResult, spurs: &[Spur]) -> Vec<(f64, f64)> {
    let Some(band) = &built.band else { return Vec::new() };
    spurs.iter().filter_map(|s| cad::surface_hit(band, s.theta_deg, 0.0).map(|(_, n)| (s.theta_deg, n[2]))).collect()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/prunus"));
    std::fs::create_dir_all(&out)?;
    let (d, lib, info) = author()?;
    let params = if draft { draft_params() } else { export_params() };
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    println!("Prunus: {} triangles in {build_s:.1} s", built.mesh.faces.len());
    let g = gates(&d, &lib, params, &built)?;
    // The gates at 384 x 192 as well: phantoms move with resolution, real obstructions converge.
    let coarse_params = BuildParams { theta_steps: 384, profile_steps: 192, ..params };
    let coarse_built = mesh::try_build(&d, &lib, coarse_params)?;
    let coarse = gates(&d, &lib, coarse_params, &coarse_built)?;
    let level = normals_at_spurs(&built, &info.spurs);
    let worst_level = level.iter().map(|(_, z)| z.abs()).fold(0.0, f64::max);
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
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
    let mut pattern_gate = serde_json::Value::Null;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.report.quality;
        let pc = mesh_crossings(&pattern.mesh);
        pattern_gate = serde_json::json!({"watertight": pattern.report.validation.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "triangles": pattern.mesh.faces.len(),
            "pass": pattern.report.validation.watertight && pq.degenerate_faces == 0 && pc == 0});
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let stones = ringdesign_core::gems::built_meshes(&d, &lib, &built);
        let mut materials = Vec::new();
        for (k, (m, tint)) in stones.iter().enumerate() {
            let (name, file) = if tint[0] < 0.2 { ("Onyx", "reference-onyx.stl".to_string()) } else { ("Diamond", format!("reference-diamond-{k}.stl")) };
            stl::write_stl(out.join(&file), m, "Prunus reference stone")?;
            materials.push(serde_json::json!({"mesh": file, "name": name, "tint": tint}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&serde_json::json!({"stones": materials}))?)?;
    }
    renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 })?;
    let mut report = serde_json::json!({
        "ring": "Prunus",
        "slug": "prunus",
        "process": d.draft.process.label(),
        "sand": format!("{:?}", d.draft.sand),
        "min_draft_deg": d.draft.min_draft_deg,
        "min_section_mm": d.draft.min_section_mm,
        "min_detail_mm": d.draft.min_detail_mm,
        "base": "Factory 012 Cushion 10 x 10, native, sand master",
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build_s": build_s,
        "design_bytes": text.len(),
        "design_format": format,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len()),
        "authored": serde_json::to_value(&info)?,
        "level": {"method": "C-V1 not on master: the raw surface normal under every spur, read on the band as swept", "normal_z_at_spurs": level, "worst_abs_normal_z": worst_level, "level": worst_level < 1e-3},
        "cold_reload_identical": cold,
        "casting_pattern": pattern_gate,
    });
    let key = if draft { "draft" } else { "export" };
    report[key] = g.clone();
    report[format!("{key}_384x192")] = coarse.clone();
    // Keep the other build's block when it was written before.
    if let Ok(old) = std::fs::read_to_string(out.join("report.json")) {
        if let Ok(old) = serde_json::from_str::<serde_json::Value>(&old) {
            for other in ["draft", "draft_384x192", "export", "export_384x192", "cold_reload_identical", "casting_pattern", "template_gate"] {
                if report.get(other).is_none_or(|v| v.is_null()) {
                    if let Some(v) = old.get(other) {
                        report[other] = v.clone();
                    }
                }
            }
        }
    }
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    println!("  {key}: {}", serde_json::to_string(&g)?);
    println!("  384x192: pass {} field {} release {} / {}", coarse["pass"], coarse["field_verdict"], coarse["release_0_100"], coarse["release_0_075"]);
    println!("  level: worst |n_z| {worst_level:.2e} at the spurs; pattern {pattern_gate}");
    ensure!(g["pass"].as_bool() == Some(true), "Prunus failed a gate at {key}; see report.json");
    ensure!(cold != Some(false), "Prunus changed on a cold reload with an empty library");
    Ok(())
}
