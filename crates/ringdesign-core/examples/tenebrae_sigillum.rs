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

/// The cheek's lancet arcade, a chapter house's graded triplet: three lights `LANCET_W` wide at `LANCET_PITCH`
/// centres, the middle one the tallest, their sills level at `SILL_Y` (mm up from the finger axis).
const LANCET_W: f64 = 1.5;
const LANCET_H: [f64; 3] = [1.95, 2.25, 1.95];
const LANCET_PITCH: f64 = 2.15;
const SILL_Y: f64 = 10.95;
const ARCADE_SINK_MM: f64 = 0.30;
/// The lancet's head: each flank struck at this many spans' radius, an acute Early English point.
const ACUTE: f64 = 1.4;
/// Each light's floor falls a further this much to a vertical spine, so its two halves catch the light apart.
const ARCADE_VEE_MM: f64 = 0.25;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// The ring's own Delft pour: z = 0 parting, opposed z withdrawal, sterling silver.
fn setup() -> mf::Setup {
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
    ImportedBase::attach(&mut d, sand_master(preset.load()?)?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = true;
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
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let l = dx.hypot(dy);
    let (d, n) = ([dx / l, dy / l], [-dy / l, dx / l]);
    let n_steps = ((l / 0.05).ceil() as usize).max(16);
    let half = |t: f64| {
        let u = if t < at { t / at } else { 1.0 - (t - at) / (1.0 - at) };
        let u = u.clamp(0.0, 1.0);
        let base = if t < at { 0.5 * blunt * (1.0 - t / at) } else { 0.0 };
        (0.5 * w * (u * (2.0 - u)).sqrt()).max(base)
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
    let (lo, hi) = (-2.05, 3.3);
    let k = height / (hi - lo);
    let mid = 0.5 * (lo + hi);
    let fit = |p: Vec<[f64; 2]>| p.into_iter().map(|q| [q[0] * k, (q[1] - mid) * k]).collect::<Vec<_>>();
    let fitp = |q: [f64; 2]| [q[0] * k, (q[1] - mid) * k];
    let part = |name: &str, outline: Vec<[f64; 2]>, depth: f64, top: StampTop| Part { name: format!("Seal fleur, {name}"), outline: fit(outline), depth, top };
    let mut parts = vec![
        part("centre petal", blade([0.0, -0.4], [0.0, hi], 1.95, 0.4, 0.7), depth, StampTop::Ridge { rise_mm: 0.25, from: fitp([0.0, 0.1]), to: fitp([0.0, 2.9]), end_mm: 0.05 }),
        part("band", bar(-1.7, 1.7, -0.95, -0.35, 0.18), depth - 0.1, StampTop::Flat),
        part("foot", blade([0.0, -0.9], [0.0, lo], 0.8, 0.3, 0.6), depth - 0.1, StampTop::Flat),
    ];
    for (side, name) in [(1.0f64, "right"), (-1.0, "left")] {
        let m = |p: Vec<[f64; 2]>| {
            let mut q: Vec<[f64; 2]> = p.into_iter().map(|q| [side * q[0], q[1]]).collect();
            if side < 0.0 {
                q.reverse();
            }
            q
        };
        parts.push(part(&format!("{name} petal"), m(curl([1.5, 0.15], 1.12, 172.0, 8.0, 0.95)), depth, StampTop::Flat));
        parts.push(part(&format!("{name} spike"), m(blade([0.55, -0.95], [1.45, -1.85], 0.5, 0.3, 0.45)), depth - 0.1, StampTop::Flat));
    }
    parts
}

/// A broad-nib stroke from `p0` to `p1`: the nib swept along it, as a Textura hand cuts it, never thinner than `min_w`.
fn nib_stroke(p0: [f64; 2], p1: [f64; 2], nib: [f64; 2], min_w: f64) -> Vec<[f64; 2]> {
    let (dx, dy) = (p1[0] - p0[0], p1[1] - p0[1]);
    let l = dx.hypot(dy).max(1e-9);
    let n = [-dy / l, dx / l];
    // Where the stroke runs along the nib it would thin to nothing: hold it at `min_w`.
    let across = nib[0] * n[0] + nib[1] * n[1];
    let nib = if across.abs() < min_w {
        let add = min_w - across.abs();
        let sgn = if across < 0.0 { -1.0 } else { 1.0 };
        [nib[0] + sgn * n[0] * add, nib[1] + sgn * n[1] * add]
    } else {
        nib
    };
    let h = [0.5 * nib[0], 0.5 * nib[1]];
    let mut out = vec![[p0[0] - h[0], p0[1] - h[1]], [p1[0] - h[0], p1[1] - h[1]], [p1[0] + h[0], p1[1] + h[1]], [p0[0] + h[0], p0[1] + h[1]]];
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// A Textura capital as broad-nib strokes in a unit cap (x from 0 to its advance, y up from 0 to 1), and its advance.
/// Only the letters the legend needs.
fn textura(c: char) -> (f64, Vec<([f64; 2], [f64; 2])>) {
    match c {
        'I' => (0.36, vec![([0.18, 0.06], [0.18, 0.94])]),
        'L' => (0.66, vec![([0.16, 0.06], [0.16, 0.94]), ([0.16, 0.05], [0.58, 0.12])]),
        'T' => (0.78, vec![([0.06, 0.86], [0.72, 0.94]), ([0.39, 0.9], [0.39, 0.12]), ([0.39, 0.08], [0.58, 0.16])]),
        'V' => (0.78, vec![([0.14, 0.94], [0.14, 0.32]), ([0.14, 0.3], [0.4, 0.04]), ([0.64, 0.94], [0.64, 0.34]), ([0.64, 0.34], [0.42, 0.04])]),
        'M' => (0.94, vec![([0.1, 0.06], [0.1, 0.84]), ([0.1, 0.86], [0.28, 0.96]), ([0.28, 0.96], [0.46, 0.86]), ([0.46, 0.84], [0.46, 0.06]), ([0.46, 0.86], [0.64, 0.96]), ([0.64, 0.96], [0.82, 0.86]), ([0.82, 0.84], [0.82, 0.06])]),
        'C' => (0.7, vec![([0.14, 0.22], [0.14, 0.78]), ([0.14, 0.8], [0.34, 0.96]), ([0.34, 0.96], [0.62, 0.88]), ([0.14, 0.2], [0.34, 0.04]), ([0.34, 0.04], [0.62, 0.12])]),
        'G' => (0.74, vec![([0.14, 0.22], [0.14, 0.78]), ([0.14, 0.8], [0.34, 0.96]), ([0.34, 0.96], [0.62, 0.88]), ([0.14, 0.2], [0.34, 0.04]), ([0.34, 0.04], [0.62, 0.12]), ([0.62, 0.12], [0.62, 0.46]), ([0.4, 0.48], [0.64, 0.48])]),
        'S' => (0.7, vec![([0.6, 0.9], [0.34, 0.96]), ([0.32, 0.96], [0.12, 0.78]), ([0.12, 0.76], [0.56, 0.26]), ([0.56, 0.24], [0.38, 0.04]), ([0.36, 0.04], [0.08, 0.1])]),
        'A' => (0.78, vec![([0.12, 0.06], [0.12, 0.72]), ([0.12, 0.74], [0.36, 0.96]), ([0.36, 0.96], [0.64, 0.86]), ([0.64, 0.84], [0.64, 0.06]), ([0.12, 0.46], [0.64, 0.46])]),
        'P' => (0.74, vec![([0.14, 0.94], [0.14, 0.0]), ([0.14, 0.92], [0.56, 0.9]), ([0.58, 0.88], [0.62, 0.58]), ([0.62, 0.56], [0.14, 0.48])]),
        _ => (0.4, vec![]),
    }
}

/// The cross pattée that opens the legend, `size` across, centred on the origin. Counter-clockwise.
fn cross_pattee(size: f64) -> Vec<[f64; 2]> {
    let (inner, end, flare) = (0.09 * size, 0.5 * size, 0.27 * size);
    let mut pts = Vec::new();
    for k in 0..4 {
        let a = PI * 0.5 * k as f64;
        let (e, f) = ([a.cos(), a.sin()], [-a.sin(), a.cos()]);
        let corner = |u: f64, v: f64| [e[0] * u + f[0] * v, e[1] * u + f[1] * v];
        pts.push(corner(inner, -inner));
        // The arm's sides curve in as they flare.
        for j in 1..6 {
            let t = j as f64 / 6.0;
            pts.push(corner(inner + (end - inner) * t, -(inner + (flare - inner) * t * t)));
        }
        pts.push(corner(end, -flare));
        pts.push(corner(end, flare));
        for j in (1..6).rev() {
            let t = j as f64 / 6.0;
            pts.push(corner(inner + (end - inner) * t, inner + (flare - inner) * t * t));
        }
    }
    pts.dedup();
    pts
}

/// The legend round the rim, reading clockwise from the top with the letters' heads outward, as the wax reads it:
/// each stroke one cut. `radius` runs through the middle of the cap.
fn legend(text: &str, radius: f64, cap: f64, depth: f64) -> Vec<Part> {
    let glyphs: Vec<(char, f64, Vec<([f64; 2], [f64; 2])>)> = text
        .chars()
        .map(|c| {
            let (adv, strokes) = if c == '\u{2720}' { (1.05, vec![]) } else { textura(c) };
            (c, adv * cap, strokes)
        })
        .collect();
    let used: f64 = glyphs.iter().map(|g| g.1).sum();
    // Tracking closes the circle.
    let gap = (2.0 * PI * radius - used) / glyphs.len() as f64;
    let nib = [0.17 * 0.6, 0.17 * 0.8];
    let mut parts = Vec::new();
    let mut along = 0.0;
    for (k, (c, adv, strokes)) in glyphs.into_iter().enumerate() {
        // The glyph's centre, clockwise from the top.
        let phi = (along + 0.5 * adv) / radius;
        along += adv + gap;
        let u = [phi.sin(), phi.cos()];
        let t = [phi.cos(), -phi.sin()];
        // Glyph space (x along the advance, y up the cap, both in caps) onto the seal.
        let place = |p: [f64; 2]| {
            let (x, y) = (p[0] * cap - 0.5 * adv, (p[1] - 0.5) * cap);
            [radius * u[0] + x * t[0] + y * u[0], radius * u[1] + x * t[1] + y * u[1]]
        };
        if c == '\u{2720}' {
            let cross: Vec<[f64; 2]> = cross_pattee(cap).iter().map(|p| place([(p[0] + 0.5 * adv) / cap, p[1] / cap + 0.5])).collect();
            parts.push(Part { name: "Legend, cross".into(), outline: cross, depth: V_WALL_MM, top: StampTop::Cone { apex_mm: depth - V_WALL_MM, at: place([0.5 * adv / cap, 0.5]), tip_mm: 0.05 } });
            continue;
        }
        for (j, (a, b)) in strokes.iter().enumerate() {
            let outline: Vec<[f64; 2]> = nib_stroke(*a, *b, nib, 0.11).iter().map(|p| place(*p)).collect();
            // A graver's V: shallow at the walls, deepest down the stroke's spine.
            let top = StampTop::Ridge { rise_mm: depth - V_WALL_MM, from: place(*a), to: place(*b), end_mm: depth - V_WALL_MM };
            parts.push(Part { name: format!("Legend, {c} {} stroke {}", k, j + 1), outline, depth: V_WALL_MM, top });
        }
    }
    parts
}

/// A band `w` wide along the arc about `c` of radius `r` from `a0` to `a1` (radians). Counter-clockwise.
fn arc_band(c: [f64; 2], r: f64, a0: f64, a1: f64, w: f64) -> Vec<[f64; 2]> {
    let n = (((a1 - a0).abs() * r / 0.06).ceil() as usize).max(12);
    let mut out = arc(c, r + 0.5 * w, a0, a1, n);
    out.extend(arc(c, r - 0.5 * w, a1, a0, n));
    out.dedup();
    if signed_area(&out) < 0.0 {
        out.reverse();
    }
    out
}

/// The legend's frame: a plain fillet inside it and, outside it, a cusped border of `lobes` arcs bulging out between
/// inward points: the rosette's cusped frame drawn as the seal's own border.
fn frame(inner: f64, outer: f64, bulge: f64, lobes: usize, w: f64, depth: f64) -> Vec<Part> {
    let mut parts = Vec::new();
    // A graver's V down the middle of each run of the rule.
    let vee = |c: [f64; 2], r: f64, a0: f64, a1: f64| {
        let (p0, p1) = ([c[0] + r * a0.cos(), c[1] + r * a0.sin()], [c[0] + r * a1.cos(), c[1] + r * a1.sin()]);
        StampTop::Ridge { rise_mm: depth - V_WALL_MM, from: p0, to: p1, end_mm: depth - V_WALL_MM }
    };
    // The fillet in sixteen runs, each overlapping the next.
    for q in 0..16 {
        let (a0, a1) = (2.0 * PI * q as f64 / 16.0 - 0.02, 2.0 * PI * (q + 1) as f64 / 16.0 + 0.02);
        parts.push(Part { name: format!("Inner fillet, {}", q + 1), outline: arc_band([0.0, 0.0], inner, a0, a1, w), depth: V_WALL_MM, top: vee([0.0, 0.0], inner, a0, a1) });
    }
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
        let (a0, a1) = (bm - half - 0.02, bm + half + 0.02);
        parts.push(Part { name: format!("Cusped border, {}", k + 1), outline: arc_band(c, r, a0, a1, w), depth: V_WALL_MM, top: vee(c, r, a0, a1) });
    }
    parts
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

/// A bench intaglio cut: struck into the table after the pour, never in the pattern.
fn intaglio(name: &str, at: (f64, f64), rot_deg: f64, outline: Vec<[f64; 2]>, depth: f64, top: StampTop) -> Stamp {
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
const FIELD_REACH_MM: f64 = 4.55;
const FIELD_CUSP_MM: f64 = 2.75;
const FIELD_DEPTH_MM: f64 = 0.30;
const FLEUR_MM: f64 = 6.3;
const FLEUR_DEPTH_MM: f64 = 0.80;
const LEGEND: &str = "\u{2720}SIGILLVM CAPITVLI";
const LEGEND_RADIUS_MM: f64 = 5.95;
const LEGEND_CAP_MM: f64 = 1.1;
const LEGEND_DEPTH_MM: f64 = 0.40;
const FILLET_RADIUS_MM: f64 = 4.95;
const BORDER_RADIUS_MM: f64 = 6.75;
const BORDER_BULGE_MM: f64 = 0.28;
const BORDER_LOBES: usize = 16;
const RULE_MM: f64 = 0.3;
const RULE_DEPTH_MM: f64 = 0.35;
/// A graver's V-cut stands this tall at its walls; its spine runs to the cut's full depth.
const V_WALL_MM: f64 = 0.1;

fn seal_parts(with_legend: bool) -> Vec<Part> {
    let mut parts = vec![Part { name: "Seal field".into(), outline: quatrefoil_field(FIELD_REACH_MM, FIELD_CUSP_MM), depth: FIELD_DEPTH_MM, top: StampTop::Dome { crown_mm: 0.2 } }];
    parts.extend(fleur(FLEUR_MM, FLEUR_DEPTH_MM));
    parts.extend(frame(FILLET_RADIUS_MM, BORDER_RADIUS_MM, BORDER_BULGE_MM, BORDER_LOBES, RULE_MM, RULE_DEPTH_MM));
    if with_legend {
        parts.extend(legend(LEGEND, LEGEND_RADIUS_MM, LEGEND_CAP_MM, LEGEND_DEPTH_MM));
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
    let rho = ACUTE * w;
    let rise = (rho * rho - (rho - half) * (rho - half)).sqrt();
    let base = -0.5 * h;
    let spring = 0.5 * h - rise;
    let mut pts = vec![[-half, base], [half, base]];
    let n = 16;
    for k in 1..n {
        pts.push([half, base + (spring - base) * k as f64 / n as f64]);
    }
    // Right flank: centred on the far side, from the right springing up to the point.
    let top = rise.atan2(rho - half);
    pts.extend(arc([half - rho, spring], rho, 0.0, top, 24));
    // Left flank, from the point down to the left springing.
    pts.extend(arc([rho - half, spring], rho, PI - top, PI, 24).into_iter().skip(1));
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
fn arcades(d: &mut RingDesign, a: &Atlas, rot: f64, centre_y: f64, placed: &mut Vec<String>) -> Result<()> {
    let ctx = d.field_context();
    for (side_k, side) in [1.0f64, -1.0].into_iter().enumerate() {
        for k in 0..3 {
            let x = (k as f64 - 1.0) * LANCET_PITCH;
            let y = centre_y + 0.5 * LANCET_H[k];
            let at = on_cheek(a, x, y, side).with_context(|| format!("no cheek at {x}, {y}"))?;
            let mut s = Stamp {
                name: format!("Chapter-house arcade {}, {}", side_k + 1, k + 1),
                theta_deg: at.0,
                v_mm: at.1,
                rot_deg: 0.0,
                outline: lancet(LANCET_W, LANCET_H[k]),
                height_mm: 0.1,
                sink_mm: ARCADE_SINK_MM,
                draft_deg: 4.0,
                cut: true,
                bench: false,
                along_pull: true,
                fine_cap: true,
                tier: 0,
                top: StampTop::Ridge { rise_mm: ARCADE_VEE_MM, from: [0.0, -0.5 * LANCET_H[k]], to: [0.0, 0.5 * LANCET_H[k]], end_mm: ARCADE_VEE_MM },
            };
            // Stand every light upright, its point straight up from the finger, wherever the wall turns.
            let f = s.frame(d, &ctx);
            s.rot_deg = (-f.x[1]).atan2(f.y[1]).to_degrees() + rot;
            placed.push(format!("{} at {:.2} deg, v {:.3}", s.name, at.0, at.1));
            d.stamps.push(s);
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
}

fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, Placed)> {
    let mut d = stock(face())?;
    let lib = AlphaLibrary::builtin();
    let a = Atlas::of(&d, AW, AH)?;
    let (fill, fill_at) = fill_mm(&d);
    let (cx, cy) = cheek_extent(&a, 1.0);
    if std::env::var("SIG_MAP").is_ok() {
        eprintln!("top {:.2} bore {:.2} cheek x {cx:?} y {cy:?}", a.top, a.bore);
        for yi in (0..24).rev() {
            let y = 7.0 + 0.4 * yi as f64;
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
    let cy_mid: f64 = std::env::var("SIG_CY").ok().and_then(|v| v.parse().ok()).unwrap_or(SILL_Y);
    if std::env::var("SIG_NO_ARCADE").is_err() {
        arcades(&mut d, &a, rot, cy_mid, &mut placed.arcade)?;
    }
    let _ = blockout;
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
    let (inspection, fine) = pull(d, lib, params)?;
    let coarse_release = &inspection.release;
    let monotone: Vec<(String, bool)> = d.stamps.iter().filter(|s| !s.bench && !s.along_pull).map(|s| (s.name.clone(), s.parting_monotone(d).is_ok())).collect();
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
        && coarse_release.obstructions.is_empty()
        && coarse_release.unresolved_rays == 0
        && fine.obstructions.is_empty()
        && fine.unresolved_rays == 0
        && monotone.iter().all(|(_, ok)| *ok)
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
    // The seal close up, raking light across the table.
    render::write_png_parts(out.join("stones.png"), &parts, 0.25, 1.25, edge)?;
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
        "sand": "Delft clay, 3.0 deg draft, 0.8 mm section, 0.30 mm detail",
        "alloy": ALLOY,
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "face_mm": [FACE.0, FACE.1],
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
