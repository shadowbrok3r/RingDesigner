//! Tenebrae — Capsa, the reliquary: a Gothic chasse on a plinth, a true skull for its relic, lost wax, two castings.
//! cargo build --release -p ringdesign-core --example tenebrae_capsa
//! target/release/examples/tenebrae_capsa [OUT_DIR] [--option across|openwork|across-openwork] [--draft] [--verify]
//!
//! Three rethinks of the block-out:
//! - `across`: the chasse lies on its back across the finger, so its west front (gable, portal, lancet) faces the face camera.
//! - `openwork`: the chasse lies along the finger as before, its roof pierced with tracery over the skull.
//! - `across-openwork`: the west front faces up, its portal opened into a traceried window over the skull, and the roof pierced.
use anyhow::{Context, Result, ensure};
use cadkernel::brep;
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    cad::{self, Attach, Boolean, Component, ComponentRole, Document, EdgeRef, EvaluatedComponent, FaceRef, FaceSeat, Feature, Operation, Placement, Profile, Stage, builders},
    castability::CastProcess,
    gem::{Gem, GemCut},
    library, mesh, render,
    sketch::{Geometry, Id, Sketch, Workplane},
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Opt {
    Across,
    Openwork,
    AcrossOpenwork,
}
impl Opt {
    fn slug(self) -> &'static str {
        match self {
            Self::Across => "across",
            Self::Openwork => "openwork",
            Self::AcrossOpenwork => "across-openwork",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [Self::Across, Self::Openwork, Self::AcrossOpenwork].into_iter().find(|o| o.slug() == s)
    }
    fn across(self) -> bool {
        self != Self::Openwork
    }
    fn openwork(self) -> bool {
        self != Self::Across
    }
}

const BORE_MM: f64 = 18.6;
/// How far the Cathedral swell lifts the crest over the nominal outer radius at the top, mm.
const CREST_LIFT_MM: f64 = 0.29;
const MIN_SECTION_MM: f64 = 0.8;
const OVERSHOOT_MM: f64 = 0.3;
/// The drawn collets: wall, clearance round the stone, how far the stone's base stands over its seat, the rim over that
/// (rubbed over at the bench), and how far the tube sinks into what it stands on.
const COLLET_WALL_MM: f64 = 0.9;
const COLLET_CLEAR_MM: f64 = 0.03;
const STONE_LIFT_MM: f64 = 0.02;
const COLLET_RIM_MM: f64 = 0.45;
const COLLET_SINK_MM: f64 = 0.3;
const COLLET_SIDES: usize = 24;
const LID_CLEAR_MM: f64 = 0.05;
/// The shoulder quatrefoils: offsets from the top and their sizes.
const PIERCINGS: [(f64, f64); 3] = [(48.0, 2.0), (60.0, 1.6), (72.0, 1.3)];
/// Tracery bars: the section floor.
const BAR_MM: f64 = 0.8;

// ---- Along the finger (the `openwork` option): the chasse of the first block-out, its roof opened.
/// The plinth's base course: along the finger, round the ring, tall; its top stands this far over the crest.
const PLINTH: P3 = [14.4, 7.2, 2.0];
const PLINTH_OVER_CREST_MM: f64 = 0.15;
const PLINTH_CHAMFER_MM: f64 = 0.35;
/// The plinth's upper step, sunk this far into the base course.
const STEP: P3 = [13.8, 6.6, 0.4];
const STEP_CHAMFER_MM: f64 = 0.25;
/// The chest: along the finger, round the ring, tall, sunk this far into the step.
const CHEST: P3 = [13.0, 5.6, 2.9];
const SINK_MM: f64 = 0.1;
const WALL_MM: f64 = 1.35;
const FLOOR_RAISE_MM: f64 = 0.3;
const NICHE_MM: f64 = 0.5;
const POST_MM: f64 = 0.9;
const BUTTRESS_W_MM: f64 = 0.8;
const BAY_W_MM: [f64; 3] = [2.75, 3.9, 2.75];
const BAY_SHARE: [f64; 3] = [0.75, 0.66, 0.75];
const BAY_SILL_MM: f64 = -1.3;
const BAY_APEX_MM: f64 = 1.3;
const ORDER_MM: f64 = 0.2;
const ORDER_INSET_MM: f64 = 0.4;
const BAY_STONE_Z_MM: f64 = -0.15;
const PORTAL_W_MM: f64 = 2.6;
const PORTAL_SILL_MM: f64 = -1.3;
const PORTAL_APEX_MM: f64 = 0.85;
const PORTAL_SHARE: f64 = 0.9;
const WINDOW_W_MM: f64 = 2.8;
const WINDOW_SILL_MM: f64 = 0.15;
const WINDOW_APEX_MM: f64 = 3.45;
const WINDOW_SHARE: f64 = 0.8;
const WINDOW_MM: f64 = 0.45;
const WINDOW_STONE_MM: f64 = 1.75;
const ARCHIVOLT_MM: f64 = 0.4;
const BUTTRESS_FOOT_MM: f64 = 0.38;
const BUTTRESS_UPPER_MM: f64 = 0.22;
const EAVES_OVER_MM: f64 = 0.55;
const EAVES_H_MM: f64 = 0.35;
const RISE_MM: f64 = 4.25;
const ENDS_OVER_MM: f64 = 0.3;
const EAVES_CHAMFER_MM: f64 = 0.2;
/// The opened roof's skin, measured square to the slope.
const ROOF_SKIN_MM: f64 = 0.9;
const FLEUR_PITCH_MM: f64 = 2.3;

// ---- Across the finger (the `across` options): the chasse on its back, its west front up.
// Frame: x along the finger toward the gable's apex, y across, z out of the plinth's top.
/// The chest: across, along the finger, deep (out of the ring); walls.
const XW: f64 = 7.6;
const XLC: f64 = 6.0;
const XD: f64 = 5.4;
const XT: f64 = 1.35;
/// The roof (the lid): eaves overhang across, eave band along, rise to the apex, and how far its gable stands proud of the chest's end wall.
const XEO: f64 = 0.5;
const XEH: f64 = 0.5;
const XR: f64 = 6.0;
const XPROUD: f64 = 0.3;
/// The chest's foot along the finger; the plinth runs past it and the finial past the apex.
const XA: f64 = -6.375;
const XPLINTH_RUN: f64 = 1.0;
/// The bed the shrine lies on, sunk into the crest, and its margin.
const XBED_MM: f64 = 2.3;
const XBED_MARGIN: f64 = 0.35;
/// The towers flanking the front: inner and outer edge across, how far their shafts reach down from the front and stand proud of it,
/// where their spires spring (past the eaves) and their tips.
const XTW_IN: f64 = 4.4;
const XTW_OUT: f64 = 5.9;
const XTW_DEPTH: f64 = 2.4;
const XTW_PROUD: f64 = 0.5;
const XSPIRE_FROM: f64 = 0.9;
const XSPIRE_TO: f64 = 4.9;
/// The rose in the gable: its petals' radius round the sapphire and their size.
const XROSE_PETALS: usize = 8;
const XROSE_R: f64 = 1.74;
const XROSE_PETAL: f64 = 0.34;
/// Opened: the skull's place along the chest from its foot, its scale, and where the band stops covering the back (the back lights start).
const XSKULL_X: f64 = 3.0;
const XSKULL_S: f64 = 1.0;
const XBACK_FROM: f64 = 2.75;
/// Opened, the gable's plate over the roof's hollow.
const XGABLE_PLATE: f64 = 2.3;
/// How deep the towers' and the roof's garnet bays sink.
const XBAY_MM: f64 = 0.6;
/// Draft on the front's sunk and raised work, so its walls slope and catch the light from straight above.
const XSPLAY_DEG: f64 = 32.0;

fn xb() -> f64 {
    XA + XLC
}
fn xe() -> f64 {
    xb() + LID_CLEAR_MM
}
fn xr() -> f64 {
    xe() + XEH
}
fn xp() -> f64 {
    xr() + XR
}
fn xhw() -> f64 {
    XW / 2.0 + XEO
}

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}
fn probe_params() -> BuildParams {
    BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
}

fn band() -> RingDesign {
    let mut d = RingDesign { name: "Capsa \u{2014} the reliquary".into(), ..RingDesign::default() };
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 4.6;
    d.profile.thickness_mm = 2.3;
    d.shank.kind = ShankKind::Cathedral;
    d.shank.amount = 0.5;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    d
}

fn garnet() -> Gem {
    Gem { l_mm: 2.3, preview_tint: Some([0.30, 0.02, 0.03]), ..Gem::cabochon(GemCut::Oval, 1.8) }
}
fn sapphire() -> Gem {
    Gem { l_mm: 2.3, preview_tint: Some([0.02, 0.06, 0.45]), ..Gem::cabochon(GemCut::Oval, 1.8) }
}

/// The plinth's top over the nominal outer radius.
fn plinth_top() -> f64 {
    CREST_LIFT_MM + PLINTH_OVER_CREST_MM
}
/// Along: height over the nominal outer radius the chest's centre is seated at.
fn chest_h() -> f64 {
    plinth_top() - SINK_MM + STEP[2] - SINK_MM + CHEST[2] / 2.0
}
/// Along: the chest's frame (x along the finger, y round the ring, z out, origin at the chest's centre).
fn seat() -> Placement {
    Placement::ring(90.0, chest_h())
}
fn local_z(h: f64) -> f64 {
    h - chest_h()
}
/// Across: the shrine's frame, origin on the plinth's top over the band's mid-plane.
fn xseat() -> Placement {
    Placement::ring(90.0, plinth_top())
}

// ---------------------------------------------------------------- sketches

fn plane(origin: P3, x: P3, y: P3) -> Workplane {
    Workplane { origin, x, y, on_face: None }
}
fn sketch_on(name: &str, wp: Workplane) -> Sketch {
    Sketch { name: name.into(), plane: wp, ..Sketch::default() }
}
fn polygon(sk: &mut Sketch, pts: &[P2]) {
    let ids: Vec<Id> = pts.iter().map(|p| sk.point(*p)).collect();
    sk.entity(Geometry::Polyline { points: ids, closed: true });
}
/// A sketch on the across frame's plan at height `z`, drawn in (x, y), its normal up.
fn plan(name: &str, z: f64) -> Sketch {
    sketch_on(name, plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]))
}
/// A sketch on the across frame's plan at height `z` for arches that point along +x: u across, v along the finger.
/// Its normal is down (u along +y) unless `up` (u along -y); every arch here is drawn symmetric or placed by `u`.
fn plan_arch(name: &str, z: f64, up: bool) -> Sketch {
    if up { sketch_on(name, plane([0.0, 0.0, z], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0])) } else { sketch_on(name, plane([0.0, 0.0, z], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0])) }
}
/// A pointed (two-centred) arch niche: `cx` its axis, `w` its span, `z0` its sill, `apex` its point, `share` each arc's radius over the span.
fn lancet(sk: &mut Sketch, cx: f64, w: f64, z0: f64, apex: f64, share: f64) {
    lancet_off(sk, cx, w, z0, apex, share, 0.0);
}
/// [`lancet`] offset by `d` (out positive): the same two centres, each arc's radius and the jambs moved by `d`, the sill kept.
fn lancet_off(sk: &mut Sketch, cx: f64, w: f64, z0: f64, apex: f64, share: f64, d: f64) {
    let a = w / 2.0;
    let r = share * w;
    let rise = (r * r - (r - a) * (r - a)).sqrt();
    let zs = apex - rise;
    let (a2, r2) = (a + d, r + d);
    let top = zs + (r2 * r2 - (r - a) * (r - a)).sqrt();
    let bl = sk.point([cx - a2, z0]);
    let br = sk.point([cx + a2, z0]);
    let sr = sk.point([cx + a2, zs]);
    let ap = sk.point([cx, top]);
    let sl = sk.point([cx - a2, zs]);
    let cr = sk.point([cx + a - r, zs]);
    let cl = sk.point([cx - a + r, zs]);
    sk.entity(Geometry::Line { a: bl, b: br });
    sk.entity(Geometry::Line { a: br, b: sr });
    sk.entity(Geometry::Arc { center: cr, start: sr, end: ap });
    sk.entity(Geometry::Arc { center: cl, start: ap, end: sl });
    sk.entity(Geometry::Line { a: sl, b: bl });
}
/// A pointed arch as a polygon (counter-clockwise), each arc in `n` chords: for drafted extrusions, which take straight sides.
fn lancet_poly(cx: f64, w: f64, z0: f64, apex: f64, share: f64, n: usize) -> Vec<P2> {
    let a = w / 2.0;
    let r = share * w;
    let rise = (r * r - (r - a) * (r - a)).sqrt();
    let zs = apex - rise;
    let t1 = rise.atan2(r - a);
    let mut p = vec![[cx - a, z0], [cx + a, z0]];
    for k in 0..n {
        let t = t1 * k as f64 / n as f64;
        p.push([cx + a - r + r * t.cos(), zs + r * t.sin()]);
    }
    p.push([cx, apex]);
    for k in (0..n).rev() {
        let t = t1 * k as f64 / n as f64;
        p.push([cx - a + r - r * t.cos(), zs + r * t.sin()]);
    }
    p
}
/// The archivolt round a pointed arch of span `w`: the arch's two arcs offset out by `band`, closed down both jambs to the sill.
fn arch_frame(w: f64, sill: f64, apex: f64, share: f64, band: f64, n: usize) -> Vec<P2> {
    let a = w / 2.0;
    let r = share * w;
    let rise = (r * r - (r - a) * (r - a)).sqrt();
    let zs = apex - rise;
    let arc = |r: f64, right: bool| -> Vec<P2> {
        let c = if right { a - share * w } else { -a + share * w };
        let reach = (r * r - c * c).sqrt();
        let t1 = reach.atan2(-c);
        let t0 = if right { 0.0 } else { PI };
        (0..=n).map(|k| { let t = t0 + (t1 - t0) * k as f64 / n as f64; [c + r * t.cos(), zs + r * t.sin()] }).collect()
    };
    let mut p = Vec::new();
    p.push([a + band, sill]);
    let mut right = arc(r + band, true);
    let mut left = arc(r + band, false);
    left.reverse();
    p.extend(right.drain(..));
    p.pop();
    p.extend(left.drain(..));
    p.push([-a - band, sill]);
    p.push([-a, sill]);
    let right_in = arc(r, true);
    let left_in = arc(r, false);
    p.extend(left_in.iter().copied());
    p.pop();
    p.extend(right_in.iter().rev().copied());
    p.push([a, sill]);
    p.dedup_by(|x, y| (x[0] - y[0]).abs() < 1e-9 && (x[1] - y[1]).abs() < 1e-9);
    p
}

/// Signed distance fields, traced to closed outlines for figurative sketches.
mod sdf {
    use super::P2;
    pub fn circle(p: P2, c: P2, r: f64) -> f64 {
        (p[0] - c[0]).hypot(p[1] - c[1]) - r
    }
    pub fn rect(p: P2, lo: P2, hi: P2) -> f64 {
        let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let h = [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0];
        let d = [(p[0] - c[0]).abs() - h[0], (p[1] - c[1]).abs() - h[1]];
        d[0].max(0.0).hypot(d[1].max(0.0)) + d[0].max(d[1]).min(0.0)
    }
    pub fn ellipse(p: P2, c: P2, a: f64, b: f64) -> f64 {
        let q = [(p[0] - c[0]) / a, (p[1] - c[1]) / b];
        (q[0].hypot(q[1]) - 1.0) * a.min(b)
    }
    /// A convex polygon (counter-clockwise), as the largest signed distance to its edges' lines.
    pub fn convex(p: P2, pts: &[P2]) -> f64 {
        let n = pts.len();
        (0..n)
            .map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let l = dx.hypot(dy);
                ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx) / l
            })
            .fold(f64::MIN, f64::max)
    }
    pub fn smin(a: f64, b: f64, k: f64) -> f64 {
        let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
        b + (a - b) * h - k * h * (1.0 - h)
    }
    /// The closed outlines where `f` crosses zero over `lo..hi`, traced on a `step` grid.
    pub fn trace(f: &dyn Fn(P2) -> f64, lo: P2, hi: P2, step: f64) -> Vec<Vec<P2>> {
        let nx = ((hi[0] - lo[0]) / step).ceil() as usize + 1;
        let ny = ((hi[1] - lo[1]) / step).ceil() as usize + 1;
        let at = |i: usize, j: usize| [lo[0] + i as f64 * step, lo[1] + j as f64 * step];
        let mut v = vec![0.0; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let x = f(at(i, j));
                v[j * nx + i] = if x.abs() < 1e-9 { 1e-9 } else { x };
            }
        }
        let cross = |k: usize| -> P2 {
            let idx = k / 2;
            let (i, j) = (idx % nx, idx / nx);
            let (i2, j2) = if k % 2 == 0 { (i + 1, j) } else { (i, j + 1) };
            let (a, b) = (v[j * nx + i], v[j2 * nx + i2]);
            let t = a / (a - b);
            let (p, q) = (at(i, j), at(i2, j2));
            [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
        };
        let mut next: std::collections::HashMap<usize, usize> = Default::default();
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let c = [v[j * nx + i], v[j * nx + i + 1], v[(j + 1) * nx + i + 1], v[(j + 1) * nx + i]];
                let e = [2 * (j * nx + i), 2 * (j * nx + i + 1) + 1, 2 * ((j + 1) * nx + i), 2 * (j * nx + i) + 1];
                let ins: Vec<bool> = c.iter().map(|x| *x < 0.0).collect();
                let mut crossings = Vec::new();
                for k in 0..4 {
                    if ins[k] != ins[(k + 1) % 4] {
                        crossings.push((k, ins[k]));
                    }
                }
                let pair = |from: usize, to: usize, next: &mut std::collections::HashMap<usize, usize>| {
                    next.insert(e[from], e[to]);
                };
                match crossings.len() {
                    2 => {
                        let (k0, in0) = crossings[0];
                        let (k1, _) = crossings[1];
                        if in0 { pair(k1, k0, &mut next) } else { pair(k0, k1, &mut next) }
                    }
                    4 => {
                        let centre = f([lo[0] + (i as f64 + 0.5) * step, lo[1] + (j as f64 + 0.5) * step]) < 0.0;
                        let ks: Vec<usize> = crossings.iter().map(|c| c.0).collect();
                        let in0 = crossings[0].1;
                        if in0 == centre {
                            if in0 { pair(ks[1], ks[0], &mut next); pair(ks[3], ks[2], &mut next) } else { pair(ks[0], ks[1], &mut next); pair(ks[2], ks[3], &mut next) }
                        } else if in0 {
                            pair(ks[3], ks[0], &mut next); pair(ks[1], ks[2], &mut next)
                        } else {
                            pair(ks[0], ks[3], &mut next); pair(ks[2], ks[1], &mut next)
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut loops = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut starts: Vec<usize> = next.keys().copied().collect();
        starts.sort();
        for s in starts {
            if seen.contains(&s) {
                continue;
            }
            let mut lp = Vec::new();
            let mut k = s;
            while seen.insert(k) {
                lp.push(cross(k));
                match next.get(&k) {
                    Some(n) => k = *n,
                    None => break,
                }
            }
            if lp.len() >= 3 {
                loops.push(lp);
            }
        }
        loops
    }
    pub fn area(p: &[P2]) -> f64 {
        (0..p.len()).map(|i| { let (a, b) = (p[i], p[(i + 1) % p.len()]); a[0] * b[1] - b[0] * a[1] }).sum::<f64>() / 2.0
    }
    /// Douglas–Peucker on a closed loop, to `tol`.
    pub fn simplify(p: &[P2], tol: f64) -> Vec<P2> {
        fn dp(p: &[P2], tol: f64, out: &mut Vec<P2>) {
            let (a, b) = (p[0], p[p.len() - 1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l = dx.hypot(dy).max(1e-12);
            let mut best = (0.0, 0);
            for (i, q) in p.iter().enumerate().take(p.len() - 1).skip(1) {
                let d = ((q[0] - a[0]) * dy - (q[1] - a[1]) * dx).abs() / l;
                if d > best.0 {
                    best = (d, i);
                }
            }
            if best.0 > tol {
                dp(&p[..=best.1], tol, out);
                dp(&p[best.1..], tol, out);
            } else {
                out.push(a);
            }
        }
        let n = p.len();
        let far = (0..n).max_by(|i, j| { let d = |k: usize| (p[k][0] - p[0][0]).hypot(p[k][1] - p[0][1]); d(*i).total_cmp(&d(*j)) }).unwrap_or(0);
        let mut out = Vec::new();
        let first: Vec<P2> = p[..=far].to_vec();
        let mut second: Vec<P2> = p[far..].to_vec();
        second.push(p[0]);
        dp(&first, tol, &mut out);
        dp(&second, tol, &mut out);
        out
    }
}

/// The outline of `f` as polygons, simplified to `tol` and at most `max` points each, counter-clockwise.
fn outlines(f: &dyn Fn(P2) -> f64, lo: P2, hi: P2, step: f64, tol: f64, max: usize) -> Vec<Vec<P2>> {
    sdf::trace(f, lo, hi, step)
        .into_iter()
        .filter(|l| sdf::area(l).abs() > 0.02)
        .map(|l| {
            let mut t = tol;
            let mut s = sdf::simplify(&l, t);
            while s.len() > max {
                t *= 1.3;
                s = sdf::simplify(&l, t);
            }
            if sdf::area(&s) < 0.0 {
                s.reverse();
            }
            s
        })
        .collect()
}
/// The one closed outline of `f`, or why there is not one.
fn one_outline(name: &str, f: &dyn Fn(P2) -> f64, lo: P2, hi: P2, step: f64, max: usize) -> Result<Vec<P2>> {
    let mut loops = outlines(f, lo, hi, step, step * 0.3, max);
    ensure!(loops.len() == 1, "{name} traced to {} loops", loops.len());
    Ok(loops.remove(0))
}

// ---------------------------------------------------------------- features

struct Ids(Id);
impl Ids {
    fn next(&mut self) -> Id {
        self.0 += 1;
        self.0
    }
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}
fn placed(placement: Placement) -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Separate, placement, ..Component::default() }
}
fn joined(role: ComponentRole, placement: Placement, blend: f64) -> Component {
    Component { role, attach: Attach::Join, placement, blend_mm: blend, ..Component::default() }
}
fn cutter(p: Placement) -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, placement: p, ..Component::default() }
}
fn extrude(sk: Sketch, height: f64) -> Operation {
    Operation::Extrude { sketch: Profile::Inline(sk), height_mm: height, draft_deg: 0.0 }
}
fn drafted(sk: Sketch, height: f64, draft: f64) -> Operation {
    Operation::Extrude { sketch: Profile::Inline(sk), height_mm: height, draft_deg: draft }
}
/// A rectangle's corners, counter-clockwise.
fn rect_pts(x0: f64, x1: f64, y0: f64, y1: f64) -> Vec<P2> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
}
fn boolean(a: Id, b: Id, kind: Boolean) -> Operation {
    Operation::Boolean { a, b, kind }
}
/// A sketch of several loops kept as its own feature, and every region of it extruded by `height`; returns the extrusion.
fn regions(doc: &mut Document, ids: &mut Ids, name: &str, sk: Sketch, height: f64, component: Component) -> Result<Id> {
    let drawn = ids.next();
    doc.append(feature(drawn, &format!("Draw {}", sk.name.to_lowercase()), Operation::Sketch { sketch: sk }, Component::default()))?;
    let id = ids.next();
    doc.append(feature(id, name, Operation::Extrude { sketch: Profile::Feature { feature: drawn }, height_mm: height, draft_deg: 0.0 }, component))?;
    Ok(id)
}

/// Part `id` of `d` as evaluated with the document stopped after it.
fn part_at(d: &RingDesign, lib: &AlphaLibrary, id: Id) -> Result<EvaluatedComponent> {
    let mut d = d.clone();
    let doc = d.cad.as_mut().context("no document")?;
    doc.through = Some(id);
    let e = cad::evaluate(&d, lib, probe_params())?;
    if let Some(err) = e.first_error() {
        anyhow::bail!("{err}");
    }
    e.components.into_iter().find(|c| c.id == id).with_context(|| format!("#{id} is not an output"))
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn dist(a: P3, b: P3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn add(p: P3, v: P3, k: f64) -> P3 {
    [p[0] + v[0] * k, p[1] + v[1] * k, p[2] + v[2] * k]
}
fn neg(v: P3) -> P3 {
    [-v[0], -v[1], -v[2]]
}
fn world_dir(d: &RingDesign, fr: &Placement, v: P3) -> Result<P3> {
    let f = fr.frame(d)?;
    Ok(std::array::from_fn(|k| f.x_axis[k] * v[0] + f.y_axis[k] * v[1] + f.z_axis[k] * v[2]))
}
/// The planar face of `c` whose world normal runs along `normal` (frame `fr`) with its centre nearest `near` (frame `fr`).
fn face_near(d: &RingDesign, fr: &Placement, c: &EvaluatedComponent, normal: P3, near: P3) -> Result<usize> {
    let n = world_dir(d, fr, normal)?;
    let p = fr.world(d, near)?;
    (0..c.body.faces.len())
        .filter_map(|i| cad::face_signature(&c.body, i, &brep::Placement::IDENTITY).map(|s| (i, s)))
        .filter(|(_, s)| s.kind == cad::SurfaceKind::Plane && dot(s.normal, n) > 0.995)
        .min_by(|a, b| dist(a.1.centre, p).total_cmp(&dist(b.1.centre, p)))
        .map(|(i, _)| i)
        .with_context(|| format!("#{} has no face along {normal:?}", c.id))
}
fn face_ref(c: &EvaluatedComponent, i: usize) -> FaceRef {
    FaceRef::signed(&c.body, i, &c.frame)
}
/// The straight edges of `c` whose midpoints (frame `fr`) satisfy `keep`.
fn edges_where(d: &RingDesign, fr: &Placement, c: &EvaluatedComponent, keep: &dyn Fn(P3) -> bool) -> Result<Vec<EdgeRef>> {
    let f = fr.frame(d)?;
    let local = |p: P3| -> P3 {
        let q = [p[0] - f.origin[0], p[1] - f.origin[1], p[2] - f.origin[2]];
        [dot(q, f.x_axis), dot(q, f.y_axis), dot(q, f.z_axis)]
    };
    let mut out = Vec::new();
    for i in 0..c.body.edges.len() {
        if let Some(s) = cad::edge_signature(&c.body, i, &brep::Placement::IDENTITY) {
            if keep(local(s.midpoint)) {
                out.push(EdgeRef::signed(&c.body, i, &c.frame));
            }
        }
    }
    Ok(out)
}

/// A cabochon on face `face` of part `c`, centred at `at` (frame `fr`), its seat `sunk` under the face, in a collet drawn as a tube:
/// a wall `COLLET_WALL_MM` thick round the stone's own oval, rising `COLLET_RIM_MM` over the seat for the setter to rub over.
/// The tube joins the ring when `ring`, else it is left for the caller to unite with its part; returns (stone, collet).
#[allow(clippy::too_many_arguments)]
fn cabochon(doc: &mut Document, ids: &mut Ids, d: &mut RingDesign, _lib: &AlphaLibrary, fr: &Placement, c: &EvaluatedComponent, face: usize, at: P3, gem: Gem, spin: f64, sunk: f64, name: &str, ring: bool) -> Result<(Id, Id)> {
    let mut s = FaceSeat::on(c, face as u32, Some(fr.world(d, at)?), STONE_LIFT_MM - sunk)?;
    s.spin_deg = spin;
    let stone = ids.next();
    let mut f = cad::stone_on_face(stone, gem, c.id, &s);
    f.name = format!("{name} cabochon");
    doc.append(f)?;
    // The stone's own frame on the face (its x the long axis), carried into the frame `fr`.
    let sf = s.frame(&s.face_of(c)?, &c.frame);
    let fm = fr.frame(d)?;
    let to = |w: P3| -> P3 { [dot(w, fm.x_axis), dot(w, fm.y_axis), dot(w, fm.z_axis)] };
    let (xdir, ydir, n) = (to(sf.x_axis), to(sf.y_axis), to(sf.z_axis));
    let o = to([sf.origin[0] - fm.origin[0], sf.origin[1] - fm.origin[1], sf.origin[2] - fm.origin[2]]);
    // The seat's floor on the face's plane, under the stone's centre (the girdle's centre stands STONE_LIFT_MM - sunk over the face).
    let foot = add(o, n, -STONE_LIFT_MM - COLLET_SINK_MM);
    let mut sk = sketch_on(&format!("Collet round the {name} cabochon"), plane(foot, xdir, ydir));
    let oval = |a: f64, b: f64, ccw: bool| -> Vec<P2> {
        let mut v: Vec<P2> = (0..COLLET_SIDES).map(|i| { let t = 2.0 * PI * i as f64 / COLLET_SIDES as f64; [a * t.cos(), b * t.sin()] }).collect();
        if !ccw {
            v.reverse();
        }
        v
    };
    let (a, b) = (0.5 * gem.l_mm.max(gem.w_mm) + COLLET_CLEAR_MM, 0.5 * gem.l_mm.min(gem.w_mm) + COLLET_CLEAR_MM);
    polygon(&mut sk, &oval(a + COLLET_WALL_MM, b + COLLET_WALL_MM, true));
    polygon(&mut sk, &oval(a, b, false));
    let tube = ids.next();
    let component = if ring { joined(ComponentRole::Head, fr.clone(), 0.0) } else { placed(fr.clone()) };
    doc.append(feature(tube, &format!("Raise the collet round the {name} cabochon"), extrude(sk, COLLET_SINK_MM + STONE_LIFT_MM + COLLET_RIM_MM), component))?;
    Ok((stone, tube))
}

/// A fleur-de-lis standing on `foot`, petals up: a pointed middle petal, two side petals leaning out, a band across.
fn fleur(p: P2, foot: P2) -> f64 {
    let (x, y) = (foot[0], foot[1]);
    let (r, k) = (0.998, 0.523);
    let petal = sdf::circle(p, [x - k, y + 1.1], r).max(sdf::circle(p, [x + k, y + 1.1], r));
    let side = |s: f64| {
        let (c, sn) = (28f64.to_radians().cos(), 28f64.to_radians().sin());
        let q = [p[0] - (x + s * 0.47), p[1] - (y + 0.78)];
        let q = [q[0] * c + s * q[1] * sn, -s * q[0] * sn + q[1] * c];
        sdf::ellipse(q, [0.0, 0.0], 0.4, 0.56)
    };
    let band = sdf::rect(p, [x - 0.42, y + 0.28], [x + 0.42, y + 0.62]) - 0.2;
    let stem = sdf::rect(p, [x - 0.4, y - 0.4], [x + 0.4, y + 0.45]);
    sdf::smin(sdf::smin(petal, side(1.0).min(side(-1.0)), 0.06), band.min(stem), 0.05)
}
/// The cresting's outline in the ridge plane (x along the finger, z out): a rail and five fleurs.
fn cresting_sdf(ridge: f64) -> impl Fn(P2) -> f64 {
    move |p: P2| {
        let rail = sdf::rect(p, [-5.4, ridge - 0.7], [5.4, ridge + 0.2]);
        let mut d = rail;
        // Fleurs at 0.55 scale, rising 1.15 over the ridge, open between.
        for k in [-2.0f64, -1.0, 0.0, 1.0, 2.0] {
            let q = [(p[0] - k * FLEUR_PITCH_MM) / 0.55, (p[1] - ridge) / 0.55];
            d = d.min(0.55 * fleur(q, [0.0, 0.0]));
        }
        d
    }
}

/// A turned finial's half-section (x out from the axis, y up), from `z` in the roof to a round knop and a blunt point.
fn finial_profile(z: f64) -> Vec<P2> {
    let mut p = vec![[0.0, z], [0.55, z], [0.55, z + 0.7], [0.72, z + 0.8], [0.72, z + 0.95], [0.42, z + 1.0]];
    let (c, r) = (z + 1.6, 0.7);
    let a0 = (0.42f64 / r).acos();
    for k in 1..12 {
        let a = -a0 + 2.0 * a0 * k as f64 / 12.0;
        p.push([r * a.cos(), c + r * a.sin()]);
    }
    p.push([0.42, c + r * a0.sin() + 0.02]);
    p.push([0.42, z + 2.55]);
    for k in 1..=6 {
        let a = 0.42 * PI * k as f64 / 6.0;
        p.push([0.42 * a.cos(), z + 2.55 + 0.42 * a.sin()]);
    }
    let top = p[p.len() - 1][1];
    p.push([0.0, top]);
    p
}

/// A turned knop of radius `r` centred at `c` (frame `fr`), its axis along the frame's x: a ball with a flat at each pole.
fn knop(doc: &mut Document, ids: &mut Ids, fr: &Placement, name: &str, c: P3, r: f64, component: Component) -> Result<Id> {
    let mut sk = sketch_on(name, plane(c, [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]));
    let a0 = 0.42 * PI;
    let mut pts = vec![[0.0, -r * a0.sin()]];
    for k in 0..=16 {
        let a = -a0 + 2.0 * a0 * k as f64 / 16.0;
        pts.push([r * a.cos(), r * a.sin()]);
    }
    pts.push([0.0, r * a0.sin()]);
    polygon(&mut sk, &pts);
    let id = ids.next();
    doc.append(feature(id, name, Operation::Revolve { sketch: Profile::Inline(sk), pivot: [0.0, 0.0, 0.0], axis: [0.0, 1.0, 0.0], degrees: 360.0, in_plane: true }, Component { placement: fr.clone(), ..component }))?;
    Ok(id)
}

// ---------------------------------------------------------------- the skull

/// The skull's frontal outline (a across, b up toward the crown), at scale 1: cranium, cheekbones, maxilla and jaw.
fn skull_sdf(p: P2) -> f64 {
    let cranium = sdf::ellipse(p, [0.0, 0.55], 1.6, 1.55);
    let cheeks = sdf::ellipse(p, [0.0, -0.35], 1.62, 0.6);
    let maxilla = sdf::rect(p, [-0.95, -1.5], [0.95, -0.45]) - 0.15;
    let jaw = sdf::rect(p, [-0.78, -2.3], [0.78, -1.45]) - 0.2;
    sdf::smin(sdf::smin(cranium, cheeks, 0.3), sdf::smin(maxilla, jaw, 0.15), 0.3)
}
/// The skull's outline sampled at `n` bearings round (0, -0.1), scaled by `s`: a star-shaped loop a loft can match section to section.
fn skull_ring(n: usize, s: f64) -> Vec<P2> {
    let c = [0.0, -0.1];
    (0..n)
        .map(|i| {
            let t = 2.0 * PI * i as f64 / n as f64 + 0.5 * PI;
            let dir = [t.cos(), t.sin()];
            let mut r = 0.0;
            while r < 4.0 && skull_sdf([c[0] + dir[0] * r, c[1] + dir[1] * r]) < 0.0 {
                r += 0.002;
            }
            [s * (c[0] + dir[0] * r), s * (c[1] + dir[1] * r)]
        })
        .collect()
}
/// Where the skull lies: its frame, the point under its face's centre, the directions across it (`a`), up it (`b`) and out of its face (`n` = a × b), its scale and height.
struct SkullSeat {
    frame: Placement,
    base: P3,
    a: P3,
    b: P3,
    n: P3,
    scale: f64,
    height: f64,
}
/// The memento mori: a skull lofted from its outline into a low dome, its sockets, nose and teeth sunk into its face; returns the skull's id.
fn skull(doc: &mut Document, ids: &mut Ids, at: &SkullSeat) -> Result<Id> {
    let sections: [(f64, f64); 7] = [(-0.2, 1.0), (0.0, 1.0), (0.45, 0.995), (0.72, 0.98), (0.86, 0.96), (0.95, 0.935), (1.0, 0.9)];
    let mut profiles = Vec::new();
    for (k, (h, s)) in sections.iter().enumerate() {
        let origin = add(at.base, at.n, h * at.height);
        let mut sk = sketch_on(&format!("Skull section {}", k + 1), plane(origin, at.a, at.b));
        polygon(&mut sk, &skull_ring(64, at.scale * s));
        profiles.push(Profile::Inline(sk));
    }
    let loft = ids.next();
    doc.append(feature(loft, "Loft the skull from its outline into a dome", Operation::Loft { sections: profiles }, placed(at.frame.clone())))?;
    // The face, sunk into the dome as cones and a groove, so no wall between them falls under the section floor;
    // each drawn on a plane just over the crown (its u runs along -a) and cut into the skull alone.
    let top = add(at.base, at.n, at.height + 0.05);
    let s = at.scale;
    let face = |name: &str| sketch_on(name, plane(top, neg(at.a), at.b));
    let socket = |c: P2| -> Vec<P2> {
        let tip = 0.2 * c[0].signum();
        (0..28)
            .map(|i| {
                let t = 2.0 * PI * i as f64 / 28.0;
                let (x, y) = (0.4 * t.cos(), 0.38 * t.sin());
                let (ct, st) = (tip.cos(), tip.sin());
                [s * (c[0] + x * ct - y * st), s * (c[1] + x * st + y * ct)]
            })
            .collect()
    };
    // One sketch for the whole face, its regions cut together with one draft: the analytic kernel takes a single cut of the lofted dome.
    let mut sk = face("Skull, the face: eye sockets, nasal cavity, the line of the teeth");
    for ax in [-0.62, 0.62] {
        polygon(&mut sk, &socket([ax, 0.0]));
    }
    polygon(&mut sk, &[[0.0, -0.36], [-0.28, -0.8], [-0.36, -1.04], [-0.12, -1.14], [0.12, -1.14], [0.36, -1.04], [0.28, -0.8]].iter().map(|p| [s * p[0], s * p[1]]).collect::<Vec<_>>());
    polygon(&mut sk, &[[s * -0.62, s * -1.7], [s * 0.62, s * -1.7], [s * 0.62, s * -1.28], [s * -0.62, s * -1.28]]);
    let drawn = ids.next();
    doc.append(feature(drawn, "Draw the skull's face", Operation::Sketch { sketch: sk }, Component::default()))?;
    let tool = ids.next();
    doc.append(feature(tool, "The skull's face, its walls sloped", Operation::Extrude { sketch: Profile::Feature { feature: drawn }, height_mm: 0.05 + 0.37, draft_deg: 24.0 }, placed(at.frame.clone())))?;
    let id = ids.next();
    doc.append(feature(id, "Sink the eye sockets, the nasal cavity and the line of the teeth into the skull", boolean(loft, tool, Boolean::Subtract), placed(Placement::Free)))?;
    if let Some(f) = doc.features.iter_mut().find(|f| f.id == id) {
        f.name = format!("{}: the memento mori", f.name);
        f.component = joined(ComponentRole::Head, Placement::Free, 0.0);
    }
    Ok(id)
}

// ---------------------------------------------------------------- tracery

/// A net for `Sketch::tracery`, drawn in the sketch's plane: a pointed window of span `w` round `cx` (sill `z0`, apex `apex`)
/// split by a mullion into two pointed lights, with a roundel in the head. Returns the net's entity ids.
fn window_net(sk: &mut Sketch, cx: f64, w: f64, z0: f64, apex: f64, share: f64) -> Vec<Id> {
    let before = sk.entities.len();
    lancet(sk, cx, w, z0, apex, share);
    let a = w / 2.0;
    let r = share * w;
    let rise = (r * r - (r - a) * (r - a)).sqrt();
    let zs = apex - rise;
    let sub_w = a;
    let sub_r = 0.8 * sub_w;
    let sub_rise = (sub_r * sub_r - (sub_r - sub_w / 2.0).powi(2)).sqrt();
    let sub_apex = zs + sub_rise * 0.95;
    let sub_rise = sub_rise * 0.95;
    let sub_r = (sub_rise * sub_rise + (sub_w / 2.0) * (sub_w / 2.0)) / sub_w;
    for c in [cx - a / 2.0, cx + a / 2.0] {
        let p0 = sk.point([c - sub_w / 2.0, zs]);
        let p1 = sk.point([c + sub_w / 2.0, zs]);
        let ap = sk.point([c, sub_apex]);
        let cl = sk.point([c - sub_w / 2.0 + sub_r, zs]);
        let cr = sk.point([c + sub_w / 2.0 - sub_r, zs]);
        sk.entity(Geometry::Arc { center: cr, start: p1, end: ap });
        sk.entity(Geometry::Arc { center: cl, start: ap, end: p0 });
    }
    let m0 = sk.point([cx, z0]);
    let m1 = sk.point([cx, sub_apex]);
    sk.entity(Geometry::Line { a: m0, b: m1 });
    // The roundel in the head, between the lights' points and the window's.
    let rc = ((apex - sub_apex) * 0.36).min(a * 0.62);
    let cy = sub_apex + (apex - sub_apex) * 0.42;
    let c = sk.point([cx, cy]);
    let rim = sk.point([cx + rc, cy]);
    sk.entity(Geometry::Circle { center: c, rim });
    sk.entities[before..].iter().map(|e| e.id).collect()
}

/// Traces `net` in `sk` at the section floor; the lights it cut.
fn trace_net(sk: &mut Sketch, net: &[Id]) -> Result<usize> {
    let t = sk.tracery(net, BAR_MM)?;
    for (at, why) in &t.skipped {
        println!("  tracery: left a cell at {:?} whole: {why}", at.at);
    }
    Ok(t.lights.len())
}

// ---------------------------------------------------------------- authoring

struct Authored {
    design: RingDesign,
    lid: Id,
    chest: Id,
    stones: Vec<Id>,
    skull: Id,
    notes: Vec<String>,
}

/// A box seated at height `h` with its top edges chamfered; returns the chamfered feature.
#[allow(clippy::too_many_arguments)]
fn moulded_box(doc: &mut Document, ids: &mut Ids, d: &mut RingDesign, lib: &AlphaLibrary, name: &str, size: P3, h: f64, chamfer: f64, component: Component) -> Result<Id> {
    let block = ids.next();
    doc.append(feature(block, &format!("{name} block"), Operation::Box { size }, placed(Placement::ring(90.0, h))))?;
    d.cad = Some(doc.clone());
    let b = part_at(d, lib, block)?;
    let ztop = local_z(h + size[2] / 2.0);
    let top = face_near(d, &seat(), &b, [0.0, 0.0, 1.0], [0.0, 0.0, ztop])?;
    let rim = edges_where(d, &seat(), &b, &|m| (m[2] - ztop).abs() < 1e-3)?;
    ensure!(rim.len() == 4, "{name} top has {} edges", rim.len());
    let id = ids.next();
    doc.append(feature(id, &format!("Chamfer the {}'s top as a moulding", name.to_lowercase()), Operation::Chamfer { source: block, edges: rim, base_face: face_ref(&b, top), distance_mm: chamfer }, component))?;
    Ok(id)
}
/// An across-frame prism: `pts` (x, y, counter-clockwise) extruded from `z0` up to `z1`, its top edges chamfered by `chamfer`; returns the chamfer.
#[allow(clippy::too_many_arguments)]
fn moulded_prism(doc: &mut Document, ids: &mut Ids, d: &mut RingDesign, lib: &AlphaLibrary, name: &str, pts: &[P2], z0: f64, z1: f64, chamfer: f64, component: Component) -> Result<Id> {
    let mut sk = plan(&format!("{name} plan"), z0);
    polygon(&mut sk, pts);
    let block = ids.next();
    doc.append(feature(block, &format!("{name} block"), extrude(sk, z1 - z0), placed(xseat())))?;
    d.cad = Some(doc.clone());
    let b = part_at(d, lib, block)?;
    let cx = pts.iter().map(|p| p[0]).sum::<f64>() / pts.len() as f64;
    let top = face_near(d, &xseat(), &b, [0.0, 0.0, 1.0], [cx, 0.0, z1])?;
    let rim = edges_where(d, &xseat(), &b, &|m| (m[2] - z1).abs() < 1e-3)?;
    ensure!(rim.len() == pts.len(), "{name} top has {} edges", rim.len());
    let id = ids.next();
    doc.append(feature(id, &format!("Chamfer the {}'s top as a moulding", name.to_lowercase()), Operation::Chamfer { source: block, edges: rim, base_face: face_ref(&b, top), distance_mm: chamfer }, component))?;
    Ok(id)
}

/// Graded quatrefoils pierced through both shoulders.
fn shoulders(doc: &mut Document, ids: &mut Ids, d: &RingDesign) -> Result<()> {
    for (off, size) in PIERCINGS {
        for (side, sign) in [("east", -1.0), ("west", 1.0)] {
            let theta = 90.0 + sign * off;
            let mut at = builders::cutters::pierce_at(d, theta, 0.0, None, builders::cutters::Shape::Quatrefoil)?;
            if let Some(m) = at.params.as_object_mut() {
                m.insert("width_mm".into(), serde_json::json!(size));
                m.insert("length_mm".into(), serde_json::json!(size));
            }
            let mut f = builders::cutters::pierce_feature(ids.next(), builders::cutters::Shape::Quatrefoil, &at);
            f.name = format!("Pierce a {size:.1} mm quatrefoil through the {side} shoulder, {off:.0} deg off the top");
            doc.append(f)?;
        }
    }
    Ok(())
}

fn make_lid(doc: &mut Document, lid: Id, note: &str) {
    if let Some(f) = doc.features.iter_mut().find(|f| f.id == lid) {
        f.name = format!("{}: the lid, cast apart", f.name);
        f.component = Component { role: ComponentRole::Other, attach: Attach::Separate, stage: Stage::Cast, bench_notes: note.into(), ..Component::default() };
    }
}

fn author(lib: &AlphaLibrary, opt: Opt) -> Result<Authored> {
    if opt.across() { author_across(lib, opt.openwork()) } else { author_along(lib) }
}

/// The chasse along the finger (the first block-out's), its lid hollowed and its slopes opened in tracery over the skull.
fn author_along(lib: &AlphaLibrary) -> Result<Authored> {
    let mut d = band();
    let mut doc = Document::default();
    let mut ids = Ids(0);
    let mut notes = Vec::new();
    let band_id = ids.next();
    doc.append(feature(band_id, "Procedural shank", Operation::Band, Component::default()))?;
    let at = |doc: &Document, d: &mut RingDesign| d.cad = Some(doc.clone());
    let fr = seat();

    moulded_box(&mut doc, &mut ids, &mut d, lib, "Plinth", PLINTH, plinth_top() - PLINTH[2] / 2.0, PLINTH_CHAMFER_MM, joined(ComponentRole::Shank, Placement::Free, 0.35))?;
    moulded_box(&mut doc, &mut ids, &mut d, lib, "Step", STEP, plinth_top() - SINK_MM + STEP[2] / 2.0, STEP_CHAMFER_MM, joined(ComponentRole::Shank, Placement::Free, 0.0))?;

    let chest_box = ids.next();
    doc.append(feature(chest_box, "Chest block", Operation::Box { size: CHEST }, placed(seat())))?;
    at(&doc, &mut d);
    let cb = part_at(&d, lib, chest_box)?;
    let open = face_near(&d, &fr, &cb, [0.0, 0.0, 1.0], [0.0, 0.0, CHEST[2] / 2.0])?;
    let hollow = ids.next();
    doc.append(feature(hollow, "Hollow the chest from the top", Operation::Shell { source: chest_box, open_faces: vec![face_ref(&cb, open)], thickness_mm: WALL_MM }, placed(Placement::Free)))?;
    at(&doc, &mut d);
    let hc = part_at(&d, lib, hollow)?;
    let floor = face_near(&d, &fr, &hc, [0.0, 0.0, 1.0], [0.0, 0.0, -CHEST[2] / 2.0 + WALL_MM])?;
    let chest = ids.next();
    doc.append(feature(chest, "Press the relic floor up", Operation::PressPull { source: hollow, face: face_ref(&hc, floor), distance_mm: FLOOR_RAISE_MM }, joined(ComponentRole::Head, Placement::Free, 0.0)))?;

    // The skull on the relic floor, face up, its crown toward the east end.
    let floor_z = -CHEST[2] / 2.0 + WALL_MM + FLOOR_RAISE_MM;
    let skull_id = skull(&mut doc, &mut ids, &SkullSeat { frame: seat(), base: [0.0, 0.0, floor_z - 0.1], a: [0.0, -1.0, 0.0], b: [1.0, 0.0, 0.0], n: [0.0, 0.0, 1.0], scale: 0.82, height: 1.2 })?;

    // Blind arcades: three pointed bays on each long wall.
    let bays = || -> Vec<(f64, f64, f64)> {
        let mut x = -CHEST[0] / 2.0 + POST_MM;
        (0..3)
            .map(|k| {
                let c = x + BAY_W_MM[k] / 2.0;
                x += BAY_W_MM[k] + POST_MM;
                (c, BAY_W_MM[k], BAY_SHARE[k])
            })
            .collect()
    };
    for (side, sign) in [("north", 1.0), ("south", -1.0)] {
        let y = sign * (CHEST[1] / 2.0 + OVERSHOOT_MM);
        for (order, inset, depth) in [("outer", 0.0, ORDER_MM), ("inner", ORDER_INSET_MM, NICHE_MM)] {
            let mut sk = sketch_on(&format!("Arcade, {side} wall, {order} order"), plane([0.0, y, 0.0], [sign, 0.0, 0.0], [0.0, 0.0, 1.0]));
            for (cx, w, share) in bays() {
                lancet_off(&mut sk, sign * cx, w, BAY_SILL_MM, BAY_APEX_MM, share, -inset);
            }
            let drawn = ids.next();
            doc.append(feature(drawn, &format!("Draw the {side} arcade's {order} order: three pointed bays between posts"), Operation::Sketch { sketch: sk }, Component::default()))?;
            let tool = ids.next();
            doc.append(feature(tool, &format!("Sink the {side} arcade's {order} order into the wall"), Operation::Extrude { sketch: Profile::Feature { feature: drawn }, height_mm: OVERSHOOT_MM + depth, draft_deg: 0.0 }, cutter(seat())))?;
        }
    }
    let posts = bays();
    let mut post_x: Vec<f64> = vec![-CHEST[0] / 2.0 + POST_MM / 2.0, CHEST[0] / 2.0 - POST_MM / 2.0];
    post_x.extend([posts[0].0 + posts[0].1 / 2.0 + POST_MM / 2.0, posts[1].0 + posts[1].1 / 2.0 + POST_MM / 2.0]);
    let (z0, top) = (-CHEST[2] / 2.0 - SINK_MM + 0.05, CHEST[2] / 2.0 - 0.15);
    let buttress: Vec<P2> = vec![[-0.1, z0], [BUTTRESS_FOOT_MM, z0], [BUTTRESS_FOOT_MM, 0.2], [BUTTRESS_UPPER_MM, 0.5], [BUTTRESS_UPPER_MM, top - 0.45], [0.0, top], [-0.1, top]];
    for (side, sign) in [("north", 1.0f64), ("south", -1.0)] {
        for (k, xc) in post_x.iter().enumerate() {
            let x0 = xc - sign * BUTTRESS_W_MM / 2.0;
            let mut sk = sketch_on(&format!("Buttress {} section, {side} wall", k + 1), plane([x0, 0.0, 0.0], [0.0, sign, 0.0], [0.0, 0.0, 1.0]));
            let prof: Vec<P2> = buttress.iter().map(|p| [CHEST[1] / 2.0 + p[0], p[1]]).collect();
            polygon(&mut sk, &prof);
            let id = ids.next();
            doc.append(feature(id, &format!("Stand buttress {} against the {side} wall", k + 1), extrude(sk, BUTTRESS_W_MM), joined(ComponentRole::Head, seat(), 0.0)))?;
        }
    }
    for (end, sign) in [("east", 1.0), ("west", -1.0)] {
        let x = sign * (CHEST[0] / 2.0 + OVERSHOOT_MM);
        let mut sk = sketch_on(&format!("Portal, {end} end"), plane([x, 0.0, 0.0], [0.0, -sign, 0.0], [0.0, 0.0, 1.0]));
        lancet(&mut sk, 0.0, PORTAL_W_MM, PORTAL_SILL_MM, PORTAL_APEX_MM, PORTAL_SHARE);
        let tool = ids.next();
        doc.append(feature(tool, &format!("Sink the {end} end's pointed portal"), extrude(sk, OVERSHOOT_MM + NICHE_MM), cutter(seat())))?;
        let x = sign * (CHEST[0] / 2.0 - 0.05);
        let mut sk = sketch_on(&format!("Archivolt, {end} end"), plane([x, 0.0, 0.0], [0.0, sign, 0.0], [0.0, 0.0, 1.0]));
        polygon(&mut sk, &arch_frame(PORTAL_W_MM, PORTAL_SILL_MM, PORTAL_APEX_MM, PORTAL_SHARE, MIN_SECTION_MM, 12));
        let frame = ids.next();
        doc.append(feature(frame, &format!("Raise the {end} portal's archivolt"), extrude(sk, 0.05 + ARCHIVOLT_MM), joined(ComponentRole::Head, seat(), 0.0)))?;
    }
    at(&doc, &mut d);
    let ch = part_at(&d, lib, chest)?;
    let mut stones = Vec::new();
    for (name, sign) in [("north arcade", 1.0), ("south arcade", -1.0)] {
        let p = [0.0, sign * CHEST[1] / 2.0, BAY_STONE_Z_MM];
        let face = face_near(&d, &fr, &ch, [0.0, sign, 0.0], p)?;
        let (s, _) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &ch, face, p, garnet(), 0.0, NICHE_MM, &format!("Garnet, {name}"), true)?;
        stones.push(s);
    }
    shoulders(&mut doc, &mut ids, &d)?;

    // The lid: a steep gabled prism, eaves chamfered, hollowed to a skin and its slopes opened in tracery.
    let base = CHEST[2] / 2.0 + LID_CLEAR_MM;
    let ew = CHEST[1] / 2.0 + EAVES_OVER_MM;
    let (eh, ridge) = (base + EAVES_H_MM, base + EAVES_H_MM + RISE_MM);
    let len = CHEST[0] + 2.0 * ENDS_OVER_MM;
    let mut sk = sketch_on("Lid section, a gable", plane([-len / 2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
    polygon(&mut sk, &[[-ew, base], [ew, base], [ew, eh], [0.0, ridge], [-ew, eh]]);
    let prism = ids.next();
    doc.append(feature(prism, "Lid block: the gable drawn at the west end, run past both ends", extrude(sk, len), placed(seat())))?;
    at(&doc, &mut d);
    let lp = part_at(&d, lib, prism)?;
    let under = face_near(&d, &fr, &lp, [0.0, 0.0, -1.0], [0.0, 0.0, base])?;
    let eaves = edges_where(&d, &fr, &lp, &|m| (m[2] - base).abs() < 1e-3 && m[0].abs() < 1.0)?;
    ensure!(eaves.len() == 2, "lid has {} eave edges", eaves.len());
    let lid_body = ids.next();
    doc.append(feature(lid_body, "Chamfer the eaves", Operation::Chamfer { source: prism, edges: eaves, base_face: face_ref(&lp, under), distance_mm: EAVES_CHAMFER_MM }, placed(Placement::Free)))?;
    at(&doc, &mut d);
    let lb = part_at(&d, lib, lid_body)?;
    let slope = RISE_MM.atan2(ew);
    let (c, sn) = (slope.cos(), slope.sin());
    let mut lid_bezels = Vec::new();
    // The slope garnets stand near the ends, clear of the lights over the skull.
    for (name, sign, xg) in [("north slope", 1.0, 4.9), ("south slope", -1.0, -4.9)] {
        let p = [xg, sign * ew * 0.45, eh + RISE_MM * 0.55];
        let face = face_near(&d, &fr, &lb, [0.0, sign * sn, c], p)?;
        let (s, b) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &lb, face, p, garnet(), 0.0, 0.0, &format!("Garnet, {name}"), false)?;
        stones.push(s);
        lid_bezels.push((b, name));
    }
    let mut gable_bezels = Vec::new();
    for (name, sign) in [("east gable", 1.0), ("west gable", -1.0)] {
        let p = [sign * len / 2.0, 0.0, base + WINDOW_STONE_MM];
        let face = face_near(&d, &fr, &lb, [sign, 0.0, 0.0], p)?;
        let (s, b) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &lb, face, p, sapphire(), 90.0, WINDOW_MM, &format!("Sapphire, {name} window"), false)?;
        stones.push(s);
        gable_bezels.push((b, name));
    }
    let mut lid = lid_body;
    for (b, name) in lid_bezels {
        let u = ids.next();
        doc.append(feature(u, &format!("Join the {name} bezel to the lid"), boolean(lid, b, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
    // Hollow the lid: the gable again, its slopes and ends moved in by the skin, open underneath.
    let skin = ROOF_SKIN_MM;
    let inner_ridge = ridge - skin / c;
    let inner_ew = ew - skin / sn * 0.0 - skin;
    let inner_eh = inner_ridge - RISE_MM / ew * inner_ew;
    let mut sk = sketch_on("Lid hollow, a gable", plane([-len / 2.0 + skin, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
    polygon(&mut sk, &[[-inner_ew, base - 0.5], [inner_ew, base - 0.5], [inner_ew, inner_eh.max(base + 0.1)], [0.0, inner_ridge], [-inner_ew, inner_eh.max(base + 0.1)]]);
    let hollow_tool = ids.next();
    doc.append(feature(hollow_tool, "The lid's hollow: the gable moved in by the skin", extrude(sk, len - 2.0 * skin), placed(seat())))?;
    let u = ids.next();
    doc.append(feature(u, "Hollow the lid to a skin, open over the chest", boolean(lid, hollow_tool, Boolean::Subtract), placed(Placement::Free)))?;
    lid = u;
    // Tracery in each slope over the skull: two-light pointed windows with roundels, drawn on the slope and cut through it.
    let slope_len = (ew * ew + RISE_MM * RISE_MM).sqrt();
    let mut lights = 0;
    for (name, sign) in [("north", 1.0f64), ("south", -1.0)] {
        // The slope's plane: u along the finger, v up the slope toward the ridge, normal into the roof.
        let foot = [0.0, sign * ew, eh];
        let up_slope = [0.0, -sign * ew / slope_len, RISE_MM / slope_len];
        let out = [0.0, sign * sn, c];
        let origin = add(foot, out, OVERSHOOT_MM);
        let mut sk = sketch_on(&format!("Tracery, {name} slope"), plane(origin, [sign, 0.0, 0.0], up_slope));
        let mut net = Vec::new();
        // Three lancets with a roundel over each, toward the ridge.
        for cx in [-2.7, 0.0, 2.7] {
            let before = sk.entities.len();
            lancet(&mut sk, cx, 2.3, 0.3, slope_len - 2.2, 0.8);
            let (c, rc) = ([cx, slope_len - 1.15], 0.85);
            let ci = sk.point(c);
            let rim = sk.point([c[0] + rc, c[1]]);
            sk.entity(Geometry::Circle { center: ci, rim });
            net.extend(sk.entities[before..].iter().map(|e| e.id));
        }
        lights += trace_net(&mut sk, &net)?;
        let tool = regions(&mut doc, &mut ids, &format!("Open the {name} slope in tracery"), sk, OVERSHOOT_MM + skin + 0.4, placed(seat()))?;
        let u = ids.next();
        doc.append(feature(u, &format!("Cut the {name} slope's tracery through"), boolean(lid, tool, Boolean::Subtract), placed(Placement::Free)))?;
        lid = u;
    }
    notes.push(format!("{lights} tracery lights in the two slopes"));
    for (end, sign) in [("east", 1.0), ("west", -1.0)] {
        let x = sign * (len / 2.0 + OVERSHOOT_MM);
        let mut sk = sketch_on(&format!("Lancet window, {end} gable"), plane([x, 0.0, 0.0], [0.0, -sign, 0.0], [0.0, 0.0, 1.0]));
        lancet(&mut sk, 0.0, WINDOW_W_MM, base + WINDOW_SILL_MM, base + WINDOW_APEX_MM, WINDOW_SHARE);
        let tool = ids.next();
        doc.append(feature(tool, &format!("Sink the {end} gable's lancet window"), extrude(sk, OVERSHOOT_MM + WINDOW_MM), placed(seat())))?;
        let u = ids.next();
        doc.append(feature(u, &format!("Cut the {end} lancet into the gable"), boolean(lid, tool, Boolean::Subtract), placed(Placement::Free)))?;
        lid = u;
    }
    for (b, name) in gable_bezels {
        let u = ids.next();
        doc.append(feature(u, &format!("Join the {name} window's bezel to the lid"), boolean(lid, b, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
    let crest = cresting_sdf(ridge);
    let cl = one_outline("cresting", &crest, [-6.0, ridge - 1.0], [6.0, ridge + 2.8], 0.02, 220)?;
    let mut sk = sketch_on("Cresting: a rail and five fleurs", plane([0.0, 0.45, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    polygon(&mut sk, &cl);
    let cresting = ids.next();
    doc.append(feature(cresting, "Cut the cresting: a rail and five fleurs", extrude(sk, 0.9), placed(seat())))?;
    let u = ids.next();
    doc.append(feature(u, "Crest the ridge", boolean(lid, cresting, Boolean::Union), placed(Placement::Free)))?;
    lid = u;
    for (end, sign) in [("east", 1.0), ("west", -1.0)] {
        let x0 = sign * (len / 2.0 - 0.45);
        let fin = knop(&mut doc, &mut ids, &seat(), &format!("Knop at the {end} end of the ridge"), [x0, 0.0, ridge + 0.35], 0.55, placed(Placement::Free))?;
        let u = ids.next();
        doc.append(feature(u, &format!("Set the {end} knop on the ridge"), boolean(lid, fin, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
    make_lid(&mut doc, lid, "Cast apart; hinge to the chest's north eave with a three-knuckle hinge on a 0.8 mm pin, soldered at the bench");
    doc.joints.push(cad::Joint { a: chest, b: lid, clearance_mm: LID_CLEAR_MM, method: "Three-knuckle hinge, 0.8 mm pin, soldered at the bench".into(), notes: "The lid lifts on the north eave; the chest's rim takes the pin's knuckles".into() });
    d.cad = Some(doc);
    Ok(Authored { design: d, lid, chest, stones, skull: skull_id, notes })
}

/// The gable's crockets and finial in plan, bound to the gable's triangle so they trace as one outline.
fn gable_crest_sdf(p: P2) -> f64 {
    let (r0, apex, hw) = (xr(), xp(), xhw());
    let tri = sdf::convex(p, &[[r0, hw], [r0, -hw], [apex, 0.0]]);
    // A band just outside the rakes and the eaves (lapping 0.1 onto the gable) that the crockets grow from.
    let mut d = (-tri - 0.1).max(tri - 0.8).max(r0 + 2.2 - p[0]);
    let l = (XR * XR + hw * hw).sqrt();
    let n_out = [hw / l, XR / l];
    for t in [0.42, 0.62, 0.82] {
        for s in [1.0, -1.0] {
            let on = [r0 + XR * t, s * hw * (1.0 - t)];
            let c = [on[0] + n_out[0] * 0.18, on[1] + s * n_out[1] * 0.18];
            d = sdf::smin(d, sdf::circle(p, c, 0.42), 0.1);
        }
    }
    // The finial: a fleur-de-lis rising off the apex (the fleur's x runs across, its y along the finger).
    sdf::smin(d, 1.3 * fleur([p[1] / 1.3, (p[0] - apex + 0.6) / 1.3], [0.0, 0.0]), 0.08)
}

/// The chasse on its back across the finger, its west front up for the face camera.
fn author_across(lib: &AlphaLibrary, openwork: bool) -> Result<Authored> {
    let mut d = band();
    let mut doc = Document::default();
    let mut ids = Ids(0);
    let mut notes = Vec::new();
    let band_id = ids.next();
    doc.append(feature(band_id, "Procedural shank", Operation::Band, Component::default()))?;
    let at = |doc: &Document, d: &mut RingDesign| d.cad = Some(doc.clone());
    let fr = xseat();
    let (xa, xb, xe, xr, xp, hw) = (XA, xb(), xe(), xr(), xp(), xhw());

    // The bed the shrine lies on, a margin round its plan, sunk into the crest; and the plinth the front stands on.
    let m = XBED_MARGIN;
    let bw = XW / 2.0 + 1.6;
    let bed: Vec<P2> = vec![[xa - XPLINTH_RUN - m, -bw], [3.4, -bw], [xp + 0.4, 0.0], [3.4, bw], [xa - XPLINTH_RUN - m, bw]];
    moulded_prism(&mut doc, &mut ids, &mut d, lib, "Bed", &bed, -XBED_MM, 0.0, 0.3, joined(ComponentRole::Shank, Placement::Free, 0.0))?;
    let step: Vec<P2> = vec![[xa - XPLINTH_RUN, -XTW_OUT], [xa + 0.7, -XTW_OUT], [xa + 0.7, XTW_OUT], [xa - XPLINTH_RUN, XTW_OUT]];
    let plinth = moulded_prism(&mut doc, &mut ids, &mut d, lib, "Plinth", &step, -0.1, XD - 1.2, 0.3, placed(Placement::Free))?;

    // The chest: a block, its cavity cut from the roof's end.
    let mut sk = plan("Chest plan", -0.1);
    polygon(&mut sk, &[[xa, -XW / 2.0], [xb, -XW / 2.0], [xb, XW / 2.0], [xa, XW / 2.0]]);
    let block = ids.next();
    doc.append(feature(block, "Chest block", extrude(sk, XD + 0.1), placed(xseat())))?;
    // Opened, the chest's foot wall thins to the section floor so the skull lies under the window.
    let foot = if openwork { 0.9 } else { XT };
    let mut sk = plan("Chest cavity plan", XT);
    polygon(&mut sk, &[[xa + foot, -(XW / 2.0 - XT)], [xb + 0.5, -(XW / 2.0 - XT)], [xb + 0.5, XW / 2.0 - XT], [xa + foot, XW / 2.0 - XT]]);
    let cavity = ids.next();
    doc.append(feature(cavity, "The chest's cavity, open to the roof", extrude(sk, XD - 2.0 * XT), placed(xseat())))?;
    // The lid line: the front's edge at the roof chamfered (on the convex block, before the cavity), to meet the roof's own chamfered edge in a V.
    at(&doc, &mut d);
    let bc = part_at(&d, lib, block)?;
    let front = face_near(&d, &fr, &bc, [0.0, 0.0, 1.0], [0.5 * (xa + xb), 0.0, XD])?;
    let edge = edges_where(&d, &fr, &bc, &|m| (m[2] - XD).abs() < 1e-3 && (m[0] - xb).abs() < 1e-3)?;
    ensure!(edge.len() == 1, "the chest's front has {} edges at the roof", edge.len());
    let lined = ids.next();
    doc.append(feature(lined, "Chamfer the front's edge at the lid line", Operation::Chamfer { source: block, edges: edge, base_face: face_ref(&bc, front), distance_mm: 0.35 }, placed(Placement::Free)))?;
    let mut chest = ids.next();
    doc.append(feature(chest, "Hollow the chest", boolean(lined, cavity, Boolean::Subtract), placed(Placement::Free)))?;

    // The skull on the chest's back wall, its face up to the front, its crown toward the roof.
    let (skull_x, skull_s, skull_h) = if openwork { (xa + XSKULL_X, XSKULL_S, 2.0) } else { (0.5 * (xa + XT + xb) + 0.08, 0.82, 1.5) };
    let skull_id = skull(&mut doc, &mut ids, &SkullSeat { frame: xseat(), base: [skull_x, 0.0, XT - 0.2], a: [0.0, -1.0, 0.0], b: [1.0, 0.0, 0.0], n: [0.0, 0.0, 1.0], scale: skull_s, height: skull_h })?;

    // Towers flanking the front: a shaft webbed to the chest's top, and a crocketed spire lofted off its top past the eaves.
    let (spire0, spire1) = (xr + XSPIRE_FROM, xr + XSPIRE_TO);
    let (zt0, zt1) = (XD - XTW_DEPTH, XD + XTW_PROUD);
    let ym = 0.5 * (XTW_IN + XTW_OUT);
    let zs = zt1 - 0.5 * (XTW_OUT - XTW_IN) - 0.1;
    let mut towers = Vec::new();
    for (side, sign) in [("north", 1.0f64), ("south", -1.0)] {
        let span = |a: f64, b: f64| if sign > 0.0 { (a, b) } else { (-b, -a) };
        let (y0, y1) = span(XTW_IN, XTW_OUT);
        let shaft = moulded_prism(&mut doc, &mut ids, &mut d, lib, &format!("{} tower", if sign > 0.0 { "North" } else { "South" }), &rect_pts(xa - 0.15, spire0, y0, y1), zt0, zt1, 0.2, joined(ComponentRole::Head, Placement::Free, 0.0))?;
        towers.push((shaft, sign, side));
        let (w0, w1) = span(XW / 2.0 - 0.2, XTW_IN + 0.1);
        let mut sk = plan(&format!("Web, {side} tower"), zt0);
        polygon(&mut sk, &rect_pts(xa - 0.15, xb, w0, w1));
        let id = ids.next();
        doc.append(feature(id, &format!("Web the {side} tower to the chest"), extrude(sk, zt1 - 0.25 - zt0), joined(ComponentRole::Head, xseat(), 0.0)))?;
        // The spire: a rectangle off the shaft's top lofted to a point.
        let base = vec![[sign * (XTW_IN + 0.06), zs - 0.65], [sign * (XTW_OUT - 0.06), zs - 0.65], [sign * (XTW_OUT - 0.06), zt1 - 0.06], [sign * (XTW_IN + 0.06), zt1 - 0.06]];
        let base: Vec<P2> = if sign > 0.0 { base } else { base.into_iter().rev().collect() };
        let tip = vec![[sign * ym - 0.45, zs - 0.45], [sign * ym + 0.45, zs - 0.45], [sign * ym + 0.45, zs + 0.45], [sign * ym - 0.45, zs + 0.45]];
        let tip: Vec<P2> = if sign > 0.0 { tip } else { tip.into_iter().rev().collect() };
        let mut s0 = sketch_on(&format!("Spire foot, {side}"), plane([spire0 - 0.05, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
        polygon(&mut s0, &base);
        let mut s1 = sketch_on(&format!("Spire tip, {side}"), plane([spire1, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
        polygon(&mut s1, &tip);
        let id = ids.next();
        doc.append(feature(id, &format!("Loft the {side} spire to its point"), Operation::Loft { sections: vec![Profile::Inline(s0), Profile::Inline(s1)] }, joined(ComponentRole::Head, xseat(), 0.0)))?;
        knop(&mut doc, &mut ids, &fr, &format!("Knop on the {side} spire"), [spire1 + 0.2, sign * ym, zs], 0.7, joined(ComponentRole::Head, Placement::Free, 0.0))?;
    }

    // The front: a pointed portal sunk into the chest's end wall, its jambs splayed, flanked by blind lancets, or (opened) a traceried window over the skull.
    let (pw, psill, papex, pshare) = if openwork { (4.6, xa + 0.3, xb - 1.2, 0.75) } else { (2.8, xa + 0.55, xb - 0.95, 0.85) };
    if openwork {
        // One great pointed light, traced inside its arch at the section floor, the skull filling it.
        let mut sk = plan_arch("West window", XD + OVERSHOOT_MM, false);
        let before = sk.entities.len();
        lancet(&mut sk, 0.0, pw, psill, papex, pshare);
        let net: Vec<Id> = sk.entities[before..].iter().map(|e| e.id).collect();
        let n = trace_net(&mut sk, &net)?;
        notes.push(format!("{n} tracery lights in the west window"));
        regions(&mut doc, &mut ids, "Open the west window in tracery over the skull", sk, OVERSHOOT_MM + XT + 0.3, cutter(xseat()))?;
        // Beyond the band, open the back wall and the bed under the window round the skull's jaw, so the relic stands against the dark.
        let light: Vec<P2> = lancet_poly(0.0, pw - BAR_MM, xa + 1.3, papex - 0.4, pshare, 12).into_iter().map(|q| [q[1], q[0]]).collect();
        let cut_x = -XBACK_FROM;
        let skull_hole = |q: P2| -> f64 { skull_sdf([-q[1] / XSKULL_S, (q[0] - xa - XSKULL_X) / XSKULL_S]) * XSKULL_S - 0.25 };
        let light_sdf = |q: P2| -> f64 { sdf::convex(q, &[[cut_x, -10.0], [cut_x, 10.0], [-20.0, 10.0], [-20.0, -10.0]]) };
        let poly = light.clone();
        let inside = move |q: P2| -> f64 {
            // The light's polygon as a distance: negative inside (winding test, distance to the nearest edge).
            let n = poly.len();
            let mut dmin = f64::MAX;
            let mut wind = false;
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                let (ex, ey) = (b[0] - a[0], b[1] - a[1]);
                let t = (((q[0] - a[0]) * ex + (q[1] - a[1]) * ey) / (ex * ex + ey * ey)).clamp(0.0, 1.0);
                dmin = dmin.min((q[0] - a[0] - t * ex).hypot(q[1] - a[1] - t * ey));
                if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * ex {
                    wind = !wind;
                }
            }
            if wind { -dmin } else { dmin }
        };
        let back = move |q: P2| -> f64 { inside(q).max(light_sdf(q)).max(-skull_hole(q)) };
        let loops = outlines(&back, [xa - 0.5, -3.0], [cut_x + 0.5, 3.0], 0.02, 0.006, 200);
        ensure!(!loops.is_empty(), "no back lights");
        for (k, l) in loops.iter().enumerate() {
            let mut sk = plan(&format!("Back light {}", k + 1), -XBED_MM - 0.3);
            polygon(&mut sk, l);
            let tool = ids.next();
            doc.append(feature(tool, &format!("Open back light {} behind the skull to the dark", k + 1), extrude(sk, XBED_MM + 0.3 + XT + 0.05), cutter(xseat())))?;
        }
        notes.push(format!("{} back lights behind the skull", loops.len()));
    } else {
        let mut sk = plan_arch("Portal", XD, false);
        polygon(&mut sk, &lancet_poly(0.0, pw, psill, papex, pshare, 10));
        let tool = ids.next();
        doc.append(feature(tool, "Sink the pointed portal into the front, its jambs splayed", drafted(sk, NICHE_MM + 0.1, XSPLAY_DEG), cutter(xseat())))?;
        for (side, cy) in [("north", 2.85), ("south", -2.85)] {
            let mut sk = plan_arch(&format!("Blind lancet, {side} of the portal"), XD, false);
            polygon(&mut sk, &lancet_poly(cy, 1.1, psill + 0.2, papex - 0.2, 0.9, 8));
            let tool = ids.next();
            doc.append(feature(tool, &format!("Sink the blind lancet {side} of the portal"), drafted(sk, 0.4, 20.0), cutter(xseat())))?;
        }
    }
    let mut sk = plan_arch("Archivolt", XD - 0.05, true);
    polygon(&mut sk, &arch_frame(pw, psill, papex, pshare, 1.05, 14));
    let frame_id = ids.next();
    doc.append(feature(frame_id, "Raise the front's archivolt, its sides weathered", drafted(sk, 0.05 + 0.4, 15.0), joined(ComponentRole::Head, xseat(), 0.0)))?;

    at(&doc, &mut d);
    let ch = part_at(&d, lib, lined)?;
    let mut stones = Vec::new();
    // The chest's long walls under the towers: a pointed bay sunk in each, its head toward the roof, a garnet in it.
    let (bay_sill, bay_apex, bay_z) = (xa + 0.8, xb - 1.0, 0.5 * (XD - XTW_DEPTH));
    for (_, sign, side) in &towers {
        let sign = *sign;
        let p = [0.5 * (bay_sill + bay_apex) - 0.3, sign * XW / 2.0, bay_z];
        let face = face_near(&d, &fr, &ch, [0.0, sign, 0.0], p)?;
        let (s, _) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &ch, face, p, garnet(), 90.0, NICHE_MM, &format!("Garnet, {side} wall"), true)?;
        stones.push(s);
        // u runs along -sign z so the normal (u × x) points into the wall; one cut, its jambs splayed like two orders.
        let mut sk = sketch_on(&format!("Bay, {side} wall"), plane([0.0, sign * (XW / 2.0 + OVERSHOOT_MM), 0.0], [0.0, 0.0, -sign], [1.0, 0.0, 0.0]));
        polygon(&mut sk, &lancet_poly(-sign * bay_z, 2.6, bay_sill, bay_apex, 0.72, 10));
        let tool = ids.next();
        doc.append(feature(tool, &format!("The {side} wall's bay"), drafted(sk, OVERSHOOT_MM + NICHE_MM, 24.0), placed(xseat())))?;
        let next = ids.next();
        doc.append(feature(next, &format!("Sink the {side} wall's bay"), boolean(chest, tool, Boolean::Subtract), placed(Placement::Free)))?;
        chest = next;
    }
    if let Some(f) = doc.features.iter_mut().find(|f| f.id == chest) {
        f.name = format!("{}: the chest", f.name);
        f.component = joined(ComponentRole::Head, Placement::Free, 0.0);
    }
    // Opened, a jewelled band of three stones in pointed niches across the plinth's front.
    let niches: Vec<(&str, f64, Gem)> = if openwork { vec![("north", 3.4, garnet()), ("middle", 0.0, sapphire()), ("south", -3.4, garnet())] } else { vec![] };
    let pl = part_at(&d, lib, plinth)?;
    let mut plinth_body = plinth;
    // On the plinth's foot (x = xa - run), u across, v out of the ring; normal +x into the plinth; one splayed cut for every niche.
    let mut niche_sk = sketch_on("Niches in the plinth's front", plane([xa - XPLINTH_RUN - OVERSHOOT_MM, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]));
    for (name, cy, gem) in &niches {
        let p = [xa - XPLINTH_RUN, *cy, 1.6];
        let face = face_near(&d, &fr, &pl, [-1.0, 0.0, 0.0], p)?;
        let (s, _) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &pl, face, p, *gem, 90.0, XBAY_MM, &format!("{}, {name} plinth niche", if gem.preview_tint == sapphire().preview_tint { "Sapphire" } else { "Garnet" }), true)?;
        stones.push(s);
        polygon(&mut niche_sk, &lancet_poly(*cy, 2.75, 0.3, XD - 2.1, 0.75, 10));
    }
    if !niches.is_empty() {
        let drawn = ids.next();
        doc.append(feature(drawn, "Draw the niches in the plinth's front", Operation::Sketch { sketch: niche_sk }, Component::default()))?;
        let tool = ids.next();
        doc.append(feature(tool, "The niches in the plinth's front, their jambs splayed", Operation::Extrude { sketch: Profile::Feature { feature: drawn }, height_mm: OVERSHOOT_MM + XBAY_MM, draft_deg: 22.0 }, placed(xseat())))?;
        let next = ids.next();
        doc.append(feature(next, "Sink the niches into the plinth's front", boolean(plinth_body, tool, Boolean::Subtract), placed(Placement::Free)))?;
        plinth_body = next;
    }
    if let Some(f) = doc.features.iter_mut().find(|f| f.id == plinth_body) {
        f.name = format!("{}: the plinth", f.name);
        f.component = joined(ComponentRole::Shank, Placement::Free, 0.0);
    }
    if !openwork {
        let p = [papex - 1.55, 0.0, XD];
        let face = face_near(&d, &fr, &ch, [0.0, 0.0, 1.0], p)?;
        let (s, _) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &ch, face, p, sapphire(), 0.0, NICHE_MM, "Sapphire, portal", true)?;
        stones.push(s);
    }
    // The shoulders stay plain: read test 6 found their marks read as dents at 300 px.

    // The lid: the roof, its gable proud of the front, its rakes the slopes.
    // The apex blunted to the section floor, under the finial.
    let roof: Vec<P2> = vec![[xe, -hw], [xr, -hw], [xp - 0.65, -0.45], [xp - 0.65, 0.45], [xr, hw], [xe, hw]];
    let mut sk = plan("Roof plan", LID_CLEAR_MM);
    polygon(&mut sk, &roof);
    let prism = ids.next();
    let ztop = XD + XPROUD;
    doc.append(feature(prism, "Lid block: the roof, its gable up", extrude(sk, ztop - LID_CLEAR_MM), placed(xseat())))?;
    at(&doc, &mut d);
    let lp = part_at(&d, lib, prism)?;
    let top = face_near(&d, &fr, &lp, [0.0, 0.0, 1.0], [xr + 1.0, 0.0, ztop])?;
    let rim = edges_where(&d, &fr, &lp, &|m| (m[2] - ztop).abs() < 1e-3 && (m[0] - xe).abs() < 1e-3)?;
    ensure!(rim.len() == 1, "the roof's gable has {} top edges at the lid line", rim.len());
    let lid_body = ids.next();
    doc.append(feature(lid_body, "Chamfer the gable's edge at the lid line", Operation::Chamfer { source: prism, edges: rim, base_face: face_ref(&lp, top), distance_mm: 0.35 }, placed(Placement::Free)))?;
    at(&doc, &mut d);
    let lb = part_at(&d, lib, lid_body)?;
    let l = (XR * XR + hw * hw).sqrt();
    // The rose in the gable: a sapphire at its eye, at the gable's in-centre.
    let inr = hw * XR / (hw + (XR * XR + hw * hw).sqrt());
    let rose = [xr + inr, 0.0];
    let p = [rose[0], 0.0, ztop];
    let face = face_near(&d, &fr, &lb, [0.0, 0.0, 1.0], p)?;
    let (s, rose_bezel) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &lb, face, p, sapphire(), 0.0, 0.0, "Sapphire, the rose's eye", false)?;
    stones.push(s);
    // Closed, a garnet on each slope of the roof, sunk in a pointed bay.
    let mut slope_bezels = Vec::new();
    if !openwork {
        for (name, sign) in [("north", 1.0f64), ("south", -1.0)] {
            let n = [hw / l, sign * XR / l, 0.0];
            let p = [xr + XR * 0.36, sign * hw * 0.64, 0.45 * XD];
            let face = face_near(&d, &fr, &lb, n, p)?;
            let (s, b) = cabochon(&mut doc, &mut ids, &mut d, lib, &fr, &lb, face, p, garnet(), 90.0, XBAY_MM, &format!("Garnet, {name} slope"), false)?;
            stones.push(s);
            slope_bezels.push((b, name));
        }
    }
    let mut lid = lid_body;
    for (name, sign) in [("north", 1.0f64), ("south", -1.0)].into_iter().filter(|_| !openwork) {
        let e = [xr, sign * hw, 0.0];
        let t = [XR / l, -sign * hw / l, 0.0];
        let out = [hw / l, sign * XR / l, 0.0];
        // u along the rake (reversed on the south so the normal u × z points in), v out of the ring.
        let u = if sign > 0.0 { t } else { neg(t) };
        let mut sk = sketch_on(&format!("Bay, {name} slope"), plane(add(e, out, OVERSHOOT_MM), u, [0.0, 0.0, 1.0]));
        polygon(&mut sk, &lancet_poly(sign * 0.36 * l, 2.4, 0.4, XD - 0.6, 0.75, 10));
        let tool = ids.next();
        doc.append(feature(tool, &format!("The {name} slope's bay"), extrude(sk, OVERSHOOT_MM + XBAY_MM), placed(xseat())))?;
        let cut = ids.next();
        doc.append(feature(cut, &format!("Sink the {name} slope's bay"), boolean(lid, tool, Boolean::Subtract), placed(Placement::Free)))?;
        lid = cut;
    }
    for (b, name) in slope_bezels {
        let u = ids.next();
        doc.append(feature(u, &format!("Join the {name} slope's bezel to the lid"), boolean(lid, b, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
    if openwork {
        // Hollow the roof to a skin, open toward the chest and underneath, and pierce each slope with roundels.
        let skin = ROOF_SKIN_MM;
        let hw_in = hw - skin * l / XR;
        let apex_in = xp - skin * l / hw - 0.4;
        let inner: Vec<P2> = vec![[xe - 0.5, -hw_in], [xr, -hw_in], [apex_in, 0.0], [xr, hw_in], [xe - 0.5, hw_in]];
        let mut sk = plan("Roof hollow plan", -0.5);
        polygon(&mut sk, &inner);
        let tool = ids.next();
        doc.append(feature(tool, "The roof's hollow, open toward the chest", extrude(sk, 0.5 + ztop - XGABLE_PLATE), placed(xseat())))?;
        let u = ids.next();
        doc.append(feature(u, "Hollow the roof to a skin", boolean(lid, tool, Boolean::Subtract), placed(Placement::Free)))?;
        lid = u;
        let mut lights = 0;
        for (name, sign) in [("north", 1.0f64), ("south", -1.0)] {
            let e = [xr, sign * hw, 0.0];
            let t = [XR / l, -sign * hw / l, 0.0];
            let out = [hw / l, sign * XR / l, 0.0];
            let origin = add(e, out, OVERSHOOT_MM);
            // u along the rake from the eave corner, v out of the ring (mirrored on the south so the normal points in).
            let vdir = [0.0, 0.0, sign];
            let mut sk = sketch_on(&format!("Tracery, {name} slope"), plane(origin, t, vdir));
            let mut net = Vec::new();
            for (uc, r) in [(1.75, 1.15), (4.0, 0.95)] {
                let vc = sign * 1.9;
                let c = sk.point([uc, vc]);
                let rim = sk.point([uc + r, vc]);
                net.push(sk.entity(Geometry::Circle { center: c, rim }));
            }
            lights += trace_net(&mut sk, &net)?;
            let tool = regions(&mut doc, &mut ids, &format!("Open the {name} slope in roundels"), sk, OVERSHOOT_MM + skin + 0.6, placed(xseat()))?;
            let u = ids.next();
            doc.append(feature(u, &format!("Cut the {name} slope's roundels through"), boolean(lid, tool, Boolean::Subtract), placed(Placement::Free)))?;
            lid = u;
        }
        notes.push(format!("{lights} tracery lights in the roof's slopes"));
    }
    // The coping round the gable (the eave moulding along its foot), the crockets and the finial: an outline and its hole, in one sketch.
    let crest = outlines(&gable_crest_sdf, [xr - 1.0, -hw - 1.0], [xp + 3.2, hw + 1.0], 0.02, 0.006, 150);
    ensure!(!crest.is_empty() && crest.len() <= 2, "the gable's crest traced to {} loops", crest.len());
    let mut sk = plan("Coping, crockets and finial", ztop - 1.2);
    for l in &crest {
        polygon(&mut sk, l);
    }
    let crockets = ids.next();
    doc.append(feature(crockets, "Cut the crockets and the fleur finial round the gable, a moulding proud of it", extrude(sk, 1.2 + 0.2), placed(xseat())))?;
    let u = ids.next();
    doc.append(feature(u, "Crest the gable", boolean(lid, crockets, Boolean::Union), placed(Placement::Free)))?;
    lid = u;
    // The rose's collet last: each kernel operand stays under the analytic kernel's face limit.
    let u = ids.next();
    doc.append(feature(u, "Join the rose's collet to the lid", boolean(lid, rose_bezel, Boolean::Union), placed(Placement::Free)))?;
    lid = u;
    make_lid(&mut doc, lid, "Cast apart; hinge to the chest's open end with a three-knuckle hinge on a 0.8 mm pin, soldered at the bench");
    doc.joints.push(cad::Joint { a: chest, b: lid, clearance_mm: LID_CLEAR_MM, method: "Three-knuckle hinge, 0.8 mm pin, soldered at the bench".into(), notes: "The roof lifts off the chest's open end; the chest's rim takes the pin's knuckles".into() });
    d.cad = Some(doc);
    Ok(Authored { design: d, lid, chest, stones, skull: skull_id, notes })
}

// ---------------------------------------------------------------- output

/// Each named view: the ring turned about the head's axis, then the camera's yaw about the finger's axis and its pitch toward the head.
fn views(opt: Opt) -> [(&'static str, f64, f64, f64); 6] {
    if let Some(h) = std::env::var("CAPSA_HERO").ok().map(|v| v.split(',').filter_map(|x| x.parse::<f64>().ok()).collect::<Vec<_>>()).filter(|h| h.len() == 3) {
        return [("hero", h[0], h[1], h[2]), ("face", 0.0, 0.0, PI * 0.5), ("palm", 0.0, PI, 1.05), ("side", 0.0, 0.0, 0.0), ("shoulder", 0.0, 1.0, 0.45), ("reverse", 0.0, PI - 0.5, 0.35)];
    }
    if opt.across() {
        [("hero", 0.0, 0.3, 1.0), ("face", 0.0, 0.0, PI * 0.5), ("palm", 0.0, PI, 1.05), ("side", 0.0, 0.0, 0.0), ("shoulder", 0.0, 1.0, 0.45), ("reverse", 0.0, PI - 0.5, 0.35)]
    } else {
        [("hero", 0.6, 0.0, 0.5), ("face", 0.0, 0.0, PI * 0.5), ("palm", 0.0, PI, 1.05), ("side", 0.0, 0.0, 0.0), ("shoulder", 0.0, 0.75, 0.6), ("reverse", 0.0, PI - 0.5, 0.35)]
    }
}

/// `m` turned `t` radians about the head's axis (world y, the top of the ring).
fn turned(m: &mesh::Mesh, t: f64) -> mesh::Mesh {
    let (s, c) = (t.sin() as f32, t.cos() as f32);
    let r = |v: mesh::Vec3| mesh::Vec3(v.0 * c + v.2 * s, v.1, -v.0 * s + v.2 * c);
    let mut out = m.clone();
    out.vertices = m.vertices.iter().map(|v| r(*v)).collect();
    out.normals = m.normals.iter().map(|v| r(*v)).collect();
    out.corner_normals = m.corner_normals.iter().map(|(f, n)| (*f, n.map(r))).collect();
    out
}

/// One view of the metal and stones to a PNG.
fn view(path: &Path, metal: &mesh::Mesh, gems: &[(mesh::Mesh, [f32; 3])], turn: f64, yaw: f64, pitch: f64, edge: usize) -> Result<()> {
    let metal = turned(metal, turn);
    let stones: Vec<(mesh::Mesh, [f32; 3])> = gems.iter().map(|(m, t)| (turned(m, turn), *t)).collect();
    let mut parts = vec![render::Part::metal(&metal, render::GOLD)];
    parts.extend(stones.iter().map(|(m, tint)| render::Part::tinted_stone(m, *tint)));
    render::write_png_parts(path, &parts, yaw, pitch, edge)?;
    Ok(())
}

fn renders(out: &Path, opt: Opt, metal: &mesh::Mesh, gems: &[(mesh::Mesh, [f32; 3])], edge: usize) -> Result<()> {
    let v = views(opt);
    for (name, turn, yaw, pitch) in v {
        view(&out.join(format!("{name}.png")), metal, gems, turn, yaw, pitch, edge)?;
    }
    let (_, t, y, p) = v[0];
    view(&out.join("hero-300.png"), metal, gems, t, y, p, 300)?;
    let (_, t, y, p) = v[1];
    view(&out.join("face-300.png"), metal, gems, t, y, p, 300)?;
    Ok(())
}

fn solid_of(m: &mesh::Mesh) -> ringdesign_core::csg::Solid {
    ringdesign_core::csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

/// Every vertex nearer the finger's axis than the bore allows: the nearest radius and how many.
fn bore_intrusion(d: &RingDesign, m: &mesh::Mesh) -> (f64, usize) {
    let bore = d.inner_radius_mm();
    m.vertices.iter().fold((f64::MAX, 0), |(least, inside), v| {
        let r = (v.0 as f64).hypot(v.1 as f64);
        (least.min(r), inside + usize::from(r < bore - 0.01))
    })
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, ringdesign_core::csg::self_crossings(&solid_of(m)))
}

/// Every gate of the brief at `params`: the numbers, and each gate's name and verdict.
struct Gates {
    json: serde_json::Value,
    passes: Vec<(String, bool)>,
}

fn gates(d: &RingDesign, lib: &AlphaLibrary, a_lid: Id, built: &mesh::BuildResult, params: BuildParams, previewed: usize, verify: bool, out: &Path, export: bool) -> Result<Gates> {
    use ringdesign_core::manufacturing as mf;
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    let e = built.parts.evaluated.as_ref().context("no evaluated parts")?;
    let failed: Vec<String> = e.features.iter().filter(|r| !matches!(r.status, cad::FeatureStatus::Ok)).map(|r| format!("#{} {}: {:?}", r.id, r.name, r.status)).collect();
    let parts: Vec<(String, bool, usize, usize)> = e
        .components
        .iter()
        .filter(|c| !c.settings.reference)
        .map(|c| {
            let (w, dg, x) = geometry(&c.mesh);
            (format!("#{} {}", c.id, c.name), w, dg, x)
        })
        .collect();
    let (least_r, inside) = bore_intrusion(d, &built.mesh);
    // The 0.8 mm section: sampled wall on a probe build of the whole finished metal, and on the lid alone.
    let probe = mesh::try_build(d, lib, BuildParams { theta_steps: 192, profile_steps: 96, ..BuildParams::default() })?;
    let wall = cad::measure::thickness(&probe.mesh, MIN_SECTION_MM);
    for f in &failed {
        println!("    feature: {f}");
    }
    let lid_mesh = e.components.iter().find(|c| c.id == a_lid).map(|c| c.mesh.clone()).unwrap_or_default();
    let lid_wall = cad::measure::thickness(&lid_mesh, MIN_SECTION_MM);
    if std::env::var("CAPSA_TRACE").is_ok() {
        let doc = d.cad.as_ref().context("no document")?;
        for f in doc.features.iter().filter(|f| matches!(f.operation, Operation::Boolean { .. })) {
            let c = part_at(d, lib, f.id)?;
            println!("  #{} {}: {} triangles, {} degenerate, {} crossings", f.id, f.name, c.mesh.faces.len(), c.mesh.quality().degenerate_faces, ringdesign_core::csg::self_crossings(&solid_of(&c.mesh)));
        }
    }
    if std::env::var("CAPSA_THIN").is_ok() {
        thin_points(d, &probe.mesh, MIN_SECTION_MM, "ring")?;
        thin_points(d, &lid_mesh, MIN_SECTION_MM, "lid")?;
    }
    let wall_ok = |t: &cad::measure::Thickness| t.rays > 0 && t.below_limit == 0 && t.unresolved == 0;
    let lands = ringdesign_core::dfm::cut_lands(d, built, MIN_SECTION_MM);
    let findings = ringdesign_core::dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, 0.0, built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let warnings: Vec<String> = stones.iter().flat_map(|r| r.seats.iter().flat_map(|s| s.warnings.iter().map(|w| format!("{}: {w}", s.label)))).collect();
    // The ring's casting pattern, and the lid's: each its own pour.
    let pattern = mesh::try_build_pattern(d, lib, params)?;
    let (pw, pd, px) = geometry(&pattern.mesh);
    let mut setup = mf::Setup::from_design(d);
    setup.component = Some(a_lid);
    setup.recipe.name = "Capsa / lid / investment / Gold 18k".into();
    setup.recipe.alloy = "Gold 18k".into();
    setup.recipe.sand = None;
    let lid_pattern = mf::prepare(d, lib, &setup, params)?;
    let (lw, ld, lx) = geometry(&lid_pattern.mesh);
    library::save_design_embedded(out.join("design.ring.json"), d, lib)?;
    let text = std::fs::read_to_string(out.join("design.ring.json"))?;
    let format = serde_json::from_str::<serde_json::Value>(&text)?.get("format_version").and_then(|v| v.as_u64()).unwrap_or(0);
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        Some(rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces && rebuilt.mesh.normals == built.mesh.normals)
    } else {
        None
    };
    if export {
        ringdesign_core::stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        ringdesign_core::stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Capsa / ring pattern")?;
        ringdesign_core::stl::write_stl(out.join("lid-pattern.stl"), &lid_pattern.mesh, "Capsa / lid pattern, cast apart")?;
    }
    let crowding: Vec<String> = stones.as_ref().map(|s| s.crowding.iter().map(|p| format!("{} to {}: {:.2} / {:.2} mm", p.a, p.b, p.gap_mm, p.gap_deep_mm)).collect()).unwrap_or_default();
    let passes: Vec<(String, bool)> = vec![
        ("1. finished mesh watertight, 0 degenerate faces, 0 self-crossings".into(), watertight && degenerate == 0 && crossings == 0),
        ("1. every made part watertight with 0 degenerate faces and 0 crossings".into(), parts.iter().all(|p| p.1 && p.2 == 0 && p.3 == 0)),
        ("1. solids and parts notes empty, every CAD feature Ok".into(), built.solids.notes.is_empty() && built.parts.notes.is_empty() && failed.is_empty()),
        ("2. nothing enters the finger hole".into(), inside == 0),
        ("3. thickness(0.8) clean on the finished metal".into(), wall_ok(&wall)),
        ("3. thickness(0.8) clean on the lid".into(), wall_ok(&lid_wall)),
        ("3. cut_lands clean at 0.8 mm".into(), lands.is_empty()),
        ("4. zero DFM findings".into(), findings.is_empty()),
        ("5. stones reported equal the preview".into(), reported == previewed),
        ("6. cold reload identical".into(), cold != Some(false)),
        ("7. ring pattern watertight, 0 degenerate, 0 crossings".into(), pw && pd == 0 && px == 0),
        ("7. lid pattern watertight, 0 degenerate, 0 crossings".into(), lw && ld == 0 && lx == 0),
        ("export within 2 million triangles".into(), built.mesh.faces.len() <= 2_000_000),
    ];
    let json = serde_json::json!({
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings, "triangles": built.mesh.faces.len(), "volume_mm3": built.report.volume_mm3},
        "parts": parts.iter().map(|p| serde_json::json!({"part": p.0, "watertight": p.1, "degenerate_faces": p.2, "self_crossings": p.3})).collect::<Vec<_>>(),
        "solids_notes": built.solids.notes, "parts_notes": built.parts.notes, "features_not_ok": failed,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "thickness": {"finished": wall, "lid": lid_wall, "probe_build": "192 x 96"},
        "cut_lands": lands.iter().map(|f| f.message.clone()).collect::<Vec<_>>(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed, "warnings": warnings, "crowding": crowding, "closest": stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm", p.a, p.b, p.gap_mm))},
        "pattern": {"ring": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()}, "lid": {"watertight": lw, "degenerate_faces": ld, "self_crossings": lx, "triangles": lid_pattern.mesh.faces.len(), "casting": format!("{:?}", lid_pattern.casting)}},
        "design": {"bytes": text.len(), "format_version": format, "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "cold_reload_identical": cold,
        "gates": passes.iter().map(|(g, p)| serde_json::json!({"gate": g, "pass": p})).collect::<Vec<_>>(),
        "gates_passed": passes.iter().all(|(_, p)| *p),
    });
    Ok(Gates { json, passes })
}

/// Where `thickness` finds metal under `limit`: every sampled point (shrine frame) and its wall, for diagnosis (CAPSA_THIN).
fn thin_points(d: &RingDesign, m: &mesh::Mesh, limit: f64, label: &str) -> Result<()> {
    let f = xseat().frame(d)?;
    let tri: Vec<_> = m.faces.iter().filter_map(|t| m.triangle(t)).collect();
    let stride = tri.len().div_ceil(384).max(1);
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    for (i, (a, b, c)) in tri.iter().enumerate().step_by(stride) {
        let n = cross(sub(*b, *a), sub(*c, *a));
        let l = dot(n, n).sqrt();
        if l < 1e-12 { continue; }
        let o: P3 = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let dir = n.map(|v| -v / l);
        let mut best = f64::MAX;
        for (j, (p, q, r)) in tri.iter().enumerate() {
            if j == i { continue; }
            let (e1, e2) = (sub(*q, *p), sub(*r, *p));
            let h = cross(dir, e2);
            let det = dot(e1, h);
            if det.abs() < 1e-12 { continue; }
            let s0 = sub(o, *p);
            let u = dot(s0, h) / det;
            if !(0.0..=1.0).contains(&u) { continue; }
            let qv = cross(s0, e1);
            let v = dot(dir, qv) / det;
            if v < 0.0 || u + v > 1.0 { continue; }
            let t = dot(e2, qv) / det;
            if t > 1e-5 && t < best { best = t; }
        }
        if best < limit {
            let q = sub(o, f.origin);
            println!("  thin {label}: {best:.3} mm at local ({:.2}, {:.2}, {:.2})", dot(q, f.x_axis), dot(q, f.y_axis), dot(q, f.z_axis));
        }
    }
    Ok(())
}

/// The close-ups and sheets the brief asks for beside the six views: the stones and the front framed close (whole parts, never a
/// cropped mesh), the bare band against the finished ring at the hero's angle, and a contact sheet of the 300 px views.
fn extra_renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, opt: Opt, metal: &mesh::Mesh, gems: &[(mesh::Mesh, [f32; 3])], edge: usize) -> Result<()> {
    let mut parts = vec![render::Part::metal(metal, render::GOLD)];
    parts.extend(gems.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    let centre = xseat().world(d, [0.0, 0.0, 0.5 * XD])?;
    render::write_png_framed(out.join("stones.png"), &parts, 0.0, PI * 0.5, render::Framing::new(centre, 8.5), edge)?;
    let v = views(opt);
    let (_, _, y, p) = v[0];
    render::write_png_framed(out.join("hero-close.png"), &parts, y, p, render::Framing::new(centre, 9.0), edge)?;
    let side = |m: &mesh::Mesh, g: &[(mesh::Mesh, [f32; 3])]| -> Vec<u8> {
        let mut ps = vec![render::Part::metal(m, render::GOLD)];
        ps.extend(g.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
        render::render_parts_ss(&ps, y, p, edge / 2, edge / 2, 3)
    };
    let bare = mesh::try_build(&band(), lib, draft_params())?;
    let (a, b) = (side(&bare.mesh, &[]), side(metal, gems));
    let half = edge / 2;
    let mut img = vec![18u8; edge * half * 3];
    for row in 0..half {
        img[row * edge * 3..row * edge * 3 + half * 3].copy_from_slice(&a[row * half * 3..(row + 1) * half * 3]);
        img[row * edge * 3 + half * 3..(row + 1) * edge * 3].copy_from_slice(&b[row * half * 3..(row + 1) * half * 3]);
    }
    image::save_buffer(out.join("bare-vs-finished.png"), &img, edge as u32, half as u32, image::ColorType::Rgb8)?;
    // The contact sheet: every named view at 300 px, three across.
    let tiles: Vec<Vec<u8>> = v.iter().map(|(_, t, y, p)| {
        let m = turned(metal, *t);
        let g: Vec<(mesh::Mesh, [f32; 3])> = gems.iter().map(|(gm, c)| (turned(gm, *t), *c)).collect();
        let mut ps = vec![render::Part::metal(&m, render::GOLD)];
        ps.extend(g.iter().map(|(m, c)| render::Part::tinted_stone(m, *c)));
        render::render_parts_ss(&ps, *y, *p, 300, 300, 3)
    }).collect();
    let (w, h) = (900usize, 600usize);
    let mut sheet = vec![18u8; w * h * 3];
    for (k, tile) in tiles.iter().enumerate() {
        let (ox, oy) = ((k % 3) * 300, (k / 3) * 300);
        for row in 0..300 {
            let dst = ((oy + row) * w + ox) * 3;
            sheet[dst..dst + 900].copy_from_slice(&tile[row * 900..(row + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let at = args.iter().position(|a| a == "--option");
    let opt = match at {
        Some(i) => Opt::parse(args.get(i + 1).map(String::as_str).unwrap_or("")).context("--option takes across, openwork or across-openwork")?,
        None => Opt::AcrossOpenwork,
    };
    let out = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && at.is_none_or(|k| *i != k + 1))
        .map(|(_, a)| PathBuf::from(a))
        .unwrap_or_else(|| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/capsa");
            if at.is_some() { root.join(opt.slug()) } else { root }
        });
    std::fs::create_dir_all(&out)?;
    let lib = AlphaLibrary::builtin();
    if std::env::var("CAPSA_COLLET").is_ok() {
        for wall in [0.8, 0.9, 1.0, 1.1] {
            for lip in [0.1, 0.2, 0.3, 0.45, 0.6] {
                let n = ringdesign_core::setting::collet_named(garnet(), wall, lip, -ringdesign_core::setting::collet_depth_mm(garnet()));
                let m = mesh::Mesh { vertices: n.solid.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), normals: vec![], faces: n.solid.f.clone(), ..Default::default() };
                let t = cad::measure::thickness(&m, MIN_SECTION_MM);
                println!("wall {wall} lip {lip}: min {:?} below {} of {}", t.sampled_min_mm, t.below_limit, t.rays);
            }
        }
        return Ok(());
    }
    println!("Capsa ({})", opt.slug());
    let t = std::time::Instant::now();
    let a = author(&lib, opt)?;
    let author_s = t.elapsed().as_secs_f64();
    for n in &a.notes {
        println!("  {n}");
    }
    let mut d = a.design;
    // A debugging switch: CAPSA_OFF names features to leave out, by any word of their names.
    if let Ok(off) = std::env::var("CAPSA_OFF") {
        if let Some(doc) = d.cad.as_mut() {
            for f in &mut doc.features {
                if off.split(',').any(|w| !w.is_empty() && f.name.contains(w)) {
                    f.enabled = false;
                }
            }
        }
    }
    let d = d;
    let mut runs = vec![(true, draft_params())];
    if !draft {
        runs.push((false, export_params()));
    }
    let mut blocks = serde_json::Map::new();
    let mut all = true;
    let mut last = None;
    for (is_draft, params) in runs {
        let t = std::time::Instant::now();
        let built = mesh::try_build(&d, &lib, params)?;
        let build_s = t.elapsed().as_secs_f64();
        let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
        let previewed = d.cad.as_ref().map_or(0, |doc| doc.features.iter().filter(|f| f.component.reference).count());
        let g = gates(&d, &lib, a.lid, &built, params, previewed, verify && !is_draft || verify && draft, &out, !is_draft)?;
        let top = built.mesh.vertices.iter().fold(f32::MIN, |m, v| m.max(v.1)) as f64;
        let crest = d.inner_radius_mm() + d.profile.thickness_mm + CREST_LIFT_MM;
        let mut j = g.json;
        j["build"] = serde_json::json!({"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "build_s": build_s, "author_s": author_s});
        j["head_over_crest_mm"] = serde_json::json!(top - crest);
        println!("  {} x {}: {} triangles in {build_s:.1} s; head {:.2} mm over the crest", params.theta_steps, params.profile_steps, built.mesh.faces.len(), top - crest);
        for (name, pass) in &g.passes {
            println!("    {} {name}", if *pass { "pass" } else { "FAIL" });
        }
        all &= g.passes.iter().all(|(_, p)| *p);
        blocks.insert(if is_draft { "draft".into() } else { "export".into() }, j);
        last = Some((built, gems));
    }
    let (built, gems) = last.context("no build")?;
    if !draft {
        let mut entries = Vec::new();
        for (m, tint) in &gems {
            let (file, name, ior) = if tint[0] > 0.1 { ("reference-garnet.stl", "Garnet", 1.79) } else { ("reference-sapphire.stl", "Sapphire", 1.77) };
            ringdesign_core::stl::write_stl(out.join(file), m, &format!("Capsa reference {name}"))?;
            entries.push(serde_json::json!({"mesh": file, "name": name, "tint": tint, "ior": ior, "cut": "oval cabochon 1.8 x 2.3"}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&serde_json::json!({"stones": entries}))?)?;
    }
    let report = serde_json::json!({
        "name": d.name,
        "option": opt.slug(),
        "process": d.draft.process.label(),
        "min_section_mm": d.draft.min_section_mm,
        "lid": a.lid,
        "chest": a.chest,
        "skull": a.skull,
        "stone_features": a.stones,
        "notes": a.notes,
        "views": views(opt).iter().map(|(n, t, y, p)| serde_json::json!({"view": n, "turn_about_head_rad": t, "yaw_rad": y, "pitch_rad": p})).collect::<Vec<_>>(),
        "draft": blocks.get("draft"),
        "export": blocks.get("export"),
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, opt, &built.mesh, &gems, if draft { 1000 } else { 1600 })?;
    extra_renders(&out, &d, &lib, opt, &built.mesh, &gems, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if all { "all pass" } else { "FAILED" });
    Ok(())
}
