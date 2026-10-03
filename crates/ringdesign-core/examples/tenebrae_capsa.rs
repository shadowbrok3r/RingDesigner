//! Tenebrae — Capsa, the reliquary: a Gothic chasse on a plinth along the finger, lost wax, two castings.
//! cargo build --release -p ringdesign-core --example tenebrae_capsa
//! target/release/examples/tenebrae_capsa [OUT_DIR] [--draft] [--verify]
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

const BORE_MM: f64 = 18.6;
/// How far the Cathedral swell lifts the crest over the nominal outer radius at the top, mm.
const CREST_LIFT_MM: f64 = 0.29;
/// The plinth's base course: along the finger, round the ring, tall; its top stands this far over the crest.
const PLINTH: P3 = [14.4, 8.4, 2.0];
const PLINTH_OVER_CREST_MM: f64 = 0.15;
const PLINTH_CHAMFER_MM: f64 = 0.35;
/// The plinth's upper step, sunk this far into the base course.
const STEP: P3 = [13.8, 7.8, 0.6];
const STEP_CHAMFER_MM: f64 = 0.25;
/// The chest: along the finger, round the ring, tall, sunk this far into the step.
const CHEST: P3 = [13.0, 6.8, 3.8];
const SINK_MM: f64 = 0.1;
const WALL_MM: f64 = 1.35;
const FLOOR_RAISE_MM: f64 = 0.3;
/// How deep the arcade's bays and the portals sink into the walls.
const NICHE_MM: f64 = 0.5;
const OVERSHOOT_MM: f64 = 0.3;
/// The arcade: three bays between posts, the end posts and the posts both this wide.
const POST_MM: f64 = 0.9;
/// A buttress is narrower than its post, so its sides never lie in a bay's jamb.
const BUTTRESS_W_MM: f64 = 0.8;
/// The bays' outer order: spans (the centre bay wider for its garnet), sill, apex, arc share; the inner order sinks deeper, inset.
const BAY_W_MM: [f64; 3] = [2.75, 3.9, 2.75];
const BAY_SHARE: [f64; 3] = [0.75, 0.66, 0.75];
const BAY_SILL_MM: f64 = -1.75;
const BAY_APEX_MM: f64 = 1.55;
const ORDER_MM: f64 = 0.2;
const ORDER_INSET_MM: f64 = 0.4;
/// The garnet's centre in the centre bay, chest frame z.
const BAY_STONE_Z_MM: f64 = -0.45;
/// The portals on the end walls.
const PORTAL_W_MM: f64 = 2.6;
const PORTAL_SILL_MM: f64 = -1.7;
const PORTAL_APEX_MM: f64 = 0.95;
const PORTAL_SHARE: f64 = 0.9;
/// The lancet window in each gable of the lid, round its sapphire: span, sill and apex over the lid's underside, share, depth.
const WINDOW_W_MM: f64 = 2.8;
const WINDOW_SILL_MM: f64 = 0.15;
const WINDOW_APEX_MM: f64 = 3.45;
const WINDOW_SHARE: f64 = 0.8;
const WINDOW_MM: f64 = 0.45;
const WINDOW_STONE_MM: f64 = 1.75;
const ARCHIVOLT_MM: f64 = 0.4;
/// Buttresses: how far they stand off the wall at the foot and after the weathering.
const BUTTRESS_FOOT_MM: f64 = 0.38;
const BUTTRESS_UPPER_MM: f64 = 0.22;
/// The lid: clearance over the chest, eaves overhang, eave height, roof rise, overhang at the ends.
const LID_CLEAR_MM: f64 = 0.05;
const EAVES_OVER_MM: f64 = 0.55;
const EAVES_H_MM: f64 = 0.45;
const RISE_MM: f64 = 3.9;
const ENDS_OVER_MM: f64 = 0.3;
const EAVES_CHAMFER_MM: f64 = 0.2;
const BEZEL_WALL_MM: f64 = 0.35;
const BEZEL_LIP: f64 = 0.3;
const MIN_SECTION_MM: f64 = 0.8;
/// The cresting's fleurs stand this far apart along the ridge.
const FLEUR_PITCH_MM: f64 = 2.3;
/// The shoulder quatrefoils: offsets from the top and their sizes.
const PIERCINGS: [(f64, f64); 3] = [(48.0, 1.6), (60.0, 1.3), (72.0, 1.0)];

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

/// The base course's top over the nominal outer radius.
fn plinth_top() -> f64 {
    CREST_LIFT_MM + PLINTH_OVER_CREST_MM
}
/// Height over the nominal outer radius the chest's centre is seated at.
fn chest_h() -> f64 {
    plinth_top() - SINK_MM + STEP[2] - SINK_MM + CHEST[2] / 2.0
}
fn seat() -> Placement {
    Placement::ring(90.0, chest_h())
}
/// A height over the nominal outer radius in the chest's frame.
fn local_z(h: f64) -> f64 {
    h - chest_h()
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
/// The archivolt round a pointed arch of span `w`: the arch's two arcs offset out by `band`, closed down both jambs to the sill.
fn arch_frame(w: f64, sill: f64, apex: f64, share: f64, band: f64, n: usize) -> Vec<P2> {
    let a = w / 2.0;
    let r = share * w;
    let rise = (r * r - (r - a) * (r - a)).sqrt();
    let zs = apex - rise;
    let arc = |r: f64, right: bool| -> Vec<P2> {
        let top = ((r * r - (r - a - 0.0).powi(2) * 0.0).sqrt()).min(r);
        let _ = top;
        let c = if right { a - share * w } else { -a + share * w };
        let reach = (r * r - c * c).sqrt();
        let t1 = reach.atan2(-c);
        let t0 = if right { 0.0 } else { PI };
        (0..=n).map(|k| { let t = t0 + (t1 - t0) * k as f64 / n as f64; [c + r * t.cos(), zs + r * t.sin()] }).collect()
    };
    let mut p = Vec::new();
    // Outer: up the right jamb, over the apex, down the left jamb.
    p.push([a + band, sill]);
    let mut right = arc(r + band, true);
    let mut left = arc(r + band, false);
    left.reverse();
    p.extend(right.drain(..));
    p.pop();
    p.extend(left.drain(..));
    p.push([-a - band, sill]);
    // Inner, back the other way.
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
    pub fn smin(a: f64, b: f64, k: f64) -> f64 {
        let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
        b + (a - b) * h - k * h * (1.0 - h)
    }
    /// The closed outlines where `f` crosses zero over `lo..hi`, traced on a `step` grid, each counter-clockwise for metal inside (f < 0).
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
        // Edge keys: horizontal edge (i,j)-(i+1,j) = 2*(j*nx+i), vertical (i,j)-(i,j+1) = 2*(j*nx+i)+1.
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
                // Walk the cell's edges counter-clockwise; segments run from an entering edge to a leaving one, metal on the left.
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
                        // Metal on the left going counter-clockwise: start where we leave the inside going ccw round the cell.
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

/// The outline of `f` as polygons, simplified to `tol` and at most `max` points each.
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
fn extrude(sk: Sketch, height: f64) -> Operation {
    Operation::Extrude { sketch: Profile::Inline(sk), height_mm: height, draft_deg: 0.0 }
}
fn boolean(a: Id, b: Id, kind: Boolean) -> Operation {
    Operation::Boolean { a, b, kind }
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
/// The chest frame's axes in the world.
fn chest_frame(d: &RingDesign) -> Result<brep::Placement> {
    seat().frame(d)
}
fn world(d: &RingDesign, p: P3) -> Result<P3> {
    seat().world(d, p)
}
fn world_dir(d: &RingDesign, v: P3) -> Result<P3> {
    let f = chest_frame(d)?;
    Ok(std::array::from_fn(|k| f.x_axis[k] * v[0] + f.y_axis[k] * v[1] + f.z_axis[k] * v[2]))
}
/// The planar face of `c` whose world normal runs along `normal` (chest frame) with its centre nearest `near` (chest frame).
fn face_near(d: &RingDesign, c: &EvaluatedComponent, normal: P3, near: P3) -> Result<usize> {
    let n = world_dir(d, normal)?;
    let p = world(d, near)?;
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
/// The straight edges of `c` whose midpoints (chest frame) satisfy `keep`.
fn edges_where(d: &RingDesign, c: &EvaluatedComponent, keep: &dyn Fn(P3) -> bool) -> Result<Vec<EdgeRef>> {
    let f = chest_frame(d)?;
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

/// A cabochon on face `face` of part `on` (from `c`), centred at `at` (chest frame) and its bezel; returns (stone, bezel).
fn cabochon(doc: &mut Document, ids: &mut Ids, d: &RingDesign, c: &EvaluatedComponent, face: usize, at: P3, gem: Gem, spin: f64, height: f64, name: &str) -> Result<(Id, Id)> {
    let mut s = FaceSeat::on(c, face as u32, Some(world(d, at)?), height)?;
    s.spin_deg = spin;
    let stone = ids.next();
    let mut f = cad::stone_on_face(stone, gem, c.id, &s);
    f.name = format!("{name} cabochon");
    doc.append(f)?;
    let bezel = ids.next();
    doc.append(builders::feature_on(bezel, &format!("Bezel round the {name} cabochon"), builders::BEZEL, stone, serde_json::json!({"wall_mm": BEZEL_WALL_MM, "lip": BEZEL_LIP})))?;
    Ok((stone, bezel))
}

/// The cresting's outline in the ridge plane (x along the finger, z out): a rail and five trefoil fleurs, every stroke at least the section floor.
fn cresting_sdf(ridge: f64) -> impl Fn(P2) -> f64 {
    move |p: P2| {
        let rail = sdf::rect(p, [-5.4, ridge - 0.7], [5.4, ridge + 0.35]);
        let mut d = rail;
        for k in [-2.0f64, -1.0, 0.0, 1.0, 2.0] {
            d = d.min(fleur(p, [k * FLEUR_PITCH_MM, ridge + 0.2]));
        }
        d
    }
}
/// A fleur-de-lis standing on `foot`: a pointed middle petal, two side petals leaning out, a band across, every stroke at least the section floor.
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

struct Authored {
    design: RingDesign,
    lid: Id,
    chest: Id,
    stones: Vec<Id>,
}

/// A box seated at height `h` with its top edges chamfered; returns the chamfered feature.
fn moulded_box(doc: &mut Document, ids: &mut Ids, d: &mut RingDesign, lib: &AlphaLibrary, name: &str, size: P3, h: f64, chamfer: f64, component: Component) -> Result<Id> {
    let block = ids.next();
    doc.append(feature(block, &format!("{name} block"), Operation::Box { size }, placed(Placement::ring(90.0, h))))?;
    d.cad = Some(doc.clone());
    let b = part_at(d, lib, block)?;
    let ztop = local_z(h + size[2] / 2.0);
    let top = face_near(d, &b, [0.0, 0.0, 1.0], [0.0, 0.0, ztop])?;
    let rim = edges_where(d, &b, &|m| (m[2] - ztop).abs() < 1e-3)?;
    ensure!(rim.len() == 4, "{name} top has {} edges", rim.len());
    let id = ids.next();
    doc.append(feature(id, &format!("Chamfer the {}'s top as a moulding", name.to_lowercase()), Operation::Chamfer { source: block, edges: rim, base_face: face_ref(&b, top), distance_mm: chamfer }, component))?;
    Ok(id)
}

/// The arcade's bays along a long wall: centre and span, chest frame x.
fn bays() -> Vec<(f64, f64, f64)> {
    let mut x = -CHEST[0] / 2.0 + POST_MM;
    (0..3)
        .map(|k| {
            let c = x + BAY_W_MM[k] / 2.0;
            x += BAY_W_MM[k] + POST_MM;
            (c, BAY_W_MM[k], BAY_SHARE[k])
        })
        .collect()
}

/// A buttress's half-section off a wall (x out from the wall face, y up the wall, chest frame z).
fn buttress_profile() -> Vec<P2> {
    let (z0, top) = (-CHEST[2] / 2.0 - SINK_MM + 0.05, CHEST[2] / 2.0 - 0.15);
    vec![
        [-0.1, z0],
        [BUTTRESS_FOOT_MM, z0],
        [BUTTRESS_FOOT_MM, 0.2],
        [BUTTRESS_UPPER_MM, 0.5],
        [BUTTRESS_UPPER_MM, top - 0.45],
        [0.0, top],
        [-0.1, top],
    ]
}

/// The whole reliquary as a feature tree.
fn author(lib: &AlphaLibrary) -> Result<Authored> {
    let mut d = band();
    let mut doc = Document::default();
    let mut ids = Ids(0);
    let band_id = ids.next();
    doc.append(feature(band_id, "Procedural shank", Operation::Band, Component::default()))?;
    let at = |doc: &Document, d: &mut RingDesign| d.cad = Some(doc.clone());
    let cutter = |p: Placement| Component { role: ComponentRole::Other, attach: Attach::Cut, placement: p, ..Component::default() };

    // The plinth: a base course sunk into the crest, and a narrower step on it, both chamfered.
    moulded_box(&mut doc, &mut ids, &mut d, lib, "Plinth", PLINTH, plinth_top() - PLINTH[2] / 2.0, PLINTH_CHAMFER_MM, joined(ComponentRole::Shank, Placement::Free, 0.35))?;
    moulded_box(&mut doc, &mut ids, &mut d, lib, "Step", STEP, plinth_top() - SINK_MM + STEP[2] / 2.0, STEP_CHAMFER_MM, joined(ComponentRole::Shank, Placement::Free, 0.0))?;

    // The chest: a box hollowed from the top, its relic floor pressed up.
    let chest_box = ids.next();
    doc.append(feature(chest_box, "Chest block", Operation::Box { size: CHEST }, placed(seat())))?;
    at(&doc, &mut d);
    let cb = part_at(&d, lib, chest_box)?;
    let open = face_near(&d, &cb, [0.0, 0.0, 1.0], [0.0, 0.0, CHEST[2] / 2.0])?;
    let hollow = ids.next();
    doc.append(feature(hollow, "Hollow the chest from the top", Operation::Shell { source: chest_box, open_faces: vec![face_ref(&cb, open)], thickness_mm: WALL_MM }, placed(Placement::Free)))?;
    at(&doc, &mut d);
    let hc = part_at(&d, lib, hollow)?;
    let floor = face_near(&d, &hc, [0.0, 0.0, 1.0], [0.0, 0.0, -CHEST[2] / 2.0 + WALL_MM])?;
    let chest = ids.next();
    doc.append(feature(chest, "Press the relic floor up", Operation::PressPull { source: hollow, face: face_ref(&hc, floor), distance_mm: FLOOR_RAISE_MM }, joined(ComponentRole::Head, Placement::Free, 0.0)))?;

    // Blind arcades: three pointed bays on each long wall.
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
    // Buttresses on the posts: a foot, a weathering, a slimmer upper stage dying into the wall under the eaves.
    let posts = bays();
    let mut post_x: Vec<f64> = vec![-CHEST[0] / 2.0 + POST_MM / 2.0, CHEST[0] / 2.0 - POST_MM / 2.0];
    post_x.extend([posts[0].0 + posts[0].1 / 2.0 + POST_MM / 2.0, posts[1].0 + posts[1].1 / 2.0 + POST_MM / 2.0]);
    for (side, sign) in [("north", 1.0f64), ("south", -1.0)] {
        for (k, xc) in post_x.iter().enumerate() {
            let x0 = xc - sign * BUTTRESS_W_MM / 2.0;
            let mut sk = sketch_on(&format!("Buttress {} section, {side} wall", k + 1), plane([x0, 0.0, 0.0], [0.0, sign, 0.0], [0.0, 0.0, 1.0]));
            let prof: Vec<P2> = buttress_profile().iter().map(|p| [CHEST[1] / 2.0 + p[0], p[1]]).collect();
            polygon(&mut sk, &prof);
            let id = ids.next();
            doc.append(feature(id, &format!("Stand buttress {} against the {side} wall", k + 1), extrude(sk, BUTTRESS_W_MM), joined(ComponentRole::Head, seat(), 0.0)))?;
        }
    }
    // A pointed portal on each end wall: a blind niche under a raised archivolt.
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
        let at = [0.0, sign * CHEST[1] / 2.0, BAY_STONE_Z_MM];
        let face = face_near(&d, &ch, [0.0, sign, 0.0], at)?;
        let (s, _) = cabochon(&mut doc, &mut ids, &d, &ch, face, at, garnet(), 0.0, builders::stand_off_mm(builders::BEZEL, garnet()) - NICHE_MM, &format!("Garnet, {name}"))?;
        stones.push(s);
    }

    // Graded quatrefoils pierced through both shoulders.
    for (off, size) in PIERCINGS {
        for (side, sign) in [("east", -1.0), ("west", 1.0)] {
            let theta = 90.0 + sign * off;
            let mut at = builders::cutters::pierce_at(&d, theta, 0.0, None, builders::cutters::Shape::Quatrefoil)?;
            if let Some(m) = at.params.as_object_mut() {
                m.insert("width_mm".into(), serde_json::json!(size));
                m.insert("length_mm".into(), serde_json::json!(size));
            }
            let mut f = builders::cutters::pierce_feature(ids.next(), builders::cutters::Shape::Quatrefoil, &at);
            f.name = format!("Pierce a {size:.1} mm quatrefoil through the {side} shoulder, {off:.0} deg off the top");
            doc.append(f)?;
        }
    }

    // The lid: a steep gabled prism over the chest, eaves chamfered, overhanging the walls and the ends.
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
    let under = face_near(&d, &lp, [0.0, 0.0, -1.0], [0.0, 0.0, base])?;
    let eaves = edges_where(&d, &lp, &|m| (m[2] - base).abs() < 1e-3 && m[0].abs() < 1.0)?;
    ensure!(eaves.len() == 2, "lid has {} eave edges", eaves.len());
    let lid_body = ids.next();
    doc.append(feature(lid_body, "Chamfer the eaves", Operation::Chamfer { source: prism, edges: eaves, base_face: face_ref(&lp, under), distance_mm: EAVES_CHAMFER_MM }, placed(Placement::Free)))?;
    at(&doc, &mut d);
    let lb = part_at(&d, lib, lid_body)?;
    let slope = RISE_MM.atan2(ew);
    let (c, sn) = (slope.cos(), slope.sin());
    let mut lid_bezels = Vec::new();
    for (name, sign) in [("north slope", 1.0), ("south slope", -1.0)] {
        let at = [0.0, sign * ew * 0.5, eh + RISE_MM * 0.5];
        let face = face_near(&d, &lb, [0.0, sign * sn, c], at)?;
        let (s, b) = cabochon(&mut doc, &mut ids, &d, &lb, face, at, garnet(), 0.0, builders::stand_off_mm(builders::BEZEL, garnet()), &format!("Garnet, {name}"))?;
        stones.push(s);
        lid_bezels.push((b, name));
    }
    let mut gable_bezels = Vec::new();
    for (name, sign) in [("east gable", 1.0), ("west gable", -1.0)] {
        let at = [sign * len / 2.0, 0.0, base + WINDOW_STONE_MM];
        let face = face_near(&d, &lb, [sign, 0.0, 0.0], at)?;
        let (s, b) = cabochon(&mut doc, &mut ids, &d, &lb, face, at, sapphire(), 90.0, builders::stand_off_mm(builders::BEZEL, sapphire()) - WINDOW_MM, &format!("Sapphire, {name} window"))?;
        stones.push(s);
        gable_bezels.push((b, name));
    }
    let mut lid = lid_body;
    for (b, name) in lid_bezels {
        let u = ids.next();
        doc.append(feature(u, &format!("Join the {name} bezel to the lid"), boolean(lid, b, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
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
    // Cresting along the ridge, and a turned finial at each end of it.
    let crest = cresting_sdf(ridge);
    let loops = outlines(&crest, [-6.0, ridge - 1.0], [6.0, ridge + 2.8], 0.02, 0.006, 220);
    ensure!(loops.len() == 1, "cresting traced to {} loops: {:?}", loops.len(), loops.iter().map(|l| (l.len(), sdf::area(l), l[0])).collect::<Vec<_>>());
    let mut sk = sketch_on("Cresting: a rail and five fleurs", plane([0.0, 0.45, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    polygon(&mut sk, &loops[0]);
    let cresting = ids.next();
    doc.append(feature(cresting, "Cut the cresting: a rail and five fleurs", extrude(sk, 0.9), placed(seat())))?;
    let u = ids.next();
    doc.append(feature(u, "Crest the ridge", boolean(lid, cresting, Boolean::Union), placed(Placement::Free)))?;
    lid = u;
    for (end, sign) in [("east", 1.0), ("west", -1.0)] {
        let x0 = sign * (len / 2.0 - 0.3);
        let mut sk = sketch_on(&format!("Finial profile, {end}"), plane([x0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        polygon(&mut sk, &finial_profile(ridge - 0.67));
        let fin = ids.next();
        doc.append(feature(fin, &format!("Turn the {end} finial"), Operation::Revolve { sketch: Profile::Inline(sk), pivot: [0.0, 0.0, 0.0], axis: [0.0, 1.0, 0.0], degrees: 360.0, in_plane: true }, placed(seat())))?;
        let u = ids.next();
        doc.append(feature(u, &format!("Set the {end} finial on the ridge"), boolean(lid, fin, Boolean::Union), placed(Placement::Free)))?;
        lid = u;
    }
    if let Some(f) = doc.features.iter_mut().find(|f| f.id == lid) {
        f.name = format!("{}: the lid, cast apart", f.name);
        f.component = Component { role: ComponentRole::Other, attach: Attach::Separate, stage: Stage::Cast, bench_notes: "Cast apart; hinge to the chest's north eave with a three-knuckle hinge on a 0.8 mm pin, soldered at the bench".into(), ..Component::default() };
    }
    doc.joints.push(cad::Joint { a: chest, b: lid, clearance_mm: LID_CLEAR_MM, method: "Three-knuckle hinge, 0.8 mm pin, soldered at the bench".into(), notes: "The lid lifts on the north eave; the chest's rim takes the pin's knuckles".into() });
    d.cad = Some(doc);
    Ok(Authored { design: d, lid, chest, stones })
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

// ---------------------------------------------------------------- output

/// Each named view: the ring turned about the head's axis, then the camera's yaw about the finger's axis and its pitch toward the head.
/// The hero stands off the gable end, a third of a right angle round the head and a quarter up from the finger's axis.
const VIEWS: [(&str, f64, f64, f64); 6] = [
    ("hero", 0.6, 0.0, 0.38),
    ("face", 0.0, 0.0, PI * 0.5),
    ("palm", 0.0, PI, 1.05),
    ("side", 0.0, 0.0, 0.0),
    ("shoulder", 0.0, 0.75, 0.6),
    ("reverse", 0.0, PI - 0.5, 0.35),
];

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

fn renders(out: &Path, built: &mesh::BuildResult, gems: &[(mesh::Mesh, [f32; 3])], edge: usize) -> Result<()> {
    for (name, turn, yaw, pitch) in VIEWS {
        view(&out.join(format!("{name}.png")), &built.mesh, gems, turn, yaw, pitch, edge)?;
    }
    let (_, t, y, p) = VIEWS[0];
    view(&out.join("hero-300.png"), &built.mesh, gems, t, y, p, 300)?;
    let (_, t, y, p) = VIEWS[1];
    view(&out.join("face-300.png"), &built.mesh, gems, t, y, p, 300)?;
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

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/capsa"));
    std::fs::create_dir_all(&out)?;
    let lib = AlphaLibrary::builtin();
    println!("Capsa");
    let t = std::time::Instant::now();
    let a = author(&lib)?;
    let author_s = t.elapsed().as_secs_f64();
    let d = a.design;
    let params = if draft { draft_params() } else { export_params() };
    let t = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = t.elapsed().as_secs_f64();
    let e = cad::evaluate(&d, &lib, params)?;
    let failed: Vec<String> = e.features.iter().filter(|r| !matches!(r.status, cad::FeatureStatus::Ok)).map(|r| format!("#{} {}: {:?}", r.id, r.name, r.status)).collect();
    let parts: Vec<serde_json::Value> = e
        .components
        .iter()
        .filter(|c| !c.settings.reference)
        .map(|c| {
            let q = c.mesh.quality();
            serde_json::json!({"id": c.id, "name": c.name, "attach": format!("{:?}", c.attach), "triangles": c.mesh.faces.len(), "watertight": c.mesh.validate().watertight, "degenerate_faces": q.degenerate_faces, "self_crossings": ringdesign_core::csg::self_crossings(&solid_of(&c.mesh))})
        })
        .collect();
    let gems = ringdesign_core::gems::built_meshes(&d, &lib, &built);
    let watertight = built.mesh.validate().watertight;
    let degenerate = built.mesh.quality().degenerate_faces;
    let crossings = ringdesign_core::csg::self_crossings(&solid_of(&built.mesh));
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let stones = d.cad.as_ref().map_or(0, |doc| doc.features.iter().filter(|f| f.component.reference).count());
    println!(
        "  authored in {author_s:.1} s; {} triangles in {build_s:.1} s; watertight {watertight}, degenerate {degenerate}, self-crossings {crossings}; nearest the axis {least_r:.3} mm ({inside} inside); {stones} stones; notes {:?} {:?}; failed {failed:?}",
        built.mesh.faces.len(),
        built.solids.notes,
        built.parts.notes
    );
    let report = serde_json::json!({
        "name": d.name,
        "stage": "block-out (read test 3 of 3 did not read; stopped before the detail rounds)",
        "process": d.draft.process.label(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3},
        "parts": parts,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "features_not_ok": failed,
        "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len()),
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "stones": {"reference_stones": stones, "previewed_groups": gems.len()},
        "lid": a.lid,
        "stone_features": a.stones,
        "chest": a.chest,
        "views": VIEWS.iter().map(|(n, t, y, p)| serde_json::json!({"view": n, "turn_about_head_rad": t, "yaw_rad": y, "pitch_rad": p})).collect::<Vec<_>>(),
        "read_tests": ["read-test-1.json", "read-test-2.json", "read-test-3.json"],
        "gates_note": "Gates are run in the detail rounds; the block-out never reached them. These are the block-out's own draft numbers.",
    });
    let name = if draft { "report.json" } else { "report-export.json" };
    std::fs::write(out.join(name), serde_json::to_vec_pretty(&report)?)?;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    renders(&out, &built, &gems, if draft { 1000 } else { 1600 })?;
    ensure!(watertight && degenerate == 0 && failed.is_empty(), "Capsa's block-out did not build clean; see {}", out.join(name).display());
    Ok(())
}
