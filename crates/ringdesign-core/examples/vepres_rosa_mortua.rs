//! Vepres — Rosa mortua, the dead rose: one thorned rose stem closes round the finger and passes itself. Arm A
//! ends in a ruby bud that never opened, nodding in five sepal claws; arm B ends in a garnet hip still crowned by
//! its dried sepals. Hooked prickles run down both arms, graded toward the palm. Lost wax.
//! cargo build --release -p ringdesign-core --example vepres_rosa_mortua
//! target/release/examples/vepres_rosa_mortua [OUT_DIR] [--draft] [--verify] [--blockout]
// The block-out's first attempt (a rose bloom of three petal rings round the ruby: `petal`, `petal_layout`, the
// annular `receptacle`, `sepal`) is kept for the rethink the third failed read test calls for; it is not built now.
#![allow(dead_code)]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, Stage, TwistPath, SurfaceKind, builders, stored},
    castability, csg, dfm,
    gem::{Gem, GemCut},
    library, mesh,
    profile::{BYPASS_OFFSET, BYPASS_SLIDE_ARC_DEG, BYPASS_SLIDE_END_DEG, MAX_PROFILE_STEPS, ShankKind, TOP_DEG},
    render, setstone, setting, stl,
    sketch::{Geometry, Id, Sketch, Workplane},
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

use cadkernel::brep::Placement as Motion;

#[path = "common/probe.rs"]
mod probe;

type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The stem: a round HighDome section, 0.3 mm deeper than the section's 2.7: at 2.7 core's seam channel at the
/// crossing leaves a 0.15 mm lip at the bore edge; at 3.0 it clears.
const STEM_W_MM: f64 = 3.6;
const STEM_T_MM: f64 = 3.0;
/// How much of core's bypass the stem takes (its slide and its seam channel).
const BYPASS_AMOUNT: f64 = 0.7;
/// Lost wax at the investment floors.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;
/// Census zones thinner than this are suspected measurement artifacts, not metal.
const ARTIFACT_MM: f64 = 0.05;
/// Census zones smaller than this are specks (a sample or two) that fill from the metal round them.
const SPECK_MM2: f64 = 0.02;
/// The bloom on arm A, just past the top, over the crossing, where the hero camera looks; the hip at arm B's far end.
const BLOOM_DEG: f64 = 97.0;
const HIP_DEG: f64 = 38.0;
/// How far each stone is lifted to stand on its arm's stem.
const BLOOM_LIFT_MM: f64 = 0.9;
const HIP_LIFT_MM: f64 = 0.0;
/// The hip stands on the low side of the band, where arm B's stem leads it.
const HIP_Z_MM: f64 = -0.3;
/// Arm B's stem stops this far short of the hip, under its collet's rim, clear of the stone.
const HIP_STEM_GAP_DEG: f64 = 9.0;
/// The two arms' stems over the band: round, `STEM_R_MM`, standing `STEM_PROUD_MM` proud, each from this far off
/// the top (buried) to under its stone.
const STEM_R_MM: f64 = 1.05;
const STEM_PROUD_MM: f64 = 0.75;
const STEM_BURIED_R_MM: f64 = 0.6;
const STEM_FROM_DEG: [f64; 2] = [42.0, 35.0];
const STEM_BLEND_MM: f64 = 0.0;
/// The ruby at the heart of the bloom, in a low collet.
const HEART_WALL_MM: f64 = 0.9;
const HEART_LIP: f64 = 0.3;
/// Four rings of petals: (count, rise over the horizontal, length, width, foot under the girdle, droop at the tip,
/// skew). The innermost ring wraps the ruby's girdle in a spiral, each petal skewed so one edge laps over its
/// neighbour; the next cups it; the third has opened; the outer one hangs dead, reflexed below the bloom's plane.
const PETAL_RINGS: [(usize, f64, f64, f64, f64, f64, f64); 4] = [(4, 104.0, 2.7, 5.2, -0.9, 0.0, 0.12), (5, 70.0, 3.0, 4.2, -1.1, 15.0, 0.08), (6, 38.0, 3.6, 4.4, -1.35, 30.0, 0.1), (7, 0.0, 4.0, 4.6, -1.6, 30.0, 0.0)];
const PETAL_START_DEG: f64 = 20.0;
const PETAL_FOOT_OUT_MM: f64 = 0.3;
/// Cupped toward the heart at the foot; the margins roll back toward the tip, as a dried petal's do.
const PETAL_CUP_MM: f64 = 0.4;
const PETAL_REFLEX_MM: f64 = 0.65;
/// Dried: a low crinkle across the blade.
const PETAL_CRINKLE_MM: f64 = 0.12;
/// How ragged the outer petals' dried edges are, as a share of the blade.
const PETAL_TEAR: f64 = 0.06;
/// How far the inner rings curve round the collet across their width (share of their half-width, at the margin).
const PETAL_WRAP: f64 = 0.3;
/// The section floor through the blade; it thins only at the free margin.
const PETAL_T_MM: f64 = 1.0;
const PETAL_EDGE_MM: f64 = 0.8;
const RECEPTACLE_FOOT_Z: f64 = -2.75;
/// Five dried sepals under the bloom, long and reflexed.
const SEPALS: usize = 5;
const SEPAL_LEN_MM: f64 = 4.4;
const SEPAL_W_MM: f64 = 1.5;
const SEPAL_T_MM: f64 = 1.05;
/// The hip: a garnet cabochon in a collet, on its swollen body, crowned by its dried sepals.
const HIP_WALL_MM: f64 = 1.0;
const HIP_LIP: f64 = 0.3;
/// The hip's body under the stone: semi-axes and how far its centre stands under the girdle.
const HIP_BODY: (f64, f64, f64, f64) = (2.6, 2.6, 1.0, 1.25);
/// The dried sepal crown on the hip: five wisps, each a lens `WISP_W x WISP_T` leaning `WISP_IN` in over the dome as
/// it rises `WISP_RISE`, then curling out over `WISP_CURL_DEG` on a `WISP_BEND` radius, turning and tapering.
const WISPS: u32 = 5;
const WISP_W_MM: f64 = 1.15;
const WISP_IN_MM: f64 = 0.6;
const WISP_T_MM: f64 = 1.0;
const WISP_RISE_MM: f64 = 1.9;
const WISP_BEND_MM: f64 = 0.9;
const WISP_CURL_DEG: f64 = 110.0;
const WISP_TWIST_DEG: f64 = 50.0;
const WISP_END: f64 = 0.82;
const WISP_ROOT_Z_MM: f64 = 0.2;
const WISP_ROOT_OUT_MM: f64 = 0.75;
/// The leaf, below the bloom down arm A's far side: where its rachis starts, how far across, the rachis's run, each
/// leaflet's (length, width), the laterals' spread; its sink, drape, thickness and teeth.
const LEAF_DEG: f64 = 134.0;
const LEAF_Z_MM: f64 = -0.2;
const LEAF_RACHIS_MM: f64 = 2.0;
const LEAF_TERMINAL: (f64, f64) = (6.0, 3.2);
const LEAF_LATERAL: (f64, f64) = (5.0, 2.6);
const LEAF_SPREAD_DEG: f64 = 50.0;
const LEAF_SINK_MM: f64 = 0.12;
const DRAPE_Z_MM: (f64, f64) = (-0.8, 0.45);
const DRAPE_SLOPE: f64 = 0.2;
const LEAF_T_MM: f64 = 0.95;
const LEAF_TOOTH: f64 = 0.12;
/// Five prickles on each arm's shoulder, degrees from the top, spaced unevenly; the palm's lower third stays smooth.
const PRICKLES: usize = 5;
const PRICKLE_OFFS_DEG: [[f64; PRICKLES]; 2] = [[72.0, 82.0, 92.0, 102.0, 112.0], [86.0, 96.0, 106.0, 116.0, 126.0]];
/// The smallest prickle's scale, at the palm end.
const PRICKLE_LAST_SCALE: f64 = 0.8;
const PRICKLE_SINK_MM: f64 = 0.3;
const PRICKLE_BLEND_MM: f64 = 0.0;
/// How far each prickle's foot flares where it meets the stem, as a share of its section.
const PRICKLE_FLARE: f64 = 0.45;
const PRICKLE_SPIN_DEG: f64 = 25.0;
const PRICKLE_SIDES: usize = 24;
/// A rose prickle: a broad flattened foot (round the ring, across), a short rise, then hooked down the stem.
const PRICKLE_FOOT_MM: (f64, f64) = (3.0, 1.8);
const PRICKLE_RISE_MM: f64 = 1.0;
const PRICKLE_BEND_MM: f64 = 1.5;
const PRICKLE_HOOK_DEG: f64 = 70.0;
/// Alternate prickles stand this far either side of the crest, canted with it.
const PRICKLE_ACROSS_MM: f64 = 0.55;
const PRICKLE_CANT_DEG: f64 = 0.0;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn ruby() -> Gem {
    Gem { preview_tint: Some([0.60, 0.05, 0.12]), ..Gem::calibrated(GemCut::Round, 5.5) }
}

fn garnet() -> Gem {
    Gem { preview_tint: Some([0.40, 0.04, 0.07]), ..Gem::cabochon(GemCut::Round, 6.0) }
}

/// The stem: a procedural bypass, HighDome 3.6 x 2.7, lost wax.
fn stem() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Rosa mortua".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    // The style read at the stem's own depth, so its crown is the 3.0 mm section's.
    d.profile.thickness_mm = STEM_T_MM;
    d.profile.apply_style(ProfileStyle::HighDome);
    d.profile.width_mm = STEM_W_MM;
    d.profile.thickness_mm = STEM_T_MM;
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Bypass;
    // 0.7 of core's full bypass: at 1.0 the seam channel cuts a HighDome this narrow to a 0.29 mm wall at the
    // crossing; at 0.7 the thinnest wall is 1.98 mm.
    d.shank.amount = BYPASS_AMOUNT;
    probe::cast_in(&mut d, &probe::wax_setup(0.1));
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d
}

fn smoother(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}

fn joined(placement: Placement, blend: f64) -> Component {
    Component { placement, attach: Attach::Join, stage: Stage::Cast, blend_mm: blend, ..Component::default() }
}

// --- Prickles ------------------------------------------------------------------------------------------

/// A prickle's centreline in the part's tangent-radial plane: `radial` mm straight out, then an arc of `bend_r`
/// turning `bend_deg` round the ring.
fn hook_path(radial: f64, bend_r: f64, bend_deg: f64) -> Sketch {
    let mut s = Sketch { plane: Workplane { x: [0.0, 1.0, 0.0], y: [0.0, 0.0, 1.0], ..Default::default() }, ..Sketch::default() };
    let a = s.point([0.0, 0.0]);
    let b = s.point([0.0, radial]);
    s.entity(Geometry::Line { a, b });
    let centre = s.point([bend_r, radial]);
    let t = (180.0 - bend_deg).to_radians();
    let start = s.point([bend_r + bend_r * t.cos(), radial + bend_r * t.sin()]);
    s.entity(Geometry::Arc { center: centre, start, end: b });
    s
}

/// A closed ellipse as a polyline on `plane`, `a` mm along the plane's y and `b` mm along its x.
fn ellipse_on(plane: Workplane, a: f64, b: f64, name: &str) -> Sketch {
    let mut s = Sketch { plane, ..Sketch::default() };
    s.name = name.into();
    let points: Vec<_> = (0..PRICKLE_SIDES)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / PRICKLE_SIDES as f64;
            s.point([0.5 * b * t.cos(), 0.5 * a * t.sin()])
        })
        .collect();
    s.entity(Geometry::Polyline { points, closed: true });
    s
}

/// A rose prickle at `scale` in its seat's frame (z out of the stem, y round the ring): an elliptic section
/// `PRICKLE_FOOT_MM` (round the ring, across) carried up `PRICKLE_RISE_MM` and round a `PRICKLE_BEND_MM` hook of
/// `PRICKLE_HOOK_DEG`, tapering to a point; its foot flares a further `PRICKLE_FLARE` where it meets the stem and is
/// sunk under it, so it grows out of the bark with its own fillet rather than a seam bead.
fn prickle_solid(scale: f64) -> csg::Solid {
    let (along, across) = (0.5 * PRICKLE_FOOT_MM.0 * scale, 0.5 * PRICKLE_FOOT_MM.1 * scale);
    let (rise, bend, hook) = (PRICKLE_RISE_MM * scale + PRICKLE_SINK_MM, PRICKLE_BEND_MM * scale, PRICKLE_HOOK_DEG.to_radians());
    let straight = rise;
    let arc = bend * hook;
    let total = straight + arc;
    // The centreline and its heading in (y, z), by arc length.
    let at = |s: f64| -> ([f64; 2], f64) {
        if s <= straight {
            ([0.0, s - PRICKLE_SINK_MM], 0.0)
        } else {
            let a = (s - straight) / bend;
            ([bend * (1.0 - a.cos()), straight - PRICKLE_SINK_MM + bend * a.sin()], a)
        }
    };
    let rings = 20;
    let around = 12;
    let mut sol = csg::Solid::default();
    let mut tip = [0.0; 3];
    for i in 0..=rings {
        let t = i as f64 / rings as f64;
        let ([y, z], a) = at(total * t);
        // Thick at the foot, flaring under the bark, tapering straight to a fine point.
        let flare = 1.0 + PRICKLE_FLARE * (1.0 - t / 0.12).max(0.0).powi(2);
        // A round-shouldered taper, so the section holds the floor to within a floor of the point.
        let k = (1.0 - t).max(0.0).powf(0.3).max(0.07) * flare;
        // The section square to the heading: across is x; along turns with the hook.
        let (sa, ca) = a.sin_cos();
        for j in 0..around {
            let u = std::f64::consts::TAU * j as f64 / around as f64;
            let (px, py) = (across * k * u.cos(), along * k * u.sin());
            sol.v.push([px, y + py * ca, z - py * sa]);
        }
        if i == rings {
            tip = [0.0, y, z];
        }
    }
    let base = at(0.0).0;
    sol.v.push([0.0, base[0], base[1]]);
    sol.v.push(tip);
    let (b, e) = ((sol.v.len() - 2) as u32, (sol.v.len() - 1) as u32);
    let ring = |i: usize, j: usize| (i * around + j % around) as u32;
    for i in 0..rings {
        for j in 0..around {
            sol.f.push([ring(i, j), ring(i, j + 1), ring(i + 1, j + 1)]);
            sol.f.push([ring(i, j), ring(i + 1, j + 1), ring(i + 1, j)]);
        }
    }
    for j in 0..around {
        sol.f.push([b, ring(0, j + 1), ring(0, j)]);
        sol.f.push([e, ring(rings, j), ring(rings, j + 1)]);
    }
    let vol: f64 = sol.f.iter().map(|f| dot(sol.v[f[0] as usize], cross3(sol.v[f[1] as usize], sol.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut sol.f {
            f.swap(1, 2);
        }
    }
    sol
}

// --- Sculpted sheets ------------------------------------------------------------------------------------

fn add3(a: P3, b: P3, k: f64) -> P3 {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

fn sub3(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross3(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit3(a: P3) -> P3 {
    let l = dot(a, a).sqrt().max(1e-12);
    a.map(|v| v / l)
}

/// A closed sheet over `(u, v)` in the unit square: `f` gives the mid-surface point, its unit normal and the
/// thickness there; the two faces stand half the thickness either side and four strips close the rim.
fn sheet(nu: usize, nv: usize, square: bool, f: impl Fn(f64, f64) -> (P3, P3, f64)) -> csg::Solid {
    let mut s = csg::Solid::default();
    let idx = |side: usize, i: usize, j: usize| (side * (nu + 1) * (nv + 1) + j * (nu + 1) + i) as u32;
    for side in 0..2 {
        for j in 0..=nv {
            for i in 0..=nu {
                let (u, v) = (i as f64 / nu as f64, j as f64 / nv as f64);
                let (p, hint, t) = f(u, v);
                // With `square`, the thickness stands square to the mid-surface as built, not to the hint, so a
                // curled margin keeps its full section (the petals); a narrow, tightly cupped blade keeps the hint,
                // whose offset cannot fold inside its own curl.
                let h = 1e-3;
                let du = sub3(f((u + h).min(1.0), v).0, f((u - h).max(0.0), v).0);
                let dv = sub3(f(u, (v + h).min(1.0)).0, f(u, (v - h).max(0.0)).0);
                let c = cross3(du, dv);
                // Halfway between the two: square enough to keep a curled margin's section, steady enough not to
                // fold where a petal's tip closes.
                let n = if square && dot(c, c) > 1e-18 { let n = unit3(c); let n = if dot(n, hint) < 0.0 { n.map(|x| -x) } else { n }; unit3(add3(n, hint, 1.0)) } else { hint };
                let k = if side == 0 { 0.5 * t } else { -0.5 * t };
                s.v.push(add3(p, n, k));
            }
        }
    }
    let quad = |s: &mut csg::Solid, a: u32, b: u32, c: u32, d: u32| {
        s.f.push([a, b, c]);
        s.f.push([a, c, d]);
    };
    for j in 0..nv {
        for i in 0..nu {
            quad(&mut s, idx(0, i, j), idx(0, i + 1, j), idx(0, i + 1, j + 1), idx(0, i, j + 1));
            quad(&mut s, idx(1, i, j), idx(1, i, j + 1), idx(1, i + 1, j + 1), idx(1, i + 1, j));
        }
    }
    for i in 0..nu {
        quad(&mut s, idx(0, i, 0), idx(1, i, 0), idx(1, i + 1, 0), idx(0, i + 1, 0));
        quad(&mut s, idx(0, i, nv), idx(0, i + 1, nv), idx(1, i + 1, nv), idx(1, i, nv));
    }
    for j in 0..nv {
        quad(&mut s, idx(0, 0, j), idx(0, 0, j + 1), idx(1, 0, j + 1), idx(1, 0, j));
        quad(&mut s, idx(0, nu, j), idx(1, nu, j), idx(1, nu, j + 1), idx(0, nu, j + 1));
    }
    // Outward by construction or wholly inward: turn it out if its volume reads negative.
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross3(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// A ribbon along a centreline: `c(u)` its point and `side(u)` the unit across it; `width(u)` and `thick(u)` its
/// sizes, and `cup` how far its margins lift toward the normal (a leaf's or a sepal's dome turned over).
fn ribbon(nu: usize, nv: usize, c: impl Fn(f64) -> P3, side: impl Fn(f64) -> P3, width: impl Fn(f64) -> f64, thick: impl Fn(f64) -> f64, cup: f64) -> csg::Solid {
    sheet(nu, nv, false, |u, v| {
        let du = 1e-3;
        let t = unit3(sub3(c((u + du).min(1.0)), c((u - du).max(0.0))));
        let b = side(u);
        let n = unit3(cross3(t, b));
        let b = cross3(n, t);
        let x = v - 0.5;
        let w = width(u);
        // A dome across: thickest on the midrib, falling to the margin.
        let p = add3(add3(c(u), b, x * w), n, cup * w * (x * x * 4.0));
        // Full thickness across the blade, thinning only within a floor of its margin.
        let to_margin = (0.5 - x.abs()) * w;
        (p, n, thick(u) * (0.88 + 0.12 * smooth01(to_margin / 0.7)))
    })
}

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_rosa_mortua".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

fn moved(mut s: csg::Solid, frame: &Motion) -> csg::Solid {
    for v in &mut s.v {
        *v = frame.point(*v);
    }
    s
}

fn smooth01(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A closed solid of revolution about the frame's z, its section `profile` (a closed loop of `(s, z)`, `s` a share
/// of the plan ellipse `a x b`), never touching the axis: an annulus.
fn revolve(profile: &[(f64, f64)], a: f64, b: f64, around: usize) -> csg::Solid {
    let n = profile.len();
    let mut s = csg::Solid::default();
    for j in 0..around {
        let phi = std::f64::consts::TAU * j as f64 / around as f64;
        for &(r, z) in profile {
            s.v.push([r * a * phi.cos(), r * b * phi.sin(), z]);
        }
    }
    let at = |j: usize, i: usize| ((j % around) * n + i % n) as u32;
    for j in 0..around {
        for i in 0..n {
            s.f.push([at(j, i), at(j + 1, i), at(j + 1, i + 1)]);
            s.f.push([at(j, i), at(j + 1, i + 1), at(j, i + 1)]);
        }
    }
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross3(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// A round tube along `path` (at least two points), its radius at each point, frames carried along it without
/// twist, capped flat at both ends.
fn tube(path: &[P3], radius: &[f64], around: usize) -> csg::Solid {
    let m = path.len();
    let tangent = |i: usize| unit3(sub3(path[(i + 1).min(m - 1)], path[i.saturating_sub(1)]));
    let t0 = tangent(0);
    let seed = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let mut nrm = unit3(cross3(cross3(t0, seed), t0));
    let mut s = csg::Solid::default();
    s.v.push(path[0]);
    s.v.push(path[m - 1]);
    for i in 0..m {
        let t = tangent(i);
        nrm = unit3(sub3(nrm, t.map(|v| v * dot(nrm, t))));
        let bin = cross3(t, nrm);
        for j in 0..around {
            let a = std::f64::consts::TAU * j as f64 / around as f64;
            s.v.push(add3(add3(path[i], nrm, radius[i] * a.cos()), bin, radius[i] * a.sin()));
        }
    }
    let ring = |i: usize, j: usize| (2 + i * around + j % around) as u32;
    for j in 0..around {
        s.f.push([0, ring(0, j + 1), ring(0, j)]);
        s.f.push([1, ring(m - 1, j), ring(m - 1, j + 1)]);
    }
    for i in 0..m - 1 {
        for j in 0..around {
            s.f.push([ring(i, j), ring(i, j + 1), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i + 1, j)]);
        }
    }
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross3(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// The plan radius of the ellipse `a x b` at azimuth `phi`.
fn ellipse_r(a: f64, b: f64, phi: f64) -> f64 {
    let (sn, cs) = phi.sin_cos();
    a * b / ((b * cs).powi(2) + (a * sn).powi(2)).sqrt()
}

/// The bud collet's outer plan, mm.
fn collet_plan(gem: Gem) -> (f64, f64) {
    (0.5 * gem.l_mm + HEART_WALL_MM, 0.5 * gem.w_mm + HEART_WALL_MM)
}

/// Where petal `k` stands: its ring (0 innermost), its azimuth, its rise, length, width, foot and droop.
struct PetalAt {
    ring: usize,
    azimuth: f64,
    rise_deg: f64,
    length: f64,
    width: f64,
    foot_z: f64,
    droop_deg: f64,
    skew: f64,
}

/// The petals ring by ring, inner to outer, each ring turned the golden angle on the one inside it.
fn petal_layout() -> Vec<PetalAt> {
    let mut out = Vec::new();
    let mut az = PETAL_START_DEG;
    for (ring, &(count, rise, length, width, foot, droop, skew)) in PETAL_RINGS.iter().enumerate() {
        for k in 0..count {
            out.push(PetalAt { ring, azimuth: (az + 360.0 * k as f64 / count as f64).to_radians(), rise_deg: rise, length, width, foot_z: foot, droop_deg: droop, skew });
        }
        az += 137.5;
    }
    out
}

/// One petal in the stone frame: an obovate blade rising from its foot by the collet at `rise_deg`, bending down
/// by `droop_deg` toward its tip, widest near its rounded tip; cupped toward the heart at the foot, its margins
/// rolled back toward the tip and its blade crinkled, as a dried petal's are. `PETAL_T_MM` through the blade,
/// thinning only within a floor of its free margin.
fn petal(gem: Gem, at: &PetalAt) -> csg::Solid {
    let (a, b) = collet_plan(gem);
    let r0 = ellipse_r(a, b, at.azimuth) + PETAL_FOOT_OUT_MM;
    let (sn, cs) = at.azimuth.sin_cos();
    let radial = [cs, sn, 0.0];
    let across = [-sn, cs, 0.0];
    let foot = add3([0.0, 0.0, at.foot_z], radial, r0);
    let seed = at.azimuth * 3.7;
    // The centreline, integrated so the droop bends it smoothly: heading starts at the rise and turns down.
    let steps = 64;
    let mut line = vec![(0.0f64, 0.0f64)];
    for i in 0..steps {
        let t = (i as f64 + 0.5) / steps as f64;
        let ang = (at.rise_deg - at.droop_deg * t * t).to_radians();
        let ds = at.length / steps as f64;
        let (o, z) = line[i];
        line.push((o + ds * ang.cos(), z + ds * ang.sin()));
    }
    let centre = move |reach: f64| {
        let f = (reach * steps as f64).clamp(0.0, steps as f64 - 1e-9);
        let i = f.floor() as usize;
        let k = f - i as f64;
        let (o0, z0) = line[i];
        let (o1, z1) = line[i + 1];
        let ang = (at.rise_deg - at.droop_deg * reach * reach).to_radians();
        (o0 + (o1 - o0) * k, z0 + (z1 - z0) * k, ang)
    };
    sheet(20, 24, true, |u, v| {
        let x = 2.0 * u - 1.0;
        // A rounded tip, its dried edge torn a little: the sides stop short of the middle, unevenly.
        let outer = at.ring as f64 / (PETAL_RINGS.len() - 1) as f64;
        let tear = PETAL_TEAR * outer * (2.5 * x + seed).sin().powi(2);
        let reach = v * (1.0 - 0.3 * x * x - tear);
        let (o, z, ang) = centre(reach);
        let (rs, rc) = ang.sin_cos();
        let along = add3(add3(foot, radial, o), [0.0, 0.0, 1.0], z);
        // Obovate: narrow at the foot, widest at two-thirds, rounding to the tip.
        let notch = 1.0 - PETAL_TEAR * outer * (6.0 * reach + seed).sin().max(0.0).powi(4);
        let half = 0.5 * at.width * (0.3 + 0.7 * (PI * (0.15 + 0.7 * reach)).sin()) * notch;
        // The blade's normal faces up-and-in; cupped at the foot, the margins rolled back toward the tip.
        let up = [-rs * cs, -rs * sn, rc];
        // The inner rings wrap the collet's circle along their whole height; the outer ones cup only at the foot.
        let wrap = PETAL_WRAP * (1.0 - outer).powi(2);
        let cup = wrap + PETAL_CUP_MM * (1.0 - reach) - PETAL_REFLEX_MM * (0.4 + 0.6 * outer) * smooth01((reach - 0.35) / 0.65);
        let crinkle = PETAL_CRINKLE_MM * reach * ((7.0 * x + seed).sin() * (5.0 * reach + seed).cos());
        // Skewed: one edge stands out and the other tucks in, so a ring of them laps round like a spiral.
        let q = add3(add3(along, across, half * x), up, cup * half * x * x + crinkle - at.skew * half * x);
        // Thinning only near the free margin: within about a floor of the tip and the sides.
        let to_tip = at.length * (1.0 - reach);
        let to_side = half * (1.0 - x.abs());
        let edge = smooth01(to_tip.min(to_side) / 0.7);
        (q, up, PETAL_EDGE_MM + (PETAL_T_MM - PETAL_EDGE_MM) * edge)
    })
}

/// A closed ellipsoid about `c` with semi-axes `(ra, rb, rc)`.
fn ellipsoid(c: P3, (ra, rb, rc): (f64, f64, f64)) -> csg::Solid {
    let (rings, around) = (14, 28);
    let at = |t: f64, p: f64| [c[0] + ra * t.sin() * p.cos(), c[1] + rb * t.sin() * p.sin(), c[2] + rc * t.cos()];
    let mut s = csg::Solid::default();
    s.v.push(at(0.0, 0.0));
    for i in 1..rings {
        let t = PI * i as f64 / rings as f64;
        for j in 0..around {
            s.v.push(at(t, std::f64::consts::TAU * j as f64 / around as f64));
        }
    }
    s.v.push(at(PI, 0.0));
    let last = (s.v.len() - 1) as u32;
    let ring = |i: usize, j: usize| (1 + (i - 1) * around + j % around) as u32;
    for j in 0..around {
        s.f.push([0, ring(1, j), ring(1, j + 1)]);
        s.f.push([last, ring(rings - 1, j + 1), ring(rings - 1, j)]);
    }
    for i in 1..rings - 1 {
        for j in 0..around {
            s.f.push([ring(i, j), ring(i + 1, j), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i, j + 1)]);
        }
    }
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross3(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
    s
}

/// The pavilion's plan share at height `z` under the girdle, as a share of the collet's plan.
fn pavilion_share(gem: Gem, z: f64) -> f64 {
    let (_, b) = collet_plan(gem);
    (0.5 * gem.w_mm / b) * (1.0 + z / gem.pavilion_mm()).max(0.0)
}

/// The receptacle under the bloom in its stone frame: a bulb round the collet's foot, hollow up the middle so the
/// pavilion stands clear.
fn receptacle(gem: Gem) -> csg::Solid {
    let (a, b) = collet_plan(gem);
    let clear = 0.08;
    let mut profile = vec![(0.06, RECEPTACLE_FOOT_Z), (0.06, -gem.pavilion_mm() - 0.05)];
    for k in 1..=6 {
        let z = -gem.pavilion_mm() + (gem.pavilion_mm() - 1.15) * k as f64 / 6.0;
        profile.push((pavilion_share(gem, z) + clear, z));
    }
    profile.extend([(0.96, -1.15), (1.1, -1.55), (1.1, -1.95), (0.95, -2.35), (0.62, RECEPTACLE_FOOT_Z + 0.05), (0.3, RECEPTACLE_FOOT_Z)]);
    revolve(&profile, a, b, 48)
}

/// One dried sepal at azimuth `psi` in the stone frame: rooted in the receptacle, out under the petals, then
/// hanging down and curling at its point.
fn sepal(gem: Gem, psi: f64) -> csg::Solid {
    let (a, b) = collet_plan(gem);
    let r = ellipse_r(a, b, psi);
    let (sn, cs) = psi.sin_cos();
    let radial = [cs, sn, 0.0];
    let tangent = [-sn, cs, 0.0];
    let k = SEPAL_LEN_MM / 4.2;
    let pts = [(0.85 * r, -2.0), (r + 1.4 * k, -1.6), (r + 2.8 * k, -1.9), (r + 3.3 * k, -3.0)];
    let c = move |u: f64| {
        let w = [(1.0 - u).powi(3), 3.0 * u * (1.0 - u).powi(2), 3.0 * u * u * (1.0 - u), u.powi(3)];
        let (mut o, mut z) = (0.0, 0.0);
        for i in 0..4 {
            o += w[i] * pts[i].0;
            z += w[i] * pts[i].1;
        }
        add3([0.0, 0.0, z], radial, o)
    };
    ribbon(30, 6, c, move |_| tangent, |u| SEPAL_W_MM * (2.6 * u.powf(0.6) * (1.0 - u)).min(1.0).max(0.25), |_| SEPAL_T_MM, 0.25)
}

/// Arm A's centre across the band at `theta`, mm: core's bypass arm, `BYPASS_OFFSET` of the half-width at full
/// slide. Arm B's is its mirror through the top.
fn arm_z(d: &RingDesign, theta: f64, arm_a: bool) -> f64 {
    let off = crate_wrap(theta - TOP_DEG);
    let half = 0.5 * d.profile.width_mm * d.shank.amount;
    if arm_a { BYPASS_OFFSET * half * smoother(-BYPASS_SLIDE_ARC_DEG, -BYPASS_SLIDE_END_DEG, off) } else { -BYPASS_OFFSET * half * smoother(-BYPASS_SLIDE_ARC_DEG, -BYPASS_SLIDE_END_DEG, -off) }
}

fn crate_wrap(x: f64) -> f64 {
    (x + 180.0).rem_euclid(360.0) - 180.0
}

/// Outer radius of the bare section at `theta` where it crosses height `z`.
fn surface_r(d: &RingDesign, theta: f64, z: f64) -> f64 {
    let reference = d.reference_loop();
    let l = d.section_at(theta, MAX_PROFILE_STEPS, None, Some(&reference));
    let pts: Vec<(f64, f64)> = l.pts.iter().filter(|p| p.surface).map(|p| (p.r, p.z)).collect();
    pts.windows(2)
        .filter_map(|w| {
            let ((r0, z0), (r1, z1)) = (w[0], w[1]);
            ((z0 - z) * (z1 - z) <= 0.0 && z0 != z1).then(|| r0 + (r1 - r0) * (z - z0) / (z1 - z0))
        })
        .reduce(f64::max)
        .unwrap_or(d.inner_radius_mm() + d.profile.thickness_mm)
}

/// How far a stem stands proud of the band at `theta` along its run from `from` (buried) to `to`: it rises out
/// over the first 30 deg and holds `STEM_PROUD_MM`.
fn stem_proud(theta: f64, from: f64, to: f64) -> f64 {
    let along = (theta - from) / (to - from);
    let rise = 30.0 / (to - from).abs();
    -STEM_R_MM + (STEM_PROUD_MM + STEM_R_MM) * smooth01(along / rise)
}

/// The stem of arm A (to the bloom) or arm B (to the hip) as a tube over the band: buried where it parts from the
/// other arm, standing proud over the top, and leading under its stone.
fn stem_tube(d: &RingDesign, arm_a: bool) -> (csg::Solid, Vec<P3>) {
    let (from, to) = stem_run(arm_a);
    let n = 60;
    let mut path = Vec::new();
    let mut radius = Vec::new();
    for i in 0..=n {
        let theta = from + (to - from) * i as f64 / n as f64;
        let z = stem_z(d, theta, arm_a);
        // Where it is buried it is thinner, so its underside never nears the bore.
        let grown = smooth01((theta - from) / (to - from) / (30.0 / (to - from).abs()));
        let rad = STEM_BURIED_R_MM + (STEM_R_MM - STEM_BURIED_R_MM) * grown;
        let proud = -STEM_BURIED_R_MM + (STEM_PROUD_MM + STEM_BURIED_R_MM) * grown;
        // Arm B's stem sinks back toward the band as it reaches the hip, which sits low in its own body.
        let proud = if arm_a { proud } else { proud - STEM_PROUD_MM * (1.0 - smooth01((theta - to).abs() / 14.0)) };
        let r = surface_r(d, theta, z) + proud - rad;
        path.push([r, theta, z]);
        radius.push(rad);
    }
    // The band's seam channel and an arm's rounded tip make the surface step under the stem: ride it smoothed.
    let raw: Vec<f64> = path.iter().map(|p| p[0]).collect();
    for (i, p) in path.iter_mut().enumerate() {
        let lo = i.saturating_sub(6);
        let hi = (i + 7).min(raw.len());
        let r = raw[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
        let t = p[1].to_radians();
        *p = [r * t.cos(), r * t.sin(), p[2]];
    }
    (tube(&path, &radius, 20), path)
}

/// Where each arm's stem runs, degrees: arm A from its shoulder over the top to under the bloom, arm B from its
/// shoulder over the top to under the hip.
fn stem_run(arm_a: bool) -> (f64, f64) {
    if arm_a { (TOP_DEG - STEM_FROM_DEG[0], BLOOM_DEG - 4.0) } else { (TOP_DEG + STEM_FROM_DEG[1], HIP_DEG + HIP_STEM_GAP_DEG) }
}

/// A stem's height across the band: its arm's slide, arm B easing past its own tip onto the hip's line.
fn stem_z(d: &RingDesign, theta: f64, arm_a: bool) -> f64 {
    let z = arm_z(d, theta, arm_a);
    if arm_a { z } else { z + (HIP_Z_MM - z) * smooth01((72.0 - theta) / (72.0 - HIP_DEG - HIP_STEM_GAP_DEG)) }
}

// --- The leaf -------------------------------------------------------------------------------------------

/// The band's outer surface in a chart: `s` mm round the ring at the crest radius from theta 0, `z` across it;
/// `lift` mm out along the surface's normal. Returns the point and that normal.
struct Chart<'a> {
    d: &'a RingDesign,
    radius: f64,
}

impl Chart<'_> {
    fn theta(&self, s: f64) -> f64 {
        (s / self.radius).to_degrees()
    }
    /// The band's surface, read only over its crown (the low side stops short of the bypass's seam channel): past
    /// `DRAPE_Z_MM` across, the chart droops on at a gentle
    /// slope, so a leaflet reaching past the stem's edge hangs over it rather than following its flank down.
    fn surface(&self, s: f64, z: f64) -> P3 {
        let theta = self.theta(s);
        let zc = z.clamp(DRAPE_Z_MM.0, DRAPE_Z_MM.1);
        let r = surface_r(self.d, theta, zc) - DRAPE_SLOPE * (z - zc).abs();
        let t = theta.to_radians();
        [r * t.cos(), r * t.sin(), z]
    }
    fn at(&self, s: f64, z: f64, lift: f64) -> (P3, P3) {
        // A wide difference, so the normal turns smoothly over the drape's start.
        let h = 0.4;
        let p = self.surface(s, z);
        let ds = sub3(self.surface(s + h, z), self.surface(s - h, z));
        let dz = sub3(self.surface(s, z + h), self.surface(s, z - h));
        let mut n = unit3(cross3(ds, dz));
        // Outward: away from the finger's axis.
        if dot(n, [p[0], p[1], 0.0]) < 0.0 {
            n = n.map(|v| -v);
        }
        (add3(p, n, lift), n)
    }
}

/// One leaflet lying on the band: its foot at chart `(s0, z0)`, its axis at `heading` (radians; 0 runs round the
/// ring toward larger theta, positive turns toward +z), `len` x `width` mm, a serrated margin of `teeth`; domed
/// to its midrib and lifted off the band so its margin stands clear.
fn leaflet(ch: &Chart<'_>, s0: f64, z0: f64, heading: f64, len: f64, width: f64, teeth: f64, sink: f64) -> csg::Solid {
    let (hs, hc) = heading.sin_cos();
    sheet(10, 22, false, |u, v| {
        let x = 2.0 * u - 1.0;
        // Ovate, pointed: widest at two-fifths, a serrated margin whose teeth lean toward the tip.
        let body = (PI * v.powf(0.75)).sin().max(0.0);
        let tooth = 1.0 - LEAF_TOOTH * (teeth * v).fract();
        let half = 0.5 * width * body.max(0.08) * if v > 0.12 && v < 0.96 { tooth } else { 1.0 };
        let (along, side) = (len * v, half * x);
        let (s, z) = (s0 + along * hc - side * hs, z0 + along * hs + side * hc);
        // Domed: thickest on its midrib, thinning to its margin; its underside sunk a constant
        // `LEAF_SINK_MM` into the band so it grows out of it, never grazing it.
        // Full thickness through the blade, thinning only within a floor of the margin and the tip.
        let to_edge = (half * (1.0 - x.abs())).min(len * (1.0 - v));
        let t = 0.88 * LEAF_T_MM + 0.12 * LEAF_T_MM * smooth01(to_edge / 0.7);
        let (p, n) = ch.at(s, z, 0.5 * t - sink);
        (p, n, t)
    })
}

/// The leaf: a terminal leaflet and a pair of laterals on a short rachis, springing from the crossing between the
/// bud and the hip and draping off the stem's low edge, where the face view sees it whole.
fn leaf(d: &RingDesign) -> Vec<(String, csg::Solid)> {
    let radius = surface_r(d, LEAF_DEG, 0.0);
    let ch = Chart { d, radius };
    let s0 = radius * LEAF_DEG.to_radians();
    let z0 = LEAF_Z_MM;
    // It lies along the crest beyond the bud, pointing down the stem toward the palm.
    let mut out = vec![("Leaf, terminal leaflet".to_string(), leaflet(&ch, s0 + LEAF_RACHIS_MM, z0, 0.0, LEAF_TERMINAL.0, LEAF_TERMINAL.1, 9.0, LEAF_SINK_MM))];
    // Each leaflet sunk a little differently, so where their feet overlap no two faces lie on one another.
    for (name, sign) in [("upper", 1.0), ("lower", -1.0)] {
        out.push((format!("Leaf, {name} lateral leaflet"), leaflet(&ch, s0 + 0.35 * LEAF_RACHIS_MM, z0 + sign * 0.2, sign * LEAF_SPREAD_DEG.to_radians(), LEAF_LATERAL.0, LEAF_LATERAL.1, 7.0, LEAF_SINK_MM + 0.035 * (1.0 + sign))));
    }
    // The rachis: a tapering stalk along the crest from the stem to the terminal leaflet.
    let path: Vec<P3> = (0..=16).map(|k| ch.at(s0 - 0.8 + (LEAF_RACHIS_MM + 1.3) * k as f64 / 16.0, z0, 0.2).0).collect();
    let radius_at: Vec<f64> = (0..=16).map(|k| 0.42 - 0.12 * k as f64 / 16.0).collect();
    out.push(("Leaf, rachis".to_string(), tube(&path, &radius_at, 16)));
    out
}

// --- The parts -----------------------------------------------------------------------------------------

/// The CAD document: the stem, the two arms' stems crossing, the bud in its petals and sepals, the hip in its
/// sepal claws, the prickles.
fn parts(d: &mut RingDesign, with_leaf: bool) -> Result<serde_json::Value> {
    let mut doc = Document::default();
    doc.append(feature(1, "Stem", Operation::Band, Component { role: ComponentRole::Shank, ..Component::default() }))?;
    let bloom_z = arm_z(d, BLOOM_DEG, true);
    let heart = ruby();
    let hip = garnet();
    // Each stone stands on its arm's stem, which stands proud of the band.
    doc.append(builders::stone_feature(
        2,
        heart,
        Placement::Ring { theta_deg: BLOOM_DEG, across_mm: bloom_z, height_mm: builders::stand_off_mm(builders::BEZEL, heart) + BLOOM_LIFT_MM, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0, level: false },
    ))?;
    doc.append(builders::feature_on(3, "Heart collet", builders::BEZEL, 2, json!({"wall_mm": HEART_WALL_MM, "lip": HEART_LIP})))?;
    doc.append(builders::stone_feature(
        4,
        hip,
        Placement::Ring { theta_deg: HIP_DEG, across_mm: HIP_Z_MM, height_mm: builders::stand_off_mm(builders::BEZEL, hip) + HIP_LIFT_MM, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0, level: false },
    ))?;
    doc.append(builders::feature_on(5, "Hip collet", builders::BEZEL, 4, json!({"wall_mm": HIP_WALL_MM, "lip": HIP_LIP})))?;
    d.cad = Some(doc);
    let (tube_a, path_a) = stem_tube(d, true);
    let (tube_b, path_b) = stem_tube(d, false);
    let doc = d.cad.as_mut().unwrap();
    let free = |blend: f64| joined(Placement::Free, blend);
    // Seated in a stone's own frame (C-V1): the part follows its stone when the stone moves or resizes.
    let on = |part: Id| joined(Placement::Relative { part, at: [0.0; 3], rotation_deg: [0.0; 3] }, 0.0);
    doc.append(feature(6, "Stem of arm A", stored_op(&tube_a, "stem", json!({"arm": "A", "radius_mm": STEM_R_MM, "proud_mm": STEM_PROUD_MM}))?, free(STEM_BLEND_MM)))?;
    doc.append(feature(7, "Stem of arm B", stored_op(&tube_b, "stem", json!({"arm": "B", "radius_mm": STEM_R_MM, "proud_mm": STEM_PROUD_MM}))?, free(STEM_BLEND_MM)))?;
    doc.append(feature(8, "Receptacle", stored_op(&receptacle(heart), "receptacle", json!({"under": "bloom"}))?, on(2)))?;
    let mut id: Id = 9;
    // One petal per ring, then the ring patterned about the ruby's own axis (the collet is round, so every copy
    // seats as its source does): four stored blades for twenty-two petals, which keeps the template light.
    let layout = petal_layout();
    for ring in 0..PETAL_RINGS.len() {
        let at = layout.iter().find(|p| p.ring == ring).expect("every ring has a petal");
        let count = PETAL_RINGS[ring].0 as u32;
        let params = json!({"ring": ring, "azimuth_deg": at.azimuth.to_degrees(), "rise_deg": at.rise_deg, "droop_deg": at.droop_deg, "length_mm": at.length, "width_mm": at.width, "thick_mm": PETAL_T_MM});
        doc.append(feature(id, &format!("Petal, ring {}", ring + 1), stored_op(&petal(heart, at), "petal", params)?, on(2)))?;
        doc.append(feature(id + 1, &format!("Petal ring {}", ring + 1), Operation::Pattern { sources: id.into(), kind: PatternKind::About { part: 2, count, span_deg: 360.0 } }, free(0.0)))?;
        id += 2;
    }
    let psi = std::f64::consts::TAU * 0.3 / SEPALS as f64;
    doc.append(feature(id, "Dried sepal", stored_op(&sepal(heart, psi), "sepal", json!({"azimuth_deg": psi.to_degrees(), "length_mm": SEPAL_LEN_MM}))?, on(2)))?;
    doc.append(feature(id + 1, "Dried sepals", Operation::Pattern { sources: id.into(), kind: PatternKind::About { part: 2, count: SEPALS as u32, span_deg: 360.0 } }, free(0.0)))?;
    id += 2;
    // The hip's dried crown: one wisp from the collet's rim, leaning in over the dome and curling out above it,
    // then four more about the hip.
    let (ex, ey, ez) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let root = add3([0.0, 0.0, WISP_ROOT_Z_MM], ex, 0.5 * hip.w_mm + WISP_ROOT_OUT_MM);
    let mut path = Sketch { plane: Workplane { origin: root, x: ex, y: ez, ..Default::default() }, ..Sketch::default() };
    let (lean_s, lean_c) = (WISP_IN_MM / WISP_IN_MM.hypot(WISP_RISE_MM), WISP_RISE_MM / WISP_IN_MM.hypot(WISP_RISE_MM));
    let a = path.point([0.0, 0.0]);
    let tip = [-WISP_IN_MM, WISP_RISE_MM];
    let b = path.point(tip);
    path.entity(Geometry::Line { a, b });
    let c = [tip[0] + WISP_BEND_MM * lean_c, tip[1] + WISP_BEND_MM * lean_s];
    let centre = path.point(c);
    let (cs, sn) = ((-WISP_CURL_DEG).to_radians().cos(), (-WISP_CURL_DEG).to_radians().sin());
    let v = [tip[0] - c[0], tip[1] - c[1]];
    let end = path.point([c[0] + v[0] * cs - v[1] * sn, c[1] + v[0] * sn + v[1] * cs]);
    path.entity(Geometry::Arc { center: centre, start: end, end: b });
    // The section stands square to the lean.
    let across = [ex[0] * lean_c + ez[0] * lean_s, ex[1] * lean_c + ez[1] * lean_s, ex[2] * lean_c + ez[2] * lean_s];
    let section = ellipse_on(Workplane { origin: root, x: across, y: ey, ..Default::default() }, WISP_W_MM, WISP_T_MM, "Wisp lens");
    doc.append(feature(id, "Hip's dried sepal", Operation::Twist { sketch: section.into(), path: TwistPath::Sketch(path), degrees: WISP_TWIST_DEG, end_scale: WISP_END, scale: Vec::new(), closed: false }, on(4)))?;
    doc.append(feature(id + 1, "Hip's dried crown", Operation::Pattern { sources: id.into(), kind: PatternKind::About { part: 4, count: WISPS, span_deg: 360.0 } }, free(0.0)))?;
    id += 2;
    // The hip's body: the swollen fruit under the cabochon, its top clear of the stone's base.
    let hip_body = ellipsoid([0.0, 0.0, -HIP_BODY.3], (HIP_BODY.0, HIP_BODY.1, HIP_BODY.2));
    doc.append(feature(id, "Hip body", stored_op(&hip_body, "hip body", json!({"semi_axes_mm": [HIP_BODY.0, HIP_BODY.1, HIP_BODY.2]}))?, on(4)))?;
    id += 1;
    if with_leaf {
        for (name, solid) in leaf(d) {
            let doc = d.cad.as_mut().unwrap();
            doc.append(feature(id, &name, stored_op(&solid, "leaf", json!({"leaflet": name}))?, free(0.0)))?;
            id += 1;
        }
    }
    // Prickles: five on each arm's shoulder, graded toward the palm and alternately spun; those on a stem stand on it.
    let mut placed = Vec::new();
    let mut id = 60;
    for arm_a in [true, false] {
        for k in 0..PRICKLES {
            let arm = usize::from(!arm_a);
            let off = PRICKLE_OFFS_DEG[arm][k];
            let theta = if arm_a { TOP_DEG - off } else { TOP_DEG + off };
            let z = arm_z(d, theta, arm_a);
            let (from, to) = stem_run(arm_a);
            let on_stem = if arm_a { theta > from } else { theta < from };
            let lift = if on_stem { stem_proud(theta, from, to).max(0.0) } else { 0.0 };
            let scale = 1.0 + (PRICKLE_LAST_SCALE - 1.0) * k as f64 / (PRICKLES - 1) as f64;
            let base_spin = if arm_a { 180.0 } else { 0.0 };
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            let spin = base_spin + side * PRICKLE_SPIN_DEG;
            let doc = d.cad.as_mut().unwrap();
            doc.append(feature(
                id,
                &format!("Prickle, arm {} {}", if arm_a { "A" } else { "B" }, k + 1),
                stored_op(&prickle_solid(scale), "prickle", json!({"scale": scale, "foot_mm": [PRICKLE_FOOT_MM.0, PRICKLE_FOOT_MM.1], "hook_deg": PRICKLE_HOOK_DEG}))?,
                joined(Placement::Ring { theta_deg: theta.rem_euclid(360.0), across_mm: z + side * PRICKLE_ACROSS_MM, height_mm: lift, spin_deg: spin, tilt_deg: 0.0, cant_deg: side * PRICKLE_CANT_DEG, level: false }, PRICKLE_BLEND_MM),
            ))?;
            placed.push(json!({"id": id, "arm": if arm_a { "A" } else { "B" }, "theta_deg": theta.rem_euclid(360.0), "across_mm": z, "scale": scale, "spin_deg": spin, "on_stem": on_stem}));
            id += 1;
        }
    }
    let ends = |p: &[P3]| json!([p[0], p[p.len() - 1]]);
    Ok(json!({"bloom": {"theta_deg": BLOOM_DEG, "across_mm": bloom_z, "petals": petal_layout().len()}, "hip": {"theta_deg": HIP_DEG, "across_mm": HIP_Z_MM}, "stems": {"a": ends(&path_a), "b": ends(&path_b)}, "prickles": placed}))
}

/// The whole ring as authored at this stage.
fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, serde_json::Value)> {
    let mut d = stem();
    let placed = parts(&mut d, !blockout)?;
    let _ = blockout;
    Ok((d, AlphaLibrary::builtin(), placed))
}

// --- Checks ---------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// Every CAD part's self-crossings, as placed.
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

fn feature_status(built: &mesh::BuildResult) -> Vec<(String, String)> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| {
            let s = match &r.status {
                FeatureStatus::Ok => "Ok".to_string(),
                FeatureStatus::Suppressed => "Suppressed".to_string(),
                FeatureStatus::Failed(m) => format!("Failed: {m}"),
                FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
            };
            (format!("#{}", r.id), s)
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

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Metal vertices standing inside each stone, 0.03 mm in from its surface: the stones must sit clear.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    setstone::record(d, Some(built))
        .stones
        .iter()
        .filter_map(|st| st.frame.map(|f| (st, f)))
        .map(|(st, f)| {
            let (a, b) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03);
            let (crown, pav) = (st.gem.crown_mm() - 0.03, st.gem.pavilion_mm() - 0.03);
            let n = built
                .mesh
                .vertices
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    let q = [p.0 as f64 - f.origin[0], p.1 as f64 - f.origin[1], p.2 as f64 - f.origin[2]];
                    let (x, y, z) = (dot(q, f.x_axis), dot(q, f.y_axis), dot(q, f.z_axis));
                    // The stone's section at height z is its plan shrunk by k about the centre: a faceted crown narrows
                    // straight to its table, a cabochon's dome as a quarter ellipse, the pavilion straight to the culet.
                    let k = if z >= 0.0 {
                        if crown <= 0.0 || z >= crown { 0.0 } else if st.gem.form == ringdesign_core::gem::GemForm::Cabochon { (1.0 - (z / crown).powi(2)).sqrt() } else { 1.0 - 0.45 * z / crown }
                    } else if pav <= 0.0 || -z >= pav { 0.0 } else { 1.0 + z / pav };
                    let inside = k > 0.0 && (x / (k * a)).powi(2) + (y / (k * b)).powi(2) < 1.0;
                    if inside && std::env::var("ROSA_DEBUG").is_ok() {
                        let owner = built.mesh.origin.get(*i).and_then(|o| o.checked_sub(mesh::SOLID_VERTEX + built.parts.first as u32)).and_then(|j| built.parts.features.get(j as usize)).and_then(|id| d.cad.as_ref()?.feature(*id)).map(|f| f.name.as_str()).unwrap_or("band or seat");
                        println!("    {} intruded by {owner} at ({x:.2}, {y:.2}, {z:.2})", st.label);
                    }
                    inside
                })
                .count();
            (st.label.clone(), n)
        })
        .collect()
}

/// Each CAD part's thinnest metal as surface-normal rays find it.
fn walls(built: &mesh::BuildResult) -> Vec<serde_json::Value> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| !matches!(c.made.as_ref().map(|m| m.key.as_str()), Some(cad::pattern::PATTERN)))
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "sampled_min_mm": t.sampled_min_mm, "rays": t.rays, "below_floor": t.below_limit, "unresolved": t.unresolved})
        })
        .collect()
}

/// Pointed and tapering parts (prickles, sepals, wisps) are details judged at the 0.15 mm detail floor; bodies at
/// the 0.8 mm section. Stones and seat cuts are not metal.
fn wall_floor(name: &str) -> Option<f64> {
    let n = name.to_lowercase();
    if n.contains("ruby") || n.contains("garnet") || n.contains("seat") || n.contains("cabochon") || n.contains("oval") || n.contains("round") {
        return None;
    }
    let pointed = ["prickle", "sepal", "wisp", "claw", "collet"].iter().any(|k| n.contains(k));
    Some(if pointed { MIN_DETAIL_MM } else { MIN_SECTION_MM })
}

fn walls_ok(walls: &[serde_json::Value]) -> bool {
    walls.iter().all(|w| match wall_floor(w["part"].as_str().unwrap_or("")) {
        None => true,
        Some(floor) => w["sampled_min_mm"].as_f64().is_some_and(|m| m >= floor),
    })
}


/// A solid as a render mesh with area-weighted smooth normals.
fn mesh_of(s: &csg::Solid) -> mesh::Mesh {
    let mut n = vec![[0.0f64; 3]; s.v.len()];
    for f in &s.f {
        let (a, b, c) = (s.v[f[0] as usize], s.v[f[1] as usize], s.v[f[2] as usize]);
        let w = cross3(sub3(b, a), sub3(c, a));
        for &i in f {
            n[i as usize] = add3(n[i as usize], w, 1.0);
        }
    }
    mesh::Mesh {
        vertices: s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        normals: n.iter().map(|v| { let u = unit3(*v); mesh::Vec3(u[0] as f32, u[1] as f32, u[2] as f32) }).collect(),
        faces: s.f.clone(),
        ..Default::default()
    }
}

/// The bud's sculpted parts alone in the stone frame, from above and from the side: a quick look while shaping.
fn bud_preview(out: &Path) -> Result<()> {
    let gem = ruby();
    let mut parts = vec![receptacle(gem)];
    parts.extend(petal_layout().iter().map(|at| petal(gem, at)));
    parts.extend((0..SEPALS).map(|k| sepal(gem, std::f64::consts::TAU * (k as f64 + 0.3) / SEPALS as f64)));
    let meshes: Vec<mesh::Mesh> = parts.iter().map(mesh_of).collect();
    // Turn z (the table axis) to the render's up, +y: x stays, (y, z) -> (-z, y).
    let turned: Vec<mesh::Mesh> = meshes.into_iter().map(|mut m| { for v in m.vertices.iter_mut().chain(m.normals.iter_mut()) { *v = mesh::Vec3(v.0, v.2, -v.1); } m }).collect();
    let rp: Vec<render::Part<'_>> = turned.iter().map(|m| render::Part::metal(m, render::GOLD)).collect();
    for (name, yaw, pitch) in [("bud-top", 0.0, PI * 0.5), ("bud-side", 0.0, 0.25), ("bud-three-quarter", -0.6, 0.7)] {
        render::write_png_parts(out.join(format!("{name}.png")), &rp, yaw, pitch, 700)?;
    }
    Ok(())
}

// --- Renders --------------------------------------------------------------------------------------------

/// The camera for each named view: yaw about the finger's axis, pitch toward it.
const VIEWS: [(&str, f64, f64); 6] = [("hero", -0.65, 0.6), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", 0.75, 0.6), ("reverse", PI - 0.5, 0.35)];

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Hero, face and side at 300 px on one sheet.
fn contact(path: &Path, parts: &[render::Part<'_>]) -> Result<()> {
    let edge = 300;
    let shots: Vec<Vec<u8>> = [VIEWS[0], VIEWS[1], VIEWS[3]].iter().map(|(_, y, p)| render::render_parts_ss(parts, *y, *p, edge, edge, 3)).collect();
    let mut out = vec![0u8; edge * 3 * edge * 3];
    for y in 0..edge {
        for (k, s) in shots.iter().enumerate() {
            out[y * edge * 9 + k * edge * 3..y * edge * 9 + (k + 1) * edge * 3].copy_from_slice(&s[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * 3) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    // Close-ups framed on whole parts (never a cropped mesh): the bud, the hip, the leaf.
    let rec = setstone::record(d, None);
    let girdle = |id: Id| rec.stones.iter().find(|s| s.cad_feature() == Some(id)).and_then(|s| s.frame).map(|f| f.origin);
    if let Some(c) = girdle(2) {
        render::write_png_framed(out.join("bloom-close.png"), &parts, render::yaw_facing(BLOOM_DEG), 0.9, render::Framing::new(c, 9.0), edge)?;
    }
    if let Some(c) = girdle(4) {
        render::write_png_framed(out.join("hip-close.png"), &parts, render::yaw_facing(HIP_DEG), 0.75, render::Framing::new(c, 6.0), edge)?;
    }
    let lt = (LEAF_DEG + 12.0).to_radians();
    let lr = surface_r(d, LEAF_DEG + 12.0, 0.0);
    render::write_png_framed(out.join("leaf-close.png"), &parts, render::yaw_facing(LEAF_DEG + 12.0), 1.0, render::Framing::new([lr * lt.cos(), lr * lt.sin(), 0.0], 6.0), edge)?;
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The stones close: looking down on the top from a little toward the hero's side.
    render::write_png_parts(out.join("stones.png"), &parts, -0.25, 1.2, edge)?;
    let bare = mesh::try_build(&stem(), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let bare_img = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    render::write_png_parts(out.join("hero-300.png"), &parts, yaw, pitch, 300)?;
    render::write_png_parts(out.join("face-300.png"), &parts, VIEWS[1].1, VIEWS[1].2, 300)?;
    contact(&out.join("contact-300.png"), &parts)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/rosa-mortua"));
    std::fs::create_dir_all(&out)?;
    if args.iter().any(|a| a == "--secdbg") {
        let d = stem();
        let reference = d.reference_loop();
        for theta in [100.0, 105.0, 110.0] {
            let l = d.section_at(theta, 96, None, Some(&reference));
            println!("theta {theta}: {}", l.pts.iter().map(|p| format!("({:.2},{:.2}{})", p.r, p.z, if p.surface { "" } else { "*" })).collect::<Vec<_>>().join(" "));
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--stemdbg") {
        let lib = AlphaLibrary::builtin();
        for (amount, comfort, crown) in [(0.7, 0.1, Some(3.0)), (0.7, 0.1, Some(3.2)), (0.7, 0.4, Some(2.7)), (0.7, 0.1, Some(-4.0))] {
            let mut d = stem();
            d.shank.amount = amount;
            d.profile.comfort_fit_mm = comfort;
            if let Some(t) = crown { if t > 0.0 { d.profile.thickness_mm = t; d.profile.apply_style(ProfileStyle::HighDome); d.profile.width_mm = STEM_W_MM; d.profile.thickness_mm = t; } else { d.profile.width_mm = -t; } }
            let (w, t, fair) = (amount, comfort, crown.unwrap_or(-1.0));
            let f = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
            let m = mesh::try_build(&d, &lib, draft_params())?;
            let c = cad::measure::thickness(&m.mesh, MIN_SECTION_MM);
            let worst = c.walls.iter().filter(|z| z.thinnest_mm >= ARTIFACT_MM).map(|z| format!("{:.3}@({:.1},{:.1},{:.1})", z.thinnest_mm, z.point[0], z.point[1], z.point[2])).collect::<Vec<_>>();
            println!("amount {w} comfort {t} crown {fair}: {} thinnest {:.2} at {:.0}; census walls {} real {:?}", f.verdict.label(), f.thinnest_wall_mm, f.thinnest_wall_theta_deg, c.below_limit, worst);
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--leafdbg") {
        let d = stem();
        for (name, solid) in leaf(&d) {
            println!("{name}: {} crossings, {} faces", csg::self_crossings(&solid), solid.f.len());
        }
        let radius = surface_r(&d, LEAF_DEG, 0.0);
        let ch = Chart { d: &d, radius };
        for z in [-2.4, -2.0, -1.6, -1.2, -1.0, -0.5, 0.0, 0.5, 1.0, 1.2, 1.6, 2.0] {
            let (p, n) = ch.at(radius * 30f64.to_radians(), z, 0.0);
            println!("z {z}: p {:?} n {:?}", p.map(|v| (v * 100.0).round() / 100.0), n.map(|v| (v * 100.0).round() / 100.0));
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--bud") {
        return bud_preview(&out);
    }
    println!("Rosa mortua{}", if blockout { " (block-out)" } else { "" });
    let started = std::time::Instant::now();
    let (d, lib, placed) = author(blockout)?;
    let author_s = started.elapsed().as_secs_f64();
    println!("  placed: {placed}");
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    println!(
        "  {} triangles in {build_s:.1} s; watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; stamps {}/{}; notes {:?} {:?}",
        built.mesh.faces.len(),
        built.solids.stamped,
        d.stamps.len(),
        built.solids.notes,
        built.parts.notes
    );
    let made = made_parts(&built);
    let (stamped, solids_notes, parts_notes) = (built.solids.stamped, built.solids.notes.clone(), built.parts.notes.clone());
    let status = feature_status(&built);
    for (n, s) in &status {
        if s != "Ok" {
            println!("    feature {n}: {s}");
        }
    }
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    if std::env::var("ROSA_DEBUG").is_ok() {
        let bore = d.inner_radius_mm();
        let mut owners: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
        for (i, v) in built.mesh.vertices.iter().enumerate() {
            let r = (v.0 as f64).hypot(v.1 as f64);
            if r < bore - 0.01 {
                let owner = built.mesh.origin.get(i).and_then(|o| o.checked_sub(mesh::SOLID_VERTEX + built.parts.first as u32)).and_then(|j| built.parts.features.get(j as usize)).and_then(|id| d.cad.as_ref()?.feature(*id)).map(|f| f.name.clone()).unwrap_or_else(|| "band".into());
                let e = owners.entry(owner).or_insert((0, f64::MAX));
                e.0 += 1;
                e.1 = e.1.min(r);
                if e.0 == 1 { println!("    bore: first at ({:.2}, {:.2}, {:.2}) theta {:.1}", v.0, v.1, v.2, (v.1 as f64).atan2(v.0 as f64).to_degrees()); }
            }
        }
        println!("    bore intruders: {owners:?}");
    }
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    // The lost-wax wall census on the finished ring (the lead's 2026-10-04 gate): real walls between 0.05 and
    // 0.8 mm must be fixed; zones under 0.05 mm are listed as suspected census artifacts, never reshaped for.
    println!("  field {} thinnest wall {:.2} mm at {:.0} deg", field.verdict.label(), field.thinnest_wall_mm, field.thinnest_wall_theta_deg);
    let census = cad::measure::thickness(&built.mesh, MIN_SECTION_MM);
    let zone = |z: &cad::measure::ThinZone| json!({"thinnest_mm": z.thinnest_mm, "point": z.point, "area_mm2": z.area_mm2, "span_mm": z.span_mm, "samples": z.samples, "depth_mm": z.depth_mm, "kind": z.kind});
    let real_walls: Vec<_> = census.walls.iter().filter(|z| z.thinnest_mm >= ARTIFACT_MM && z.area_mm2 >= SPECK_MM2).collect();
    let specks: Vec<_> = census.walls.iter().filter(|z| z.thinnest_mm >= ARTIFACT_MM && z.area_mm2 < SPECK_MM2).collect();
    let artifacts: Vec<_> = census.walls.iter().filter(|z| z.thinnest_mm < ARTIFACT_MM).collect();
    println!("  wall census: {} wall samples ({:.2} mm²), {} edge samples, {} unresolved; real walls {}, suspected artifacts {}", census.below_limit, census.wall_area_mm2, census.edge_below_limit, census.unresolved, real_walls.len(), artifacts.len());
    let owner_near = |p: [f64; 3]| -> String {
        let (mut best, mut name) = (f64::MAX, String::from("band"));
        for (i, v) in built.mesh.vertices.iter().enumerate() {
            let dd = (v.0 as f64 - p[0]).powi(2) + (v.1 as f64 - p[1]).powi(2) + (v.2 as f64 - p[2]).powi(2);
            if dd < best {
                best = dd;
                name = built.mesh.origin.get(i).and_then(|o| o.checked_sub(mesh::SOLID_VERTEX + built.parts.first as u32)).and_then(|j| built.parts.features.get(j as usize)).and_then(|id| d.cad.as_ref()?.feature(*id)).map(|f| f.name.clone()).unwrap_or_else(|| "band".into());
            }
        }
        name
    };
    for z in &real_walls {
        println!("    real wall {:.3} mm at ({:.2}, {:.2}, {:.2}) over {:.2} mm² on {}", z.thinnest_mm, z.point[0], z.point[1], z.point[2], z.area_mm2, owner_near(z.point));
    }
    let findings = dfm::findings_in(&d, &lib);
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let previewed = gems.len();
    let intrusions = metal_in_stones(&d, &built);
    println!("  stones reported {reported}, previewed {previewed}; metal in stones {intrusions:?}");
    let coarse = mesh::try_build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() })?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}; notes {:?} {:?}", coarse.solids.notes, coarse.parts.notes);
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    println!("  casting pattern: {} triangles, watertight {pw}, degenerate {pd}, crossings {px}", pattern.mesh.faces.len());
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let _ = std::fs::remove_file(out.join("design.ring.json.bak"));
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let design_format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = ringdesign_core::manufacturing::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let triangles = built.mesh.faces.len();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let crowding: Vec<String> = stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect()).unwrap_or_default();
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings", made.iter().all(|(_, n)| *n == 0)),
        ("solids and parts notes empty, every stamp resolved, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && stamped == d.stamps.len() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax verdict Castable, thinnest wall at the 0.8 mm section", field.process == castability::CastProcess::LostWax && field.verdict == castability::Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("lost-wax wall census: no real wall of 0.02 mm² or more between 0.05 and 0.8 mm (thinner zones and specks listed)", census.assessed && real_walls.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview, no metal inside a stone, crowding clean", reported == previewed && reported == 2 && intrusions.iter().all(|(_, n)| *n == 0) && crowding.is_empty()),
        ("gates hold at 384 x 192", coarse_ok),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "draft": serde_json::to_value(&d.draft)?,
        "alloy_for_weight": "Gold 18k",
        "grams_18k": grams,
        "size": d.size.display(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": triangles, "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings},
        "made_parts": made,
        "features": status,
        "placed": placed,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "undercut_percent_reported_only": field.undercut_fraction() * 100.0, "notes": field.notes},
        "wall_census": {"floor_mm": MIN_SECTION_MM, "assessed": census.assessed, "rays": census.rays, "unresolved": census.unresolved, "wall_samples": census.below_limit, "wall_area_mm2": census.wall_area_mm2, "edge_samples": census.edge_below_limit, "edge_area_mm2": census.edge_area_mm2, "sampled_min_mm": census.sampled_min_mm, "real_walls": real_walls.iter().map(|z| zone(z)).collect::<Vec<_>>(), "suspected_census_artifacts": artifacts.iter().map(|z| zone(z)).collect::<Vec<_>>(), "specks_under_0_02_mm2": specks.iter().map(|z| zone(z)).collect::<Vec<_>>(), "edges": census.edges.iter().map(zone).collect::<Vec<_>>(), "rule": "Walls between 0.05 and 0.8 mm across a made feature are fixed; zones under 0.05 mm are suspected census artifacts and zones under 0.02 mm² are specks that fill from the metal round them (the lead's 2026-10-04 notes); both are listed, not reshaped for. Edges are reported as read."},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stamps": {"count": d.stamps.len(), "resolved": stamped},
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "stones": {"reported": reported, "previewed": previewed, "metal_inside": intrusions, "carats": stones.as_ref().map_or(0.0, |s| s.total_carats), "crowding": crowding},
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "stamped": coarse.solids.stamped, "notes": [&coarse.solids.notes, &coarse.parts.notes]},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "design": {"bytes": text.len(), "format_version": design_format, "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass, "blocking": !g.starts_with("lost-wax wall census")})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().filter(|(g, _)| !g.starts_with("lost-wax wall census")).all(|(_, p)| *p),
        "census_note": "Per the lead's 2026-10-04 note, census walls do not block a review on their own: every zone is listed under wall_census with its point, area and the part it lies on.",
    });
    let report = if draft {
        report
    } else {
        let mut r = report;
        if let Ok(old) = std::fs::read_to_string(out.join("draft-gates.json")) {
            r["draft_build"] = serde_json::from_str(&old)?;
        }
        r
    };
    if draft {
        std::fs::write(out.join("draft-gates.json"), serde_json::to_vec_pretty(&report)?)?;
    }
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let finished = render::finished_from(&d, &lib, built);
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &finished.metal, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Rosa mortua / investment pattern")?;
        let mut entries = Vec::new();
        for (m, tint) in &gems {
            let (file, name, ior, dispersion, transmission) = if tint[0] > 0.5 { ("reference-ruby.stl", "Ruby", 1.77, 0.018, 0.72) } else { ("reference-garnet.stl", "Garnet", 1.79, 0.024, 0.62) };
            stl::write_stl(out.join(file), m, &format!("Rosa mortua reference {name}"))?;
            entries.push(json!({"mesh": file, "name": name, "tint": tint, "ior": ior, "dispersion": dispersion, "roughness": 0.065, "transmission": transmission}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": entries}))?)?;
    }
    renders(&out, &d, &lib, &finished, if draft { 1000 } else { 1600 })?;
    let _ = setting::claw_count;
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().filter(|(g, _)| !g.starts_with("lost-wax wall census")).all(|(_, p)| *p), "Rosa mortua failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
