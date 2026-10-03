//! Tenebrae — Sigillum, the chapter seal: a cathedral chapter's seal ring on factory 005 Rosette, whose cusped table
//! is the seal's own cusped border. The seal is cut at the bench, mirrored intaglio with drafted walls: a cusped
//! quatrefoil field, a fleur-de-lis and the chapter's legend. What casts is cast along the pull: a lancet arcade in
//! each cheek and graded quatrefoil roundels on the shoulders' side faces. Delft sand through the sand master.
//! cargo build --release -p ringdesign-core --example tenebrae_sigillum
//! target/release/examples/tenebrae_sigillum [OUT_DIR] [--draft] [--verify] [--blockout] [--probe]
#![recursion_limit = "256"]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, FeatureStatus},
    castability::{self, SandProcess},
    csg, dfm,
    imported_base::{ImportedBase, PRESETS, SurfaceChart, sand_master},
    library, manufacturing as mf, mesh, render,
    setting::{Stamp, StampTop},
    skin::{Atlas, Hide},
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const AW: usize = 2048;
const AH: usize = 768;

/// The face asked for, length round the ring by width across it (005's native 18 x 21, its own aspect).
/// Factory 017 Tonneau, the section's fallback: 005 Rosette's head is a domed four-lobed pillow (1.1 mm of sag 4 mm
/// off its crown, and 0.67 mm of envelope fill), which leaves no table to cut a seal in.
const STOCK: &str = "017";
const FACE: (f64, f64) = (16.0, 15.6);
fn face() -> (f64, f64) {
    match (std::env::var("SIG_FL").ok().and_then(|v| v.parse().ok()), std::env::var("SIG_FW").ok().and_then(|v| v.parse().ok())) {
        (Some(l), Some(w)) => (l, w),
        _ => FACE,
    }
}
const BORE_MM: f64 = 18.6;
const ALLOY: &str = "Silver 925";

/// The cheek's lancet arcade, a chapter house's blind arcade: five lights `LANCET_W` wide at `LANCET_PITCH` centres,
/// on one level sill, the centre light the tallest and each one out from it a step lower, as a chapter house grades them.
const LANCET_W: f64 = 1.3;
const LANCETS: usize = 5;
const LANCET_PITCH: f64 = 2.2;
/// Each light stands from just above the wall's foot to `HEAD_Y` (mm up from the finger axis), under the table's edge.
const HEAD_Y: f64 = 12.9;
const SILL_CLEAR_MM: f64 = 0.2;
/// Metal kept between the wall's foot and the bore: lost wax's 0.8 mm section and a little over.
const LANCET_BORE_LAND_MM: f64 = 0.9;
/// How squarely the wall must face the cheek's way for a light to stand on it: the lights run down over the
/// wall's roll toward the bore, as only a bench cut can.
const CHEEK_FACING: f64 = 0.6;
/// Each light out from the centre stands this much lower at its head.
const HEAD_STEP_MM: f64 = 0.15;
/// Longest edge any outline keeps, mm.
const OUTLINE_STEP_MM: f64 = 0.4;
const ARCADE_SINK_MM: f64 = 0.5;
/// The lancet's head: each flank struck at this many spans' radius, an acute Early English point.
const ACUTE: f64 = 1.4;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The ring's own Delft pour: z = 0 parting, opposed z withdrawal, sterling silver.
fn setup() -> mf::Setup {
    let mut s = sand_setup();
    // Logan's rule (2026-10-03): judged as lost wax, 0.8 mm minimum section and no pull rule. The Delft pour is
    // still measured, as a bonus.
    s.recipe.process = castability::CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.name = "Sigillum / lost wax / Silver 925".into();
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = 0.8;
    s
}

/// The same ring poured in Delft sand, z = 0 parting, opposed z withdrawal.
fn sand_setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.name = "Sigillum / Delft clay / Silver 925".into();
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Factory 005 Rosette through the sand master, z = 0 parting, opposed z withdrawal. The cheek arcades and \
        shoulder roundels are struck along the pull and cast. The table pours plain: after the pour the seal is cut at the \
        bench as mirrored intaglio with drafted walls (the quatrefoil field 0.30 deep at 15 deg, the fleur-de-lis 0.70 at \
        12 deg, the legend 0.40), all centred on the table's centre on the parting line. Polish the table and cheeks; \
        leave the seal's floors satin so a wax impression lifts clean."
        .into();
    s
}

/// Factory 005 Rosette through the sand master at `face` (length, width), on its own Flat chart.
fn stock(face: (f64, f64)) -> Result<RingDesign> {
    let id = std::env::var("SIG_STOCK").unwrap_or_else(|_| STOCK.into());
    let preset = PRESETS.iter().find(|p| p.id == id).context("no stock")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, if std::env::var("SIG_RAW").is_ok() { preset.load()? } else { sand_master(preset.load()?)? })?;
    d.imported_base.as_mut().unwrap().sand_envelope = std::env::var("SIG_NOENV").is_err();
    d.name = "Sigillum \u{2014} the chapter seal".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = face.1;
    d.shank.head.length_mm = face.0;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = export_params();
    let s = setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.manufacturing = Some(s);
    Ok(d)
}

/// The design as the Delft pour would judge it.
fn as_sand(d: &RingDesign) -> RingDesign {
    let mut d = d.clone();
    let s = sand_setup();
    d.draft.process = s.recipe.process;
    d.draft.sand = s.recipe.sand;
    d.draft.min_detail_mm = s.recipe.min_detail_mm;
    d.draft.min_section_mm = s.recipe.min_section_mm;
    d.draft.min_draft_deg = s.recipe.min_draft_deg;
    d.manufacturing = Some(s);
    d
}

/// The worst sand the envelope must add anywhere round the ring, mm, and where (`stock_spike`'s measure).
fn fill_mm(d: &RingDesign) -> (f64, f64) {
    let mut worst = (0.0, 0.0);
    for k in 0..360 {
        let theta = k as f64 + 0.5;
        let l = d.section_at(theta, 512, None, None);
        let n = l.pts.len();
        let mut reach = [vec![f64::MIN; 400], vec![f64::MIN; 400]];
        for i in 0..n {
            let (p, q) = (&l.pts[i], &l.pts[(i + 1) % n]);
            let steps = ((q.z - p.z).abs() / 0.01).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (r, z) = (p.r + (q.r - p.r) * t, p.z + (q.z - p.z) * t);
                let bin = (z.abs() / 0.05) as usize;
                if bin < 400 {
                    let side = &mut reach[usize::from(z < 0.0)];
                    side[bin] = side[bin].max(r);
                }
            }
        }
        for side in &reach {
            let mut beyond = f64::MIN;
            for r in side.iter().rev().filter(|r| **r > f64::MIN) {
                beyond = beyond.max(*r);
                if beyond - r > worst.0 {
                    worst = (beyond - r, theta);
                }
            }
        }
    }
    worst
}

fn release_line(r: &mf::release::ReleaseReport) -> String {
    let depth = r.obstructions.iter().map(|o| o.depth_mm).fold(0.0, f64::max);
    format!("{:?}, {} obstructions (deepest {depth:.3} mm), {} unresolved", r.status, r.obstructions.len(), r.unresolved_rays)
}

/// The ring's own pull at `params`: the inspection at 0.100 mm and the rays again at 0.075 mm.
fn pull(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(mf::Inspection, mf::release::ReleaseReport)> {
    let s = d.manufacturing.clone().unwrap_or_else(setup);
    let inspection = mf::inspect(d, lib, &s, params)?;
    let mut fine = s.clone();
    fine.sample_pitch_mm = 0.075;
    let release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    Ok((inspection, release))
}

// --- The seal -----------------------------------------------------------------------------------------------------

fn add2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

/// Points along the circle about `c` of radius `r` from angle `a0` to `a1` (radians), `n` steps, both ends included.
fn arc(c: [f64; 2], r: f64, a0: f64, a1: f64, n: usize) -> Vec<[f64; 2]> {
    (0..=n).map(|k| a0 + (a1 - a0) * k as f64 / n as f64).map(|a| add2(c, [r * a.cos(), r * a.sin()])).collect()
}

/// The seal's quatrefoil field: four pointed foils on the axes, each a pointed arch reaching `reach` from the centre
/// over the chord between two inner cusps that stand `cusp` out on the diagonals. Counter-clockwise.
fn quatrefoil_field(reach: f64, cusp: f64) -> Vec<[f64; 2]> {
    let k = cusp / 2f64.sqrt();
    // The foil on +x: two arcs from the cusps (k, -k) and (k, k) meeting in a point at (reach, 0).
    let h = reach - k;
    let s = (h * h - k * k) / (2.0 * k);
    let rho = k + s;
    let lower = arc([k, s], rho, -PI * 0.5, (-s).atan2(h), 40);
    let upper = arc([k, -s], rho, s.atan2(h), PI * 0.5, 40);
    let mut foil: Vec<[f64; 2]> = lower;
    foil.extend(upper.into_iter().skip(1));
    foil.pop();
    let mut pts = Vec::new();
    for q in 0..4 {
        let (sn, co) = (PI * 0.5 * q as f64).sin_cos();
        pts.extend(foil.iter().map(|p| [p[0] * co - p[1] * sn, p[0] * sn + p[1] * co]));
    }
    pts
}

/// A blade from `p0` to `p1`, `w` across at its widest a share `at` of the way along, pointed at its tip and at its
/// root unless `blunt` holds the root open that wide. Counter-clockwise.
fn blade(p0: [f64; 2], p1: [f64; 2], w: f64, at: f64, blunt: f64) -> Vec<[f64; 2]> {
    blade_tip(p0, p1, w, at, blunt, true)
}

/// [`blade`], its tip a straight sharp point or, unless `sharp`, rounded like its root.
fn blade_tip(p0: [f64; 2], p1: [f64; 2], w: f64, at: f64, blunt: f64, sharp: bool) -> Vec<[f64; 2]> {
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let l = dx.hypot(dy);
    let (d, n) = ([dx / l, dy / l], [-dy / l, dx / l]);
    let n_steps = ((l / 0.05).ceil() as usize).max(16);
    let half = |t: f64| {
        let u = if t < at { t / at } else { 1.0 - (t - at) / (1.0 - at) };
        let u = u.clamp(0.0, 1.0);
        let base = if t < at { 0.5 * blunt * (1.0 - t / at) } else { 0.0 };
        // Rounded below its widest, then a straight taper to a sharp point.
        let shape = if t < at || !sharp { (u * (2.0 - u)).sqrt() } else { u.powf(0.85) };
        (0.5 * w * shape).max(base)
    };
    let at_t = |t: f64, side: f64| [p0[0] + d[0] * l * t + side * n[0] * half(t), p0[1] + d[1] * l * t + side * n[1] * half(t)];
    let first = if blunt > 0.0 { 0 } else { 1 };
    let mut out: Vec<[f64; 2]> = (first..=n_steps).map(|k| at_t(k as f64 / n_steps as f64, -1.0)).collect();
    out.extend((first..n_steps).rev().map(|k| at_t(k as f64 / n_steps as f64, 1.0)));
    if blunt <= 0.0 {
        out.insert(0, p0);
    }
    out.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// A petal arched along a circle about `c` of radius `r` from `a0` to `a1` (degrees), `w0` wide at its root and
/// running to a point. Counter-clockwise.
fn curl(c: [f64; 2], r: f64, a0: f64, a1: f64, w0: f64) -> Vec<[f64; 2]> {
    let n = 48;
    let at = |t: f64, side: f64| {
        let a = (a0 + (a1 - a0) * t).to_radians();
        let w = 0.5 * w0 * (1.0 - t).powf(0.8);
        let rr = r + side * w;
        [c[0] + rr * a.cos(), c[1] + rr * a.sin()]
    };
    let mut out: Vec<[f64; 2]> = (0..n).map(|k| at(k as f64 / n as f64, 1.0)).collect();
    out.push(at(1.0, 0.0));
    out.extend((0..n).rev().map(|k| at(k as f64 / n as f64, -1.0)));
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// A rounded bar from `x0` to `x1` and `y0` to `y1`, its corners rounded `r`. Counter-clockwise.
fn bar(x0: f64, x1: f64, y0: f64, y1: f64, r: f64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for (c, a) in [([x1 - r, y0 + r], -90.0f64), ([x1 - r, y1 - r], 0.0), ([x0 + r, y1 - r], 90.0), ([x0 + r, y0 + r], 180.0)] {
        out.extend(arc(c, r, a.to_radians(), (a + 90.0).to_radians(), 8));
    }
    out.dedup();
    out
}

/// One part of the seal as it reads in the wax: its outline in the seal's own plane (x right and y up in the face
/// view, mm from the table's centre), its depth and its floor. The matrix cuts each mirrored.
struct Part {
    name: String,
    outline: Vec<[f64; 2]>,
    depth: f64,
    top: StampTop,
}

/// The chapter's fleur-de-lis, `height` from the foot's point to the centre petal's tip, +y up the centre petal: a
/// tall pointed centre petal, two side petals arching out and down to open points, a straight band, and a foot of
/// three spikes. No petal closes on itself.
fn fleur(height: f64, depth: f64) -> Vec<Part> {
    let (lo, hi) = (-2.5, 3.3);
    let k = height / (hi - lo);
    let mid = 0.5 * (lo + hi);
    let fitp = |q: [f64; 2]| [q[0] * k, (q[1] - mid) * k];
    let fit = |p: Vec<[f64; 2]>| p.into_iter().map(fitp).collect::<Vec<_>>();
    // Each part is a modelled hollow: shallow at its walls, falling to a spine or a point, so its two sides catch the
    // light apart in any view.
    let wall = depth - FLEUR_MODEL_MM;
    // A pillowed hollow: creaseless, deepest along the part's middle.
    let spine = |_a: [f64; 2], _b: [f64; 2]| StampTop::Pillow { crown_mm: FLEUR_MODEL_MM };
    let part = |name: &str, outline: Vec<[f64; 2]>, top: StampTop| Part { name: format!("Seal fleur, {name}"), outline: fit(outline), depth: wall, top };
    let mut parts = vec![
        part("centre petal", blade([0.0, -0.75], [0.0, hi], 1.5, 0.42, 0.9), spine([0.0, -0.1], [0.0, 3.0])),
        part("band", bar(-1.8, 1.8, -1.05, -0.35, 0.34), spine([-1.6, -0.7], [1.6, -0.7])),
        // The foot stands clear of the band by a land, as the spikes do.
        // The foot hangs from the band's underside, one figure with it.
        part("foot", blade_tip([0.0, -0.9], [0.0, lo + 0.25], 1.05, 0.4, 0.85, false), spine([0.0, -1.5], [0.0, lo + 0.2])),
    ];
    for (side, name) in [(1.0f64, "right"), (-1.0, "left")] {
        let flip = |q: [f64; 2]| [side * q[0], q[1]];
        let m = |p: Vec<[f64; 2]>| {
            let mut q: Vec<[f64; 2]> = p.into_iter().map(flip).collect();
            if side < 0.0 {
                q.reverse();
            }
            q
        };
        // The side petal springs from the band beside the centre petal's foot and curls out and down.
        parts.push(part(&format!("{name} petal"), m(curl([1.95, -0.15], 1.05, 188.0, 2.0, 0.8)), StampTop::Pillow { crown_mm: FLEUR_MODEL_MM }));
        parts.push(part(&format!("{name} spike"), m(blade_tip([0.55, -0.95], [1.05, -2.0], 0.72, 0.35, 0.65, false)), spine(flip([0.7, -1.5]), flip([1.45, -2.2]))));
    }
    parts
}

/// A closed loop with points nearer than `gap` to the last kept dropped, then thinned (Douglas-Peucker) to within
/// `tol` of itself: a slice's slivers gone, its curves kept.
fn simplify(l: &[[f64; 2]], tol: f64, gap: f64) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for p in l {
        if pts.last().is_none_or(|q: &[f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]) >= gap) {
            pts.push(*p);
        }
    }
    while pts.len() > 3 && (pts[0][0] - pts[pts.len() - 1][0]).hypot(pts[0][1] - pts[pts.len() - 1][1]) < gap {
        pts.pop();
    }
    fn dp(p: &[[f64; 2]], tol: f64, out: &mut Vec<[f64; 2]>) {
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy).max(1e-12);
        let (mut worst, mut at) = (0.0, 0);
        for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
            let d = ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / l;
            if d > worst {
                (worst, at) = (d, i);
            }
        }
        if worst > tol {
            dp(&p[..=at], tol, out);
            dp(&p[at..], tol, out);
        } else {
            out.push(a);
        }
    }
    // Split the loop at its farthest point from the start, so both halves are open runs.
    let far = (0..pts.len()).max_by(|i, j| {
        let d = |k: usize| (pts[k][0] - pts[0][0]).hypot(pts[k][1] - pts[0][1]);
        d(*i).total_cmp(&d(*j))
    }).unwrap_or(0);
    let mut out = Vec::new();
    let mut first = pts[..=far].to_vec();
    if first.len() >= 2 { dp(&first, tol, &mut out); } else { out.extend(first.drain(..)); }
    let mut second = pts[far..].to_vec();
    second.push(pts[0]);
    if second.len() >= 2 { dp(&second, tol, &mut out); }
    out
}

/// The union of overlapping counter-clockwise outlines as closed loops, the largest first: each outline stood up as
/// a prism, the prisms united by the kernel's boolean, and the union sliced through its middle.
fn union_outline(polys: &[Vec<[f64; 2]>]) -> Result<Vec<Vec<[f64; 2]>>> {
    let prism = |poly: &Vec<[f64; 2]>| -> Result<csg::Solid> {
        let n = poly.len();
        let key = |p: [f64; 2]| ((p[0] * 1e7).round() as i64, (p[1] * 1e7).round() as i64);
        let index: std::collections::HashMap<(i64, i64), u32> = poly.iter().enumerate().map(|(i, p)| (key(*p), i as u32)).collect();
        let mut s = csg::Solid::default();
        s.v.extend(poly.iter().map(|p| [p[0], p[1], -0.5]));
        s.v.extend(poly.iter().map(|p| [p[0], p[1], 0.5]));
        for t in ringdesign_core::sketch::fill::triangulate(&[poly.clone()]) {
            let ids: Vec<u32> = t.iter().map(|p| index.get(&key(*p)).copied()).collect::<Option<_>>().context("a cap point off the outline")?;
            let ccw = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]) > 0.0;
            let (a, b, c) = if ccw { (ids[0], ids[1], ids[2]) } else { (ids[0], ids[2], ids[1]) };
            s.f.push([a + n as u32, b + n as u32, c + n as u32]);
            s.f.push([a, c, b]);
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            s.f.push([i, j, j + n as u32]);
            s.f.push([i, j + n as u32, i + n as u32]);
        }
        Ok(s)
    };
    let solids: Vec<csg::Solid> = polys.iter().map(prism).collect::<Result<_>>()?;
    let u = csg::union_all(&solids).map_err(|e| anyhow::anyhow!("union: {e:?}"))?;
    // Slice at z = 0: each triangle crossing it gives one segment, chained into loops by their shared ends.
    let mut segs: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for f in &u.f {
        let p = f.map(|i| u.v[i as usize]);
        let mut pts = Vec::new();
        for k in 0..3 {
            let (a, b) = (p[k], p[(k + 1) % 3]);
            if (a[2] < 0.0) != (b[2] < 0.0) {
                let t = a[2] / (a[2] - b[2]);
                pts.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            }
        }
        if pts.len() == 2 {
            // Oriented so the solid lies to the left: along the face normal crossed with +z.
            let (e1, e2) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
            let nrm = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2]];
            let dir = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1]];
            // The outward normal points right of the walk on a counter-clockwise loop.
            if dir[0] * nrm[1] - dir[1] * nrm[0] > 0.0 { segs.push((pts[1], pts[0])) } else { segs.push((pts[0], pts[1])) }
        }
    }
    let key = |p: [f64; 2]| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    let mut next: std::collections::HashMap<(i64, i64), usize> = segs.iter().enumerate().map(|(i, s)| (key(s.0), i)).collect();
    let mut loops = Vec::new();
    let mut used = vec![false; segs.len()];
    for start in 0..segs.len() {
        if used[start] {
            continue;
        }
        let mut l = Vec::new();
        let mut i = start;
        while !used[i] {
            used[i] = true;
            l.push(segs[i].0);
            match next.remove(&key(segs[i].1)) {
                Some(j) => i = j,
                None => break,
            }
        }
        let l = simplify(&l, std::env::var("SIG_TOL").ok().and_then(|v| v.parse().ok()).unwrap_or(0.003), std::env::var("SIG_GAP").ok().and_then(|v| v.parse().ok()).unwrap_or(0.02));
        if l.len() >= 3 {
            loops.push(l);
        }
    }
    loops.sort_by(|a, b| signed_area(b).abs().total_cmp(&signed_area(a).abs()));
    Ok(loops)
}

/// One glyph region as one closed outline: each counter joined to the outline by a keyhole, so the cut leaves a
/// hairline of metal across the stroke where the counter's island holds on. Counter-clockwise.
fn keyholed(mut outer: Vec<[f64; 2]>, holes: Vec<Vec<[f64; 2]>>) -> Vec<[f64; 2]> {
    if signed_area(&outer) < 0.0 {
        outer.reverse();
    }
    for mut hole in holes {
        if signed_area(&hole) > 0.0 {
            hole.reverse();
        }
        let d2 = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
        let (mut bi, mut bj, mut best) = (0, 0, f64::MAX);
        for (i, a) in outer.iter().enumerate() {
            for (j, b) in hole.iter().enumerate() {
                let dd = d2(*a, *b);
                if dd < best {
                    (bi, bj, best) = (i, j, dd);
                }
            }
        }
        let n = hole.len();
        let mut merged: Vec<[f64; 2]> = outer[..=bi].to_vec();
        // Round the counter from the bridge's foot, stopping one point short, and back out beside the way in.
        merged.extend((0..n).map(|k| hole[(bj + k) % n]));
        merged.extend(outer[bi + 1..].iter().copied());
        outer = merged;
    }
    outer
}

/// The legend in real Textura (UnifrakturMaguntia through `sketch::text`), reading clockwise from the cross at
/// the top with the letters standing outward, as the wax reads it, fitted between radii `r_in` and `r_out` with
/// its tracking opened until it closes the circle. One pillowed cut per glyph region.
fn textura_legend(text: &str, r_in: f64, r_out: f64, depth: f64) -> Result<Vec<Part>> {
    use ringdesign_core::sketch::text::{TextAlign, TextArc, TextLayout};
    let lay = |cap: f64, radius: f64, tracking: f64| -> Result<Vec<(Vec<Vec<[f64; 2]>>, [f64; 2])>> {
        let mut l = TextLayout::new(ringdesign_core::text::TextFont::Textura, text, cap);
        l.arc = Some(TextArc { radius_mm: radius, start_deg: 90.0 + LEGEND_LEAD_DEG, clockwise: true });
        l.align = TextAlign::Start;
        l.tracking = tracking;
        let mut out = Vec::new();
        for sk in l.parts()? {
            for r in sk.profile_regions()? {
                let inside = r.inside().unwrap_or([0.0, 0.0]);
                out.push((r.polygons(LEGEND_CHORD_MM), inside));
            }
        }
        Ok(out)
    };
    let extent = |g: &[(Vec<Vec<[f64; 2]>>, [f64; 2])]| {
        let rs = g.iter().flat_map(|x| x.0.iter()).flatten().map(|p| p[0].hypot(p[1]));
        rs.fold((f64::MAX, f64::MIN), |(lo, hi), r| (lo.min(r), hi.max(r)))
    };
    // How far round the circle the text runs, clockwise from the start, degrees.
    let sweep = |g: &[(Vec<Vec<[f64; 2]>>, [f64; 2])]| {
        let start = 90.0 + LEGEND_LEAD_DEG;
        g.iter().flat_map(|x| x.0.iter()).flatten().map(|p| (start - p[1].atan2(p[0]).to_degrees()).rem_euclid(360.0)).filter(|a| *a < 359.0).fold(0.0, f64::max)
    };
    // Fit the band: size the cap to the band's depth, then set the baseline so the text sits in it.
    let (mut cap, mut radius) = (1.0, 0.5 * (r_in + r_out));
    for _ in 0..3 {
        let g = lay(cap, radius, 0.0)?;
        let (lo, hi) = extent(&g);
        cap *= (r_out - r_in) / (hi - lo);
        radius += 0.5 * (r_in + r_out) - 0.5 * (lo + hi);
    }
    // Open the tracking until the legend runs round to just short of the cross again.
    let (mut t0, mut t1) = (0.0, 0.6);
    for _ in 0..24 {
        let t = 0.5 * (t0 + t1);
        // A layout that would run past one turn is refused: too far as well.
        let short = lay(cap, radius, t).map_or(false, |g| sweep(&g) < 360.0 - 2.0 * LEGEND_LEAD_DEG - LEGEND_GAP_DEG);
        if short { t0 = t } else { t1 = t }
    }
    let g = lay(cap, radius, t0)?;
    Ok(g
        .into_iter()
        .enumerate()
        .map(|(k, (mut loops, _inside))| {
            let outer = loops.remove(0);
            // A gable floor whose ridge runs out along the radius through the glyph's middle: every stroke either
            // side of it tilts, so the letter reads head-on as well as raked.
            let n = outer.len() as f64;
            let c = outer.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
            let r = c[0].hypot(c[1]).max(1e-9);
            let u = [c[0] / r, c[1] / r];
            let (from, to) = ([u[0] * (r_in - 0.3), u[1] * (r_in - 0.3)], [u[0] * (r_out + 0.3), u[1] * (r_out + 0.3)]);
            Part { name: format!("Legend, glyph {}", k + 1), outline: keyholed(outer, loops), depth: depth - LEGEND_PILLOW_MM, top: StampTop::Ridge { rise_mm: LEGEND_PILLOW_MM, from, to, end_mm: LEGEND_PILLOW_MM } }
        })
        .collect())
}

/// A band `w` wide along the arc about `c` of radius `r` from `a0` to `a1` (radians). Counter-clockwise.
fn arc_band(c: [f64; 2], r: f64, a0: f64, a1: f64, w: f64) -> Vec<[f64; 2]> {
    let n = (((a1 - a0).abs() * r / 0.2).ceil() as usize).max(6);
    let mut out = arc(c, r + 0.5 * w, a0, a1, n);
    out.extend(arc(c, r - 0.5 * w, a1, a0, n));
    out.dedup();
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// A graver's V along the arc about `c` of radius `r` from `a0` to `a1`, cut `depth` at its spine.
fn arc_vee(c: [f64; 2], r: f64, a0: f64, a1: f64, depth: f64) -> StampTop {
    let (p0, p1) = ([c[0] + r * a0.cos(), c[1] + r * a0.sin()], [c[0] + r * a1.cos(), c[1] + r * a1.sin()]);
    StampTop::Ridge { rise_mm: depth - V_WALL_MM, from: p0, to: p1, end_mm: depth - V_WALL_MM }
}

/// A V-cut rule along an arc in runs short enough that each run's spine stays on the arc, each overlapping the next
/// and every other one a hundredth of a millimetre further out, so no two walls coincide.
fn rule(name: &str, c: [f64; 2], r: f64, a0: f64, a1: f64, w: f64, depth: f64, out: &mut Vec<Part>) {
    let runs = (((a1 - a0).abs() * r / 1.1).ceil() as usize).max(1);
    let lap = 0.12 / r;
    for q in 0..runs {
        let (b0, b1) = (a0 + (a1 - a0) * q as f64 / runs as f64, a0 + (a1 - a0) * (q + 1) as f64 / runs as f64);
        let (b0, b1) = (if q == 0 { b0 } else { b0 - lap }, if q + 1 == runs { b1 } else { b1 + lap });
        let rq = if q % 2 == 0 { r } else { r + 0.012 };
        out.push(Part { name: format!("{name}, {}", q + 1), outline: arc_band(c, rq, b0, b1, w), depth: V_WALL_MM, top: arc_vee(c, rq, b0, b1, depth) });
    }
}

/// The legend's outer frame, a cusped circle: `lobes` round foils bulging `bulge` out from the circle through their
/// cusps to touch a plain outer circle, so each spandrel between two foils and the circle is a sharp Gothic cusp. The
/// rosette's cusped frame drawn as the seal's own border.
fn frame(outer: f64, bulge: f64, lobes: usize, w: f64, depth: f64) -> Vec<Part> {
    let mut parts = Vec::new();
    for k in 0..lobes {
        let (b0, b1) = (2.0 * PI * k as f64 / lobes as f64, 2.0 * PI * (k + 1) as f64 / lobes as f64);
        let bm = 0.5 * (b0 + b1);
        let apex = [(outer + bulge) * bm.cos(), (outer + bulge) * bm.sin()];
        // The circle through both cusps and the apex.
        let chord = outer * (0.5 * (b1 - b0)).sin();
        let sag = (outer + bulge) - outer * (0.5 * (b1 - b0)).cos();
        let r = (chord * chord + sag * sag) / (2.0 * sag);
        let c = [apex[0] - r * bm.cos(), apex[1] - r * bm.sin()];
        let half = (chord / r).asin();
        // Each foil runs past its cusps far enough that neighbours cross square, not at a graze.
        let lap = 0.06;
        rule(&format!("Cusped border, foil {}", k + 1), c, r, bm - half - lap, bm + half + lap, w, depth, &mut parts);
    }
    // Each cusp runs on into the legend band as a tapering point, so the cusping reads as points and not as scallops.
    for k in 0..lobes {
        let b = 2.0 * PI * k as f64 / lobes as f64;
        let (p0, p1) = ([(outer + 0.1) * b.cos(), (outer + 0.1) * b.sin()], [(outer - CUSP_POINT_MM) * b.cos(), (outer - CUSP_POINT_MM) * b.sin()]);
        parts.push(Part { name: format!("Cusped border, point {}", k + 1), outline: blade(p0, p1, 2.4 * w, 0.12, 0.0), depth: V_WALL_MM, top: StampTop::Ridge { rise_mm: depth - V_WALL_MM, from: p0, to: p1, end_mm: 0.0 } });
    }
    // The circle the foils touch, overlapping their points so its rule and theirs cross cleanly.
    let off = 0.3;
    rule("Cusped border, circle", [0.0, 0.0], outer + bulge + off * w, 0.0, 2.0 * PI, w, depth, &mut parts);
    parts
}

/// The quatrefoil field's eight arcs, as `quatrefoil_field` draws them: centre, radius and the angles they run between.
fn quatrefoil_arcs(reach: f64, cusp: f64) -> Vec<([f64; 2], f64, f64, f64)> {
    let k = cusp / 2f64.sqrt();
    let h = reach - k;
    let s = (h * h - k * k) / (2.0 * k);
    let rho = k + s;
    let mut out = Vec::new();
    for q in 0..4 {
        let turn = PI * 0.5 * q as f64;
        let (sn, co) = turn.sin_cos();
        let rot = |p: [f64; 2]| [p[0] * co - p[1] * sn, p[0] * sn + p[1] * co];
        out.push((rot([k, s]), rho, -PI * 0.5 + turn, (-s).atan2(h) + turn));
        out.push((rot([k, -s]), rho, s.atan2(h) + turn, PI * 0.5 + turn));
    }
    out
}

fn signed_area(p: &[[f64; 2]]) -> f64 {
    0.5 * (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
}

/// The floor of a mirrored outline, mirrored with it.
fn mirror_top(t: StampTop) -> StampTop {
    match t {
        StampTop::Cone { apex_mm, at, tip_mm } => StampTop::Cone { apex_mm, at: [-at[0], at[1]], tip_mm },
        other => other.mirrored(),
    }
}

/// Mirror an outline left for right, as a seal is cut so its impression reads true, keeping it counter-clockwise.
fn mirrored(p: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = p.iter().map(|q| [-q[0], q[1]]).collect();
    out.reverse();
    out
}

/// An outline with every edge longer than `step` split evenly, so its walls drape onto a curved surface.
fn densify(outline: &[[f64; 2]], step: f64) -> Vec<[f64; 2]> {
    let n = outline.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let k = ((a[0] - b[0]).hypot(a[1] - b[1]) / step).ceil().max(1.0) as usize;
        out.extend((0..k).map(|j| {
            let t = j as f64 / k as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        }));
    }
    out
}

/// A bench intaglio cut: struck into the table after the pour, never in the pattern.
fn intaglio(name: &str, at: (f64, f64), rot_deg: f64, outline: Vec<[f64; 2]>, depth: f64, top: StampTop) -> Stamp {
    let outline = densify(&outline, if name == "Seal fleur" { std::env::var("SIG_FSTEP").ok().and_then(|v| v.parse().ok()).unwrap_or(OUTLINE_STEP_MM) } else { OUTLINE_STEP_MM });
    Stamp {
        name: name.into(),
        theta_deg: at.0,
        v_mm: at.1,
        rot_deg,
        outline,
        height_mm: 0.1,
        sink_mm: depth,
        draft_deg: 0.0,
        cut: true,
        bench: true,
        along_pull: false,
        fine_cap: !top.is_flat(),
        tier: 0,
        top,
    }
}

/// The seal's geometry, how the wax reads it: the quatrefoil field, the fleur, the legend and its frame, mm.
const FIELD_REACH_MM: f64 = 4.1;
const FIELD_CUSP_MM: f64 = 2.75;
const FIELD_DEPTH_MM: f64 = 0.40;
const FLEUR_MM: f64 = 6.3;
const FLEUR_DEPTH_MM: f64 = 0.95;
/// How far each fleur hollow falls from its walls to its spine.
const FLEUR_MODEL_MM: f64 = 0.35;
const LEGEND: &str = "\u{2720} sigillum capituli ecclesie cathedralis";
/// The legend's band, from the quatrefoil's points out to the border's cusps.
const LEGEND_IN_MM: f64 = 4.45;
const LEGEND_OUT_MM: f64 = 5.55;
const LEGEND_DEPTH_MM: f64 = 0.42;
/// How far each glyph's floor dishes below its walls, so a letter catches the light in any view.
const LEGEND_PILLOW_MM: f64 = 0.12;
/// Chord to which the glyphs' curves are walked, mm.
const LEGEND_CHORD_MM: f64 = 0.06;
/// The cross's lead past the top, and the gap left before it at the legend's end, degrees.
const LEGEND_LEAD_DEG: f64 = 4.0;
const LEGEND_GAP_DEG: f64 = 3.0;
const BORDER_RADIUS_MM: f64 = 6.42;
const BORDER_BULGE_MM: f64 = 0.7;
const BORDER_LOBES: usize = 12;
/// How far each cusp's point runs in past the circle through the cusps.
const CUSP_POINT_MM: f64 = 0.8;
const RULE_MM: f64 = 0.3;
const RULE_DEPTH_MM: f64 = 0.5;
/// A graver's V-cut stands this tall at its walls; its spine runs to the cut's full depth.
const V_WALL_MM: f64 = 0.3;

fn seal_parts(with_legend: bool) -> Vec<Part> {
    let mut parts = vec![Part { name: "Seal field".into(), outline: quatrefoil_field(FIELD_REACH_MM, FIELD_CUSP_MM), depth: FIELD_DEPTH_MM, top: StampTop::Flat }];
    // The field's edge graved round, so the four pointed foils frame the fleur in any light.
    for (k, (c, r, a0, a1)) in quatrefoil_arcs(FIELD_REACH_MM, FIELD_CUSP_MM).into_iter().enumerate() {
        // Just outside the field's wall, so the rule's walls and the field's never meet edge on edge.
        rule(&format!("Quatrefoil rule {}", k + 1), c, r + 0.5 * RULE_MM + 0.04, a0, a1, RULE_MM, RULE_DEPTH_MM, &mut parts);
    }
    // The fleur cut as one figure: its petals, band and foot united into a single outline, one modelled hollow.
    let pieces = fleur(FLEUR_MM, FLEUR_DEPTH_MM);
    let band = pieces.iter().find(|p| p.name.ends_with("band")).map(|p| p.outline.clone());
    let mut loops = union_outline(&pieces.iter().map(|p| p.outline.clone()).collect::<Vec<_>>()).expect("the fleur unites");
    if std::env::var("SIG_DEBUG").is_ok() {
        std::fs::write("/tmp/claude-0/fleur.json", serde_json::to_string(&loops).unwrap()).ok();
        eprintln!("fleur union: {} loops, sizes {:?}, crossing {:?}, check {:?}, area {:.3}", loops.len(), loops.iter().map(|l| l.len()).collect::<Vec<_>>(), ringdesign_core::outline::self_crossing(&loops[0]), ringdesign_core::outline::check(&loops[0]), signed_area(&loops[0]));
    }
    let outer = loops.remove(0);
    parts.push(Part { name: "Seal fleur".into(), outline: keyholed(outer, loops), depth: FLEUR_DEPTH_MM - FLEUR_MODEL_MM, top: StampTop::Pillow { crown_mm: FLEUR_MODEL_MM } });
    // The band drawn as an engraver draws it: a graved line along its top and one along its foot, cut below the
    // figure's floor, so the bar reads across the fleur.
    if let Some(band) = band {
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for p in &band {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let inset = 0.5 * (hi[1] - lo[1]);
        for (k, y) in [hi[1] - 0.06, lo[1] + 0.06].into_iter().enumerate() {
            let (p0, p1) = ([lo[0] + inset, y], [hi[0] - inset, y]);
            parts.push(Part { name: format!("Seal fleur, band line {}", k + 1), outline: path_band(&[p0, p1], 0.16), depth: FLEUR_DEPTH_MM + 0.05, top: StampTop::Flat });
        }
    }
    parts.extend(frame(BORDER_RADIUS_MM, BORDER_BULGE_MM, BORDER_LOBES, RULE_MM, RULE_DEPTH_MM));
    if with_legend {
        parts.extend(textura_legend(LEGEND, LEGEND_IN_MM, LEGEND_OUT_MM, LEGEND_DEPTH_MM).expect("the Textura legend lays out"));
    }
    parts
}

/// The seal, cut at the bench into the table's centre on the parting line, mirrored so the impression reads true.
/// Every cut drapes on the barrelled table at an even depth.
fn seal(d: &mut RingDesign, a: &Atlas, with_legend: bool) -> Result<()> {
    let at = Hide::of(a).crest_at(a, 0.0);
    // Turned so the seal stands upright in the face view.
    let turn = 180.0;
    for p in seal_parts(with_legend) {
        d.stamps.push(intaglio(&p.name, at, turn, mirrored(&p.outline), p.depth, mirror_top(p.top)));
    }
    Ok(())
}

// --- The cast stamps ----------------------------------------------------------------------------------------------

/// A lancet light, `w` wide and `h` tall: straight jambs and an acute pointed head struck from two centres `ACUTE`
/// spans apart, the point at +y. Counter-clockwise from the sill's left corner.
fn lancet(w: f64, h: f64) -> Vec<[f64; 2]> {
    let half = 0.5 * w;
    let mut rho = ACUTE * w;
    let mut rise = (rho * rho - (rho - half) * (rho - half)).sqrt();
    // A short light keeps a jamb: its head eases toward equilateral, never below a semicircle.
    if rise > h - 0.2 {
        rise = (h - 0.2).max(half * 1.02);
        rho = (rise * rise + half * half) / (2.0 * half);
    }
    let base = -0.5 * h;
    let spring = 0.5 * h - rise;
    let mut pts = vec![[-half, base], [half, base]];
    let n = 4;
    for k in 1..n {
        pts.push([half, base + (spring - base) * k as f64 / n as f64]);
    }
    // Right flank: centred on the far side, from the right springing up to the point.
    let top = rise.atan2(rho - half);
    pts.extend(arc([half - rho, spring], rho, 0.0, top, 12));
    // Left flank, from the point down to the left springing.
    pts.extend(arc([rho - half, spring], rho, PI - top, PI, 12).into_iter().skip(1));
    for k in 1..n {
        pts.push([-half, spring + (base - spring) * k as f64 / n as f64]);
    }
    pts
}

/// The nearest sample squarely on a head's wall to `(x, y)` seen along the finger, on the `side` of the parting line.
fn on_cheek(a: &Atlas, x: f64, y: f64, side: f64) -> Option<(f64, f64)> {
    a.samples
        .iter()
        .filter(|s| s.p[2] * side > 0.0 && a.cheek(s) > 0.9)
        .map(|s| ((s.p[0] - x).hypot(s.p[1] - y), s))
        .filter(|(d, _)| *d < 0.2)
        .min_by(|p, q| p.0.total_cmp(&q.0))
        .map(|(_, s)| (s.theta, s.v))
}

/// The cheek's wall as the stamps can use it: its span round the ring (x) and up from the axis (y), mm.
fn cheek_extent(a: &Atlas, side: f64) -> ([f64; 2], [f64; 2]) {
    let (mut x, mut y) = ([f64::MAX, f64::MIN], [f64::MAX, f64::MIN]);
    for s in a.samples.iter().filter(|s| s.p[2] * side > 0.0 && a.cheek(s) > 0.9) {
        x = [x[0].min(s.p[0]), x[1].max(s.p[0])];
        y = [y[0].min(s.p[1]), y[1].max(s.p[1])];
    }
    (x, y)
}

/// Both cheeks' chapter-house arcade: three lancets each, recessed along the pull.
fn arcades(d: &mut RingDesign, a: &Atlas, rot: f64, placed: &mut Vec<String>) -> Result<()> {
    let ctx = d.field_context();
    for (side_k, side) in [1.0f64, -1.0].into_iter().enumerate() {
        // One level sill for the whole arcade, over the wall's highest foot; the heads step down from the centre.
        let foot_at = |x: f64| {
            a.samples
                .iter()
                .filter(|s| s.p[2] * side > 0.0 && s.p[1] > a.bore + 0.4 && s.n[2] * side > CHEEK_FACING && s.p[0].hypot(s.p[1]) > a.bore + LANCET_BORE_LAND_MM && (s.p[0] - x).abs() < 0.5 * LANCET_W + 0.1)
                .map(|s| s.p[1])
                .fold(f64::MAX, f64::min)
        };
        let xs: Vec<f64> = (0..LANCETS).map(|k| (k as f64 - 0.5 * (LANCETS - 1) as f64) * LANCET_PITCH).collect();
        // Bench work needs no square wall under it: the sill drops onto the wall's roll, a full land above the bore.
        let sill = (xs.iter().map(|x| foot_at(*x)).fold(f64::MIN, f64::max) + SILL_CLEAR_MM).min(a.bore + LANCET_BORE_LAND_MM + 0.3);
        ensure!(sill < HEAD_Y - 1.2, "no wall under the arcade: sill {sill:.2}, feet {:?}", (0..LANCETS).map(|k| foot_at((k as f64 - 0.5 * (LANCETS - 1) as f64) * LANCET_PITCH)).collect::<Vec<_>>());
        for (k, x) in xs.into_iter().enumerate() {
            let step = (k as f64 - 0.5 * (LANCETS - 1) as f64).abs();
            let h = HEAD_Y - HEAD_STEP_MM * step - sill;
            let y = sill + 0.5 * h;
            let at = on_cheek(a, x, y, side).with_context(|| format!("no cheek at {x}, {y}"))?;
            let mut s = Stamp {
                name: format!("Chapter-house arcade {}, {}", side_k + 1, k + 1),
                theta_deg: at.0,
                v_mm: at.1,
                rot_deg: 0.0,
                outline: lancet(LANCET_W, h),
                height_mm: 0.1,
                sink_mm: ARCADE_SINK_MM,
                draft_deg: 4.0,
                cut: true,
                // Lost wax: cut at the bench with the seal, so the poured form is plain stock and every recess is crisp.
                bench: true,
                along_pull: false,
                fine_cap: true,
                tier: 0,
                // A dished floor, deepest at the light's heart: it faces the pull everywhere and catches the light.
                top: StampTop::Pillow { crown_mm: 0.2 },
            };
            // Stand every light upright, its point straight up from the finger, wherever the wall turns.
            let f = s.frame(d, &ctx);
            s.rot_deg = (-f.x[1]).atan2(f.y[1]).to_degrees() + rot;
            placed.push(format!("{} at {:.2} deg, v {:.3}, {:.2} mm tall", s.name, at.0, at.1, h));
            if 2 * k + 1 == LANCETS {
                // The arcade's sill: one graved bar under all five lights, struck from the centre light's frame.
                let reach = 0.5 * (LANCETS - 1) as f64 * LANCET_PITCH + 0.5 * LANCET_W + 0.25;
                let yb = -0.5 * h - 0.25;
                let mut bar = s.clone();
                bar.name = format!("Chapter-house sill {}", side_k + 1);
                bar.outline = densify(&path_band(&[[-reach, yb], [reach, yb]], 0.24), OUTLINE_STEP_MM);
                bar.sink_mm = 0.3;
                bar.fine_cap = false;
                bar.top = StampTop::Flat;
                d.stamps.push(bar);
            }
            d.stamps.push(s);
        }
    }
    Ok(())
}

/// A roundel's cusped quatrefoil, `d` across: four round foils on the axes meeting in cusps. Counter-clockwise.
fn roundel(d: f64) -> Vec<[f64; 2]> {
    // Foils parted by deep cusps, so each of the four reads on its own.
    let (rho, c) = (0.245 * d, 0.255 * d);
    let m = 0.5 * (c + (2.0 * rho * rho - c * c).sqrt());
    let (a0, a1) = ((-m).atan2(m - c), m.atan2(m - c));
    let foil = arc([c, 0.0], rho, a0, a1, 18);
    let mut pts = Vec::new();
    for q in 0..4 {
        let (sn, co) = (PI * 0.5 * q as f64).sin_cos();
        pts.extend(foil.iter().take(18).map(|p| [p[0] * co - p[1] * sn, p[0] * sn + p[1] * co]));
    }
    pts
}

/// The shoulders' side faces past each end of the head: ring angles and the roundel struck at each, graded down
/// the shoulder.
const ROUNDEL_MM: [f64; 3] = [1.7, 1.3, 0.95];
const ROUNDEL_FIRST_DEG: f64 = 48.0;
/// The pier of polished wall left between two roundels, mm.
const ROUNDEL_PIER_MM: f64 = 0.3;
const ROUNDEL_SINK_MM: f64 = 0.35;

/// Three graded roundels on each shoulder's side faces, struck along the pull at mid-wall, as wide as the wall allows.
fn shoulder_roundels(d: &mut RingDesign, a: &Atlas, placed: &mut Vec<String>) -> Result<()> {
    if std::env::var("SIG_WALLS").is_ok() {
        for t in (20..70).step_by(2) {
            let theta = t as f64;
            let near: Vec<_> = a.samples.iter().filter(|s| (s.theta - theta).abs() < 0.6 && s.p[2] > 0.0).collect();
            let best = near.iter().filter(|s| s.n[2] > 0.85).map(|s| s.p[0].hypot(s.p[1])).fold((f64::MAX, f64::MIN), |(l, h), r| (l.min(r), h.max(r)));
            let nz = near.iter().map(|s| s.n[2]).fold(f64::MIN, f64::max);
            let crown: Vec<_> = a.samples.iter().filter(|s| (s.theta - theta).abs() < 0.6 && {
                let r = s.p[0].hypot(s.p[1]);
                (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / r > 0.85 && r > a.bore + 1.0
            }).collect();
            let z = crown.iter().map(|s| s.p[2]).fold((f64::MAX, f64::MIN), |(l, h), z| (l.min(z), h.max(z)));
            let r = crown.iter().map(|s| s.p[0].hypot(s.p[1])).fold(f64::MIN, f64::max);
            eprintln!("theta {theta}: max nz {nz:.3}, r range {best:?}, crown z {z:?} r {r:.2}");
        }
    }
    for (side_k, side) in [1.0f64, -1.0].into_iter().enumerate() {
        for (sh, mirror) in [("right", false), ("left", true)] {
            // Struck one after another down the shoulder, each a pier's width clear of the last.
            let mut along = ROUNDEL_FIRST_DEG;
            let mut last: Option<(f64, f64)> = None;
            for (k, dia) in ROUNDEL_MM.into_iter().enumerate() {
                if let Some((prev_theta, prev_dia)) = last {
                    along = prev_theta - (0.5 * (prev_dia + dia) + ROUNDEL_PIER_MM) / 11.4f64 * 180.0 / PI;
                }
                let theta = if mirror { 180.0 - along } else { along };
                let wall: Vec<&ringdesign_core::skin::Sample> = a
                    .samples
                    .iter()
                    .filter(|s| (s.theta - theta).abs() < 0.6 && s.p[2] * side > 0.0 && s.n[2] * side > 0.85 && s.p[0].hypot(s.p[1]) > a.bore + 0.6)
                    .collect();
                ensure!(!wall.is_empty(), "no side face at {theta} deg");
                let r = |s: &&ringdesign_core::skin::Sample| s.p[0].hypot(s.p[1]);
                let (lo, hi) = (wall.iter().map(r).fold(f64::MAX, f64::min), wall.iter().map(r).fold(f64::MIN, f64::max));
                // A full land on the bore side, where the comfort roll begins, and a lesser one under the crown.
                let (lo, hi) = (lo + 0.35, hi - 0.4);
                let mid = 0.5 * (lo + hi);
                let at = wall.iter().min_by(|p, q| ((r(p) - mid).abs() + (p.theta - theta).abs() * 0.1).total_cmp(&((r(q) - mid).abs() + (q.theta - theta).abs() * 0.1))).unwrap();
                let dia = dia.min(hi - lo);
                if dia < 0.75 {
                    break;
                }
                d.stamps.push(Stamp {
                    name: format!("Shoulder roundel {}, {sh} {}", side_k + 1, k + 1),
                    theta_deg: at.theta,
                    v_mm: at.v,
                    rot_deg: 45.0,
                    // Under 2 mm a quatrefoil's foils pinch the sand between them below Delft's 0.30 mm detail: the
                    // smaller roundels are plain oculi.
                    outline: roundel(dia),
                    height_mm: 0.1,
                    sink_mm: ROUNDEL_SINK_MM,
                    draft_deg: 4.0,
                    cut: true,
                    bench: true,
                    along_pull: false,
                    fine_cap: true,
                    tier: 0,
                    top: StampTop::Pillow { crown_mm: 0.15 },
                });
                placed.push(format!("Shoulder roundel {}, {sh} {} at {:.2} deg, v {:.3}: wall {:.2} to {:.2} mm from the axis, {:.2} across", side_k + 1, k + 1, at.theta, at.v, lo, hi, dia));
                last = Some((along, dia));
            }
        }
    }
    Ok(())
}

/// A band `w` wide along the open path `pts`, mitred at each turn and cut square at its ends. Counter-clockwise.
fn path_band(pts: &[[f64; 2]], w: f64) -> Vec<[f64; 2]> {
    let n = pts.len();
    let unit = |a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy).max(1e-12);
        [dx / l, dy / l]
    };
    let side = |sign: f64| -> Vec<[f64; 2]> {
        (0..n)
            .map(|i| {
                let d0 = if i == 0 { unit(pts[0], pts[1]) } else { unit(pts[i - 1], pts[i]) };
                let d1 = if i + 1 == n { unit(pts[n - 2], pts[n - 1]) } else { unit(pts[i], pts[i + 1]) };
                let (n0, n1) = ([-d0[1], d0[0]], [-d1[1], d1[0]]);
                let m = [n0[0] + n1[0], n0[1] + n1[1]];
                let ml = m[0].hypot(m[1]).max(1e-12);
                let m = [m[0] / ml, m[1] / ml];
                let k = (0.5 * w / (m[0] * n0[0] + m[1] * n0[1]).max(0.3)).min(1.5 * w);
                [pts[i][0] + sign * m[0] * k, pts[i][1] + sign * m[1] * k]
            })
            .collect()
    };
    let mut out = side(-1.0);
    out.extend(side(1.0).into_iter().rev());
    out.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}


/// Half a window's frame on the `side` (-1 left, +1 right): up the jamb from the sill and over the flank to just
/// past the point.
fn frame_half(side: f64, width: f64, spring: f64, sill: f64) -> Vec<[f64; 2]> {
    let half = 0.5 * width;
    let top = (0.866 * width).atan2(half);
    let mut pts = vec![[side * half, sill]];
    // Up the flank, struck from the far springing, to a little past the point.
    let c = [-side * half, spring];
    let a0 = if side > 0.0 { 0.0 } else { PI };
    // Each half runs a hair past the point, enough to lap the other without a spur.
    let lap = 0.03;
    let a1 = if side > 0.0 { top + lap } else { PI - top - lap };
    pts.extend(arc(c, width, a0, a1, 14));
    pts.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
    pts
}

/// A two-light Gothic window graved in outline, `width` across and `height` from sill to point, +y up to its point:
/// an equilateral arch on its jambs, a sill, a mullion, two pointed lights under the arch and a cusped oculus in its
/// head. Each run one continuous cut, every one at its own depth so no two floors lie in one surface.
fn window(name: &str, width: f64, height: f64, w: f64, depth: f64) -> Vec<Part> {
    // Two lights sunk into the shoulder with a mullion standing between them, an oculus over them, and a hood-mould
    // graved over all: the tracery is the metal left standing.
    let mullion = 0.5;
    let light_w = 0.5 * (width - 2.0 * w - 0.5 - mullion);
    let (sill, top) = (-0.5 * height, 0.5 * height);
    let light_h = 0.56 * height;
    let light_y = sill + 0.15 + 0.5 * light_h;
    let cx = 0.5 * (mullion + light_w);
    let light = |n: &str, x: f64| Part {
        name: format!("{name}, {n}"),
        outline: lancet(light_w, light_h).into_iter().map(|p| [p[0] + x, p[1] + light_y]).collect(),
        depth: depth - 0.15,
        top: StampTop::Ridge { rise_mm: 0.15, from: [x, light_y - 0.5 * light_h], to: [x, light_y + 0.5 * light_h], end_mm: 0.15 },
    };
    let rise = 0.866 * width;
    let spring = top - rise;
    let o_lo = light_y + 0.5 * light_h + 0.3;
    let o_d = (top - 0.45 - o_lo).min(1.4);
    vec![
        light("left light", -cx),
        light("right light", cx),
        Part { name: format!("{name}, oculus"), outline: roundel(o_d).into_iter().map(|p| [p[0], p[1] + o_lo + 0.5 * o_d]).collect(), depth: depth - 0.1, top: StampTop::Pillow { crown_mm: 0.1 } },
        // The hood and the sill its feet stand on, a closed frame round the lights: two halves, each a jamb, a flank
        // and half the sill, lapping at the point and at the sill's middle at their own depths.
        Part { name: format!("{name}, frame left"), outline: path_band(&frame_half(-1.0, width, spring, sill + 0.1), w), depth, top: StampTop::Flat },
        Part { name: format!("{name}, frame right"), outline: path_band(&frame_half(1.0, width, spring, sill + 0.1), w), depth: depth + 0.04, top: StampTop::Flat },
        Part { name: format!("{name}, sill"), outline: path_band(&[[-0.5 * width - 0.5 * w, sill + 0.1], [0.5 * width + 0.5 * w, sill + 0.1]], w), depth: depth + 0.08, top: StampTop::Flat },
    ]
}

/// The shoulder window's span across the shoulder and its length up it to its point, mm, and where it stands.
const WINDOW_MM: (f64, f64) = (3.6, 5.6);
const WINDOW_THETA_DEG: f64 = 37.0;
const WINDOW_RULE_MM: f64 = 0.24;
const WINDOW_DEPTH_MM: f64 = 0.45;

/// A graved two-light window on each shoulder's crown, its point toward the head: the cathedral's own lights carried
/// down the shank. Bench work, like the seal.
fn shoulder_windows(d: &mut RingDesign, a: &Atlas, placed: &mut Vec<String>) -> Result<()> {
    let ctx = d.field_context();
    for (sh, theta) in [("right", WINDOW_THETA_DEG), ("left", 180.0 - WINDOW_THETA_DEG)] {
        let at = a
            .samples
            .iter()
            .filter(|s| (s.theta - theta).abs() < 0.6 && s.p[0].hypot(s.p[1]) > a.bore + 1.0)
            .min_by(|p, q| p.p[2].abs().total_cmp(&q.p[2].abs()).then(q.p[0].hypot(q.p[1]).total_cmp(&p.p[0].hypot(p.p[1]))))
            .context("no shoulder crown")?;
        let mut probe = intaglio("probe", (at.theta, at.v), 0.0, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], 0.1, StampTop::Flat);
        probe.bench = true;
        let f = probe.frame(d, &ctx);
        // Up the shoulder toward the head.
        let t = { let th = theta.to_radians(); let s = if theta < 90.0 { 1.0 } else { -1.0 }; [-th.sin() * s, th.cos() * s, 0.0] };
        let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
        let rot = dot(t, f.y).atan2(dot(t, f.x)).to_degrees();
        // Window space (x across, y up to the point) to the stamp's own: a quarter turn, so the outline keeps its hand.
        let map = |p: [f64; 2]| [p[1], -p[0]];
        for part in window(&format!("Shoulder window, {sh}"), WINDOW_MM.0, WINDOW_MM.1, WINDOW_RULE_MM, WINDOW_DEPTH_MM) {
            let top = part.top;
            d.stamps.push(intaglio(&part.name, (at.theta, at.v), rot, part.outline.iter().map(|p| map(*p)).collect(), part.depth, top));
        }
        placed.push(format!("Shoulder window, {sh} at {:.2} deg, v {:.3}, turned {rot:.1} deg", at.theta, at.v));
    }
    Ok(())
}

/// The shank's blind arcade: pointed lancets sunk side by side down each shank's crown from the window toward the
/// palm, graded smaller as the shank narrows, stopping short of the plain palm.
const SHANK_ARCADE: (f64, f64, f64) = (22.0, -60.0, 1.0);
const SHANK_ARCH_MM: (f64, f64) = (1.2, 2.2);
/// The polished land kept between a light and the crown's edge, mm.
const SHANK_EDGE_LAND_MM: f64 = 0.6;
/// Polished crown kept between the shoulder window's foot and the first shank light, mm.
const SHANK_WINDOW_GAP_MM: f64 = 0.6;
const SHANK_RULE_MM: f64 = 0.2;
/// The bar left standing between two lights, mm.
const SHANK_BAR_MM: f64 = 0.5;
const SHANK_DEPTH_MM: f64 = 0.4;

/// A stamp's turn that sends its outline's +x along `t`, read off its frame at no turn.
fn turn_to(d: &RingDesign, at: (f64, f64), t: [f64; 3]) -> f64 {
    let mut probe = intaglio("probe", at, 0.0, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]], 0.1, StampTop::Flat);
    probe.bench = true;
    let f = probe.frame(d, &d.field_context());
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    dot(t, f.y).atan2(dot(t, f.x)).to_degrees()
}

/// The crown's sample on the parting line at `theta`.
fn crown_at(a: &Atlas, theta: f64) -> Option<(f64, f64)> {
    let theta = theta.rem_euclid(360.0);
    a.samples
        .iter()
        .filter(|s| ((s.theta - theta + 180.0).rem_euclid(360.0) - 180.0).abs() < 0.6 && s.p[0].hypot(s.p[1]) > a.bore + 1.0)
        .min_by(|p, q| p.p[2].abs().total_cmp(&q.p[2].abs()).then(q.p[0].hypot(q.p[1]).total_cmp(&p.p[0].hypot(p.p[1]))))
        .map(|s| (s.theta, s.v))
}

/// Half the width of the crown at `theta`, where it still faces out of the ring, mm.
fn crown_half_width(a: &Atlas, theta: f64) -> f64 {
    let theta = theta.rem_euclid(360.0);
    a.samples
        .iter()
        .filter(|s| ((s.theta - theta + 180.0).rem_euclid(360.0) - 180.0).abs() < 0.6 && {
            let r = s.p[0].hypot(s.p[1]);
            (s.n[0] * s.p[0] + s.n[1] * s.p[1]) / r > 0.85 && r > a.bore + 1.0
        })
        .map(|s| s.p[2].abs())
        .fold(0.0, f64::max)
}

fn shank_arcade(d: &mut RingDesign, a: &Atlas, placed: &mut Vec<String>) -> Result<()> {
    let (_, to, _) = SHANK_ARCADE;
    for (sh, sign) in [("right", 1.0f64), ("left", -1.0)] {
        // The first light's head stands half a millimetre short of the shoulder window's foot.
        let window_foot = WINDOW_THETA_DEG - (0.5 * WINDOW_MM.1 / 12.6).to_degrees();
        let mut head = window_foot - (SHANK_WINDOW_GAP_MM / 12.0).to_degrees();
        let from = head;
        let mut k = 0;
        loop {
            // Graded: full size under the window, three quarters toward the palm.
            let g = 1.0 - 0.25 * ((from - head) / (from - to)).clamp(0.0, 1.0);
            let (w0, h) = (SHANK_ARCH_MM.0 * g, SHANK_ARCH_MM.1 * g);
            let r = 11.6;
            let centre = head - (0.5 * h / r).to_degrees();
            if centre - (0.5 * h / r).to_degrees() < to {
                break;
            }
            let th = if sign > 0.0 { centre } else { 180.0 - centre };
            let at = crown_at(a, th).with_context(|| format!("no shank crown at {th}"))?;
            // Wholly inside the crown, with polished crown either side of it.
            let half = crown_half_width(a, th);
            let w = w0.min(2.0 * (half - SHANK_EDGE_LAND_MM));
            // Each light lies along the shank, its point toward the table, as a window in the shank's wall would.
            let thr = th.to_radians();
            let t = if sign > 0.0 { [-thr.sin(), thr.cos(), 0.0] } else { [thr.sin(), -thr.cos(), 0.0] };
            let rot = turn_to(d, at, t);
            let map = |p: [f64; 2]| [p[1], -p[0]];
            let outline: Vec<[f64; 2]> = lancet(w, h).into_iter().map(map).collect();
            k += 1;
            let mut s = intaglio(&format!("Shank arcade, {sh} {k}"), at, rot, outline, SHANK_DEPTH_MM, StampTop::Pillow { crown_mm: 0.15 });
            s.bench = true;
            d.stamps.push(s);
            // Its sill: a graved bar across the light's foot.
            let y = -0.5 * h - 0.22;
            let sill: Vec<[f64; 2]> = path_band(&[[-0.5 * w - 0.15, y], [0.5 * w + 0.15, y]], SHANK_RULE_MM).into_iter().map(map).collect();
            d.stamps.push(intaglio(&format!("Shank sill, {sh} {k}"), at, rot, sill, 0.3, StampTop::Flat));
            placed.push(format!("Shank arcade, {sh} {k} at {:.2} deg, {:.2} across x {:.2} along, crown {:.2} mm across", at.0, w, h, 2.0 * half));
            head = centre - ((0.5 * h + 0.22 + SHANK_BAR_MM) / r).to_degrees();
        }
    }
    Ok(())
}

#[derive(Default, serde::Serialize)]
struct Placed {
    face_mm: [f64; 2],
    envelope_fill_mm: f64,
    envelope_fill_theta_deg: f64,
    cheek_x_mm: [f64; 2],
    cheek_y_mm: [f64; 2],
    table_crown_mm: f64,
    table_sag_at_field_edge_mm: f64,
    arcade: Vec<String>,
    roundels: Vec<String>,
    windows: Vec<String>,
    shank_arcade: Vec<String>,
}

fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock(face())?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let (fill, fill_at) = fill_mm(&d);
    let (cx, cy) = cheek_extent(&a, 1.0);
    if std::env::var("SIG_MAP").is_ok() {
        eprintln!("top {:.2} bore {:.2} cheek x {cx:?} y {cy:?}", a.top, a.bore);
        for yi in (0..30).rev() {
            let y = 2.0 + 0.4 * yi as f64;
            let row: String = (0..44).map(|xi| {
                let x = -11.0 + 0.5 * xi as f64;
                let near = a.samples.iter().filter(|s| s.p[2] > 0.0 && (s.p[0] - x).abs() < 0.25 && (s.p[1] - y).abs() < 0.2);
                let best = near.map(|s| a.cheek(s)).fold(-1.0f64, f64::max);
                if best < 0.0 { ' ' } else if best > 0.9 { '#' } else if best > 0.3 { '+' } else { '.' }
            }).collect();
            eprintln!("{y:5.1} {row}");
        }
    }
    // How far the table falls from its crown to the field's lobes: the bench cuts are measured from the crown.
    let table = |x: f64, z: f64| {
        a.samples.iter().filter(|s| s.p[1] > a.top - 6.0 && s.n[1] > 0.2).min_by(|p, q| ((p.p[0] - x).powi(2) + (p.p[2] - z).powi(2)).total_cmp(&((q.p[0] - x).powi(2) + (q.p[2] - z).powi(2)))).map_or(0.0, |s| s.p[1])
    };
    let crown = table(0.0, 0.0);
    if std::env::var("SIG_DOME").is_ok() {
        for zi in (-9..=9).rev() {
            let z = zi as f64;
            let row: Vec<String> = (-9..=9).map(|xi| format!("{:5.2}", crown - table(xi as f64, z))).collect();
            eprintln!("z {z:+.0}: {}", row.join(" "));
        }
    }
    let sag = [(FIELD_REACH_MM, 0.0), (0.0, FIELD_REACH_MM)].iter().map(|(x, z)| crown - table(*x, *z)).fold(0.0, f64::max);
    let mut placed = Placed { face_mm: [FACE.0, FACE.1], envelope_fill_mm: fill, envelope_fill_theta_deg: fill_at, cheek_x_mm: cx, cheek_y_mm: cy, table_crown_mm: crown, table_sag_at_field_edge_mm: sag, ..Default::default() };
    seal(&mut d, &a, std::env::var("SIG_NO_LEGEND").is_err())?;
    let rot: f64 = std::env::var("SIG_ROT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    if std::env::var("SIG_NO_ARCADE").is_err() {
        arcades(&mut d, &a, rot, &mut placed.arcade)?;
    }
    if std::env::var("SIG_NO_ROUNDEL").is_err() {
        shoulder_roundels(&mut d, &a, &mut placed.roundels)?;
    }
    if std::env::var("SIG_NO_WINDOW").is_err() {
        shoulder_windows(&mut d, &a, &mut placed.windows)?;
    }
    if std::env::var("SIG_NO_SHANK").is_err() {
        shank_arcade(&mut d, &a, &mut placed.shank_arcade)?;
    }
    let _ = blockout;
    if let Ok(skip) = std::env::var("SIG_SKIP") {
        let skip: Vec<&str> = skip.split('|').collect();
        d.stamps.retain(|s| !skip.iter().any(|k| s.name.starts_with(k)));
    }
    let lib = mf::source_library(&d, &lib).into_owned();
    Ok((d, lib, placed))
}

// --- Gates -------------------------------------------------------------------------------------------------------

fn self_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() })
}

/// The closest any vertex comes to the finger axis, less the bore radius, mm.
fn bore_margin(d: &RingDesign, m: &mesh::Mesh) -> f64 {
    let r = d.inner_radius_mm();
    m.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64) - r).fold(f64::MAX, f64::min)
}

/// Metal section by rays: from the middle of every sampled well-shaped triangle, straight in along its inward
/// normal to the first surface met. Triangles thinner than 5 degrees at a corner, the boolean's slivers along a cut's
/// rim, are not sampled, and a ray ignores what it meets in its first 0.01 mm. The thinnest few, mm, and where.
fn section_by_rays(m: &mesh::Mesh, samples: usize) -> Vec<(f64, [f64; 3])> {
    let p = |i: u32| { let v = m.vertices[i as usize]; [v.0 as f64, v.1 as f64, v.2 as f64] };
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let tris: Vec<[[f64; 3]; 3]> = m.faces.iter().map(|f| [p(f[0]), p(f[1]), p(f[2])]).collect();
    let angle_ok = |t: &[[f64; 3]; 3]| (0..3).all(|k| {
        let (a, b) = (sub(t[(k + 1) % 3], t[k]), sub(t[(k + 2) % 3], t[k]));
        let c = dot(a, b) / (dot(a, a).sqrt() * dot(b, b).sqrt()).max(1e-30);
        c.clamp(-1.0, 1.0).acos().to_degrees() > 5.0
    });
    let stride = (tris.len() / samples).max(1);
    let mut out: Vec<(f64, [f64; 3])> = tris
        .iter()
        .step_by(stride)
        .filter(|t| angle_ok(t))
        .map(|t| {
            let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
            let l = dot(n, n).sqrt().max(1e-30);
            let d = [-n[0] / l, -n[1] / l, -n[2] / l];
            let o = [(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0, (t[0][2] + t[1][2] + t[2][2]) / 3.0];
            let mut best = f64::MAX;
            for q in &tris {
                let (e1, e2) = (sub(q[1], q[0]), sub(q[2], q[0]));
                let h = cross(d, e2);
                let a = dot(e1, h);
                if a.abs() < 1e-12 { continue; }
                let f = 1.0 / a;
                let s0 = sub(o, q[0]);
                let u = f * dot(s0, h);
                if !(0.0..=1.0).contains(&u) { continue; }
                let qv = cross(s0, e1);
                let v = f * dot(d, qv);
                if v < 0.0 || u + v > 1.0 { continue; }
                let tt = f * dot(e2, qv);
                if tt > 0.01 && tt < best { best = tt; }
            }
            (best, o)
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.truncate(5);
    out
}

/// Every CAD feature's status, and each made part's self-crossings.
fn cad_gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool)> {
    if d.cad.is_none() {
        return Ok((json!({ "features": [], "made_parts": [], "note": "No CAD features: the seal is bench-cut stamps draped on the barrelled table, the arcades cast stamps." }), true));
    }
    let e = cad::evaluate(d, lib, params)?;
    let statuses: Vec<Value> = e
        .features
        .iter()
        .map(|r| json!({ "id": r.id, "name": r.name, "status": match &r.status { FeatureStatus::Ok => "Ok".to_string(), FeatureStatus::Suppressed => "Suppressed".into(), FeatureStatus::Failed(m) => format!("Failed: {m}"), FeatureStatus::Skipped(m) => format!("Skipped: {m}") } }))
        .collect();
    let all_ok = e.features.iter().all(|r| r.status.is_ok());
    let parts: Vec<Value> = e
        .components
        .iter()
        .filter(|c| !c.mesh.faces.is_empty())
        .map(|c| {
            let n = self_crossings(&c.mesh);
            json!({ "id": c.id, "name": c.name, "stage": format!("{:?}", c.stage), "attach": format!("{:?}", c.attach), "triangles": c.mesh.faces.len(), "watertight": c.mesh.validate().watertight, "self_crossings": n })
        })
        .collect();
    let parts_ok = parts.iter().all(|p| p["self_crossings"] == json!(0) && p["watertight"] == json!(true));
    Ok((json!({ "features": statuses, "made_parts": parts }), all_ok && parts_ok))
}

/// Every gate at one build size, and whether all are green.
fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool, mesh::BuildResult)> {
    let started = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = self_crossings(&built.mesh);
    let margin = bore_margin(d, &built.mesh);
    let (cad, cad_ok) = cad_gates(d, lib, params)?;
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    // The lost-wax section gate, on what is poured: the casting pattern, without the seal and the other bench cuts
    // engraved after it, sampled over a mesh the measure can take whole.
    let thin = mesh::try_build_pattern(d, lib, BuildParams { theta_steps: 384, profile_steps: 160, refine: None, ..BuildParams::default() })?;
    let thickness = ringdesign_core::cad::measure::thickness(&thin.mesh, 0.8);
    let thick_ok = thickness.rays > 0 && thickness.below_limit == 0;
    // The bonus: the same ring judged as the Delft pour.
    let sand = as_sand(d);
    let mut sand_field = castability::attributed_field_report(&sand, lib, &sand.draft, 256, 128);
    castability::judge_parts(&mut sand_field, &sand, &built);
    let (inspection, fine) = pull(&sand, lib, params)?;
    let coarse_release = &inspection.release;
    let monotone: Vec<(String, bool)> = d.stamps.iter().filter(|s| !s.bench && !s.along_pull).map(|s| (s.name.clone(), s.parting_monotone(&sand).is_ok())).collect();
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::preview_vertices(d, lib).len() / 36;
    let castable = field.verdict == castability::Verdict::Castable;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && cad_ok
        && margin >= -0.01
        && castable
        && thick_ok
        && findings.is_empty()
        && reported == 0
        && previewed == 0
        && built.mesh.faces.len() <= 2_000_000;
    let g = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "min_angle_deg": q.min_angle_deg,
        "self_crossings": crossings,
        "cad": cad,
        "cad_ok": cad_ok,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "parts_cut": built.parts.cut,
        "stamps_struck": built.solids.stamped,
        "bore_margin_mm": margin,
        "process": "Lost wax (Logan's rule, 2026-10-03): 0.8 mm minimum section, no pull rule",
        "thickness_0p8": { "sampled_min_mm": thickness.sampled_min_mm, "rays": thickness.rays, "unresolved": thickness.unresolved, "below_limit": thickness.below_limit, "note": thickness.note, "build": [384, 160], "mesh": "casting pattern (bench engraving left out)" },
        "sand_bonus": {
            "field_verdict": sand_field.verdict.label(),
            "undercut_percent": sand_field.undercut_fraction() * 100.0,
            "release_0100": release_line(coarse_release),
            "release_0075": release_line(&fine),
            "parting_monotone_all": monotone.iter().all(|(_, ok)| *ok),
            "pulls_from_delft": sand_field.verdict == castability::Verdict::Castable && coarse_release.obstructions.is_empty() && coarse_release.unresolved_rays == 0 && fine.obstructions.is_empty() && fine.unresolved_rays == 0,
        },
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "undercut_percent": field.undercut_fraction() * 100.0,
        "worst_draft_deg": field.worst_draft_deg,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "release_0100": release_line(coarse_release),
        "release_0100_obstructions": coarse_release.obstructions.len(),
        "release_0100_unresolved": coarse_release.unresolved_rays,
        "release_0100_low_draft_area_mm2": coarse_release.low_draft_area_mm2,
        "release_0100_sand_findings": coarse_release.sand_findings.iter().map(|f| format!("{} at [{:.2}, {:.2}, {:.2}]", f.message, f.point[0], f.point[1], f.point[2])).collect::<Vec<_>>(),
        "release_0100_notes": coarse_release.notes,
        "release_0075": release_line(&fine),
        "release_0075_obstructions": fine.obstructions.len(),
        "release_0075_obstruction_at": fine.obstructions.iter().map(|o| format!("[{:.2}, {:.2}, {:.2}] {:.3} mm deep", o.world[0], o.world[1], o.world[2], o.depth_mm)).collect::<Vec<_>>(),
        "release_0075_unresolved": fine.unresolved_rays,
        "clamp_bites_mm": [],
        "clamp_note": "No painted relief: nothing passes through skin::draft_clamp.",
        "parting_monotone": monotone,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "grams_925": built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams),
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders -----------------------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn save_rgb(path: &Path, rgb: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let small: Vec<Vec<u8>> = VIEWS.iter().map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3)).collect();
    save_rgb(&out.join("hero-300.png"), &small[0], 300, 300)?;
    save_rgb(&out.join("face-300.png"), &small[1], 300, 300)?;
    let (cols, rows) = (3, 2);
    let mut sheet = vec![0u8; cols * 300 * rows * 300 * 3];
    for (i, img) in small.iter().enumerate() {
        let (cx, cy) = (i % cols, i / cols);
        for y in 0..300 {
            let dst = ((cy * 300 + y) * cols * 300 + cx * 300) * 3;
            sheet[dst..dst + 900].copy_from_slice(&img[y * 900..(y + 1) * 900]);
        }
    }
    save_rgb(&out.join("contact-300.png"), &sheet, cols * 300, rows * 300)?;
    // Close-ups framed on a point of the whole ring, never a cropped mesh: the seal under a raking light, the cheek's
    // arcade, and the shoulder's window over its roundels.
    let crown = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    render::write_png_framed(out.join("stones.png"), &parts, render::yaw_facing(90.0) + 0.25, 1.2, render::Framing::new([0.0, crown - 0.3, 0.0], 8.5), edge)?;
    render::write_png_framed(out.join("cheek-close.png"), &parts, 0.15, 0.3, render::Framing::new([0.0, 12.1, -7.6], 6.0), edge)?;
    let w = WINDOW_THETA_DEG.to_radians();
    render::write_png_framed(out.join("shoulder-close.png"), &parts, render::yaw_facing(WINDOW_THETA_DEG) - 0.35, 0.75, render::Framing::new([12.0 * w.cos(), 12.0 * w.sin(), 1.8], 6.0), edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.imported_base.as_mut().unwrap().bare = true;
    bare.stamps.clear();
    bare.layers.layers.clear();
    bare.cad = None;
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let left = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    Ok(())
}

// --- Main --------------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/sigillum"));
    std::fs::create_dir_all(&out)?;
    println!("Sigillum");
    let (d, lib, placed) = author(blockout)?;
    println!("  placed {}", serde_json::to_string(&placed)?);
    if args.iter().any(|a| a == "--thick") {
        let mut d = d.clone();
        if std::env::var("SIG_BARE").is_ok() {
            d.stamps.clear();
        }
        let thin = mesh::try_build_pattern(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 160, refine: None, ..BuildParams::default() })?;
        let t = ringdesign_core::cad::measure::thickness(&thin.mesh, 0.8);
        println!("  faces {} thickness min {:?} at {:?}, below {} of {}", thin.mesh.faces.len(), t.sampled_min_mm, t.point, t.below_limit, t.rays);
        println!("  section by rays: {:?}", section_by_rays(&thin.mesh, 3000));
        return Ok(());
    }
    if args.iter().any(|a| a == "--cross") {
        let params = if draft { draft_params() } else { export_params() };
        let built = mesh::try_build(&d, &lib, params)?;
        println!("  stamps {} crossings {} watertight {}", d.stamps.len(), self_crossings(&built.mesh), built.report.validation.watertight);
        return Ok(());
    }
    if args.iter().any(|a| a == "--probe") {
        let built = mesh::try_build(&d, &lib, draft_params())?;
        let (cad, ok) = cad_gates(&d, &lib, draft_params())?;
        println!("  cad ok {ok}: {cad}");
        println!("  watertight {} notes {:?} {:?}", built.report.validation.watertight, built.solids.notes, built.parts.notes);
        let fin = render::finished_from(&d, &lib, built);
        let parts = fin.parts(render::GOLD);
        for (name, yaw, pitch) in VIEWS {
            render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, 900)?;
        }
        render::write_png_parts(out.join("rake.png"), &parts, 0.25, 1.25, 900)?;
        let yaw: f64 = std::env::var("SIG_YAW").ok().and_then(|v| v.parse().ok()).unwrap_or(0.35);
        let pitch: f64 = std::env::var("SIG_PITCH").ok().and_then(|v| v.parse().ok()).unwrap_or(0.3);
        render::write_png_parts(out.join("probe.png"), &parts, yaw, pitch, 900)?;
        let ctx = d.field_context();
        for st in d.stamps.iter().filter(|s| s.along_pull).take(6) {
            let f = st.frame(&d, &ctx);
            println!("  frame {}: origin {:?} x {:?} y {:?} z {:?}", st.name, f.origin, f.x, f.y, f.z);
        }
        return Ok(());
    }
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: pass {draft_pass}");
    let (export_gates, export_pass, built) = if draft {
        (Value::Null, true, draft_built)
    } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: pass {p}");
        (g, p, b)
    };
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
    let mut pattern_gates = Value::Null;
    let mut pattern_pass = true;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.mesh.quality();
        let pc = self_crossings(&pattern.mesh);
        let pv = pattern.mesh.validate();
        pattern_pass = pv.watertight && pq.degenerate_faces == 0 && pc == 0;
        pattern_gates = json!({ "triangles": pattern.mesh.faces.len(), "watertight": pv.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "pass": pattern_pass });
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": [] }))?)?;
    }
    let all = draft_pass && export_pass && pattern_pass && cold != Some(false) && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": "sigillum",
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "process_rule": "Judged as lost wax under Logan's rule of 2026-10-03 (0.8 mm minimum section, no pull rule); the Delft pour (3.0 deg draft, 0.8 mm section, 0.30 mm detail) is reported in each block's sand_bonus.",
        "alloy": ALLOY,
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "face_mm": [FACE.0, FACE.1],
        "stock": "Factory 017 Tonneau through the sand master (the section's fallback)",
        "stock_fallback_why": "005 Rosette's head is a domed four-lobed pillow: 1.1 mm of sag 4 mm off its crown and 0.67 mm of envelope fill at 16 x 18.7, so it has no table to cut a seal in. 017 is a true signet table (0.011 mm fill); 16 x 15.6 is the largest face 017 resizes to (130% of its native 12 mm width).",
        "table_over_bore_mm": placed.table_crown_mm - BORE_MM * 0.5,
        "deepest_bench_cut_mm": FLEUR_DEPTH_MM,
        "metal_under_deepest_cut_mm": placed.table_crown_mm - BORE_MM * 0.5 - FLEUR_DEPTH_MM,
        "release_status_why": "Sand bonus only. Review, not Clear, with 0 obstructions and 0 unresolved rays: the factory table and bore walls carry under the sand's 3 deg draft; the bare stock reports the same.",
        "placed": placed,
        "features": d.cad.as_ref().map(|c| c.features.iter().map(|f| json!({ "id": f.id, "name": f.name, "stage": format!("{:?}", f.component.stage), "attach": format!("{:?}", f.component.attach) })).collect::<Vec<_>>()),
        "stamps": d.stamps.iter().map(|s| json!({ "name": s.name, "theta_deg": s.theta_deg, "v_mm": s.v_mm, "bench": s.bench, "cut": s.cut, "tier": s.tier, "along_pull": s.along_pull, "sink_mm": s.sink_mm })).collect::<Vec<_>>(),
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "design_bytes": text.len(),
        "design_format": serde_json::from_str::<Value>(&text)?.get("format_version").cloned(),
        "draft": draft_gates,
        "export": export_gates,
        "casting_pattern": pattern_gates,
        "cold_reload_identical": cold,
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if all { "passed" } else { "FAILED" });
    ensure!(all || draft, "Sigillum failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
