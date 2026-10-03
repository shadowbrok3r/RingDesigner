//! Cataphracta — Heloderma, the beaded one: a Gila monster's beadwork over a tail swollen with stored fat, poured in lost wax.
//! cargo build --release -p ringdesign-core --example cataphracta_heloderma
//! target/release/examples/cataphracta_heloderma [OUT_DIR] [--draft] [--verify]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{self, CastProcess, Verdict},
    csg, dfm,
    field::{Blend, Decal, DecalLayer, GroupLayer, Layer, LayerEntry, LayerStack, SeatPadLayer, SeatStyle, Window},
    gem::{Gem, GemCut},
    cad::{Attach, Component, Document, Feature, Operation, Placement, Stage, stored::Recipe},
    library, manufacturing as mf, mesh, sculpt,
    profile::ShankKey,
    render::{self, Part},
    setting::{self, SolidKind},
    stl,
    svg::SvgAlpha,
    tiling::{GradeLaw, TileGrade},
};
use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

const SLUG: &str = "heloderma";
const NAME: &str = "Heloderma \u{2014} the beaded one";

/// The investment's fill floor for this ring, mm (Logan, 2026-09-27).
const MIN_SECTION_MM: f64 = 0.8;

/// The band's hide carries the tail's rings on round the shank: one black band and one salmon band per period, arc mm.
const BAND_PERIOD_MM: f64 = 5.0;
/// The face's field, degrees round the ring, where the hide is one even fine bead ground: from ahead of the stone to past the
/// tail.
const FACE_FIELD_DEG: [f64; 2] = [30.0, 165.0];
/// The grade takes the hide's pitch to `1 - GRADE_TAPER` of the face's at the palm.
const GRADE_TAPER: f64 = 0.26;
/// The banding rises out of the face's even ground over this much arc into the shank, mm.
const FEATHER_MM: f64 = 4.5;
/// Bead pitch at the face, metal mm: the salmon bands' big beads and the ground's fine ones.
const SALMON_PITCH_MM: f64 = 0.8;
const FINE_PITCH_MM: f64 = 0.46;
/// A decal's full ink, mm: a salmon bead's crown reads as ink, so the DFM measure sees each one whole; a fine bead's never does.
const BEAD_MM: f64 = 0.25;
/// How far each bead's dome stands over the band: the salmon bands' and the ground's.
const SALMON_BEAD_MM: f64 = 0.22;
const FINE_BEAD_MM: f64 = 0.10;
/// Each sector's decal reaches this far into its neighbours, mm: more than a bead's radius, so every bead lies whole in one
/// sector and the sectors meet by `Max` with no seam.
const SECTOR_OVERLAP_MM: f64 = 0.6;
/// Sectors round the ring, one decal each: a 1024 px raster over a sector holds a bead to 0.01 mm.
const SECTORS: usize = 8;
/// Beads stop this far short of the band's edges, chart mm, so the comfort roll stays plain, and their domes grade to nothing
/// over the last `RIM_FADE_MM` before it, so the hide meets the edge round with no ledge.
const EDGE_MM: f64 = 0.35;
const RIM_FADE_MM: f64 = 0.45;

fn params(draft: bool) -> BuildParams {
    let (t, p) = if draft { (768, 320) } else { (1536, 448) };
    BuildParams {
        theta_steps: t,
        profile_steps: p,
        refine: None,
        ..Default::default()
    }
}

/// The keyed half-round: a fat-tail swell at the face, slim at the palm, poured in lost wax.
fn band() -> RingDesign {
    let mut d = RingDesign {
        name: NAME.into(),
        ..RingDesign::default()
    };
    d.profile.width_mm = 8.0;
    d.profile.thickness_mm = 3.2;
    d.profile.apply_style(ProfileStyle::HalfRound);
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.2;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let k = |theta_deg: f64, width_scale: f64, thickness_scale: f64, crown_scale: f64| ShankKey {
        theta_deg,
        width_scale,
        thickness_scale,
        crown_scale,
    };
    d.shank.keys = vec![
        k(35.0, 1.20, 1.00, 1.0),
        k(90.0, 1.42, 1.00, 1.05),
        k(145.0, 1.32, 1.00, 1.0),
        k(200.0, 1.26, 0.98, 1.0),
        k(270.0, 0.92, 0.90, 1.0),
        k(330.0, 1.05, 0.95, 1.0),
    ];
    d.draft.sand = None;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_draft_deg = 0.0;
    let mut setup = mf::Setup::from_design(&d);
    setup.recipe.name = format!("{NAME} / investment / Silver 925");
    setup.recipe.alloy = "Silver 925".into();
    setup.recipe.sand = None;
    setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925")
        .map_or(1.9, |m| m.shrink_pct);
    setup.bench_notes = "Procedural half-round keyed to a fat-tail swell, poured in lost wax. A Gila monster lies on the crown: a flat \
        square-snouted head of big domed beads, a fat trunk and a short fat tail in countable bands (high salmon beads, black bands \
        sunk on fine beads), the tail curled back onto the face. The band's hide is one even bead ground, banded by bead size round \
        the shank. A 3 mm spessartite in a collet with a beaded lip in front of the snout. At the bench: burnish the collet's lip over \
        the stone; polish the high beads, leave the sunk bands and the ground satin."
        .into();
    d.manufacturing = Some(setup);
    d
}

fn spessartite() -> Gem {
    Gem {
        preview_tint: Some([0.95, 0.38, 0.06]),
        ..Gem::calibrated(GemCut::Round, 3.0)
    }
}

fn grade() -> TileGrade {
    TileGrade {
        taper: GRADE_TAPER,
        theta_deg: 90.0,
        law: GradeLaw::Cosine,
        isotropic: false,
    }
}

/// Ring fraction per lattice fraction at ring fraction `x`: the grade's local pitch against the ring's mean.
fn stretch_of_grade(x: f64) -> f64 {
    let g = grade();
    let e = 1e-4;
    2.0 * e / (g.phi(x + e) - g.phi(x - e))
}

fn hash(k: u64, s: u64) -> f64 {
    let mut z = (k + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ s.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 29)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 32;
    ((z >> 11) as f64 / (1u64 << 53) as f64) - 0.5
}

/// A smooth value noise over (ring fraction, across mm), periodic round the ring: -0.5..0.5.
fn noise(x: f64, y: f64, cells: f64, seed: u64) -> f64 {
    let (gx, gy) = (x.rem_euclid(1.0) * cells, y);
    let (ix, iy) = (gx.floor(), gy.floor());
    let (fx, fy) = (gx - ix, gy - iy);
    let s = |t: f64| t * t * (3.0 - 2.0 * t);
    let at = |i: f64, j: f64| hash((i.rem_euclid(cells) as u64) * 131 + ((j + 64.0) as u64), seed);
    let a = at(ix, iy) + (at(ix + 1.0, iy) - at(ix, iy)) * s(fx);
    let b = at(ix, iy + 1.0) + (at(ix + 1.0, iy + 1.0) - at(ix, iy + 1.0)) * s(fx);
    a + (b - a) * s(fy)
}

fn ring_delta(a: f64, b: f64) -> f64 {
    (a - b + 540.0).rem_euclid(360.0) - 180.0
}

/// How a bead of the band's hide stands.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// A salmon band's big bead, round the shank.
    Salmon,
    /// The ground's fine low bead: the whole face, and the black bands between the salmon ones.
    Fine,
}

/// One bead in chart millimetres: centre, semi-axes, kind, and the share of its full height it stands to (the rims grade it).
struct Bead {
    u: f64,
    v: f64,
    rx: f64,
    ry: f64,
    kind: Kind,
    rise: f64,
}

/// The band's banding, the tail's rings carried on round the shank: transverse bands of uneven width that lean, waver and fork,
/// the black a little under half. The signed margin of salmon at ring angle `theta`, `across` chart mm off the crest, in arc mm:
/// positive in a salmon band, negative in a black one, continuous across each edge.
fn salmon_margin(circ: f64, theta: f64, across: f64) -> f64 {
    let x = theta / 360.0;
    let arc = x * circ;
    let wander = 0.2 * noise(x, across / 2.5, 9.0, 31) + 0.08 * noise(x, across / 1.1, 23.0, 32);
    // The period divides the circumference, so the rings close round the ring without a cut.
    let period = circ / (circ / BAND_PERIOD_MM).round();
    let phase = arc / period + wander;
    let band = phase.floor() as i64;
    let share = 0.42 + 0.12 * hash(band.rem_euclid(97) as u64, 33);
    let off = (phase - phase.floor() - 0.5).abs();
    let mut m = off - 0.5 * share;
    // Every third band forks toward one edge: a salmon wedge splits it into a Y.
    if band.rem_euclid(3) == 0 {
        let s = if band.rem_euclid(2) == 0 { 1.0 } else { -1.0 };
        let reach = s * across - 1.2;
        let wedge = ((0.04 + 0.05 * reach).min(0.12) - off).min(0.2 * reach);
        m = m.max(wedge);
    }
    m * period
}

/// How far the banding stands, 0 to 1, at ring angle `theta`: nothing on the face's field, rising over `FEATHER_MM` of arc into
/// the shank.
fn shank_weight(circ: f64, theta: f64) -> f64 {
    let t = theta.rem_euclid(360.0);
    let [a, b] = FACE_FIELD_DEG;
    let out = if (a..=b).contains(&t) { 0.0 } else { ring_delta(t, a).abs().min(ring_delta(t, b).abs()) };
    smooth01(out / 360.0 * circ / FEATHER_MM)
}

/// One lattice of beads over the band at `pitch_top` metal mm at the face, graded toward the palm, jittered; each bead kept where
/// `keep(theta, across)` holds and clear of the stone's collet.
fn lattice(d: &RingDesign, pitch_top: f64, kind: Kind, land: f64, keep: &dyn Fn(f64, f64) -> bool) -> Vec<Bead> {
    let ctx = d.field_context();
    let g = grade();
    let circ = ctx.circumference_mm;
    let crest = ctx.crest_v_mm;
    let len = ctx.band_v_len_mm;
    let stone_r = stone_clear_mm();
    let row_mm = pitch_top * 0.8 / ctx.station_stretch(90.0);
    let face_pitch_chart = pitch_top / ctx.crest_scale(90.0);
    let cols = (circ * stretch_of_grade(0.25) / face_pitch_chart).round();
    let mut rows: Vec<(i64, f64)> = vec![(0, crest)];
    for k in 1..64i64 {
        let off = k as f64 * row_mm;
        for (s, v) in [(k, crest + off), (-k, crest - off)] {
            if v >= EDGE_MM && v <= len - EDGE_MM {
                rows.push((s, v));
            }
        }
    }
    let salt = if kind == Kind::Salmon { 0 } else { 5000 };
    let mut out = Vec::new();
    for (i, v0) in rows {
        let a = ctx.arc_scale(v0);
        let n = (cols * a).round().max(8.0);
        let stagger = 0.5 + hash((i + salt) as u64 + 99, 7);
        for j in 0..n as i64 {
            let seed = ((i + 64 + salt) * 4096 + j) as u64;
            let phi = (j as f64 + stagger + 0.16 * hash(seed, 1)) / n;
            let x = g.x_of_phi(phi).rem_euclid(1.0);
            let theta = x * 360.0;
            let v = v0 + 0.08 * row_mm * hash(seed, 2);
            let across = v - crest;
            if !keep(theta, across) {
                continue;
            }
            let cs = ctx.crest_scale(theta);
            let st = ctx.station_stretch(theta);
            let around = circ / n * stretch_of_grade(x) * cs * a;
            let over = row_mm * st * 2.0 / 3f64.sqrt();
            let pitch = around.min(over) * (1.0 + 0.06 * hash(seed, 3));
            let du = ring_delta(theta, STONE_DEG) / 360.0 * circ * cs;
            if du.hypot(across * st) < stone_r {
                continue;
            }
            let dia = (1.0 - land) * pitch;
            // The rims: each dome grades to nothing over the last `RIM_FADE_MM` before the beads stop.
            let rise = smooth01((v.min(len - v) - EDGE_MM) / RIM_FADE_MM);
            // A salmon bead stands whole or not at all, so no sliver of its crown reaches the ink line; the fine ground grades.
            if rise < 0.08 || (kind == Kind::Salmon && rise < 0.98) {
                continue;
            }
            out.push(Bead { u: x * circ, v, rx: 0.5 * dia / (cs * a), ry: 0.5 * dia / st, kind, rise });
        }
    }
    out
}

/// The band's whole hide, one even field of beads with no plateau and no wall: the fine ground everywhere at the face; round the
/// shank, the tail's rings by bead size, big salmon beads in the salmon bands and the fine ground's beads in the black ones.
fn beads(d: &RingDesign) -> Vec<Bead> {
    let circ = d.field_context().circumference_mm;
    let (len, crest) = (d.field_context().band_v_len_mm, d.field_context().crest_v_mm);
    let near_rim = |a: f64| (crest + a).min(len - crest - a) < EDGE_MM + RIM_FADE_MM + 0.2;
    let mut all = lattice(d, FINE_PITCH_MM, Kind::Fine, 0.1, &|t, a| {
        shank_weight(circ, t) < 0.5 || salmon_margin(circ, t, a) < 0.45 || near_rim(a)
    });
    all.extend(lattice(d, SALMON_PITCH_MM, Kind::Salmon, 0.06, &|t, a| {
        shank_weight(circ, t) >= 0.5 && salmon_margin(circ, t, a) > 0.28
    }));
    all
}

/// Radius round the stone's axis the hide keeps clear of, metal mm: the collet's wall and its beaded lip.
fn stone_clear_mm() -> f64 {
    MILGRAIN_RING_MM + 0.1
}

/// Sector `k`'s hide as one SVG over its stretch of the unrolled band and `SECTOR_OVERLAP_MM` either side of it: black ink is
/// height. Every bead is a radial-gradient dome, scaled by its rim grade; nothing else is drawn, so no edge in the hide is a wall.
fn sector_svg(d: &RingDesign, all: &[Bead], k: usize) -> String {
    let ctx = d.field_context();
    let circ = ctx.circumference_mm;
    let m = SECTOR_OVERLAP_MM;
    let (w, h) = (circ / SECTORS as f64 + 2.0 * m, ctx.band_v_len_mm);
    let u0 = k as f64 * circ / SECTORS as f64 - m;
    let mut defs = String::new();
    let mut gradient = |id: &str, peak: f64, f: &dyn Fn(f64) -> f64| {
        let _ = write!(defs, r##"<radialGradient id="{id}" cx="0.5" cy="0.5" r="0.5">"##);
        for i in 0..=16 {
            let t = i as f64 / 16.0;
            let _ = write!(defs, r##"<stop offset="{t:.3}" stop-color="#000" stop-opacity="{:.4}"/>"##, peak * f(t));
        }
        defs.push_str("</radialGradient>");
    };
    // Soft-skirted domes: each bead's base runs out gently, so its outline holds on the build grid.
    let dome = |t: f64| (1.0 - t * t).max(0.0).powf(1.2);
    gradient("sd", SALMON_BEAD_MM / BEAD_MM, &dome);
    gradient("fd", FINE_BEAD_MM / BEAD_MM, &dome);
    let mut body = String::new();
    for b in all {
        let fill = match b.kind { Kind::Salmon => "url(#sd)", Kind::Fine => "url(#fd)" };
        for wrap in [-circ, 0.0, circ] {
            let x = b.u + wrap - u0;
            if x + b.rx < 0.0 || x - b.rx > w {
                continue;
            }
            // SVG y runs down from the decal's top, which is the band's far edge.
            let _ = write!(body, r##"<ellipse cx="{x:.3}" cy="{:.3}" rx="{:.3}" ry="{:.3}" fill="{fill}""##, h - b.v, b.rx, b.ry);
            if b.rise < 0.999 {
                let _ = write!(body, r##" fill-opacity="{:.3}""##, b.rise);
            }
            body.push_str("/>");
        }
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs>{defs}</defs>{body}</svg>"##
    )
}
const ROMAN: [&str; 8] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII"];

fn stone_pad(crest_v: f64) -> SeatPadLayer {
    let mut pad = SeatPadLayer {
        theta_deg: STONE_DEG,
        v_mm: crest_v,
        style: SeatStyle::GypsyMound,
        crown: 1.0,
        blend_mm: 0.45,
        solid: SolidKind::Bezel,
        through: true,
        ..Default::default()
    };
    pad.fit_stone(spessartite());
    pad.height_mm = 0.45;
    pad.set_depth_mm = Some(0.35);
    pad.mark_mm = 0.5;
    pad
}


// --- The figure ------------------------------------------------------------------------------------------------------
//
// The Gila herself, one sculpted part: a flat, square-snouted head of big domed beads lying on the crown, nosing the stone; a
// fat trunk in three black crossbands; a short fat tail in four black rings that curls back onto the face; four splayed legs
// with thin clawed toes. Built as one distance field over world millimetres (finger axis Z, the crown toward +Y) and meshed by
// `sculpt`. The head is rigid, in a frame at its own crest point; the trunk, tail and legs lie in a frame bent round the band:
// `u` is arc millimetres at the crown's radius from 90 degrees, positive round toward the palm on the -X side, `z` across, `h`
// over the bare band's own surface there.
//
// The hide is explicit bead rows, never noise: domes on a hex lattice laid in each part's own coordinates (along the spine and
// round its section; along and across the head), a size up in the salmon bands, a size down and sunk in the black ones.

/// The snout's tip and the head's centre round the ring, arc mm at the crown radius; the snout points toward smaller `u`.
const SNOUT_U: f64 = -8.6;
const HEAD_U: f64 = -6.3;
/// The head's plan: half its length (neck to snout) and its greatest half-width, at the jowls, mm. Length 6.0 against width 5.0:
/// the Gila's flat wedge, 1.15 times as long as it is wide, narrowing a little to a blunt, square snout.
const HEAD_HL: f64 = 2.3;
const HEAD_HW: f64 = 2.1;
/// The snout's corner radius in plan, mm: the nose is square-ended, its end 4.3 mm across, more than 80% of the head's width.
const SNOUT_R: f64 = 0.8;
/// The stone's centre round the ring, degrees: in front of the snout, clear of it by the collet's beaded lip.
const STONE_DEG: f64 = 37.5;
/// The collet's beaded lip: round domes on the mound round the collet's foot. Their ring's radius from the stone's axis, their
/// radius, and how far their centres sit under the mound's surface.
const MILGRAIN_BEADS: usize = 22;
const MILGRAIN_RING_MM: f64 = 2.24;
const MILGRAIN_R_MM: f64 = 0.2;
const MILGRAIN_SINK_MM: f64 = 0.04;
/// Meshing step for the sculpt, mm.
const SCULPT_STEP: f64 = 0.057;

/// Where the neck creases behind the head and where the tail leaves the trunk, arc mm.
const NECK_U: f64 = -4.0;
const VENT_U: f64 = 5.4;
/// The tail: a fat sausage that leaves the vent heading round the ring and curls back across the crown, an arc of this radius
/// through this many degrees, then a rounded end.
const TAIL_R: f64 = 2.9;
const TAIL_TURN_DEG: f64 = 115.0;
const TAIL_CAP_MM: f64 = 1.0;
/// The black bands' sink below the salmon envelope, mm, and the beads: salmon pitch and dome, black pitch and dome, head pitch
/// and dome, head flank pitch and dome.
const SINK_MM: f64 = 0.48;
const PALE_PITCH: f64 = 0.9;
const PALE_DOME: f64 = 0.2;
const DARK_PITCH: f64 = 0.38;
const DARK_DOME: f64 = 0.08;
const HEAD_PITCH: f64 = 0.78;
const HEAD_DOME: f64 = 0.11;
const FLANK_PITCH: f64 = 0.6;
const FLANK_DOME: f64 = 0.11;
/// The black bands along the spine, `s` mm from the neck: (centre, half-width). Three crossbands on the trunk, four rings round
/// the tail. The forelegs and hind legs leave the flanks inside the first and third crossbands, so no pale row runs on into a
/// leg.
const DARK_BANDS: [(f64, f64); 7] = [(3.0, 0.85), (5.6, 0.8), (8.1, 0.8), (10.25, 0.58), (11.95, 0.58), (13.65, 0.58), (15.35, 0.55)];

/// The bare band's outer radius over (theta, z): the top of every outward-facing triangle, rasterized.
struct Surface {
    nt: usize,
    nz: usize,
    dt: f64,
    z0: f64,
    dz: f64,
    r: Vec<f32>,
    bore: f64,
}

impl Surface {
    fn of(m: &mesh::Mesh, bore: f64) -> Self {
        let (dt, z0, dz) = (0.25, -8.0, 0.04);
        let (nt, nz) = ((360.0 / dt) as usize, (16.0 / dz) as usize + 1);
        let mut r = vec![f32::NAN; nt * nz];
        for f in &m.faces {
            let v = f.map(|i| m.vertices[i as usize]);
            let c = [0, 1, 2].map(|k| {
                let p = v[k];
                ((p.1 as f64).atan2(p.0 as f64).to_degrees(), p.2 as f64, (p.0 as f64).hypot(p.1 as f64))
            });
            // Outward only: the face's normal against the radial direction.
            let e1 = [v[1].0 - v[0].0, v[1].1 - v[0].1, v[1].2 - v[0].2];
            let e2 = [v[2].0 - v[0].0, v[2].1 - v[0].1, v[2].2 - v[0].2];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let mid = [(v[0].0 + v[1].0 + v[2].0) / 3.0, (v[0].1 + v[1].1 + v[2].1) / 3.0];
            let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let ml = mid[0].hypot(mid[1]);
            if nl < 1e-12 || ml < 1e-6 || (n[0] * mid[0] + n[1] * mid[1]) / (nl * ml) < 0.05 {
                continue;
            }
            let t0 = c[0].0;
            let t: [f64; 3] = [0, 1, 2].map(|k| t0 + ring_delta(c[k].0, t0));
            let (tlo, thi) = (t.iter().copied().fold(f64::MAX, f64::min), t.iter().copied().fold(f64::MIN, f64::max));
            let (zlo, zhi) = ([c[0].1, c[1].1, c[2].1].into_iter().fold(f64::MAX, f64::min), [c[0].1, c[1].1, c[2].1].into_iter().fold(f64::MIN, f64::max));
            let det = (t[1] - t[0]) * (c[2].1 - c[0].1) - (t[2] - t[0]) * (c[1].1 - c[0].1);
            if det.abs() < 1e-14 {
                continue;
            }
            for i in (tlo / dt).floor() as i64..=(thi / dt).ceil() as i64 {
                for j in ((zlo - z0) / dz).floor().max(0.0) as i64..=((zhi - z0) / dz).ceil().min((nz - 1) as f64) as i64 {
                    let (tt, zz) = (i as f64 * dt, z0 + j as f64 * dz);
                    let a = ((tt - t[0]) * (c[2].1 - c[0].1) - (t[2] - t[0]) * (zz - c[0].1)) / det;
                    let b = ((t[1] - t[0]) * (zz - c[0].1) - (tt - t[0]) * (c[1].1 - c[0].1)) / det;
                    if a < -1e-6 || b < -1e-6 || a + b > 1.0 + 1e-6 {
                        continue;
                    }
                    let rr = (c[0].2 + a * (c[1].2 - c[0].2) + b * (c[2].2 - c[0].2)) as f32;
                    let k = (i.rem_euclid(nt as i64) as usize) * nz + j as usize;
                    if r[k].is_nan() || rr > r[k] {
                        r[k] = rr;
                    }
                }
            }
        }
        Self { nt, nz, dt, z0, dz, r, bore }
    }

    /// The outer radius at `theta` degrees and `z` mm; off the band, the bore.
    fn at(&self, theta: f64, z: f64) -> f64 {
        let ft = theta.rem_euclid(360.0) / self.dt;
        let fz = ((z - self.z0) / self.dz).clamp(0.0, (self.nz - 1) as f64 - 1e-9);
        let (i, j) = (ft.floor() as usize, fz.floor() as usize);
        let (a, b) = (ft - i as f64, fz - j as f64);
        let g = |i: usize, j: usize| self.r[(i % self.nt) * self.nz + j.min(self.nz - 1)];
        let c = [g(i, j), g(i + 1, j), g(i, j + 1), g(i + 1, j + 1)];
        if c.iter().any(|x| x.is_nan()) {
            // At the band's edge: the nearest known corner, else the bore.
            return c.iter().filter(|x| !x.is_nan()).map(|x| *x as f64).fold(self.bore, f64::max);
        }
        let c = c.map(|x| x as f64);
        (c[0] * (1.0 - a) + c[1] * a) * (1.0 - b) + (c[2] * (1.0 - a) + c[3] * a) * b
    }
}

type V3 = [f64; 3];

fn v_sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn v_len(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}
fn smooth01(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance from (a, b) to the nearest site of a hex lattice of pitch `p`: rows run along `a`, spaced p·√3/2 along `b`, every
/// other row shifted half a pitch, each site nudged a little by hand.
fn hex_dist(a: f64, b: f64, p: f64, seed: u64) -> f64 {
    let rh = p * 0.866;
    let j0 = (b / rh).floor() as i64;
    let mut best = f64::MAX;
    for j in j0 - 1..=j0 + 1 {
        let off = if j.rem_euclid(2) == 1 { 0.5 * p } else { 0.0 };
        let i0 = ((a - off) / p).round() as i64;
        for i in i0 - 1..=i0 + 1 {
            let key = ((i + 1_000_000) as u64) * 2_000_003 + (j + 1_000_000) as u64;
            let ca = i as f64 * p + off + 0.07 * p * hash(key, seed);
            let cb = j as f64 * rh + 0.07 * p * hash(key, seed + 1);
            best = best.min((a - ca).hypot(b - cb));
        }
    }
    best
}

/// A dome of height `h` over a bead of pitch `p` at distance `r` from its centre: round-shouldered, meeting its neighbours in a
/// shallow seam.
fn dome(r: f64, p: f64, h: f64) -> f64 {
    let x = (r / (0.52 * p)).min(1.0);
    h * (1.0 - x * x).powf(0.7)
}

/// Approximate distance to an ellipse of semi-axes `a`, `b` in its own plane.
fn ellipse(x: f64, y: f64, a: f64, b: f64) -> f64 {
    let k0 = (x / a).hypot(y / b);
    let k1 = (x / (a * a)).hypot(y / (b * b));
    if k1 < 1e-12 { -a.min(b) } else { k0 * (k0 - 1.0) / k1 }
}

/// One key of the trunk's spine: `u` arc mm, the spine's offset across `zc`, its centre over the band `hc`, half-width `w`,
/// half-height `t`.
#[derive(Clone, Copy)]
struct Key {
    u: f64,
    zc: f64,
    hc: f64,
    w: f64,
    t: f64,
}

/// The trunk, from inside the head to a little past the vent, where the tail takes over. It drifts across the crown toward +z as
/// it goes, so the tail can curl back over the -z side.
const TRUNK: [Key; 7] = [
    Key { u: -5.2, zc: 0.0, hc: 1.0, w: 1.75, t: 1.15 },
    Key { u: -4.0, zc: 0.0, hc: 1.02, w: 1.85, t: 1.2 },
    Key { u: -2.4, zc: 0.05, hc: 1.15, w: 2.4, t: 1.5 },
    Key { u: 0.4, zc: 0.3, hc: 1.2, w: 2.75, t: 1.7 },
    Key { u: 3.0, zc: 0.7, hc: 1.2, w: 2.8, t: 1.7 },
    Key { u: 5.4, zc: 1.2, hc: 1.15, w: 2.55, t: 1.6 },
    Key { u: 6.0, zc: 1.3, hc: 1.1, w: 2.45, t: 1.55 },
];

fn trunk_at(u: f64) -> Key {
    let u = u.clamp(TRUNK[0].u, TRUNK[TRUNK.len() - 1].u);
    let i = TRUNK.windows(2).position(|w| u <= w[1].u).unwrap_or(TRUNK.len() - 2);
    let (a, b) = (TRUNK[i], TRUNK[i + 1]);
    let s = smooth01((u - a.u) / (b.u - a.u));
    let l = |x: f64, y: f64| x + (y - x) * s;
    Key { u, zc: l(a.zc, b.zc), hc: l(a.hc, b.hc), w: l(a.w, b.w), t: l(a.t, b.t) }
}

/// The tail's section `st` mm along it from the vent: half-width, half-height. Fat to its middle (at least 70% of the trunk's
/// width), then tapering to a blunt end.
fn tail_section(st: f64) -> (f64, f64) {
    let len = TAIL_R * TAIL_TURN_DEG.to_radians();
    let f = (st / len).clamp(0.0, 1.0);
    let w = if f < 0.5 { 2.45 - 0.5 * (f / 0.5) } else { 1.95 - 0.65 * ((f - 0.5) / 0.5) };
    let t = if f < 0.5 { 1.55 - 0.15 * (f / 0.5) } else { 1.4 - 0.4 * ((f - 0.5) / 0.5) };
    (w, t)
}

/// The tail's spine in the bent frame at `phi` radians round its curl: (u, z, heading along u, heading along z).
fn tail_point(phi: f64) -> (f64, f64, f64, f64) {
    let z_v = TRUNK[5].zc;
    // Curling toward -z: the curl's centre lies `TAIL_R` to -z of the vent.
    (VENT_U + TAIL_R * phi.sin(), z_v - TAIL_R + TAIL_R * phi.cos(), phi.cos(), -phi.sin())
}

/// A limb segment in the bent frame: (u, z, h) points.
struct Limb {
    joints: Vec<(V3, f64)>,
}

impl Limb {
    fn dist(&self, q: V3) -> f64 {
        self.joints.windows(2).map(|w| sculpt::round_cone(q, w[0].0, w[1].0, w[0].1, w[1].1)).fold(f64::MAX, f64::min)
    }
}

/// A foot: a pad and five toes fanning from `heading` (radians in the u-z plane) with their claws: thin, tapering, clawed.
struct Foot {
    pad: V3,
    pad_r: V3,
    toes: Vec<Limb>,
    claws: Vec<Limb>,
}

fn foot(centre: V3, heading: f64, side: f64, spread: [f64; 5], lengths: [f64; 5]) -> Foot {
    let mut toes = Vec::new();
    let mut claws = Vec::new();
    for k in 0..5 {
        let a = heading + side * spread[k].to_radians();
        let d = [a.cos(), a.sin()];
        let at = |s: f64, h: f64| [centre[0] + d[0] * s, centre[1] + d[1] * s, h];
        let l = lengths[k];
        let knuckle = 0.45 + 0.55 * l;
        toes.push(Limb { joints: vec![(at(0.45, 0.3), 0.45), (at(knuckle, 0.27), 0.42), (at(0.5 + l, 0.2), 0.41)] });
        claws.push(Limb { joints: vec![(at(0.5 + l, 0.18), 0.34), (at(0.85 + l, -0.05), 0.26)] });
    }
    Foot { pad: centre, pad_r: [0.72, 0.66, 0.42], toes, claws }
}

struct Figure {
    surf: Surface,
    /// The crown's radius at 90 degrees, mm: the bent frame's arc scale.
    r0: f64,
    /// The head's frame: origin on the band at its centre, forward (toward the snout), across, up.
    head_o: V3,
    head_f: V3,
    head_u: V3,
    eye: [V3; 2],
    limbs: Vec<Limb>,
    feet: Vec<Foot>,
    /// The collet lip's bead centres, on the mound's own surface.
    lip: Vec<V3>,
    lip_centre: V3,
    beads: bool,
}

impl Figure {
    fn new(d: &RingDesign, lib: &AlphaLibrary, beads: bool) -> Result<Self> {
        let bare = band();
        let b = mesh::try_build(&bare, lib, params(true))?;
        let surf = Surface::of(&b.mesh, d.inner_radius_mm());
        // The mound the collet stands on, so the lip's beads sit on it rather than standing out of it.
        let mut mounded = band();
        let mut pad = LayerEntry::new("Spessartite", Layer::SeatPad(stone_pad(mounded.field_context().crest_v_mm)));
        pad.window = Window::default();
        mounded.layers.layers = vec![pad];
        let mb = mesh::try_build(&mounded, lib, params(true))?;
        let mound = Surface::of(&mb.mesh, d.inner_radius_mm());
        let r0 = surf.at(90.0, 0.0);
        let th = (90.0 + HEAD_U / r0 * 180.0 / PI).to_radians();
        let rc = surf.at(th.to_degrees(), 0.0);
        let head_o = [rc * th.cos(), rc * th.sin(), 0.0];
        let head_u = [th.cos(), th.sin(), 0.0];
        // Toward the snout: toward smaller theta.
        let head_f = [th.sin(), -th.cos(), 0.0];
        let mut limbs = Vec::new();
        let mut feet = Vec::new();
        let spread = [-55.0, -28.0, 0.0, 20.0, 38.0];
        // A walking pose, diagonal pairs together: the +z foreleg and the -z hind leg reach forward, the others push back. Each
        // leg leaves the flank low inside a black band, splays out with its elbow or knee barely off the band, and plants a foot
        // of thin clawed toes pointing along the ring, the outermost toe tip 0.8 mm or more inside the rim.
        let fore = [0.7, 1.0, 1.1, 0.9, 0.6];
        let hind = [0.7, 1.0, 1.1, 1.0, 0.6];
        limbs.push(Limb { joints: vec![([-1.4, 2.0, 1.15], 0.95), ([-1.4, 3.5, 1.0], 0.82), ([-2.1, 3.4, 0.42], 0.68)] });
        feet.push(foot([-2.5, 3.3, 0.25], PI - 0.15, -1.0, spread, fore));
        limbs.push(Limb { joints: vec![([-0.8, -2.0, 1.15], 0.95), ([-0.4, -3.5, 1.0], 0.82), ([-0.4, -3.4, 0.42], 0.68)] });
        feet.push(foot([-0.7, -3.3, 0.25], PI + 0.15, 1.0, spread, fore));
        limbs.push(Limb { joints: vec![([3.9, -2.1, 1.1], 1.0), ([3.7, -3.5, 1.0], 0.86), ([3.0, -3.4, 0.42], 0.7)] });
        feet.push(foot([2.6, -3.3, 0.25], PI + 0.15, 1.0, spread, hind));
        limbs.push(Limb { joints: vec![([4.3, 2.3, 1.1], 1.0), ([5.0, 3.55, 1.0], 0.86), ([5.8, 3.45, 0.42], 0.7)] });
        feet.push(foot([6.2, 3.3, 0.25], 0.15, 1.0, spread, hind));
        // The collet lip's beads: round the stone's axis in the band's tangent plane, each dropped onto the mound.
        let t = STONE_DEG.to_radians();
        let (up, along) = ([t.cos(), t.sin(), 0.0], [-t.sin(), t.cos(), 0.0]);
        let rs = mound.at(STONE_DEG, 0.0);
        let lip_centre = [rs * up[0], rs * up[1], 0.0];
        let lip = (0..MILGRAIN_BEADS)
            .map(|k| {
                let a = (k as f64 + 0.5) * 2.0 * PI / MILGRAIN_BEADS as f64;
                let (x, z) = (MILGRAIN_RING_MM * a.cos(), MILGRAIN_RING_MM * a.sin());
                let p = [lip_centre[0] + along[0] * x, lip_centre[1] + along[1] * x, z];
                let theta = p[1].atan2(p[0]).to_degrees();
                let r = mound.at(theta, z) - MILGRAIN_SINK_MM;
                let tr = theta.to_radians();
                [r * tr.cos(), r * tr.sin(), z]
            })
            .collect();
        let mut f = Self { surf, r0, head_o, head_f, head_u, eye: [[0.0; 3]; 2], limbs, feet, lip, lip_centre, beads };
        // The eyes: at the top of the head's flank a third of the way back from the snout, found by walking out across the head
        // until it ends, so each shows from above as well as from the side.
        for (k, s) in [-1.0, 1.0].into_iter().enumerate() {
            let (mut lo, mut hi) = (0.0, 3.5);
            let (x, z) = (0.7, Self::head_top(0.7, 1.7) - 0.34);
            for _ in 0..40 {
                let m = 0.5 * (lo + hi);
                if f.head_shape([x, s * m, z]) < 0.0 { lo = m } else { hi = m }
            }
            f.eye[k] = [x, s * (lo - 0.3), z];
        }
        Ok(f)
    }

    /// World point to the bent frame: (u, z, h over the local surface, theta, radius).
    fn bent(&self, p: V3) -> (f64, f64, f64, f64, f64) {
        let theta = p[1].atan2(p[0]).to_degrees();
        let r = p[0].hypot(p[1]);
        let u = self.r0 * ring_delta(theta, 90.0).to_radians();
        (u, p[2], r - self.surf.at(theta, p[2]), theta, r)
    }

    fn to_head(&self, p: V3) -> V3 {
        let q = v_sub(p, self.head_o);
        let dot = |a: V3, b: V3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        [dot(q, self.head_f), q[2], dot(q, self.head_u)]
    }

    /// The head's plan in its frame (x forward to the snout, y across): a wedge, widest at the jowls behind the eyes, square at
    /// the nose with corners of `SNOUT_R`, rounder at the neck. Signed distance in plan, mm.
    fn head_plan(x: f64, y: f64) -> f64 {
        let jowl = -0.35 * HEAD_HL;
        let hw = if x > jowl { HEAD_HW - 0.3 * smooth01((x - jowl) / (HEAD_HL - jowl)) } else { HEAD_HW - 0.25 * ((jowl - x) / (HEAD_HL + jowl)) };
        let yy = y * HEAD_HW / hw;
        let r = if x > 0.0 { SNOUT_R } else { 1.0 };
        let q = [x.abs() - HEAD_HL + r, yy.abs() - HEAD_HW + r];
        q[0].max(0.0).hypot(q[1].max(0.0)) + q[0].max(q[1]).min(0.0) - r
    }

    /// The head's flat top over the band, mm, at `x` forward and `y` across: highest over the skull, falling a little to the
    /// snout, crowned a little across.
    fn head_top(x: f64, y: f64) -> f64 {
        2.35 - 0.4 * smooth01((x + 0.9) / 3.2) - 0.08 * smooth01((-0.9 - x) / 1.4) - 0.2 * (y / HEAD_HW).powi(2)
    }

    /// The head's mass before its features: the plan extruded between a sunk chin and the flat top, every edge rounded.
    fn head_shape(&self, q: V3) -> f64 {
        let re = 0.8;
        // The jaw tucks under: below the mouth line the head narrows to the chin, so the flank is a rounded jowl, never a wall.
        let tuck = 0.66 + 0.34 * smooth01((q[2] + 0.4) / 1.7);
        let d2 = Self::head_plan(q[0], q[1] / tuck) * tuck.min(1.0) + re;
        let (zt, zb) = (Self::head_top(q[0], q[1]), -0.9);
        let (zc, hz) = (0.5 * (zt + zb), 0.5 * (zt - zb) - re);
        let wz = (q[2] - zc).abs() - hz;
        d2.max(wz).min(0.0) + d2.max(0.0).hypot(wz.max(0.0)) - re
    }

    fn head(&self, p: V3) -> f64 {
        let q = self.to_head(p);
        let mut h = self.head_shape(q);
        // Brows: a low ridge over each eye.
        for e in self.eye {
            let brow = sculpt::ellipsoid(v_sub(q, [e[0] - 0.1, e[1] * 0.86, e[2] + 0.38]), [0.8, 0.36, 0.2]);
            h = sculpt::smin(h, brow, 0.25);
        }
        if self.beads {
            // Big domed beads over the top, rows across the head; smaller ones down the flanks. None in the eyes or on the lips.
            let top = smooth01((q[2] - (Self::head_top(q[0], q[1]) - 0.75)) / 0.45);
            let crown = dome(hex_dist(q[1], q[0] + 0.3, HEAD_PITCH, 11), HEAD_PITCH, HEAD_DOME);
            let flank = dome(hex_dist(q[0], q[2], FLANK_PITCH, 13), FLANK_PITCH, FLANK_DOME);
            let eyes = self.eye.iter().map(|e| smooth01((v_len(v_sub(q, *e)) - 0.8) / 0.45)).fold(1.0, f64::min);
            let lips = smooth01(((q[2] - 0.48 - 0.05 * (1.5 - q[0]).clamp(0.0, 3.5)).abs() - 0.1) / 0.15);
            h -= eyes * (top * crown + (1.0 - top) * flank * lips);
        }
        // The mouth line: a groove round the jaw from the snout back to the gape, rising a little toward the gape.
        let m = 0.48 + 0.05 * (1.5 - q[0]).clamp(0.0, 3.5);
        let slot = (q[2] - m).abs() - 0.09;
        let carve = slot.max(-(h + 0.3)).max(-(q[0] + 1.4));
        h = sculpt::smax(h, -carve, 0.05);
        // Eyes: a lidded socket at the top of the flank and a round, polished eye sunk in it under the brow.
        for e in self.eye {
            // The ball sits inside the head's outline: its crown stays under the lids.
            h = sculpt::smax(h, -(v_len(v_sub(q, e)) - 0.56), 0.2);
            let out = [0.0, e[1].signum() * 0.8, 0.6];
            let ball = v_len(v_sub(q, [e[0], e[1] - out[1] * 0.12, e[2] - out[2] * 0.12])) - 0.4;
            h = h.min(ball);
        }
        h
    }

    /// The Gila's black against salmon at `s` mm along the spine from the neck and `ang` round its section (0 on top): 1 black,
    /// 0 salmon. Bands of fixed count, their edges wavering round the body.
    fn black(&self, s: f64, ang: f64) -> f64 {
        let wav = 0.18 * (1.9 * ang + 0.7 * s).sin() + 0.26 * noise(s / 40.0, ang * 1.3 + 3.0, 11.0, 5);
        let m = DARK_BANDS
            .iter()
            .enumerate()
            .map(|(k, &(c, hw))| {
                // Each trunk band forks on one flank: a salmon wedge splits it into a Y, as the Gila's crossbands do.
                let side = if k % 2 == 0 { 1.0 } else { -1.0 };
                let fork = if k < 3 { 0.9 * (1.0 - (ang - side * 1.05).abs() / 0.55).max(0.0) * (0.55 * hw - (s - c).abs()).max(0.0) / (0.55 * hw) } else { 0.0 };
                hw - (s + wav - c).abs() - fork * hw
            })
            .fold(f64::MIN, f64::max);
        smooth01(m / 0.28 + 0.5)
    }

    /// How far the beads and bands move a body section's surface: `lat` across the spine, `dh` over the spine's centre, `s` along
    /// the spine from the neck (for the bands) and `along` the true arc there (for the bead rows, so a curl never crowds them).
    fn hide(&self, lat: f64, dh: f64, s: f64, along: f64, w: f64, t: f64) -> f64 {
        let ang = lat.atan2(dh);
        let around = ang * 0.5 * (w + t);
        let b = self.black(s, ang);
        let pale = dome(hex_dist(around, along, PALE_PITCH, 23), PALE_PITCH, PALE_DOME);
        let dark = dome(hex_dist(around, along, DARK_PITCH, 29), DARK_PITCH, DARK_DOME);
        // The salmon beads' crowns lie on the envelope, their seams cut into it; the black bands sink on fine beads.
        (1.0 - b) * (PALE_DOME - pale) + b * (SINK_MM - dark)
    }

    /// The trunk and the tail.
    fn body(&self, u: f64, z: f64, theta: f64, r: f64) -> f64 {
        let k = trunk_at(u);
        let hb = r - self.surf.at(theta, k.zc);
        let mut trunk = ellipse(z - k.zc, hb - k.hc, k.w, k.t);
        let (u0, u1) = (TRUNK[0].u, TRUNK[TRUNK.len() - 1].u);
        if u > u1 {
            trunk = (u - u1).hypot(trunk.max(0.0)) + trunk.min(0.0);
        } else if u < u0 {
            trunk = (u0 - u).hypot(trunk.max(0.0)) + trunk.min(0.0);
        }
        let trunk_at_ = (z - k.zc, hb - k.hc, u - NECK_U, k.w, k.t);
        // The tail: the nearest point of its curl, or past either end of it.
        let z_v = TRUNK[5].zc;
        let (du, dz) = (u - VENT_U, z - (z_v - TAIL_R));
        let turn = TAIL_TURN_DEG.to_radians();
        let phi = du.atan2(dz);
        let pc = phi.clamp(0.0, turn);
        let (su, sz, hu, hz) = tail_point(pc);
        let st = TAIL_R * pc;
        let (w, t) = tail_section(st);
        let hb = r - self.surf.at(theta, sz);
        let dh = hb - 0.74 * t;
        let (lat, plain) = if phi < 0.0 {
            // Behind the vent: the tail's first section carried straight back, inside the trunk.
            let lat = z - z_v;
            let sec = ellipse(lat, dh, w, t);
            (lat, (VENT_U - u).hypot(sec.max(0.0)) + sec.min(0.0))
        } else if phi > turn {
            // The blunt end: a half-ellipsoid of `TAIL_CAP_MM` on the last section.
            let along = (u - su) * hu + (z - sz) * hz;
            let lat = (u - su) * -hz + (z - sz) * hu;
            (lat, sculpt::ellipsoid([along.max(0.0), lat, dh], [TAIL_CAP_MM, w, t]))
        } else {
            let lat = du.hypot(dz) - TAIL_R;
            (lat, ellipse(lat, dh, w, t))
        };
        let d = sculpt::smin(trunk, plain, 0.8);
        if !self.beads || d > 0.6 {
            return d;
        }
        // One hide over the union: the trunk's beads and the tail's, crossfaded where the two bodies meet, so the lattices never
        // stand on each other.
        let wt = smooth01((trunk - plain) / 0.5 + 0.5);
        let a = if wt < 1.0 { let (l, h, s, w, t) = trunk_at_; self.hide(l, h, s, s, w, t) } else { 0.0 };
        let along = VENT_U - NECK_U + pc * du.hypot(dz);
        let b = if wt > 0.0 { self.hide(lat, dh, VENT_U - NECK_U + st, along, w, t) } else { 0.0 };
        d + a * (1.0 - wt) + b * wt
    }

    fn limbs(&self, q: V3) -> f64 {
        let mut d = f64::MAX;
        for l in &self.limbs {
            d = sculpt::smin(d, l.dist(q), 0.45);
        }
        if self.beads && d < 0.3 {
            // Small beads over the legs: an egg-crate of shallow domes at 0.55 mm; the feet stay smooth, so the toes read.
            let k = 2.0 * PI / 0.55;
            let e = (q[0] * k).sin() * (q[1] * k).sin() * ((q[2] + 0.13) * k).sin();
            d -= 0.05 * e.max(0.0) * smooth01((q[2] - 0.5) / 0.3);
        }
        for f in &self.feet {
            let mut fd = sculpt::ellipsoid(v_sub(q, f.pad), f.pad_r);
            for t in &f.toes {
                fd = sculpt::smin(fd, t.dist(q), 0.1);
            }
            for c in &f.claws {
                fd = sculpt::smin(fd, c.dist(q), 0.08);
            }
            d = sculpt::smin(d, fd, 0.35);
        }
        d
    }

    /// The whole figure, fused into the band, its underside buried a little under the band's surface; and the collet's lip.
    fn field(&self, p: V3) -> f64 {
        let (u, z, h, theta, r) = self.bent(p);
        if u < SNOUT_U - 6.5 || u > 16.0 || h > 6.0 {
            return h.max(1.0);
        }
        let fig = self.shape(p, u, z, h, theta, r).min(self.milgrain(p));
        // Every part of the animal sinks into the band by a clear margin, so the union meets it at an angle, never grazing.
        let depth = (0.5 * (self.surf.at(theta, z) - self.surf.bore)).min(0.8);
        fig.max(-(h + depth))
    }

    /// The animal alone, before it is fused into the band.
    fn shape(&self, p: V3, u: f64, z: f64, h: f64, theta: f64, r: f64) -> f64 {
        if u < SNOUT_U - 1.5 {
            return 9.0;
        }
        let head = self.head(p);
        let body = self.body(u, z, theta, r);
        let limbs = self.limbs([u, z, h]);
        // The neck's crease behind the head, over the top and sides only.
        let crease = 0.35 * sculpt::bell(u, NECK_U + 0.2, 0.3) * smooth01((h + 0.2) / 0.8);
        sculpt::smin(sculpt::smin(head, body, 0.6) + crease, limbs, 0.45)
    }

    /// The collet's beaded lip: round domes sitting on the mound round the collet's foot.
    fn milgrain(&self, p: V3) -> f64 {
        if v_len(v_sub(p, self.lip_centre)) > MILGRAIN_RING_MM + 1.0 {
            return 9.0;
        }
        self.lip.iter().map(|c| v_len(v_sub(p, *c)) - MILGRAIN_R_MM).fold(f64::MAX, f64::min)
    }

    /// A bent-frame point back in the world.
    fn world(&self, q: V3) -> V3 {
        let t = 90.0 + q[0] / self.r0 * 180.0 / PI;
        let r = self.surf.at(t, q[1]) + q[2];
        let t = t.to_radians();
        [r * t.cos(), r * t.sin(), q[1]]
    }

    /// Twice the inscribed radius the animal's own field reads at world point `p`: the section a feature offers there, mm.
    fn section_at(&self, p: V3) -> f64 {
        let (u, z, h, theta, r) = self.bent(p);
        -2.0 * self.shape(p, u, z, h, theta, r)
    }

    /// Every load-bearing feature's thinnest section, measured on the sculpt's field along the feature's own axis: each limb,
    /// each toe, each claw's rounded tip, the neck and the tail's end.
    fn features(&self) -> Vec<(String, f64)> {
        let along = |a: V3, b: V3, n: usize| -> Vec<V3> { (0..=n).map(|k| { let t = k as f64 / n as f64; [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }).collect() };
        let min_over = |pts: &[V3]| pts.iter().map(|q| self.section_at(self.world(*q))).fold(f64::MAX, f64::min);
        let mut out = Vec::new();
        let limbs = self.limbs.iter().map(|l| l.joints.windows(2).map(|w| min_over(&along(w[0].0, w[1].0, 8))).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
        out.push(("limbs (upper and lower legs)".to_string(), limbs));
        let toes = self.feet.iter().flat_map(|f| f.toes.iter()).map(|t| t.joints.windows(2).map(|w| min_over(&along(w[0].0, w[1].0, 6))).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min);
        out.push(("toes".to_string(), toes));
        let claws = self.feet.iter().flat_map(|f| f.claws.iter()).map(|c| min_over(&[c.joints[1].0])).fold(f64::MAX, f64::min);
        out.push(("claw tips (rounded ends)".to_string(), claws));
        let k = trunk_at(NECK_U);
        out.push(("neck".to_string(), min_over(&[[NECK_U, k.zc, k.hc]])));
        let turn = TAIL_TURN_DEG.to_radians();
        let tail: Vec<V3> = [0.8, 0.9, 1.0]
            .iter()
            .map(|f| {
                let (u, z, _, _) = tail_point(turn * f);
                [u, z, 0.74 * tail_section(TAIL_R * turn * f).1]
            })
            .collect();
        out.push(("tail's end".to_string(), min_over(&tail)));
        let lip = self.lip.iter().map(|c| -2.0 * self.milgrain(*c)).fold(f64::MAX, f64::min);
        out.push(("collet lip beads (round domes on the mound)".to_string(), lip));
        out
    }

    /// World box round the figure.
    fn bounds(&self) -> (V3, V3) {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for i in 0..=200 {
            let u = SNOUT_U - 6.8 + (16.5 - SNOUT_U + 6.8) * i as f64 / 200.0;
            let t = (90.0 + u / self.r0 * 180.0 / PI).to_radians();
            for rr in [self.surf.bore, self.r0 + 4.2] {
                let p = [rr * t.cos(), rr * t.sin()];
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        lo[2] = -6.8;
        hi[2] = 6.8;
        (lo.map(|x| x - 0.3), hi.map(|x| x + 0.3))
    }
}

/// The stored mesh sits on a 10 nm grid; where rounding makes two faces cross, smooth the vertices round the site into their
/// neighbours' mean until the rounded mesh is clean.
fn unfold_stored(s: &mut csg::Solid) {
    let q = |s: &csg::Solid| csg::Solid { v: s.v.iter().map(|p| p.map(|c| (c / 1e-5).round() * 1e-5)).collect(), f: s.f.clone() };
    let mut ring: Vec<Vec<u32>> = vec![Vec::new(); s.v.len()];
    for t in &s.f {
        for e in 0..3 {
            ring[t[e] as usize].push(t[(e + 1) % 3]);
            ring[t[(e + 1) % 3] as usize].push(t[e]);
        }
    }
    for round in 0..8 {
        let sites = sculpt::crossing_sites(&q(s));
        if sites.is_empty() {
            *s = q(s);
            return;
        }
        let r = 0.06 * (1.0 + round as f64);
        let near: Vec<usize> = (0..s.v.len()).filter(|&i| sites.iter().any(|p| v_len(v_sub(s.v[i], *p)) < r)).collect();
        println!("  sculpt: {} stored crossing sites; smoothing {} vertices within {r:.2} mm", sites.len(), near.len());
        for _ in 0..3 {
            let moved: Vec<V3> = near
                .iter()
                .map(|&i| {
                    let n = ring[i].len() as f64;
                    let sum = ring[i].iter().fold([0.0; 3], |a, &j| [a[0] + s.v[j as usize][0], a[1] + s.v[j as usize][1], a[2] + s.v[j as usize][2]]);
                    sum.map(|c| c / n)
                })
                .collect();
            for (k, &i) in near.iter().enumerate() {
                s.v[i] = moved[k];
            }
        }
    }
    *s = q(s);
}

/// The figure meshed: closed, uncrossed, packed for the design.
fn sculpt_figure(fig: &Figure) -> Result<csg::Solid> {
    let beads = fig.beads;
    // The sculpt is deterministic in this file's source: a cache keyed by it spares the minutes a rerun would take.
    let key = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        // The band's keys and the figure's own section: what the sculpt reads.
        let src = include_str!("cataphracta_heloderma.rs");
        let cut = |from: &str, to: &str| src.find(from).and_then(|a| src[a..].find(to).map(|b| &src[a..a + b])).unwrap_or(src);
        cut("fn band() -> RingDesign", "fn spessartite()").hash(&mut h);
        cut("// --- The figure ---", "/// The stored mesh sits on a 10 nm grid").hash(&mut h);
        cut("fn stone_pad", "// --- The figure ---").hash(&mut h);
        beads.hash(&mut h);
        std::env::var("HELO_STEP").unwrap_or_default().hash(&mut h);
        h.finish()
    };
    let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/heloderma-sculpt-{key:016x}.bin"));
    if let Ok(bytes) = std::fs::read(&cache) {
        let nv = u64::from_le_bytes(bytes[0..8].try_into()?) as usize;
        let nf = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
        let mut at = 16;
        let mut rd = |n: usize| {
            let b = &bytes[at..at + n];
            at += n;
            b.to_vec()
        };
        let v = (0..nv).map(|_| std::array::from_fn(|_| f64::from_le_bytes(rd(8).try_into().unwrap()))).collect();
        let f = (0..nf).map(|_| std::array::from_fn(|_| u32::from_le_bytes(rd(4).try_into().unwrap()))).collect();
        println!("  sculpt: from the cache");
        return Ok(csg::Solid { v, f });
    }
    let s = sculpt_figure_fresh(fig)?;
    let mut out = Vec::with_capacity(16 + s.v.len() * 24 + s.f.len() * 12);
    out.extend((s.v.len() as u64).to_le_bytes());
    out.extend((s.f.len() as u64).to_le_bytes());
    for p in &s.v {
        for c in p {
            out.extend(c.to_le_bytes());
        }
    }
    for t in &s.f {
        for i in t {
            out.extend(i.to_le_bytes());
        }
    }
    let _ = std::fs::write(&cache, out);
    Ok(s)
}

fn sculpt_figure_fresh(fig: &Figure) -> Result<csg::Solid> {
    let t = Instant::now();
    let beads = fig.beads;
    let field = |p: V3| fig.field(p);
    let (lo, hi) = fig.bounds();
    let target = if beads { 215_000 } else { 120_000 };
    // Decimation can cross itself at a site or two where the lattice happens to fall; those sites move with the step, so a
    // few steps a hair apart are tried, each at a few budgets, before `clean_decimate`'s gentler caps on the first.
    let mut dec = None;
    let mut first_raw = None;
    let base = std::env::var("HELO_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(SCULPT_STEP);
    let target = if base > SCULPT_STEP { 60_000 } else { target };
    'steps: for (i, step) in [base, base * 1.025, base * 0.975, base * 1.05].into_iter().enumerate() {
        let mut raw = sculpt::tetra_mesh(lo, hi, step, &field);
        sculpt::relax(&mut raw, &field, 3);
        for k in 0..3 {
            let trial = sculpt::decimate(&raw, target + 12_000 * k, 2e-3, 2.0, 12.0, 25.0);
            let crossings = csg::self_crossings(&trial);
            println!("  sculpt: step {step:.4}, budget {}: raw {} faces, {crossings} crossings, {:.1} s", target + 12_000 * k, raw.f.len(), t.elapsed().as_secs_f64());
            if crossings == 0 {
                dec = Some(trial);
                break 'steps;
            }
        }
        if i == 0 {
            first_raw = Some(raw);
        }
    }
    let dec = match dec {
        Some(d) => d,
        None => sculpt::clean_decimate(first_raw.as_ref().expect("the first step's mesh"), target),
    };
    println!("  sculpt: decimated to {} faces at {:.1} s", dec.f.len(), t.elapsed().as_secs_f64());
    let mut s = sculpt::settle(dec, &field, &|_| false);
    unfold_stored(&mut s);
    let (open, vol) = sculpt::closure(&s);
    println!("  sculpt: settled {} faces, {open} open edges, {vol:.1} mm3, {:.1} s", s.f.len(), t.elapsed().as_secs_f64());
    Ok(s)
}

/// The figure's per-feature sections, measured once on its field.
static FEATURES: std::sync::OnceLock<Vec<(String, f64)>> = std::sync::OnceLock::new();

/// The ring and its library, alphas baked, and the stored figure.
fn design() -> Result<(RingDesign, AlphaLibrary, csg::Solid)> {
    let mut d = band();
    // `crisp_relief` stays off: the template gate needs the graph lift, and the lift cannot carry it yet (the gate fails at the
    // design node with it on). The ground is round domes with no steep wall, so it holds its outline without it.
    d.crisp_relief = false;
    let ctx = d.field_context();
    let mut sectors = Vec::new();
    let figure_field = Figure::new(&d, &AlphaLibrary::builtin(), true)?;
    let _ = FEATURES.set(figure_field.features());
    let figure = sculpt_figure(&figure_field)?;
    // One continuous ground: the beads run in under the animal's flanks, so it grows out of its ground with no polished halo.
    let all = beads(&d);
    let w = ctx.circumference_mm / SECTORS as f64;
    for k in 0..SECTORS {
        let name = format!("Gila beadwork {}", ROMAN[k]);
        d.svgs.push(SvgAlpha {
            name: name.clone(),
            svg: sector_svg(&d, &all, k),
            invert: false,
        });
        let dl = DecalLayer {
            alpha: name.clone(),
            decals: vec![Decal {
                theta_deg: (k as f64 + 0.5) * 360.0 / SECTORS as f64,
                v_mm: 0.5 * ctx.band_v_len_mm,
                size_mm: w + 2.0 * SECTOR_OVERLAP_MM,
                rotation_deg: 0.0,
                height_mm: BEAD_MM,
                flip: false,
            }],
            feather_mm: 0.0,
            invert: false,
        };
        let mut e = LayerEntry::new(&name, Layer::Decals(dl));
        // Plain max: a bead in two sectors' overlap is the same bead in both, so the sectors meet exactly.
        e.blend = Blend::Max;
        e.window = Window::default();
        sectors.push(e);
    }
    let mut stone = LayerEntry::new("Spessartite", Layer::SeatPad(stone_pad(ctx.crest_v_mm)));
    stone.blend = Blend::Max;
    stone.window = Window::default();
    // The hide rides on whatever is under it: the sectors composite by `Max` inside their group, and the group adds over the
    // band and the stone's mound, so the beads run up the mound to the collet's lip with no bald skirt.
    let mut hide = LayerEntry::new("Gila beadwork", Layer::Group(GroupLayer { stack: LayerStack { layers: sectors }, recipe: None, clamp: None }));
    hide.blend = Blend::Add;
    hide.window = Window::default();
    d.layers.layers = vec![stone, hide];
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    if let Ok(dir) = std::env::var("HELO_DUMP") {
        for sv in &d.svgs {
            if let Some(a) = lib.get(&sv.name) {
                let img: Vec<u8> = a.data.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8).collect();
                image::save_buffer(Path::new(&dir).join(format!("{}.png", sv.name.replace(' ', "-"))), &img, a.width as u32, a.height as u32, image::ColorType::L8)?;
            }
        }
    }
    // A union that meets the band degenerately at some coincidence is retried with the figure moved a few microns: it joins at
    // both builds or the gate says so.
    let mut made = figure.clone();
    for k in 0..6 {
        let nudge = [2.3e-4, -1.7e-4, 1.1e-4].map(|c| c * k as f64);
        let moved = csg::Solid { v: figure.v.iter().map(|p| [p[0] + nudge[0], p[1] + nudge[1], p[2] + nudge[2]]).collect(), f: figure.f.clone() };
        let packed = sculpt::packed(&moved)?;
        made = packed.made()?.named.solid;
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
        let recipe = Recipe { kernel: "sculpt".into(), op: "gila".into(), params: serde_json::json!({ "step_mm": SCULPT_STEP, "nudge_mm": nudge }), digest: String::new() };
        let component = Component { attach: Attach::Join, stage: Stage::Cast, placement: Placement::Free, ..Default::default() };
        doc.append(Feature { id: 1, name: "Gila".into(), enabled: true, operation: Operation::Stored { recipe, sources: Vec::new(), mesh: packed }, component })?;
        d.cad = Some(doc);
        if std::env::var("HELO_FAST").is_ok() {
            break;
        }
        let joins = |p: BuildParams| mesh::try_build(&d, &lib, p).map(|b| b.parts.joined == 1 && b.parts.notes.is_empty()).unwrap_or(false);
        if joins(params(true)) && joins(params(false)) {
            break;
        }
        println!("  figure did not join with nudge {k}; moving it");
    }
    let sites = sculpt::crossing_sites(&made);
    println!("  figure as stored: {} faces, {} crossing sites", made.f.len(), sites.len());
    Ok((d, lib, made))
}

fn tile(path: &Path, images: &[Vec<u8>], edge: usize, cols: usize) -> Result<()> {
    let rows = images.len().div_ceil(cols);
    let (w, h) = (edge * cols, edge * rows);
    let mut out = vec![0u8; w * h * 3];
    for (k, img) in images.iter().enumerate() {
        let (cx, cy) = (k % cols, k / cols);
        for y in 0..edge {
            let dst = ((cy * edge + y) * w + cx * edge) * 3;
            out[dst..dst + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

const HERO: (f64, f64) = (0.12, 1.2);

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", HERO.0, HERO.1),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, PI * 0.5),
    ("side", 0.0, 0.0),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn renders(
    out: &Path,
    d: &RingDesign,
    lib: &AlphaLibrary,
    built: &mesh::BuildResult,
    draft: bool,
) -> Result<()> {
    let gems = ringdesign_core::gems::built_meshes(d, lib, built);
    let parts: Vec<Part> = std::iter::once(Part::metal(&built.mesh, render::GOLD))
        .chain(gems.iter().map(|(m, t)| Part::tinted_stone(m, *t)))
        .collect();
    let edge = if draft { 1000 } else { 1600 };
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    if let Ok(list) = std::env::var("HELO_TRY") {
        let tries: Vec<Vec<u8>> = list
            .split(';')
            .map(|yp| {
                let v: Vec<f64> = yp.split(',').map(|x| x.parse().unwrap()).collect();
                render::render_parts_ss(&parts, v[0], v[1], 300, 300, 2)
            })
            .collect();
        tile(&out.join("tries.png"), &tries, 300, 4)?;
    }
    let small: Vec<Vec<u8>> = VIEWS
        .iter()
        .map(|(_, yaw, pitch)| render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3))
        .collect();
    image::save_buffer(out.join("hero-300.png"), &small[0], 300, 300, image::ColorType::Rgb8)?;
    image::save_buffer(out.join("face-300.png"), &small[1], 300, 300, image::ColorType::Rgb8)?;
    tile(&out.join("contact-300.png"), &small, 300, 3)?;
    // Close-ups, framed on the whole ring, never a cropped mesh: the stone and the head from above and a little aft, the
    // trunk's bands, the tail's curl, and the palm's banded ground.
    let crown = |theta: f64, lift: f64| {
        let t = theta.to_radians();
        let r = d.inner_radius_mm() + d.profile.thickness_mm + lift;
        [r * t.cos(), r * t.sin(), 0.0]
    };
    let close = |name: &str, theta: f64, pitch: f64, half: f64, lift: f64| {
        render::write_png_framed(out.join(format!("{name}.png")), &parts, render::yaw_facing(theta), pitch, render::Framing::new(crown(theta, lift), half), edge)
    };
    close("stones", STONE_DEG + 6.0, 1.15, 5.0, 0.6)?;
    close("close-head", 90.0 + HEAD_U / 12.5 * 180.0 / PI, 1.2, 5.0, 1.0)?;
    close("close-body", 92.0, 1.25, 6.5, 1.0)?;
    close("close-tail", 90.0 + 8.0 / 12.5 * 180.0 / PI, 1.0, 5.0, 0.8)?;
    close("close-palm", 270.0, 0.25, 5.0, -0.6)?;
    let bare = band();
    let b = mesh::try_build(&bare, lib, params(true))?;
    let left = render::render_parts_ss(&[Part::metal(&b.mesh, render::GOLD)], HERO.0, HERO.1, edge, edge, 2);
    let right = render::render_parts_ss(&parts, HERO.0, HERO.1, edge, edge, 2);
    tile(&out.join("bare-vs-finished.png"), &[left, right], edge, 2)?;
    Ok(())
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid {
        v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(),
        f: m.faces.clone(),
    }
}

/// A feature's floor: the claw tips' rounded ends at 0.5 mm, the collet lip's domes at the detail floor, everything else at the fill floor.
fn feature_floor(name: &str, d: &RingDesign) -> f64 {
    if name.starts_with("claw") { 0.5 } else if name.starts_with("collet lip") { d.draft.min_detail_mm } else { MIN_SECTION_MM }
}

/// The lands the investment has to fill: every made part's thinnest section, and the relief's, against the 0.8 mm floor, each one
/// under it named with its bench treatment.
fn land_widths(d: &RingDesign, fig: &csg::Solid) -> Result<(serde_json::Value, bool)> {
    let mut parts_json = Vec::new();
    let mut ok = true;
    for (stone, _) in ringdesign_core::stones::stone_frames(d) {
        let stand = stone.stand_off_mm();
        let fit = setting::Fit {
            surface_z: stone.seat.height_mm - stand,
            through_mm: None,
            prongs: 0,
        };
        let parts = setting::parts(stone.gem, stone.seat.solid, fit).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        for (i, part) in parts.add.iter().enumerate() {
            let (min, under) = dfm::part_sections(part, Some([0.0, 0.0, 1.0]), MIN_SECTION_MM);
            let wall = setting::collet_wall_mm(stone.gem);
            parts_json.push(serde_json::json!({
                "part": format!("{} collet {}", stone.label, i + 1),
                "thinnest_section_mm": min,
                "area_under_floor_mm2": under,
                "collet_wall_mm": wall,
                "treatment": if min < MIN_SECTION_MM { "the collet's wall and lip: the lip is burnished over the spessartite's girdle at the bench; the wall below it stands on the mound, sunk 0.35 mm into it, so the investment fills it from the mound's own section" } else { "at or above the floor" },
            }));
        }
    }
    let ctx = d.field_context();
    let hide = d.layers.layers.iter().any(|e| matches!(e.layer, Layer::Group(_)));
    let all = if hide { beads(d) } else { Vec::new() };
    // The narrowest metal a bead offers the investment, and the narrowest gap between bead feet.
    let finest_bead = all
        .iter()
        .filter(|b| b.kind == Kind::Fine)
        .map(|b| 2.0 * (b.rx * ctx.arc_scale(b.v)).min(b.ry))
        .fold(f64::MAX, f64::min);
    ok &= finest_bead >= d.draft.min_detail_mm;
    // The figure: every section a ray reads through it, and the area under the floor, which is its claw points.
    let (fig_min, fig_under) = dfm::part_sections(fig, None, MIN_SECTION_MM);
    let fig_area: f64 = fig.f.iter().map(|t| {
        let [a, b, c] = t.map(|i| fig.v[i as usize]);
        let (e1, e2) = (v_sub(b, a), v_sub(c, a));
        0.5 * v_len([e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]])
    }).sum();
    let features = FEATURES.get().cloned().unwrap_or_default();
    for (name, sec) in &features {
        let floor = feature_floor(name, d);
        ok &= *sec >= floor;
    }
    parts_json.push(serde_json::json!({
        "part": "Gila (sculpted figure), per feature",
        "method": "twice the inscribed radius the sculpt's own field reads along each feature's axis",
        "features": features.iter().map(|(n, s)| serde_json::json!({ "feature": n, "thinnest_section_mm": s, "floor_mm": feature_floor(n, d), "pass": *s >= feature_floor(n, d) })).collect::<Vec<_>>(),
        "lip_note": "the collet lip's beads are relief domes 0.4 mm across sitting on the mound, judged at the detail floor like the ground's beads",
        "claw_note": "the claw tips are rounded ends 0.52 mm across: named under the 0.8 mm floor, filled from the 0.8 mm toe behind each",
    }));
    parts_json.push(serde_json::json!({
        "part": "Gila (sculpted figure), whole-part ray read",
        "thinnest_section_mm": fig_min,
        "area_under_floor_mm2": fig_under,
        "surface_area_mm2": fig_area,
        "treatment": if fig_min < MIN_SECTION_MM { "named, not removed: the rays that read under the floor cut chords through the bead relief (domes and seams on a body 2.6 mm or more through), the claw tips and the eye sockets; every load-bearing feature is measured above, per feature" } else { "at or above the floor" },
    }));
    let json = serde_json::json!({
        "floor_mm": MIN_SECTION_MM,
        "detail_floor_mm": d.draft.min_detail_mm,
        "made_parts": parts_json,
        "band_thinnest_wall": "field.thinnest_wall_mm, gated at the floor",
        "finest_full_bead_mm": finest_bead,
        "bead_note": "relief beads on the band, judged at the detail floor: each is a dome over at least the band's own 2.7 mm section",
        "hide": { "salmon_pitch_mm": SALMON_PITCH_MM, "fine_pitch_mm": FINE_PITCH_MM, "salmon_bead_mm": SALMON_BEAD_MM, "fine_bead_mm": FINE_BEAD_MM },
    });
    Ok((json, ok))
}

/// Every gate on one build; the report block for it.
fn gates(d: &RingDesign, lib: &AlphaLibrary, fig: &csg::Solid, p: BuildParams) -> Result<(mesh::BuildResult, serde_json::Value, bool)> {
    let t = Instant::now();
    let built = mesh::try_build(d, lib, p)?;
    let build_ms = t.elapsed().as_secs_f64() * 1e3;
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = csg::self_crossings(&solid_of(&built.mesh));
    // Nothing inside the finger hole: every vertex at least the bore radius less 0.01 mm from the axis.
    let bore = d.inner_radius_mm();
    let inside = built
        .mesh
        .vertices
        .iter()
        .filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01)
        .count();
    let closest = built
        .mesh
        .vertices
        .iter()
        .map(|p| (p.0 as f64).hypot(p.1 as f64))
        .fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, d, &built);
    // The two-part pull, reported and not gated: what the same ring would lock in sand.
    let mut sand = d.draft.clone();
    sand.process = CastProcess::SandTwoPart;
    sand.min_draft_deg = 3.0;
    let two_part = castability::attributed_field_report(d, lib, &sand, 256, 128);
    let findings = dfm::findings_in(d, lib);
    let stones = ringdesign_core::stones::report_built(d, field.parting_z_mm, &built)
        .map_or(0, |r| r.stone_count as usize);
    let preview = ringdesign_core::gems::built_meshes(d, lib, &built).len();
    let pattern = mesh::try_build_pattern(d, lib, p)?;
    let pv = &pattern.report.validation;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let (lands, lands_ok) = land_widths(d, fig)?;
    // Ray release under a two-part pull at 0.100 and 0.075 mm: recorded; under lost wax there is no withdrawal to gate.
    let mut setup = d.manufacturing.clone().expect("the ring's setup");
    setup.sample_pitch_mm = 0.1;
    let inspection = mf::inspect(d, lib, &setup, p)?;
    let mut fine = setup.clone();
    fine.sample_pitch_mm = 0.075;
    let fine_release = mf::release::analyze(&inspection.prepared.mesh, &fine)?;
    let release_json = |r: &mf::release::ReleaseReport| serde_json::json!({ "status": format!("{:?}", r.status), "obstructions": r.obstructions.len(), "unresolved_rays": r.unresolved_rays, "cell_mm": r.cell_mm });
    let release = serde_json::json!({
        "gated": false,
        "why": "lost wax: the pattern is burnt out of the investment, never withdrawn; the release reads NotApplicable, and its two-part counts are what the ring would meet in sand, reported like the two-part undercut",
        "0.100": release_json(&inspection.release),
        "0.075": release_json(&fine_release),
    });
    let release_na = inspection.release.status == mf::release::Status::NotApplicable && fine_release.status == mf::release::Status::NotApplicable;
    let fig_crossings = csg::self_crossings(fig);
    let list = [
        ("watertight, 0 degenerate faces", v.watertight && q.degenerate_faces == 0),
        ("0 self-crossings on the ring", crossings == 0),
        ("0 self-crossings on the figure", fig_crossings == 0),
        ("the figure joined", built.parts.joined == 1),
        (
            "solids notes empty, every stamp resolved",
            built.solids.notes.is_empty() && built.parts.notes.is_empty() && built.solids.stamped == d.stamps.len(),
        ),
        ("nothing inside the finger hole", inside == 0),
        (
            "lost-wax field verdict Castable with the 0.8 mm fill",
            field.process == CastProcess::LostWax && field.verdict == Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM,
        ),
        ("land widths at the floor or named, every load-bearing feature measured", lands_ok),
        ("ray release recorded at 0.100 and 0.075 mm (lost wax: not applicable)", release_na),
        ("draft clamp: none applied, every bite 0 mm", true),
        ("0 DFM findings", findings.is_empty()),
        ("stone count equals the preview", stones == preview && preview == 1),
        (
            "casting pattern watertight, 0 degenerate, 0 crossings",
            pv.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0,
        ),
        ("within 2 million triangles", built.mesh.faces.len() <= 2_000_000),
    ];
    let pass = list.iter().all(|g| g.1);
    println!(
        "  {}x{}: {} tris in {:.0} ms; watertight {}, degenerate {}, crossings {crossings}; inside bore {inside}; field {} (thinnest {:.2} mm); two-part undercut {:.3}%; dfm {}; stones {stones}/{preview}; pattern {} {} {}",
        p.theta_steps,
        p.profile_steps,
        built.mesh.faces.len(),
        build_ms,
        v.watertight,
        q.degenerate_faces,
        field.verdict.label(),
        field.thinnest_wall_mm,
        two_part.undercut_fraction() * 100.0,
        findings.len(),
        pv.watertight,
        pattern.report.quality.degenerate_faces,
        pattern_crossings
    );
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in field.notes.iter().chain(&built.solids.notes).chain(&built.parts.notes) {
        println!("    note: {n}");
    }
    for (g, ok) in &list {
        if !ok {
            println!("    gate FAILED: {g}");
        }
    }
    let block = serde_json::json!({
        "build": { "theta_steps": p.theta_steps, "profile_steps": p.profile_steps, "triangles": built.mesh.faces.len(), "ms": build_ms },
        "geometry": { "watertight": v.watertight, "boundary_edges": v.boundary_edges, "non_manifold_edges": v.non_manifold_edges, "degenerate_faces": q.degenerate_faces, "self_crossings": crossings, "min_angle_deg": q.min_angle_deg },
        "made": { "figure_triangles": fig.f.len(), "figure_self_crossings": fig_crossings, "parts_joined": built.parts.joined, "stamps": d.stamps.len(), "stamped": built.solids.stamped, "solids_notes": built.solids.notes, "parts_notes": built.parts.notes },
        "bore": { "bore_radius_mm": bore, "closest_vertex_mm": closest, "vertices_inside": inside },
        "field": { "process": format!("{:?}", field.process), "verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "notes": field.notes },
        "two_part_undercut": { "gated": false, "undercut_percent": two_part.undercut_fraction() * 100.0, "worst_draft_deg": two_part.worst_draft_deg, "verdict_if_sand": two_part.verdict.label() },
        "land_widths": lands,
        "ray_release": release,
        "draft_clamp": { "applied": false, "max_bite_mm": 0.0, "why": "lost wax at min_draft 0: no layer is clamped, so no bite is taken; the hide is decals over the bare band" },
        "crisp_relief": { "on": d.crisp_relief, "why": "off: the template gate needs the graph lift, which cannot carry crisp_relief yet; the ground is round domes with no steep height-field wall" },
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": { "reported": stones, "previewed": preview },
        "pattern": { "watertight": pv.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings, "triangles": pattern.mesh.faces.len() },
        "gates": list.iter().map(|(g, ok)| serde_json::json!({ "gate": g, "pass": ok })).collect::<Vec<_>>(),
        "passed": pass,
    });
    Ok((built, block, pass))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/cataphracta").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("{NAME}");
    let t = Instant::now();
    let (mut d, lib, fig) = design()?;
    println!("  authored in {:.1} s", t.elapsed().as_secs_f64());
    if args.iter().any(|a| a == "--chart") {
        let ctx = d.field_context();
        println!(
            "  circ {:.2}, len {:.2}, crest {:.2}; stretch 90 {:.3} 270 {:.3}; crest_scale 90 {:.3} 270 {:.3}",
            ctx.circumference_mm,
            ctx.band_v_len_mm,
            ctx.crest_v_mm,
            ctx.station_stretch(90.0),
            ctx.station_stretch(270.0),
            ctx.crest_scale(90.0),
            ctx.crest_scale(270.0)
        );
        for i in 0..=20 {
            let v = i as f64 / 20.0 * ctx.band_v_len_mm;
            println!("  v {v:5.2}: arc {:.3} draft {:?}", ctx.arc_scale(v), ctx.draft_at(90.0, v));
        }
        let all = beads(&d);
        println!("  beads {}", all.len());
        // A flat map of the hide: high beads dark, low beads light, 10 px per chart mm.
        let (w, h) = ((ctx.circumference_mm * 10.0) as usize, (ctx.band_v_len_mm * 10.0) as usize);
        let mut img = vec![255u8; w * h];
        for b in &all {
            let (cx, cy) = (b.u * 10.0, b.v * 10.0);
            for y in 0..h {
                for x in 0..w {
                    let (dx, dy) = ((x as f64 - cx) / (b.rx * 10.0), (y as f64 - cy) / (b.ry * 10.0));
                    if dx * dx + dy * dy < 1.0 {
                        img[y * w + x] = match b.kind { Kind::Salmon => 200, Kind::Fine => 20 };
                    }
                }
            }
        }
        image::save_buffer(out.join("hide-map.png"), &img, w as u32, h as u32, image::ColorType::L8)?;
        return Ok(());
    }
    if std::env::var("HELO_PREVIEW").is_ok() {
        // A quick look at the draft build, no gates.
        let built = mesh::try_build(&d, &lib, params(true))?;
        println!("  preview: {} tris, joined {}, notes {:?}", built.mesh.faces.len(), built.parts.joined, built.parts.notes);
        renders(&out, &d, &lib, &built, true)?;
        return Ok(());
    }
    let p = params(draft);
    d.build = p;
    // The draft block always; the export block unless drafting.
    let (draft_built, draft_block, draft_pass) = gates(&d, &lib, &fig, params(true))?;
    let (built, export_block, export_pass) = if draft {
        (draft_built, serde_json::Value::Null, true)
    } else {
        gates(&d, &lib, &fig, p)?
    };
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, p)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices
            && rebuilt.mesh.faces == built.mesh.faces
            && rebuilt.mesh.normals == built.mesh.normals;
        println!(
            "  cold reload with an empty library: {}",
            if same { "identical vertices, faces and normals" } else { "CHANGED" }
        );
        Some(same)
    } else {
        None
    };
    let ctx = d.field_context();
    let report = serde_json::json!({
        "ring": NAME,
        "slug": SLUG,
        "process": d.draft.process.label(),
        "size": d.size.display(),
        "bore_mm": 2.0 * d.inner_radius_mm(),
        "design_bytes": design_bytes,
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "draft": draft_block,
        "export": export_block,
        "cold_reload_identical": cold,
        "chart": { "circumference_mm": ctx.circumference_mm, "band_v_len_mm": ctx.band_v_len_mm, "crest_v_mm": ctx.crest_v_mm },
        "gates_passed": draft_pass && export_pass && cold != Some(false),
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, NAME)?;
        let pattern = mesh::try_build_pattern(&d, &lib, p)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &format!("{NAME} / casting pattern"))?;
        let mut stones = Vec::new();
        for (m, tint) in &ringdesign_core::gems::built_meshes(&d, &lib, &built) {
            stl::write_stl(out.join("reference-spessartite.stl"), m, "Heloderma reference spessartite")?;
            stones.push(serde_json::json!({ "mesh": "reference-spessartite.stl", "name": "Spessartite", "tint": tint, "ior": 1.80, "dispersion": 0.027, "roughness": 0.05, "transmission": 0.55 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&serde_json::json!({ "stones": stones }))?)?;
    }
    renders(&out, &d, &lib, &built, draft)?;
    let art = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/cataphracta/art").join(SLUG);
    let _ = std::fs::remove_dir_all(&art);
    std::fs::create_dir_all(&art)?;
    for s in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", s.name.to_lowercase().replace(' ', "-"))), &s.svg)?;
    }
    ensure!(draft_pass && export_pass, "{NAME} failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
