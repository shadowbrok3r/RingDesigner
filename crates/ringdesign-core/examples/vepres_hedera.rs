//! Vepres — Hedera, the strangling ivy: a polished D-shape court band strangled by an ivy stem that climbs it from
//! edge to edge, rolling over each edge onto the side faces, with five-lobed palmate leaves on petioles lying in the
//! bays the stem leaves, and an umbel of seven black spinel berries in collets on the crown where the stem crosses it.
//! Lost wax. cargo build --release -p ringdesign-core --example vepres_hedera
//! target/release/examples/vepres_hedera [OUT_DIR] [--draft] [--verify] [--quick]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, Stage, SurfaceKind, builders, stored},
    castability, csg, dfm,
    gem::{Gem, GemCut},
    library, mesh,
    profile::{MAX_PROFILE_STEPS, ProfileSample, ShankKind},
    render, stl,
};
use serde_json::json;
use std::f64::consts::{PI, TAU};
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The host: a D-shape court band.
const WIDTH_MM: f64 = 5.5;
const THICK_MM: f64 = 2.0;
/// A rounder edge than the D-shape's own, so the stem and the leaves roll over it onto the side faces.
const EDGE_ROUND_MM: f64 = 0.55;
/// The lost-wax section floor and the investment's detail floor.
const MIN_SECTION_MM: f64 = 0.8;
const MIN_DETAIL_MM: f64 = 0.15;
/// The crown, where the umbel stands on the stem's crossing.
const CROWN_DEG: f64 = 90.0;

/// The stem: from its cut end at `STEM_FROM_DEG` round `STEM_SPAN_DEG`, crossing the band `STEM_WAVES` times a
/// turn, its centreline swinging `STEM_REACH_MM` either side of the crest.
const STEM_FROM_DEG: f64 = 250.0;
const STEM_SPAN_DEG: f64 = 330.0;
const STEM_WAVES: f64 = 2.0;
/// How far either side of the crest the stem's centreline swings: over the crown, short of the edges' rounds.
const STEM_REACH_MM: f64 = 2.1;
/// The stem's radius, the share of its diameter sunk into the band, and its growing tip's radius.
const STEM_R_MM: f64 = 0.68;
const STEM_SUNK: f64 = 0.38;
const STEM_TIP_R_MM: f64 = 0.44;

/// The umbel: seven black spinel round cabochons.
const BERRY_MM: f64 = 2.2;
const SPINEL_TINT: [f32; 3] = [0.03, 0.03, 0.04];
/// The umbel's hub over the crown, where the peduncle from the stem ends and the seven stalks fan out, and the
/// reach from it to each berry's girdle.
const UMBEL_HUB_MM: f64 = 0.6;
const UMBEL_R_MM: f64 = 3.55;
/// Each berry's direction from the hub: (angle from the crown's normal, turn from +theta toward +z), degrees.
/// Spread so that no two collets come nearer than 0.5 mm, every one clears the band and the leaves, and the spray
/// stays within 3 mm of the crown round the ring, leaving the crown leaves square to the face.
const BERRIES: [(f64, f64); 7] = [(31.3, 92.8), (33.6, 268.1), (57.3, 356.6), (78.6, 147.3), (80.3, 211.6), (90.4, 60.2), (92.0, 296.9)];
/// The collet's wall and lip: a setting, a thin rim round each berry, judged at the detail floor as settings are.
const COLLET_WALL_MM: f64 = 0.45;
const COLLET_LIP: f64 = 0.1;
/// The receptacle each berry's collet stands on: its face's radius (the collet's foot), the square band under the
/// face, how deep it flares back down into the stalk, and its neck.
const RECEPTACLE_R_MM: f64 = 1.52;
const RECEPTACLE_BAND_MM: f64 = 0.2;
const RECEPTACLE_DEEP_MM: f64 = 1.15;
const RECEPTACLE_NECK_MM: f64 = 0.7;
/// The stalks: each berry's pedicel and the peduncle from the stem to the hub.
const PEDICEL_R_MM: f64 = 0.5;
const PEDUNCLE_R_MM: f64 = 0.62;
/// The rootlets: where each starts out from the stem's centreline, its reach, its radius at the root and the tip,
/// and the pitch of each of the four arrays along the stem (two each side, offset half a pitch).
const ROOTLET_FROM_MM: f64 = 0.55;
const ROOTLET_REACH_MM: f64 = 0.75;
const ROOTLET_ROOT_R_MM: f64 = 0.2;
const ROOTLET_TIP_R_MM: f64 = 0.11;
const ROOTLET_PITCH_MM: f64 = 1.0;

/// A leaf's sink under the band's surface, its height over it at the margin and at the hub, and the vein relief.
const LEAF_SINK_MM: f64 = 0.45;
/// Where a leaf leaves the band toward an edge (the surface's tilt), and the radius it droops at past there.
const LEAF_LIP_DEG: f64 = 24.0;
const LEAF_DROOP_MM: f64 = 6.0;
const LEAF_DROOP_MAX_DEG: f64 = 12.0;
const LEAF_RIM_SINK_MM: f64 = 0.12;
const LEAF_EDGE_MM: f64 = 0.83;
const LEAF_HUB_MM: f64 = 1.02;
/// The round on the blade's top edge.
const LEAF_EDGE_ROUND_MM: f64 = 0.12;
/// The plate's least thickness, top to floor: the lost-wax section with a little to spare.
const LEAF_PLATE_MM: f64 = 0.95;
/// The veins: incised from the hub out along each lobe, this deep and this wide at the top.
const VEIN_DEEP_MM: f64 = 0.14;
const VEIN_W_MM: f64 = 0.16;
/// The least plate left under a vein's groove.
const LEAF_UNDER_VEIN_MM: f64 = 0.86;
/// The petiole's radius.
const PETIOLE_R_MM: f64 = 0.42;
/// The petiole's centre over the band: sunk a little, standing 0.4 mm proud.
const PETIOLE_H_MM: f64 = 0.12;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() }
}

/// The host band: a Uniform D-shape 5.5 x 2.0, comfort fit, bore 18.6, cast in lost wax at the investment floors.
fn host() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Hedera".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.width_mm = WIDTH_MM;
    d.profile.thickness_mm = THICK_MM;
    d.profile.apply_style(ProfileStyle::DShape);
    d.profile.edge_round_mm = EDGE_ROUND_MM;
    d.profile.comfort_fit_mm = 0.15;
    d.shank.kind = ShankKind::Uniform;
    probe::cast_in(&mut d, &probe::wax_setup(0.1));
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d
}

// --- The band's chart -----------------------------------------------------------------------------------------

/// The band's outer surface as one section (the host is uniform round the ring): a point and its normal at `w` mm
/// of arc from the crest, positive toward +z, so every motif is laid on the band as drawn in `(theta, w)`.
struct Chart {
    s: Vec<ProfileSample>,
    crest_v: f64,
    sign: f64,
}

impl Chart {
    fn of(d: &RingDesign) -> Chart {
        let reference = d.reference_loop();
        let l = d.section_at(0.0, MAX_PROFILE_STEPS, None, Some(&reference));
        let mut s: Vec<ProfileSample> = l.pts.iter().filter(|p| p.surface).cloned().collect();
        s.sort_by(|a, b| a.v_mm.total_cmp(&b.v_mm));
        s.dedup_by(|a, b| (a.v_mm - b.v_mm).abs() < 1e-9);
        let crest_v = reference.crest_v_mm;
        let mut c = Chart { s, crest_v, sign: 1.0 };
        let z1 = c.raw(crest_v + 1.0).1;
        c.sign = if z1 >= 0.0 { 1.0 } else { -1.0 };
        c
    }

    /// (r, z, nr, nz) at surface arc `v`.
    fn raw(&self, v: f64) -> (f64, f64, f64, f64) {
        let s = &self.s;
        let v = v.clamp(s[0].v_mm, s[s.len() - 1].v_mm);
        let k = s.partition_point(|p| p.v_mm < v).clamp(1, s.len() - 1);
        let (a, b) = (&s[k - 1], &s[k]);
        let t = ((v - a.v_mm) / (b.v_mm - a.v_mm).max(1e-12)).clamp(0.0, 1.0);
        let l = |x: f64, y: f64| x + (y - x) * t;
        let (nr, nz) = (l(a.nr, b.nr), l(a.nz, b.nz));
        let n = nr.hypot(nz).max(1e-12);
        (l(a.r, b.r), l(a.z, b.z), nr / n, nz / n)
    }

    /// (r, z, nr, nz) at `w` mm from the crest.
    fn at(&self, w: f64) -> (f64, f64, f64, f64) {
        self.raw(self.crest_v + self.sign * w)
    }

    /// The world point `h` mm out along the normal from the surface at `(theta_deg, w)`.
    fn world(&self, theta_deg: f64, w: f64, h: f64) -> P3 {
        let (r, z, nr, nz) = self.at(w);
        let (s, c) = theta_deg.to_radians().sin_cos();
        let rr = r + h * nr;
        [rr * c, rr * s, z + h * nz]
    }

    /// [`Chart::world`] on the surface a leaf lies on: the band's own over the crown, then past `LEAF_LIP_DEG` of tilt
    /// toward an edge a surface that carries on from there and droops `1 / LEAF_DROOP_MM` a millimetre, so a lobe
    /// reaching over the edge stands off it and breaks the band's outline.
    fn leaf_world(&self, theta_deg: f64, w: f64, h: f64) -> P3 {
        let side = if w < 0.0 { -1.0 } else { 1.0 };
        let lip = self.lip_w(side);
        if w * side <= lip * side {
            return self.world(theta_deg, w, h);
        }
        let (r, z, nr, nz) = self.at(lip);
        // The tangent toward the edge: the normal turned a quarter toward `side`.
        let (tr, tz) = (-nz * side, nr * side);
        // Bending down to at most LEAF_DROOP_MAX_DEG, then running straight on.
        let run = (w - lip).abs();
        let a = (run / LEAF_DROOP_MM).min(LEAF_DROOP_MAX_DEG.to_radians());
        let straight = run - a * LEAF_DROOP_MM;
        let (sa, ca) = a.sin_cos();
        let (ar, az) = (tr * ca - nr * sa, tz * ca - nz * sa);
        let pr = r + LEAF_DROOP_MM * (sa * tr - (1.0 - ca) * nr) + straight * ar;
        let pz = z + LEAF_DROOP_MM * (sa * tz - (1.0 - ca) * nz) + straight * az;
        let (qr, qz) = (nr * ca + tr * sa, nz * ca + tz * sa);
        let (s, co) = theta_deg.to_radians().sin_cos();
        let rr = pr + h * qr;
        [rr * co, rr * s, pz + h * qz]
    }

    /// The `w` past which a leaf leaves the band's surface toward `side`.
    fn lip_w(&self, side: f64) -> f64 {
        let mut w = 0.0;
        while w < 6.0 {
            let (_, _, nr, nz) = self.at(side * w);
            if (nz * side).atan2(nr).to_degrees() > LEAF_LIP_DEG {
                return side * w;
            }
            w += 0.005;
        }
        side * w
    }

    /// The `w` where the edge's round turns the normal 45 degrees toward `side` (+1 or -1): where the crown ends.
    fn edge_w(&self, side: f64) -> f64 {
        let mut w = 0.0;
        while w < 6.0 {
            let (_, _, nr, nz) = self.at(side * w);
            if (nz * side) > nr {
                return side * w;
            }
            w += 0.01;
        }
        side * w
    }
}

/// A chart point `(x, y)` mm in a motif's own plan, about a hub at `(theta, w)` with its x turned `axis_deg` from the
/// ring's tangent toward +w: its `(theta, w)`.
fn plan_to_chart(c: &Chart, hub: (f64, f64), axis_deg: f64, p: [f64; 2]) -> (f64, f64) {
    let (s, co) = axis_deg.to_radians().sin_cos();
    let (dx, dy) = (p[0] * co - p[1] * s, p[0] * s + p[1] * co);
    let w = hub.1 + dy;
    // Round the ring at the radius of the crest under the hub: the leaf keeps its plan along the band.
    let r = c.at(hub.1).0;
    (hub.0 + (dx / r).to_degrees(), w)
}

// --- Tubes: the stem, petioles and stalks ----------------------------------------------------------------------

fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: P3, k: f64) -> P3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P3, b: P3) -> P3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: P3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P3) -> P3 {
    scale(a, 1.0 / norm(a).max(1e-12))
}

/// A closed tube round `line` (centreline points, each with its radius), its frames rotation-minimising along it,
/// each end closed by a dome of `end_rings` rings (0: a flat cut).
fn tube(line: &[(P3, f64)], sides: usize, start_dome: bool, end_dome: bool) -> csg::Solid {
    let n = line.len();
    let tangent = |i: usize| unit(sub(line[(i + 1).min(n - 1)].0, line[i.saturating_sub(1)].0));
    // Rotation-minimising frames by double reflection.
    let t0 = tangent(0);
    let seed = if t0[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let mut u = unit(cross(t0, seed));
    let mut frames = vec![(u, cross(t0, u))];
    for i in 0..n - 1 {
        let (x0, x1) = (line[i].0, line[i + 1].0);
        let (ti, tn) = (tangent(i), tangent(i + 1));
        let v1 = sub(x1, x0);
        let c1 = dot(v1, v1).max(1e-18);
        let rl = sub(u, scale(v1, 2.0 / c1 * dot(v1, u)));
        let tl = sub(ti, scale(v1, 2.0 / c1 * dot(v1, ti)));
        let v2 = sub(tn, tl);
        let c2 = dot(v2, v2);
        u = if c2 < 1e-18 { rl } else { sub(rl, scale(v2, 2.0 / c2 * dot(v2, rl))) };
        u = unit(sub(u, scale(tn, dot(u, tn))));
        frames.push((u, cross(tn, u)));
    }
    let mut rings: Vec<(P3, f64, P3, P3)> = Vec::new();
    const DOME: usize = 4;
    if start_dome {
        for k in (1..=DOME).rev() {
            let a = PI * 0.5 * k as f64 / (DOME + 1) as f64;
            let (p, r) = line[0];
            rings.push((sub(p, scale(tangent(0), r * a.sin())), r * a.cos(), frames[0].0, frames[0].1));
        }
    }
    for i in 0..n {
        rings.push((line[i].0, line[i].1, frames[i].0, frames[i].1));
    }
    if end_dome {
        for k in 1..=DOME {
            let a = PI * 0.5 * k as f64 / (DOME + 1) as f64;
            let (p, r) = line[n - 1];
            rings.push((add3(p, scale(tangent(n - 1), r * a.sin())), r * a.cos(), frames[n - 1].0, frames[n - 1].1));
        }
    }
    let mut s = csg::Solid::default();
    let first = rings[0];
    let last = rings[rings.len() - 1];
    let start_tip = if start_dome { sub(line[0].0, scale(tangent(0), line[0].1)) } else { first.0 };
    let end_tip = if end_dome { add3(line[n - 1].0, scale(tangent(n - 1), line[n - 1].1)) } else { last.0 };
    s.v.push(start_tip);
    s.v.push(end_tip);
    for &(c, r, a, b) in &rings {
        for j in 0..sides {
            let t = TAU * j as f64 / sides as f64;
            s.v.push(add3(c, add3(scale(a, r * t.cos()), scale(b, r * t.sin()))));
        }
    }
    let m = rings.len();
    let ring = |i: usize, j: usize| (2 + i * sides + j % sides) as u32;
    for j in 0..sides {
        s.f.push([0, ring(0, j + 1), ring(0, j)]);
        s.f.push([1, ring(m - 1, j), ring(m - 1, j + 1)]);
    }
    for i in 0..m - 1 {
        for j in 0..sides {
            s.f.push([ring(i, j), ring(i, j + 1), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i + 1, j)]);
        }
    }
    s
}

/// The stem's chart: `w` at `theta`, the wave crossing the crown under the umbel.
fn stem_w(c: &Chart, theta: f64) -> f64 {
    let _ = c;
    STEM_REACH_MM * (STEM_WAVES * (theta - CROWN_DEG)).to_radians().sin()
}

/// The stem's centreline height over the surface.
fn stem_h() -> f64 {
    STEM_R_MM * (1.0 - 2.0 * STEM_SUNK)
}

/// The stem: a tube along the chart wave from its cut end to its tapering growing tip.
fn stem_solid(c: &Chart) -> csg::Solid {
    let steps = (STEM_SPAN_DEG / 1.25) as usize;
    let line: Vec<(P3, f64)> = (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let theta = STEM_FROM_DEG + STEM_SPAN_DEG * t;
            // Full girth for most of its length, tapering over the last fifth to the growing tip.
            let taper = ((t - 0.8) / 0.2).clamp(0.0, 1.0);
            let r = STEM_R_MM + (STEM_TIP_R_MM - STEM_R_MM) * taper * taper;
            let h = r * (1.0 - 2.0 * STEM_SUNK);
            (c.world(theta, stem_w(c, theta), h), r)
        })
        .collect();
    tube(&line, 18, false, true)
}

// --- Leaves ---------------------------------------------------------------------------------------------------

/// An ivy leaf's plan, hub (where the petiole meets the blade and the veins fan out) at the origin and the
/// terminal lobe along +x: palmate lobes `(angle, reach)` with pointed tips and rounded sinuses, a cordate notch
/// behind the hub.
#[derive(Clone)]
struct Ivy {
    /// The lobes' tips `(angle, reach)`: the terminal first, then round the blade.
    lobes: Vec<(f64, f64, f64)>,
    /// The blade's reach at `RHO_STEPS` angles round the hub, from -pi.
    table: Vec<f64>,
}

const RHO_STEPS: usize = 2048;

impl Ivy {
    /// The juvenile five-lobed leaf, `len` mm from the hub to the terminal tip: a pointed terminal lobe, two
    /// laterals near square to it, two small basal lobes, shallow rounded sinuses and the cordate notch at the
    /// stalk, every edge between a tip and a sinus bowed a little out.
    fn five(len: f64) -> Ivy {
        // Round the blade from the notch: (angle, reach) of every tip and sinus.
        let tips = [(0.0, 1.0), (1.22, 0.8), (2.32, 0.52)];
        let sinuses = [(0.6, 0.6), (1.78, 0.46), (PI, 0.16)];
        let mut corners: Vec<(f64, f64, bool)> = Vec::new();
        for side in [1.0, -1.0] {
            for k in 0..3 {
                corners.push((side * tips[k].0, tips[k].1 * len, true));
                corners.push((side * sinuses[k].0, sinuses[k].1 * len, false));
            }
        }
        corners.sort_by(|a, b| a.0.total_cmp(&b.0));
        corners.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9);
        // The outline as points: each edge a quadratic curve bowed out by an eighth of its length.
        let pt = |(a, r, _): (f64, f64, bool)| [r * a.cos(), r * a.sin()];
        let mut poly = Vec::new();
        let n = corners.len();
        for k in 0..n {
            let (p, q) = (pt(corners[k]), pt(corners[(k + 1) % n]));
            let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
            let mid = [0.5 * (p[0] + q[0]), 0.5 * (p[1] + q[1])];
            // Outward: the left of a counter-clockwise edge's direction is inward, so bow to its right.
            let bow = 0.12;
            let ctrl = [mid[0] + dy * bow, mid[1] - dx * bow];
            for i in 0..24 {
                let t = i as f64 / 24.0;
                let u = 1.0 - t;
                poly.push([u * u * p[0] + 2.0 * u * t * ctrl[0] + t * t * q[0], u * u * p[1] + 2.0 * u * t * ctrl[1] + t * t * q[1]]);
            }
        }
        // Every tip blunted to a round of nearly half a millimetre, so no lobe is thinner than the section near
        // its point: passes of neighbour averaging.
        for _ in 0..11 {
            let m = poly.len();
            poly = (0..m).map(|k| {
                let (a, b, c) = (poly[(k + m - 1) % m], poly[k], poly[(k + 1) % m]);
                [0.25 * a[0] + 0.5 * b[0] + 0.25 * c[0], 0.25 * a[1] + 0.5 * b[1] + 0.25 * c[1]]
            }).collect();
        }
        // The reach at each angle: where a ray from the hub meets the outline.
        let table = (0..RHO_STEPS)
            .map(|i| {
                let phi = -PI + TAU * i as f64 / RHO_STEPS as f64;
                let d = [phi.cos(), phi.sin()];
                let mut best = 0.0f64;
                for k in 0..poly.len() {
                    let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                    let e = [b[0] - a[0], b[1] - a[1]];
                    let den = d[0] * e[1] - d[1] * e[0];
                    if den.abs() < 1e-12 { continue; }
                    let t = (a[0] * e[1] - a[1] * e[0]) / den;
                    let u = (a[0] * d[1] - a[1] * d[0]) / den;
                    if t > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u) {
                        best = best.max(t);
                    }
                }
                best
            })
            .collect();
        let lobes = tips.iter().flat_map(|&(a, r)| if a == 0.0 { vec![(0.0, r * len, 0.0)] } else { vec![(a, r * len, 0.0), (-a, r * len, 0.0)] }).collect();
        Ivy { lobes, table }
    }

    /// The reach of the blade from the hub at plan angle `phi`.
    fn rho(&self, phi: f64) -> f64 {
        let x = ((phi + PI).rem_euclid(TAU)) / TAU * RHO_STEPS as f64;
        let i = x.floor() as usize % RHO_STEPS;
        let f = x - x.floor();
        self.table[i] * (1.0 - f) + self.table[(i + 1) % RHO_STEPS] * f
    }

    /// Perpendicular distance from plan point `(r, phi)` to the nearest vein, and that vein's reach.
    fn vein(&self, r: f64, phi: f64) -> (f64, f64) {
        let mut best = (f64::MAX, 1.0);
        for &(a, l, _) in &self.lobes {
            let d = (phi - a + PI).rem_euclid(TAU) - PI;
            let dist = if d.abs() < PI * 0.5 { r * d.sin().abs() } else { r };
            if dist < best.0 {
                best = (dist, l);
            }
        }
        best
    }

    /// The floor's height over the band at `(r, phi)`: sunk deep under the hub, rising toward the margin.
    fn floor(&self, r: f64, phi: f64) -> f64 {
        let s = (r / self.rho(phi)).clamp(0.0, 1.0);
        let t = ((s - 0.3) / 0.6).clamp(0.0, 1.0);
        -LEAF_SINK_MM + (LEAF_SINK_MM - LEAF_RIM_SINK_MM) * t * t * (3.0 - 2.0 * t)
    }

    /// The top's height over the band at `(r, phi)`: domed from the hub to the margin, the veins standing on it in
    /// their grooves, never nearer the floor than the plate.
    fn top(&self, r: f64, phi: f64) -> f64 {
        let rho = self.rho(phi);
        let s = (r / rho).clamp(0.0, 1.0);
        let dome = LEAF_EDGE_MM + (LEAF_HUB_MM - LEAF_EDGE_MM) * (1.0 - s * s);
        let (dist, reach) = self.vein(r, phi);
        // Each vein a rounded groove from the hub to four fifths of its lobe, fading out before the margin.
        let along = (r / (0.85 * reach)).clamp(0.0, 1.0);
        let fade = (1.0 - along * along).max(0.0);
        let groove = VEIN_DEEP_MM * (-(dist / (0.5 * VEIN_W_MM)).powi(2)).exp() * fade;
        dome.max(self.floor(r, phi) + LEAF_PLATE_MM) - groove.min((dome.max(self.floor(r, phi) + LEAF_PLATE_MM) - self.floor(r, phi) - LEAF_UNDER_VEIN_MM).max(0.0))
    }
}

/// The leaf as a closed solid laid on the band: a polar sheet about its hub, its top domed and veined, the margin
/// a full round from top to floor, and the floor sunk under the band's surface.
fn leaf_solid(c: &Chart, ivy: &Ivy, hub: (f64, f64), axis_deg: f64, curl_mm: f64) -> csg::Solid {
    const AROUND: usize = 200;
    const RINGS: usize = 18;
    const RIM: usize = 5;
    let len = ivy.lobes[0].1;
    // The blade's mid-surface: the band (or the surface a lobe leaves it on) lifted by the curl, which rises past a
    // third of the length, most at the terminal lobe's tip.
    let mid = |x: f64, y: f64| {
        let (t, w) = plan_to_chart(c, hub, axis_deg, [x, y]);
        let u = ((x - 0.33 * len) / (0.67 * len)).max(0.0);
        c.leaf_world(t, w, curl_mm * u * u)
    };
    // Heights are measured along the blade's own normal, so every wall stands square to the blade.
    let to_world = |r: f64, phi: f64, h: f64| {
        let (x, y) = (r * phi.cos(), r * phi.sin());
        let e = 0.01;
        let m = mid(x, y);
        let n = unit(cross(sub(mid(x + e, y), mid(x - e, y)), sub(mid(x, y + e), mid(x, y - e))));
        let (t, w) = plan_to_chart(c, hub, axis_deg, [x, y]);
        let out = sub(c.leaf_world(t, w, 1.0), c.leaf_world(t, w, 0.0));
        let n = if dot(n, out) < 0.0 { scale(n, -1.0) } else { n };
        add3(m, scale(n, h))
    };
    // Each column: the top from the hub out, the rim's round, then the floor back in.
    let mut cols: Vec<Vec<P3>> = Vec::new();
    for j in 0..AROUND {
        let phi = -PI + TAU * j as f64 / AROUND as f64;
        let rho = ivy.rho(phi);
        // The margin: a wall square to the band from the floor to the top, its top edge rounded.
        let e = LEAF_EDGE_ROUND_MM;
        let inner = rho - e;
        let mut col = Vec::new();
        for i in 1..=RINGS {
            let r = inner * i as f64 / RINGS as f64;
            col.push(to_world(r, phi, ivy.top(r, phi)));
        }
        let top = ivy.top(inner, phi);
        for k in 1..RIM {
            let a = 0.5 * PI * k as f64 / RIM as f64;
            col.push(to_world(inner + e * a.sin(), phi, top - e + e * a.cos()));
        }
        let floor = ivy.floor(rho, phi);
        col.push(to_world(rho, phi, top - e));
        col.push(to_world(rho, phi, 0.5 * (top - e + floor)));
        col.push(to_world(rho, phi, floor));
        for i in (1..RINGS).rev() {
            let r = rho * i as f64 / RINGS as f64;
            col.push(to_world(r, phi, ivy.floor(r, phi)));
        }
        cols.push(col);
    }
    let mut s = csg::Solid::default();
    s.v.push(to_world(0.0, 0.0, ivy.top(0.0, 0.0)));
    s.v.push(to_world(0.0, 0.0, ivy.floor(0.0, 0.0)));
    let k = cols[0].len();
    for col in &cols {
        s.v.extend(col.iter().copied());
    }
    let at = |j: usize, i: usize| (2 + (j % AROUND) * k + i) as u32;
    for j in 0..AROUND {
        s.f.push([0, at(j, 0), at(j + 1, 0)]);
        for i in 0..k - 1 {
            s.f.push([at(j, i), at(j, i + 1), at(j + 1, i + 1)]);
            s.f.push([at(j, i), at(j + 1, i + 1), at(j + 1, i)]);
        }
        s.f.push([1, at(j + 1, k - 1), at(j, k - 1)]);
    }
    orient_out(&mut s);
    s
}

/// Flip every face when the solid's signed volume is negative.
fn orient_out(s: &mut csg::Solid) {
    let vol: f64 = s.f.iter().map(|f| dot(s.v[f[0] as usize], cross(s.v[f[1] as usize], s.v[f[2] as usize]))).sum();
    if vol < 0.0 {
        for f in &mut s.f {
            f.swap(1, 2);
        }
    }
}

/// Where a petiole leaves the stem for a leaf: the stem's point nearest the line running straight back from the
/// leaf's hub, against its axis, so the petiole runs into the blade without turning.
fn petiole_root(c: &Chart, hub: (f64, f64), axis_deg: f64) -> (f64, f64) {
    let mut best = (f64::MAX, hub);
    let mut t = 0.6;
    while t <= 4.5 {
        let (theta, w) = plan_to_chart(c, hub, axis_deg, [-t, 0.0]);
        let gap = (w - stem_w(c, theta)).abs();
        let on_stem = (theta - STEM_FROM_DEG).rem_euclid(360.0) <= STEM_SPAN_DEG - 6.0;
        if on_stem && gap < best.0 {
            best = (gap, (theta, stem_w(c, theta)));
        }
        t += 0.01;
    }
    assert!(best.0 < 0.05, "the line back from the leaf at {hub:?} misses the stem by {:.2} mm", best.0);
    best.1
}

/// A petiole from the stem to the leaf's hub and on into the blade along its axis, sunk a little into the band.
fn petiole_solid(c: &Chart, hub: (f64, f64), axis_deg: f64) -> csg::Solid {
    let start = petiole_root(c, hub, axis_deg);
    let into = plan_to_chart(c, hub, axis_deg, [0.6, 0.0]);
    let n = 40;
    let line: Vec<(P3, f64)> = (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            // Straight in the chart from the stem's centre to the hub, then on under the blade.
            let (theta, w, h) = if t < 0.8 {
                let u = t / 0.8;
                let ease = u * u * (3.0 - 2.0 * u);
                (start.0 + (hub.0 - start.0) * u, start.1 + (hub.1 - start.1) * u, stem_h() + (PETIOLE_H_MM - stem_h()) * ease)
            } else {
                let u = (t - 0.8) / 0.2;
                (hub.0 + (into.0 - hub.0) * u, hub.1 + (into.1 - hub.1) * u, PETIOLE_H_MM)
            };
            (c.world(theta, w, h), PETIOLE_R_MM)
        })
        .collect();
    tube(&line, 14, true, true)
}

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_hedera".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

/// One leaf as authored: hub `(theta, w)`, axis, plan, and where its petiole leaves the stem.
struct LeafAt {
    name: &'static str,
    hub: (f64, f64),
    axis_deg: f64,
    len: f64,
    curl: f64,
}

fn leaves(c: &Chart) -> Vec<LeafAt> {
    // The four large leaves lie in the bays the stem leaves: where the stem rides one edge, the hub stands just off
    // it and the blade reaches diagonally across the band to the far edge, its near lobes under the stem. Two smaller
    // leaves sit where the stem crosses the shoulders, pointing back toward the palm.
    let _ = c;
    vec![
        LeafAt { name: "Crown leaf, east", hub: (52.0, -0.7), axis_deg: 128.0, len: 6.4, curl: 1.1 },
        LeafAt { name: "Crown leaf, west", hub: (128.0, 0.7), axis_deg: -52.0, len: 6.4, curl: 1.1 },
        LeafAt { name: "Shoulder leaf, east", hub: (-14.0, -0.8), axis_deg: -158.0, len: 5.0, curl: 0.7 },
        LeafAt { name: "Shoulder leaf, west", hub: (194.0, 0.8), axis_deg: 22.0, len: 5.0, curl: 0.7 },
        LeafAt { name: "Low leaf, west", hub: (214.0, -0.6), axis_deg: 62.0, len: 5.2, curl: 0.2 },
        LeafAt { name: "Low leaf, east", hub: (326.0, 0.6), axis_deg: -118.0, len: 5.2, curl: 0.2 },
    ]
}

/// The radial direction at `theta`.
fn up_at(theta: f64) -> [f64; 3] {
    let (s, c) = theta.to_radians().sin_cos();
    [c, s, 0.0]
}

/// The umbel's hub, over the stem where it crosses the crown.
fn umbel_hub(c: &Chart) -> P3 {
    c.world(CROWN_DEG, 0.0, UMBEL_HUB_MM)
}

/// The umbel: seven berries spread from the hub, `(girdle centre, table axis)`, each axis pointing out along its
/// stalk.
fn umbel(c: &Chart) -> Vec<(P3, P3)> {
    let hub = umbel_hub(c);
    let up = up_at(CROWN_DEG);
    let (s, co) = CROWN_DEG.to_radians().sin_cos();
    let round = [-s, co, 0.0];
    let across = [0.0, 0.0, 1.0];
    BERRIES
        .iter()
        .map(|&(polar, psi)| {
            let (polar, psi) = (polar.to_radians(), psi.to_radians());
            let dir = unit(add3(scale(up, polar.cos()), scale(add3(scale(round, psi.cos()), scale(across, psi.sin())), polar.sin())));
            (add3(hub, scale(dir, UMBEL_R_MM)), dir)
        })
        .collect()
}

/// Two unit vectors square to `a` and to each other, `x` cross `y` along `a`.
fn square_to(a: P3) -> (P3, P3) {
    let seed = if a[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let x = unit(cross(seed, a));
    (x, cross(a, x))
}

/// Where a berry's collet stands: the receptacle's face, the bezel's stand-off under the girdle.
fn receptacle_face(g: P3, a: P3) -> P3 {
    sub(g, scale(a, builders::stand_off_mm(builders::BEZEL, spinel())))
}

/// The receptacle: a disc as wide as the collet's foot, square for a fifth of a millimetre under its face, then
/// flaring in like a calyx down to the stalk's neck; revolved about the berry's axis.
fn receptacle(g: P3, a: P3) -> Operation {
    let (x, _) = square_to(a);
    let mut s = ringdesign_core::sketch::Sketch { plane: ringdesign_core::sketch::Workplane { origin: receptacle_face(g, a), x, y: a, ..Default::default() }, ..Default::default() };
    s.name = "Receptacle".into();
    let (r, band, deep, neck) = (RECEPTACLE_R_MM, RECEPTACLE_BAND_MM, RECEPTACLE_DEEP_MM, RECEPTACLE_NECK_MM);
    let mut pts = vec![[0.0, 0.0], [r, 0.0], [r, -band]];
    // The flare: concave, leaving the band near level and meeting the neck near square.
    let n = 10;
    for i in 1..=n {
        let t = i as f64 / n as f64;
        pts.push([neck + (r - neck) * (1.0 - t).powi(2), -band - (deep - band) * t]);
    }
    pts.push([0.0, -deep]);
    let ids: Vec<_> = pts.iter().map(|p| s.point(*p)).collect();
    s.entity(ringdesign_core::sketch::Geometry::Polyline { points: ids, closed: true });
    Operation::Revolve { sketch: s.into(), pivot: [0.0; 3], axis: [0.0, 1.0, 0.0], degrees: 360.0, in_plane: true }
}

/// A berry's stalk from just behind the hub up into its receptacle's neck.
fn pedicel_solid(c: &Chart, g: P3, a: P3) -> csg::Solid {
    let start = sub(umbel_hub(c), scale(a, 0.25));
    let end = sub(receptacle_face(g, a), scale(a, 0.55 * RECEPTACLE_DEEP_MM));
    let n = 24;
    let line: Vec<(P3, f64)> = (0..=n).map(|i| (add3(start, scale(sub(end, start), i as f64 / n as f64)), PEDICEL_R_MM)).collect();
    tube(&line, 18, true, true)
}

/// The peduncle: up from the stem's centre where it crosses the crown to a knob at the hub.
fn peduncle_solid(c: &Chart) -> csg::Solid {
    let n = 12;
    let line: Vec<(P3, f64)> = (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            (c.world(CROWN_DEG, 0.0, stem_h() + (UMBEL_HUB_MM - stem_h()) * t), PEDUNCLE_R_MM)
        })
        .collect();
    tube(&line, 18, true, true)
}

/// The stem's chart path for the rootlet arrays, `[theta, v]`, from `start` degrees on to short of the growing tip.
fn rootlet_path(c: &Chart, start: f64) -> Vec<[f64; 2]> {
    let end = STEM_FROM_DEG + STEM_SPAN_DEG * 0.88;
    let mut out = Vec::new();
    let mut t = start;
    while t <= end {
        out.push([t, c.crest_v + c.sign * stem_w(c, t)]);
        t += 1.0;
    }
    out
}

/// One rootlet standing at the first station of `path`: out from under the stem's side `side` (+1 right of the
/// path, -1 left), leaning `lean` mm along the stem, tapering from the root to a rounded tip pressed into the band.
fn rootlet_solid(c: &Chart, path: &[[f64; 2]], side: f64, lean: f64, reach: f64) -> csg::Solid {
    let w_of = |p: [f64; 2]| c.sign * (p[1] - c.crest_v);
    let (t0, w0) = (path[0][0], w_of(path[0]));
    let o = c.world(t0, w0, 0.0);
    let z = unit(sub(c.world(t0, w0, 1.0), o));
    let y = unit(sub(c.world(path[1][0], w_of(path[1]), 0.0), o));
    let y = unit(sub(y, scale(z, dot(y, z))));
    let x = cross(y, z);
    let n = 12;
    let line: Vec<(P3, f64)> = (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let across = ROOTLET_FROM_MM + reach * t;
            // Lying along the band, half sunk, following its fall across the crown, pressed in at the tip.
            let h = -0.02 - 0.16 * t * t - across * across / 12.0;
            let p = add3(add3(o, scale(x, side * across)), add3(scale(y, lean * t), scale(z, h)));
            (p, ROOTLET_ROOT_R_MM + (ROOTLET_TIP_R_MM - ROOTLET_ROOT_R_MM) * t)
        })
        .collect();
    tube(&line, 10, true, true)
}

fn spinel() -> Gem {
    let mut g = Gem::cabochon(GemCut::Round, BERRY_MM);
    g.preview_tint = Some(SPINEL_TINT);
    g
}

/// The whole ring as authored.
fn author() -> Result<(RingDesign, AlphaLibrary, serde_json::Value)> {
    let mut d = host();
    let c = Chart::of(&d);
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Host band".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Default::default() } })?;
    let free = Component { attach: Attach::Join, stage: Stage::Cast, ..Component::default() };
    let mut next = 2u64;
    let mut add = |doc: &mut Document, name: String, op: Operation| -> Result<u64> {
        let id = next;
        next += 1;
        doc.append(Feature { id, name, enabled: true, operation: op, component: free.clone() })?;
        Ok(id)
    };
    add(&mut doc, "Ivy stem".into(), stored_op(&stem_solid(&c), "stem", json!({"from_deg": STEM_FROM_DEG, "span_deg": STEM_SPAN_DEG, "waves": STEM_WAVES, "radius_mm": STEM_R_MM}))?)?;
    let mut record = Vec::new();
    for l in leaves(&c) {
        let ivy = Ivy::five(l.len);
        if std::env::var("HEDERA_DEBUG").is_ok() {
            let sol = leaf_solid(&c, &ivy, l.hub, l.axis_deg, l.curl);
            let least = sol.v.iter().map(|p| p[0].hypot(p[1])).fold(f64::MAX, f64::min);
            println!("    {}: nearest the axis {least:.3} mm", l.name);
        }
        add(&mut doc, l.name.into(), stored_op(&leaf_solid(&c, &ivy, l.hub, l.axis_deg, l.curl), "ivy_leaf", json!({"len_mm": l.len, "hub": [l.hub.0, l.hub.1], "axis_deg": l.axis_deg, "curl_mm": l.curl}))?)?;
        if std::env::var("HEDERA_DEBUG").is_ok() {
            let p = petiole_solid(&c, l.hub, l.axis_deg);
            println!("    {} petiole: root {:?} crossings {}", l.name, petiole_root(&c, l.hub, l.axis_deg), csg::self_crossings(&p));
        }
        add(&mut doc, format!("{}: petiole", l.name), stored_op(&petiole_solid(&c, l.hub, l.axis_deg), "petiole", json!({"root": petiole_root(&c, l.hub, l.axis_deg)}))?)?;
        record.push(json!({"leaf": l.name, "hub": [l.hub.0, l.hub.1], "axis_deg": l.axis_deg, "len_mm": l.len}));
    }
    add(&mut doc, "Peduncle".into(), stored_op(&peduncle_solid(&c), "peduncle", json!({"radius_mm": PEDUNCLE_R_MM, "hub_mm": UMBEL_HUB_MM}))?)?;
    let berries = umbel(&c);
    let mut receptacles = Vec::new();
    for (k, &(g, a)) in berries.iter().enumerate() {
        add(&mut doc, format!("Berry {}: pedicel", k + 1), stored_op(&pedicel_solid(&c, g, a), "pedicel", json!({"radius_mm": PEDICEL_R_MM}))?)?;
        receptacles.push(add(&mut doc, format!("Berry {}: receptacle", k + 1), receptacle(g, a))?);
    }
    // Each berry stands on its receptacle's face, so its collet reaches down to the receptacle and no further.
    d.cad = Some(doc.clone());
    let bare = cad::evaluate(&d, &AlphaLibrary::builtin(), draft_params())?;
    let mut id = 100u64;
    for (k, (&(g, a), &on)) in berries.iter().zip(&receptacles).enumerate() {
        let part = bare.components.iter().find(|p| p.id == on).ok_or_else(|| anyhow::anyhow!("receptacle #{on} did not build"))?;
        let face = (0..part.body.faces.len() as u32)
            .filter_map(|f| {
                let seat = cad::FaceSeat::on(part, f, Some(receptacle_face(g, a)), builders::stand_off_mm(builders::BEZEL, spinel())).ok()?;
                let sig = seat.face.signature.clone()?;
                (dot(sig.normal, a) > 0.99).then_some(seat)
            })
            .next()
            .ok_or_else(|| anyhow::anyhow!("receptacle #{on} has no face square to its berry"))?;
        let mut stone = cad::stone_on_face(id, spinel(), on, &face);
        stone.name = format!("Berry {}", k + 1);
        doc.append(stone)?;
        doc.append(builders::feature_on(id + 1, &format!("Berry {}: collet", k + 1), builders::BEZEL, id, json!({"wall_mm": COLLET_WALL_MM, "lip": COLLET_LIP})))?;
        id += 2;
    }
    // The rootlets: four arrays along the stem's chart path, two each side, the second pair half a pitch on and
    // leaning the other way, so the fringe never runs as one regular row.
    let per_deg = c.at(0.0).0 * PI / 180.0;
    for (k, (start, lean, reach)) in [(STEM_FROM_DEG + 4.0, 0.25, ROOTLET_REACH_MM), (STEM_FROM_DEG + 4.0 + 0.5 * ROOTLET_PITCH_MM / per_deg, -0.2, 0.8 * ROOTLET_REACH_MM)].into_iter().enumerate() {
        let path = rootlet_path(&c, start);
        let mut sources = Vec::new();
        for side in [1.0, -1.0] {
            let solid = rootlet_solid(&c, &path, side, lean * side, reach);
            sources.push(add(&mut doc, format!("Rootlet {}{}", k + 1, if side > 0.0 { "R" } else { "L" }), stored_op(&solid, "rootlet", json!({"reach_mm": reach, "lean_mm": lean}))?)?);
        }
        let along = cad::pattern::Along { path: cad::pattern::AlongPath::Chart(path.clone()), pitch_mm: Some(ROOTLET_PITCH_MM), ..Default::default() };
        add(&mut doc, format!("Rootlets along the stem, set {}", k + 1), Operation::Pattern { sources: cad::pattern::Sources(sources), kind: cad::pattern::PatternKind::Along(along) })?;
    }
    d.cad = Some(doc);
    let info = json!({"edge_w_mm": [c.edge_w(-1.0), c.edge_w(1.0)], "lip_w_mm": [c.lip_w(-1.0), c.lip_w(1.0)], "leaves": record});
    Ok((d, AlphaLibrary::builtin(), info))
}

// --- Checks ---------------------------------------------------------------------------------------------------

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

/// Metal vertices standing inside each cabochon, 0.03 mm in from its surface.
fn metal_in_stones(d: &RingDesign, built: &mesh::BuildResult) -> Vec<(String, usize)> {
    ringdesign_core::stones::all_stone_frames_built(d, built)
        .into_iter()
        .map(|(st, f)| {
            let (ra, rb, h) = (st.gem.l_mm * 0.5 - 0.03, st.gem.w_mm * 0.5 - 0.03, st.gem.depth_mm() - 0.03);
            let n = built
                .mesh
                .vertices
                .iter()
                .filter(|p| {
                    let q = [p.0 as f64 - f.girdle[0], p.1 as f64 - f.girdle[1], p.2 as f64 - f.girdle[2]];
                    let (x, y, z) = (dot(q, f.long), dot(q, f.short), dot(q, f.normal));
                    z > 0.03 && z < h && (x / ra).powi(2) + (y / rb).powi(2) < 1.0 - (z / h).powi(2)
                })
                .count();
            (st.label, n)
        })
        .collect()
}

/// Connected pieces of a mesh, its vertices welded at a micron.
fn pieces(m: &mesh::Mesh) -> usize {
    let mut index = std::collections::HashMap::new();
    let ids: Vec<usize> = m.vertices.iter().map(|v| {
        let key = [v.0, v.1, v.2].map(|c| (c as f64 * 1e3).round() as i64);
        let n = index.len();
        *index.entry(key).or_insert(n)
    }).collect();
    let mut parent: Vec<usize> = (0..index.len()).collect();
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r { r = p[r]; }
        let mut j = i;
        while p[j] != r { let n = p[j]; p[j] = r; j = n; }
        r
    }
    for f in &m.faces {
        let a = find(&mut parent, ids[f[0] as usize]);
        for k in 1..3 {
            let b = find(&mut parent, ids[f[k] as usize]);
            parent[b] = a;
        }
    }
    (0..parent.len()).filter(|&i| find(&mut parent, i) == i).count()
}

/// Each CAD part's thinnest metal as surface-normal rays find it.
fn walls(built: &mesh::BuildResult) -> Vec<serde_json::Value> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .filter(|c| !c.settings.reference)
        .filter(|c| !matches!(c.made.as_ref().map(|m| m.key.as_str()), Some(cad::pattern::PATTERN)))
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "sampled_min_mm": t.sampled_min_mm, "at": t.point, "rays": t.rays, "below_floor": t.below_limit, "unresolved": t.unresolved})
        })
        .collect()
}

/// The camera for each named view: yaw about the finger's axis, pitch toward it.
const VIEWS: [(&str, f64, f64); 6] = [("hero", 0.5, 1.0), ("face", 0.0, PI * 0.5), ("palm", PI, 1.05), ("side", 0.0, 0.0), ("shoulder", 0.75, 0.6), ("reverse", PI - 0.5, 0.35)];

fn save_rgb(path: &Path, img: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, img, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, fin: &render::Finished, edge: usize, quick: bool) -> Result<()> {
    let parts = fin.parts(render::GOLD);
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
    // The crown framed from above and from the hero's side: the umbel and the two crown leaves, whole metal, never a
    // cropped mesh.
    let crown = umbel_hub(&Chart::of(d));
    render::write_png_framed(out.join("crown-close.png"), &parts, render::yaw_facing(CROWN_DEG), PI * 0.5, render::Framing::new(crown, 7.5), edge)?;
    render::write_png_framed(out.join("crown-hero.png"), &parts, VIEWS[0].1, 0.9, render::Framing::new(crown, 8.0), edge)?;
    if quick {
        return Ok(());
    }
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The umbel close-up: the berries on their stalks, from above and a little ahead.
    render::write_png_framed(out.join("stones.png"), &parts, -0.3, 1.2, render::Framing::new(umbel_hub(&Chart::of(d)), 5.0), edge)?;
    let bare = mesh::try_build(&host(), lib, draft_params())?;
    let (yaw, pitch) = (VIEWS[0].1, VIEWS[0].2);
    let left = render::render_parts_ss(&[render::Part::metal(&bare.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let right = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    let _ = d;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let quick = args.iter().any(|a| a == "--quick");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/hedera"));
    std::fs::create_dir_all(&out)?;
    println!("Hedera");
    let started = std::time::Instant::now();
    let (d, lib, info) = author()?;
    println!("  authored in {:.1} s: {info}", started.elapsed().as_secs_f64());
    let params = if draft || quick { draft_params() } else { export_params() };
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
    let status = feature_status(&built);
    for (n, s) in &status {
        if s != "Ok" {
            println!("    feature {n}: {s}");
        }
    }
    if quick {
        let fin = render::finished_from(&d, &lib, built);
        renders(&out, &d, &lib, &fin, 1000, true)?;
        return Ok(());
    }
    let made = made_parts(&built);
    let (stamped, solids_notes, parts_notes) = (built.solids.stamped, built.solids.notes.clone(), built.parts.notes.clone());
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let wall = walls(&built);
    println!("  field {} thinnest wall {:.2} mm at {:.0} deg", field.verdict.label(), field.thinnest_wall_mm, field.thinnest_wall_theta_deg);
    for w in &wall {
        println!("    wall {}: {:.3} mm", w["part"].as_str().unwrap_or(""), w["sampled_min_mm"].as_f64().unwrap_or(f64::NAN));
    }
    let findings = dfm::findings_in(&d, &lib);
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).iter().map(|(m, _)| pieces(m)).sum::<usize>();
    let in_stones = metal_in_stones(&d, &built);
    let closest = stones.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm));
    let stone_warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    println!("  stones {reported} reported, {previewed} previewed; metal inside {:?}; closest {closest:?}; warnings {stone_warnings:?}", in_stones);
    let coarse = mesh::try_build(&d, &lib, coarse_params())?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}");
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
    // Body parts (stem, leaves, petioles, stalks) hold the 0.8 mm section; settings (collets and the receptacles
    // under them) and the rootlets are details, held at the 0.15 mm detail floor as Rubus's prickles were.
    let detail = |name: &str| name.contains("collet") || name.contains("receptacle") || name.starts_with("Rootlet");
    let walls_ok = wall.iter().all(|w| {
        let name = w["part"].as_str().unwrap_or("");
        w["sampled_min_mm"].as_f64().is_some_and(|m| m >= if detail(name) { MIN_DETAIL_MM } else { MIN_SECTION_MM })
    });
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings", made.iter().all(|(_, n)| *n == 0)),
        ("solids and parts notes empty, every stamp resolved, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && stamped == d.stamps.len() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax verdict Castable with the 0.8 mm section", field.process == castability::CastProcess::LostWax && field.verdict == castability::Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("every body part's ray-sampled wall at least 0.8 mm; settings and rootlets at least the 0.15 mm detail floor", walls_ok),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview, no metal inside a stone", reported == previewed && reported == 7 && in_stones.iter().all(|(_, n)| *n == 0)),
        ("gates hold at 384 x 192", coarse_ok),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", triangles <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "draft": serde_json::to_value(&d.draft)?,
        "alloy_for_weight": "Gold 18k",
        "grams_18k": grams,
        "size": d.size.display(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": triangles, "build_s": build_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings},
        "made_parts": made,
        "features": status,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "notes": field.notes},
        "ray_walls": wall,
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stamps": {"count": d.stamps.len(), "resolved": stamped},
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "stones": {"reported": reported, "previewed": previewed, "metal_inside": in_stones, "closest": closest, "warnings": stone_warnings, "carats": stones.as_ref().map_or(0.0, |s| s.total_carats)},
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "stamped": coarse.solids.stamped},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "design": {"bytes": text.len(), "format_version": design_format, "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "authoring": info,
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    if draft {
        std::fs::write(out.join("draft-gates.json"), serde_json::to_vec_pretty(&report)?)?;
    }
    let report = if draft {
        report
    } else {
        let mut r = report;
        if let Ok(old) = std::fs::read_to_string(out.join("draft-gates.json")) {
            r["draft_build"] = serde_json::from_str(&old)?;
        }
        r
    };
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let fin = render::finished_from(&d, &lib, built);
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &fin.metal, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Hedera / investment pattern")?;
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-spinel.stl".to_string() } else { format!("reference-spinel-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Hedera reference stone")?;
            materials.push(json!({"mesh": file, "name": "Black spinel", "tint": tint, "ior": 1.72, "dispersion": 0.02, "roughness": 0.05, "transmission": 0.0}));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": materials}))?)?;
    }
    renders(&out, &d, &lib, &fin, if draft { 1000 } else { 1600 }, false)?;
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Hedera failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
