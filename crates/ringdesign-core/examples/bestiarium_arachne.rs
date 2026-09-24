//! Bestiarium — Arachne, the weaver: a spider clasping the finger on its own web, cast in lost wax.
//! cargo build --offline --release -p ringdesign-core --example bestiarium_arachne
//! target/release/examples/bestiarium_arachne [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, Blend, BuildParams, Layer, LayerEntry, ProfileLoop, ProfileStyle, RingDesign, ShankKind,
    cad::{Attach, Component, Document, Feature, MirrorPlane, Operation, PatternKind, Placement},
    castability::{self, CastProcess},
    csg, dfm, library, manufacturing as mf,
    field::{Decal, DecalLayer, SeatPadLayer, SeatStyle, SideFacePick, VGate},
    gem::{Gem, GemCut},
    mesh, profile::ShankKey, render, stl,
    setting::{self, SolidKind},
    sketch::{Geometry, Id, Sketch, Workplane},
    svg::SvgAlpha,
    tiling::TilingLayer,
};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

const CARAPACE_DEG: f64 = 64.0;
const ABDOMEN_DEG: f64 = 108.0;
const WEB: &str = "Orb web";
const PALM_WEB: &str = "Palm orb web";
const PALM_DEG: f64 = 270.0;
/// The waist between the two body parts, and the spinnerets behind the abdomen on the web's crest thread.
const PEDICEL_DEG: f64 = 78.6;
const SPINNERET_DEG: f64 = 140.0;
/// Half the palm web's reach round the ring and across the crown, chart mm.
const PALM_WEB_HALF: [f64; 2] = [26.9, 2.8];

type P2 = [f64; 2];
type P3 = [f64; 3];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

/// The keyed LowDome band, squared at the cheeks: broad and deep under the spider, slim at the palm.
fn band() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Arachne \u{2014} the weaver".into();
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.width_mm = 5.6;
    d.profile.thickness_mm = 2.4;
    d.profile.crown_mm = 0.72;
    d.profile.comfort_fit_mm = 0.2;
    d.profile.flatten_sides();
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    let key = |theta_deg: f64, width_scale: f64, thickness_scale: f64| ShankKey { theta_deg, width_scale, thickness_scale, crown_scale: 1.0 };
    d.shank.keys = vec![
        key(90.0, 1.35, 1.25),
        key(30.0, 1.16, 1.10),
        key(150.0, 1.16, 1.10),
        key(210.0, 0.97, 0.98),
        key(270.0, 0.92, 0.95),
        key(330.0, 0.97, 0.98),
    ];
    CastProcess::LostWax.apply(&mut d.draft);
    d.build = export_params();
    d
}

fn garnet() -> Gem {
    let mut g = Gem::cabochon(GemCut::Round, 3.5);
    g.preview_tint = Some([0.42, 0.02, 0.05]);
    g
}
fn onyx() -> Gem {
    let mut g = Gem::cabochon(GemCut::Oval, 8.0);
    g.preview_tint = Some([0.015, 0.015, 0.02]);
    g
}

/// The cephalothorax seat: a garnet cabochon in a collet on a domed pad.
fn carapace(ctx: &ringdesign_core::FieldContext) -> SeatPadLayer {
    let mut seat = SeatPadLayer {
        theta_deg: CARAPACE_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::Boss,
        metal_true: true,
        solid: SolidKind::Bezel,
        blend_mm: 0.6,
        crown: 0.72,
        ..Default::default()
    };
    seat.fit_stone(garnet());
    seat.diameter_mm = 5.0;
    seat.elong = 1.08;
    seat.height_mm = 0.8;
    seat
}

/// The abdomen seat: an onyx oval cabochon in a collet on a domed pad.
fn abdomen(ctx: &ringdesign_core::FieldContext) -> SeatPadLayer {
    let mut seat = SeatPadLayer {
        theta_deg: ABDOMEN_DEG,
        v_mm: ctx.crest_v_mm,
        style: SeatStyle::Boss,
        metal_true: true,
        solid: SolidKind::Bezel,
        blend_mm: 0.6,
        crown: 0.72,
        ..Default::default()
    };
    seat.fit_stone(onyx());
    seat.height_mm = 1.0;
    seat
}

/// The cheek web tile: radials across the face, capture threads sagging toward the hub and climbing one row per tile.
fn web_svg(cell_w: f64, cell_h: f64, radials: usize, pitch: f64) -> String {
    let (radial_w, spiral_w) = (0.34, 0.30);
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cell_w:.4} {cell_h:.4}" width="{cell_w:.4}" height="{cell_h:.4}"><rect width="{cell_w:.4}" height="{cell_h:.4}" fill="#fff"/><g fill="none" stroke="#000" stroke-linecap="round">"##
    );
    let step = cell_w / radials as f64;
    for j in 0..=radials {
        let x = j as f64 * step;
        s += &format!(r##"<line x1="{x:.4}" y1="-1" x2="{x:.4}" y2="{:.4}" stroke-width="{radial_w}"/>"##, cell_h + 1.0);
    }
    let rows = (cell_h / pitch).ceil() as i32 + 2;
    for m in -1..rows {
        for j in 0..radials {
            let (x0, x1) = (j as f64 * step, (j + 1) as f64 * step);
            let y = |x: f64| m as f64 * pitch + pitch * x / cell_w + 0.5 * pitch;
            let (y0, y1) = (y(x0), y(x1));
            let sag = 0.62 * pitch;
            s += &format!(
                r##"<path d="M{x0:.4} {y0:.4} Q{:.4} {:.4} {x1:.4} {y1:.4}" stroke-width="{spiral_w}"/>"##,
                0.5 * (x0 + x1),
                0.5 * (y0 + y1) - 2.0 * sag
            );
        }
    }
    s += "</g></svg>";
    s
}

fn web(d: &mut RingDesign) -> Result<()> {
    let ctx = d.field_context();
    let mut t = TilingLayer::default_for(WEB, &ctx);
    ensure!(t.fit_to_side_faces(&ctx, ringdesign_core::field::SIDE_FACE_MIN_DRAFT_DEG), "The band carries no side face for the web");
    t.repeats_around = 8;
    t.height_mm = 0.32;
    t.continuous = true;
    t.feather_mm = 0.0;
    let (cw, ch) = t.cell_size(&ctx);
    println!("  web cell {cw:.2} x {ch:.2} mm on v {:.2}..{:.2}", t.v_center_mm - t.v_span_mm * 0.5, t.v_center_mm + t.v_span_mm * 0.5);
    d.svgs.push(SvgAlpha { name: WEB.into(), svg: web_svg(cw, ch, 3, 0.56), invert: false });
    let mut e = LayerEntry::new("Orb web on both cheeks", Layer::Tiling(t));
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(())
}

/// The palm orb web in chart mm: a hub, radials to the crest ends and to edge anchors, capture threads round the hub.
fn palm_web_svg(half: [f64; 2]) -> String {
    let [l, h] = half;
    let (radial_w, capture_w, hub_r) = (0.36, 0.32, 0.55);
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{:.4} {:.4} {:.4} {:.4}" width="{:.4}" height="{:.4}"><rect x="{:.4}" y="{:.4}" width="{:.4}" height="{:.4}" fill="#fff"/><g fill="none" stroke="#000" stroke-linecap="round">"##,
        -l, -h, 2.0 * l, 2.0 * h, 2.0 * l, 2.0 * h, -l, -h, 2.0 * l, 2.0 * h
    );
    // Radials to the crest ends and to anchors just past both band edges.
    let crest_end = l - 0.35;
    let mut ends: Vec<[f64; 2]> = vec![[crest_end, 0.0], [-crest_end, 0.0]];
    for u in [0.95, 2.7, 5.4, 9.2, 12.4, 16.2, 20.0, 24.2] {
        for (su, sv) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            ends.push([su * u, sv * h * 1.08]);
        }
    }
    ends.sort_by(|a, b| a[1].atan2(a[0]).total_cmp(&b[1].atan2(b[0])));
    for e in &ends {
        s += &format!(r##"<line x1="0" y1="0" x2="{:.4}" y2="{:.4}" stroke-width="{radial_w}"/>"##, e[0], e[1]);
    }
    let len = |e: &[f64; 2]| e[0].hypot(e[1]);
    for d in [1.15, 1.75, 2.4, 3.15, 4.0, 5.0, 6.1, 7.3, 8.6, 10.0] {
        for k in 0..ends.len() {
            let (a, b) = (ends[k], ends[(k + 1) % ends.len()]);
            if len(&a) < d || len(&b) < d {
                continue;
            }
            let (pa, pb) = ([a[0] / len(&a) * d, a[1] / len(&a) * d], [b[0] / len(&b) * d, b[1] / len(&b) * d]);
            let sag = 0.62;
            s += &format!(
                r##"<path d="M{:.4} {:.4} Q{:.4} {:.4} {:.4} {:.4}" stroke-width="{capture_w}"/>"##,
                pa[0], pa[1], 0.5 * (pa[0] + pb[0]) * sag, 0.5 * (pa[1] + pb[1]) * sag, pb[0], pb[1]
            );
        }
    }
    s += &format!(
        r##"</g><circle cx="0" cy="0" r="{hub_r}" fill="#000"/><circle cx="{crest_end:.4}" cy="0" r="0.32" fill="#000"/><circle cx="{:.4}" cy="0" r="0.32" fill="#000"/></svg>"##,
        -crest_end
    );
    s
}

/// Adds the palm orb web, its crest threads running up to the carapace in front and past the spinnerets behind.
fn palm(d: &mut RingDesign) -> Result<()> {
    let ctx = d.field_context();
    d.svgs.push(SvgAlpha { name: PALM_WEB.into(), svg: palm_web_svg(PALM_WEB_HALF), invert: false });
    let stamp = Decal { theta_deg: PALM_DEG, v_mm: ctx.crest_v_mm, size_mm: 2.0 * PALM_WEB_HALF[0], rotation_deg: 0.0, height_mm: 0.3, flip: false };
    let mut e = LayerEntry::new("Orb web across the palm", Layer::Decals(DecalLayer { alpha: PALM_WEB.into(), decals: vec![stamp], feather_mm: 0.05, invert: false }));
    e.blend = Blend::Max;
    d.layers.layers.push(e);
    Ok(())
}

fn dot(a: P3, b: P3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The bare band's modulated sections, for landing feet on its cheeks and keeping legs clear of it.
struct Bare {
    d: RingDesign,
    reference: ProfileLoop,
}

impl Bare {
    fn new(d: &RingDesign) -> Self {
        Self { d: d.clone(), reference: d.reference_loop() }
    }
    fn section(&self, theta: f64) -> Vec<P2> {
        self.d.section_at(theta, 256, None, Some(&self.reference)).pts.iter().map(|p| [p.r, p.z]).collect()
    }
    /// The low cheek at a ring angle: its half-width, the radius of its top edge and the bore's.
    fn cheek(&self, theta: f64) -> (f64, f64, f64) {
        let s = self.section(theta);
        let hw = s.iter().map(|p| p[1].abs()).fold(0.0, f64::max);
        let edge = s.iter().filter(|p| p[1].abs() >= hw - 0.02).map(|p| p[0]).fold(f64::MIN, f64::max);
        let bore = s.iter().map(|p| p[0]).fold(f64::MAX, f64::min);
        (hw, edge, bore)
    }
    /// Distance from a world point to the band's section at its own angle, negative inside the metal.
    fn signed_distance(&self, w: P3) -> f64 {
        let theta = w[1].atan2(w[0]).to_degrees();
        let q = [w[0].hypot(w[1]), w[2]];
        let s = self.section(theta);
        let mut inside = false;
        let mut near = f64::MAX;
        for i in 0..s.len() {
            let (a, b) = (s[i], s[(i + 1) % s.len()]);
            if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < a[0] + (q[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
            let ab = [b[0] - a[0], b[1] - a[1]];
            let t = (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-18)).clamp(0.0, 1.0);
            near = near.min((q[0] - a[0] - ab[0] * t).hypot(q[1] - a[1] - ab[1] * t));
        }
        if inside { -near } else { near }
    }
}

/// A part frame on the ring: origin and axes, as the build seats it.
#[derive(Clone, Copy)]
struct Frame {
    o: P3,
    x: P3,
    y: P3,
    z: P3,
}

impl Frame {
    fn spun(&self, deg: f64) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        let x = std::array::from_fn(|k| self.x[k] * c + self.y[k] * s);
        let y = std::array::from_fn(|k| self.y[k] * c - self.x[k] * s);
        Self { x, y, ..*self }
    }
    /// A point of the part's section plane, `s` along x and `h` along z, in the world.
    fn world(&self, p: P2) -> P3 {
        std::array::from_fn(|k| self.o[k] + self.x[k] * p[0] + self.z[k] * p[1])
    }
    /// A world point in this frame's own coordinates.
    fn local(&self, w: P3) -> P3 {
        let d = sub3(w, self.o);
        [dot(d, self.x), dot(d, self.y), dot(d, self.z)]
    }
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
fn bezier(c: &[P2; 4], t: f64) -> P2 {
    let u = 1.0 - t;
    let (a, b, cc, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    [a * c[0][0] + b * c[1][0] + cc * c[2][0] + d * c[3][0], a * c[0][1] + b * c[1][1] + cc * c[2][1] + d * c[3][1]]
}
/// The smallest radius of curvature along a cubic.
fn tightest_bend(c: &[P2; 4]) -> f64 {
    (0..=200)
        .map(|i| {
            let t = i as f64 / 200.0;
            let u = 1.0 - t;
            let d1: P2 = std::array::from_fn(|k| 3.0 * (u * u * (c[1][k] - c[0][k]) + 2.0 * u * t * (c[2][k] - c[1][k]) + t * t * (c[3][k] - c[2][k])));
            let d2: P2 = std::array::from_fn(|k| 6.0 * (u * (c[2][k] - 2.0 * c[1][k] + c[0][k]) + t * (c[3][k] - 2.0 * c[2][k] + c[1][k])));
            let speed = d1[0].hypot(d1[1]);
            speed.powi(3) / (d1[0] * d2[1] - d1[1] * d2[0]).abs().max(1e-12)
        })
        .fold(f64::MAX, f64::min)
}

fn dir(deg: f64) -> P2 {
    let a = deg.to_radians();
    [a.cos(), a.sin()]
}
fn add(p: P2, v: P2, k: f64) -> P2 {
    [p[0] + v[0] * k, p[1] + v[1] * k]
}

/// One leg's placement and shape.
#[derive(Clone, Copy)]
struct LegSpec {
    name: &'static str,
    /// Turn of the leg's upright plane round the carapace's normal; negative reaches forward.
    spin_deg: f64,
    /// Knee position as a share of the way from the coxa to the ankle; past 1 it stands outside the foot.
    knee_share: f64,
    /// Knee height over the higher of the coxa's end and the ankle.
    knee_rise_mm: f64,
    /// Direction the tibia leaves the knee in.
    knee_out_deg: f64,
    /// Degrees round the ring the tarsus runs along the cheek from the ankle, signed.
    run_deg: f64,
    /// Where the foot sinks into the cheek, as a share of the cheek's height above the bore.
    foot_share: f64,
}

/// Coxa root, direction and length in the leg's upright plane.
const ROOT: P2 = [2.4, -0.95];
const COXA_DEG: f64 = 50.0;
const COXA_MM: f64 = 0.7;
/// Section radii at the root, femur end, knee, tibia end, ankle and tip.
const ROOT_MM: f64 = 0.72;
const FEMUR_END_MM: f64 = 0.5;
const KNEE_MM: f64 = 0.6;
const TIBIA_END_MM: f64 = 0.42;
const ANKLE_MM: f64 = 0.48;
const TIP_MM: f64 = 0.29;
/// The ankle: this far out from the cheek and down from its top edge.
const ANKLE_OUT_MM: f64 = 0.3;
const ANKLE_DOWN_MM: f64 = 0.3;
/// How deep the tarsus sits in the cheek at its middle and at its tip.
const TARSUS_EMBED_MM: f64 = 0.16;
const TIP_EMBED_MM: f64 = 0.42;

/// One swept piece of a leg: its name, path, section and taper.
struct Segment {
    name: &'static str,
    path: Sketch,
    section: Sketch,
    end_scale: f64,
}

/// One leg in the carapace's frame: femur, tibia and tarsus, and the knuckles at the knee and the ankle.
struct Leg {
    segments: Vec<Segment>,
    knuckles: Vec<(&'static str, P3, f64)>,
}

/// The point of a leg's plane at world height `z` and radius `r`, with the ring angle it lands on.
fn plane_point(f: &Frame, z: f64, r: f64) -> (P2, f64) {
    let s = z / f.x[2];
    let (mut lo, mut hi) = (-10.0, 6.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let w = f.world([s, mid]);
        if w[0].hypot(w[1]) < r { lo = mid } else { hi = mid }
    }
    let h = 0.5 * (lo + hi);
    let w = f.world([s, h]);
    ([s, h], w[1].atan2(w[0]).to_degrees())
}

/// The point of a leg's plane `off` out from the low cheek, `down` below the cheek's top edge.
fn on_cheek(f: &Frame, bare: &Bare, off: f64, down: f64) -> P2 {
    let mut theta = CARAPACE_DEG;
    let mut p = [0.0, 0.0];
    for _ in 0..10 {
        let (hw, edge, _) = bare.cheek(theta);
        let (q, t) = plane_point(f, -(hw + off), edge - down);
        p = q;
        theta = t;
    }
    p
}

/// A world point beside the low cheek at a ring angle: `share` of its height above the bore, `off` out from it.
fn beside_cheek(bare: &Bare, theta: f64, share: f64, off: f64) -> P3 {
    let (hw, edge, bore) = bare.cheek(theta);
    let r = bore + share * (edge - bore);
    let (s, c) = theta.to_radians().sin_cos();
    [r * c, r * s, -(hw + off)]
}

/// A cubic from `a` leaving along `da` to `b` arriving the way a circular arc would, its handles `h` of the span.
fn cubic(a: P2, da: f64, b: P2, h: f64) -> [P2; 4] {
    let span = (b[0] - a[0]).hypot(b[1] - a[1]);
    let chord = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees();
    [a, add(a, dir(da), h * span), add(b, dir(2.0 * chord - da), -h * span), b]
}

/// The cubic with handles nearest 0.4 of the span that bends gentler than `radius` and clears the band over `window`.
fn gentle(f: &Frame, bare: &Bare, a: P2, da: f64, b: P2, radius: [f64; 2], window: [f64; 2]) -> Option<[P2; 4]> {
    (0..=24)
        .map(|k| 0.22 + 0.02 * k as f64)
        .filter_map(|h| {
            let c = cubic(a, da, b, h);
            if tightest_bend(&c) < 1.15 * radius[0].max(radius[1]) {
                return None;
            }
            let clear = (0..=40)
                .map(|i| window[0] + (window[1] - window[0]) * i as f64 / 40.0)
                .map(|t| bare.signed_distance(f.world(bezier(&c, t))) - (radius[0] + (radius[1] - radius[0]) * t))
                .fold(f64::MAX, f64::min);
            (clear >= 0.08).then_some((h, c))
        })
        .min_by(|x, y| (x.0 - 0.4).abs().total_cmp(&(y.0 - 0.4).abs()))
        .map(|(_, c)| c)
}

/// A circle of `radius` square to a path leaving `at` along `along`, both in the carapace's frame.
fn section_at(at: P3, along: P3, radius: f64) -> Sketch {
    let t = unit3(along);
    let helper = if t[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let x = unit3(cross3(helper, t));
    let y = cross3(t, x);
    let mut s = Sketch::circle(radius);
    s.plane = Workplane { origin: at, x, y, on_face: None };
    s
}

/// Solves one leg: femur to a high knee, tibia to an ankle past the band's edge, tarsus along the cheek.
fn solve_leg(spec: &LegSpec, frame: &Frame, bare: &Bare) -> Result<Leg> {
    let f = frame.spun(spec.spin_deg);
    let (sa, ca) = spec.spin_deg.to_radians().sin_cos();
    let upright = Workplane { origin: [0.0; 3], x: [ca, sa, 0.0], y: [0.0, 0.0, 1.0], on_face: None };
    let lift = |p: P2| -> P3 { [p[0] * ca, p[0] * sa, p[1]] };
    let lift_dir = |deg: f64| -> P3 { let d = dir(deg); [d[0] * ca, d[0] * sa, d[1]] };
    let trochanter = add(ROOT, dir(COXA_DEG), COXA_MM);
    let ankle = on_cheek(&f, bare, ANKLE_MM + ANKLE_OUT_MM, ANKLE_DOWN_MM);
    let knee = [
        trochanter[0] + spec.knee_share * (ankle[0] - trochanter[0]),
        trochanter[1].max(ankle[1]) + spec.knee_rise_mm,
    ];
    let femur = gentle(&f, bare, trochanter, COXA_DEG + 4.0, knee, [ROOT_MM, FEMUR_END_MM], [0.4, 1.0])
        .with_context(|| format!("{}: no femur rises gently and clear to its knee", spec.name))?;
    let tibia = gentle(&f, bare, knee, spec.knee_out_deg, ankle, [KNEE_MM, TIBIA_END_MM], [0.0, 0.85])
        .with_context(|| format!("{}: no tibia falls gently and clear to its ankle", spec.name))?;
    let mut femur_path = Sketch { name: "Femur path".into(), plane: upright.clone(), ..Sketch::default() };
    let root = femur_path.point(ROOT);
    let pts = femur.map(|q| femur_path.point(q));
    femur_path.entity(Geometry::Line { a: root, b: pts[0] });
    femur_path.entity(Geometry::Bezier { points: pts });
    let mut tibia_path = Sketch { name: "Tibia path".into(), plane: upright, ..Sketch::default() };
    let pts = tibia.map(|q| tibia_path.point(q));
    tibia_path.entity(Geometry::Bezier { points: pts });
    // Tarsus: a circular arc from the ankle through the cheek's middle to the sunk foot.
    let a_world = f.world(ankle);
    let a_theta = a_world[1].atan2(a_world[0]).to_degrees();
    let (_, edge, bore) = bare.cheek(a_theta);
    let a_share = ((a_world[0].hypot(a_world[1]) - bore) / (edge - bore)).clamp(0.0, 1.2);
    let m_theta = a_theta + 0.55 * spec.run_deg;
    let f_theta = a_theta + spec.run_deg;
    let mid = beside_cheek(bare, m_theta, 0.5 * (a_share + spec.foot_share), 0.5 * (ANKLE_MM + TIP_MM) - TARSUS_EMBED_MM);
    let foot = beside_cheek(bare, f_theta, spec.foot_share, TIP_MM - TIP_EMBED_MM);
    let (al, ml, fl) = (lift(ankle), frame.local(mid), frame.local(foot));
    let (a, b) = (sub3(ml, al), sub3(fl, al));
    let n = unit3(cross3(a, b));
    let (aa, bb, ab) = (dot(a, a), dot(b, b), dot(a, b));
    let den = 2.0 * (aa * bb - ab * ab);
    ensure!(den.abs() > 1e-9, "{}: the tarsus's three points are in a line", spec.name);
    let (ka, kb) = (bb * (aa - ab) / den, aa * (bb - ab) / den);
    let centre: P3 = std::array::from_fn(|k| al[k] + ka * a[k] + kb * b[k]);
    let r = dot(sub3(al, centre), sub3(al, centre)).sqrt();
    ensure!(r > 1.2, "{}: the tarsus would bend at {r:.2} mm", spec.name);
    let u = unit3(sub3(al, centre));
    let v = cross3(n, u);
    let angle = |p: P3| {
        let d = sub3(p, centre);
        dot(d, v).atan2(dot(d, u)).rem_euclid(std::f64::consts::TAU)
    };
    let (am, af) = (angle(ml), angle(fl));
    ensure!(am < af, "{}: the tarsus's arc runs the long way round", spec.name);
    let mut tarsus_path = Sketch { name: "Tarsus path".into(), plane: Workplane { origin: centre, x: u, y: v, on_face: None }, ..Sketch::default() };
    let (o, s0, s1) = (tarsus_path.point([0.0, 0.0]), tarsus_path.point([r, 0.0]), tarsus_path.point([r * af.cos(), r * af.sin()]));
    tarsus_path.entity(Geometry::Arc { center: o, start: s0, end: s1 });
    // Metatarsus joint, part way along the tarsus.
    let (sm, cm) = (0.42 * af).sin_cos();
    let metatarsus: P3 = std::array::from_fn(|k| centre[k] + r * (cm * u[k] + sm * v[k]));
    let c = COXA_DEG.to_radians();
    let mut femur_section = Sketch::circle(ROOT_MM);
    femur_section.plane = Workplane { origin: lift(ROOT), x: [-sa, ca, 0.0], y: [-c.sin() * ca, -c.sin() * sa, c.cos()], on_face: None };
    println!(
        "  {}: knee ({:.2}, {:.2}), ankle at {a_theta:.1} deg share {a_share:.2}; tarsus r {r:.1} over {:.0} deg to {f_theta:.1}",
        spec.name,
        knee[0],
        knee[1],
        af.to_degrees()
    );
    Ok(Leg {
        segments: vec![
            Segment { name: "femur", path: femur_path, section: femur_section, end_scale: FEMUR_END_MM / ROOT_MM },
            Segment { name: "tibia", path: tibia_path, section: section_at(lift(knee), lift_dir(spec.knee_out_deg), KNEE_MM * 0.97), end_scale: TIBIA_END_MM / (KNEE_MM * 0.97) },
            Segment { name: "tarsus", path: tarsus_path, section: section_at(al, v, ANKLE_MM * 0.97), end_scale: TIP_MM / (ANKLE_MM * 0.97) },
        ],
        knuckles: vec![
            ("trochanter", lift(trochanter), ROOT_MM * 1.02),
            ("knee", lift(knee), KNEE_MM),
            ("ankle", al, ANKLE_MM),
            ("metatarsus", metatarsus, (ANKLE_MM + (TIP_MM - ANKLE_MM) * 0.42) * 1.12),
        ],
    })
}

fn legs(d: &mut RingDesign, lib: &AlphaLibrary) -> Result<()> {
    // The carapace frame, read off the band as swept.
    let swept = mesh::try_build(d, lib, draft_params())?;
    let surface = swept.band.clone().context("The band did not sweep")?;
    let seat = Placement::ring(CARAPACE_DEG, 0.0).frame_on(d, Some(&surface))?;
    let frame = Frame { o: seat.origin, x: seat.x_axis, y: seat.y_axis, z: seat.z_axis };
    let bare = Bare::new(d);
    let specs = [
        LegSpec { name: "Leg I", spin_deg: -56.0, knee_share: 0.62, knee_rise_mm: 1.2, knee_out_deg: -62.0, run_deg: -30.0, foot_share: 0.34 },
        LegSpec { name: "Leg II", spin_deg: -22.0, knee_share: 1.22, knee_rise_mm: 1.1, knee_out_deg: -92.0, run_deg: -20.0, foot_share: 0.30 },
        LegSpec { name: "Leg III", spin_deg: 12.0, knee_share: 1.22, knee_rise_mm: 1.1, knee_out_deg: -92.0, run_deg: 18.0, foot_share: 0.30 },
        LegSpec { name: "Leg IV", spin_deg: 42.0, knee_share: 0.68, knee_rise_mm: 1.2, knee_out_deg: -66.0, run_deg: 28.0, foot_share: 0.34 },
    ];
    let doc = d.cad.get_or_insert_with(Document::default);
    if doc.band().is_none() {
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() })?;
    }
    let mut next: Id = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let mut append = |doc: &mut Document, name: String, operation: Operation, component: Component| -> Result<Id> {
        let id = next;
        next += 1;
        doc.append(Feature { id, name, enabled: true, operation, component })?;
        Ok(id)
    };
    let at = Placement::ring(CARAPACE_DEG, 0.0);
    let part = |placement: Placement| Component { attach: Attach::Join, placement, ..Component::default() };
    for spec in &specs {
        let leg = solve_leg(spec, &frame, &bare)?;
        let mut outputs = Vec::new();
        for s in leg.segments {
            let op = Operation::Twist { sketch: s.section.into(), path: s.path, degrees: 0.0, end_scale: s.end_scale };
            outputs.push(append(doc, format!("{} {}, low side", spec.name, s.name), op, part(at.clone()))?);
        }
        for (joint, centre, radius) in leg.knuckles {
            let ball = append(doc, format!("{} {joint} knuckle", spec.name), Operation::Sphere { radius_mm: radius }, Component::default())?;
            let moved = Operation::Transform { source: ball, translation: centre, rotation_deg: [0.0; 3] };
            outputs.push(append(doc, format!("{} {joint}, low side", spec.name), moved, part(at.clone()))?);
        }
        for source in outputs {
            let name = doc.feature(source).map(|f| f.name.replace("low side", "high side")).unwrap_or_default();
            append(doc, name, Operation::Pattern { source, kind: PatternKind::Mirror { plane: MirrorPlane::Band } }, part(Placement::Free))?;
        }
    }
    Ok(())
}

/// A closed polygonal ellipse on the part's horizontal plane at height `z`.
fn ellipse(a: f64, b: f64, z: f64) -> Sketch {
    let mut s = Sketch { name: "Abdomen row".into(), plane: Workplane { origin: [0.0, 0.0, z], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], on_face: None }, ..Sketch::default() };
    let ids: Vec<Id> = (0..BELLY_SIDES)
        .map(|i| {
            let t = (i as f64 + 0.5) / BELLY_SIDES as f64 * std::f64::consts::TAU;
            s.point([a * t.cos(), b * t.sin()])
        })
        .collect();
    s.entity(Geometry::Polyline { points: ids, closed: true });
    s
}

/// Sides round each row of the abdomen's loft, and its rows.
const BELLY_SIDES: usize = 144;
const BELLY_ROWS: usize = 14;

/// The abdomen's belly: a lofted body from the collet's wall under the stone's back, swelling and falling into the band.
fn abdomen_body(d: &mut RingDesign) -> Result<()> {
    let doc = d.cad.get_or_insert_with(Document::default);
    let next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    let gem = onyx();
    let wall = ringdesign_core::setting::collet_wall_mm(gem);
    // The collet wall's plan just under the stone's back; x across the band, y round the ring.
    let (a, b) = (gem.w_mm * 0.5 + 0.03 + 0.78 * wall, gem.l_mm * 0.5 + 0.03 + 0.78 * wall);
    let (top, bottom, widest, swell, foot) = (0.26, -2.3, 0.4, 0.14, 0.34);
    // Plan scale down the belly: out to the swell, then round in to the foot.
    let scale = |t: f64| {
        if t <= widest {
            1.0 + swell * (0.5 * PI * t / widest).sin()
        } else {
            let u = (t - widest) / (1.0 - widest);
            foot + (1.0 + swell - foot) * (1.0 - u * u).max(0.0).sqrt()
        }
    };
    // Rows bottom up, along their planes' normal.
    let sections = (0..BELLY_ROWS)
        .rev()
        .map(|i| {
            let t = i as f64 / (BELLY_ROWS - 1) as f64;
            let t = 1.0 - (1.0 - t).powf(1.4);
            ellipse(a * scale(t), b * scale(t), top + (bottom - top) * t).into()
        })
        .collect();
    let component = Component { attach: Attach::Join, placement: Placement::ring(ABDOMEN_DEG, 0.0), ..Component::default() };
    doc.append(Feature { id: next, name: "Abdomen belly".into(), enabled: true, operation: Operation::Loft { sections }, component })?;
    Ok(())
}

/// The pedicel knot between carapace and abdomen, and the spinneret nub the dragline leaves from.
fn nubs(d: &mut RingDesign) -> Result<()> {
    let doc = d.cad.get_or_insert_with(Document::default);
    let mut next = doc.features.iter().map(|f| f.id).max().unwrap_or(0) + 1;
    for (name, theta, radius, height) in [("Pedicel", PEDICEL_DEG, 0.95, 0.35), ("Spinnerets", SPINNERET_DEG, 0.62, 0.12)] {
        let component = Component { attach: Attach::Join, placement: Placement::ring(theta, height), ..Component::default() };
        doc.append(Feature { id: next, name: name.into(), enabled: true, operation: Operation::Sphere { radius_mm: radius }, component })?;
        next += 1;
    }
    Ok(())
}

fn author() -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = band();
    let ctx = d.field_context();
    println!("  reference crest v {:.2} of {:.2}, crest r {:.2}", ctx.crest_v_mm, ctx.band_v_len_mm, ctx.crest_radius_mm);
    web(&mut d)?;
    palm(&mut d)?;
    let mut body = LayerEntry::new("Carapace, garnet in its collet", Layer::SeatPad(carapace(&ctx)));
    body.blend = Blend::SmoothMax;
    d.layers.layers.push(body);
    let mut belly_pad = LayerEntry::new("Abdomen, onyx in its collet", Layer::SeatPad(abdomen(&ctx)));
    belly_pad.blend = Blend::SmoothMax;
    d.layers.layers.push(belly_pad);
    let mut lib = AlphaLibrary::builtin();
    d.bake_all(&mut lib);
    legs(&mut d, &lib)?;
    abdomen_body(&mut d)?;
    nubs(&mut d)?;
    Ok((d, lib))
}

/// The ring's gates and figures, as report.json carries them.
#[derive(serde::Serialize)]
struct Report {
    name: String,
    process: String,
    size: String,
    bore_mm: f64,
    build: [usize; 2],
    triangles: usize,
    build_s: f64,
    watertight: bool,
    boundary_edges: usize,
    non_manifold_edges: usize,
    degenerate_faces: usize,
    made_parts: Vec<(String, usize)>,
    seat_solids: Vec<(String, usize)>,
    solids_notes: Vec<String>,
    parts_notes: Vec<String>,
    parts_joined: usize,
    field_verdict: String,
    field_notes: Vec<String>,
    /// Undercut read on the CAD parts against a two-part pull; reported under lost wax, never gating.
    parts_undercut_mm2: f64,
    parts_area_mm2: f64,
    undercut_percent: f64,
    worst_draft_deg: f64,
    thinnest_wall_mm: f64,
    thinnest_wall_theta_deg: f64,
    investment_min_section_mm: f64,
    leg_min_diameter_mm: f64,
    leg_min_exposed_diameter_mm: f64,
    clamp_bite_mm: Option<f64>,
    dfm_findings: Vec<String>,
    stones_reported: u32,
    stones_previewed: usize,
    stone_carats: f64,
    stone_warnings: Vec<String>,
    closest_stones: Option<String>,
    layers: Vec<String>,
    cad_features: usize,
    grams_18k: f64,
    cold_reload_identical: Option<bool>,
    gates_passed: bool,
}

/// The preview stones split by tint, each welded and smoothed so a cabochon reads as polished.
fn stones(d: &RingDesign, lib: &AlphaLibrary) -> Vec<(mesh::Mesh, [f32; 3])> {
    let v = ringdesign_core::gems::preview_vertices(d, lib);
    const STRIDE: usize = 12;
    let mut groups: Vec<([f32; 3], Vec<[f32; 3]>)> = Vec::new();
    for tri in v.chunks_exact(STRIDE * 3) {
        let tint = [tri[6], tri[7], tri[8]];
        let at = match groups.iter().position(|(t, _)| *t == tint) {
            Some(i) => i,
            None => {
                groups.push((tint, Vec::new()));
                groups.len() - 1
            }
        };
        for k in 0..3 {
            groups[at].1.push([tri[k * STRIDE], tri[k * STRIDE + 1], tri[k * STRIDE + 2]]);
        }
    }
    groups
        .into_iter()
        .map(|(tint, pts)| {
            let mut m = mesh::Mesh::default();
            let mut index = std::collections::HashMap::new();
            for tri in pts.chunks_exact(3) {
                let f = std::array::from_fn(|k| {
                    let key = tri[k].map(|c| (c * 1e4).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        m.vertices.push(mesh::Vec3(tri[k][0], tri[k][1], tri[k][2]));
                        (m.vertices.len() - 1) as u32
                    })
                });
                m.faces.push(f);
            }
            m.normals = vec![mesh::Vec3(0.0, 0.0, 0.0); m.vertices.len()];
            for f in &m.faces {
                let [a, b, c] = f.map(|i| m.vertices[i as usize]);
                let (e1, e2) = ([b.0 - a.0, b.1 - a.1, b.2 - a.2], [c.0 - a.0, c.1 - a.1, c.2 - a.2]);
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                for &i in f {
                    let v = &mut m.normals[i as usize];
                    v.0 += n[0];
                    v.1 += n[1];
                    v.2 += n[2];
                }
            }
            for n in &mut m.normals {
                let l = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt().max(1e-12);
                *n = mesh::Vec3(n.0 / l, n.1 / l, n.2 / l);
            }
            (m, tint)
        })
        .collect()
}

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let near = |i: u32| {
        let p = m.vertices[i as usize];
        (p.0 as f64 - centre[0]).hypot(p.1 as f64 - centre[1]).hypot(p.2 as f64 - centre[2]) < radius
    };
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    for f in m.faces.iter().filter(|f| f.iter().all(|&i| near(i))) {
        let g = f.map(|i| {
            *index.entry(i).or_insert_with(|| {
                out.vertices.push(m.vertices[i as usize]);
                out.normals.push(m.normals.get(i as usize).copied().unwrap_or(mesh::Vec3(0.0, 0.0, 1.0)));
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

fn side_by_side(path: &Path, left: &[u8], right: &[u8], edge: usize) -> Result<()> {
    let mut out = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        out[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        out[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    image::save_buffer(path, &out, (edge * 2) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// Studio-gold renders with stones: named views, a spider close-up, and bare stock against the finished ring.
fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult, edge: usize) -> Result<()> {
    let gems = stones(d, lib);
    let stone_parts = || {
        gems.iter()
            .map(|(m, tint)| {
                let mut p = render::Part::stone(m);
                p.tint = *tint;
                p.smooth = true;
                p
            })
            .collect::<Vec<_>>()
    };
    let mut parts = vec![render::Part::metal(&built.mesh, render::GOLD)];
    parts.extend(stone_parts());
    for (name, yaw, pitch) in [
        ("hero", 0.55, 0.95),
        ("face", 0.0, PI * 0.5),
        ("palm", PI, 1.05),
        ("side", 0.0, 0.0),
        ("shoulder", -0.9, 0.62),
        ("reverse", 1.6, 0.8),
    ] {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    // The close-up frames on the metal round the stones and draws the whole ring.
    let frames = ringdesign_core::stones::stone_frames(d);
    let centre: P3 = std::array::from_fn(|k| frames.iter().map(|(_, f)| f.girdle[k]).sum::<f64>() / frames.len().max(1) as f64);
    let spider = crop(&built.mesh, centre, 11.0);
    let mut close = vec![render::Part::metal(&spider, render::GOLD), render::Part::metal(&built.mesh, render::GOLD)];
    close.extend(stone_parts());
    render::write_png_parts(out.join("stones.png"), &close, 0.35, 1.05, edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = band();
    bare.name = d.name.clone();
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], 0.55, 0.95, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, 0.55, 0.95, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    Ok(())
}

/// Every made part's self-crossings: the CAD parts as placed, and each seat's collet and relief.
fn crossings(d: &RingDesign, built: &mesh::BuildResult) -> (Vec<(String, usize)>, Vec<(String, usize)>) {
    let parts = built
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
        .collect();
    let mut seats = Vec::new();
    for (st, _) in ringdesign_core::stones::stone_frames(d) {
        let fit = setting::Fit { surface_z: -0.35, through_mm: None, prongs: 0 };
        if let Ok(p) = setting::parts(st.gem, SolidKind::Bezel, fit) {
            for (k, s) in p.add.iter().chain(p.cut.iter()).enumerate() {
                seats.push((format!("{} collet solid {}", st.gem.display(), k + 1), csg::self_crossings(s)));
            }
        }
    }
    (parts, seats)
}

/// The legs' narrowest section, and the narrowest left standing clear of the cheek.
fn leg_sections() -> (f64, f64) {
    // Exposed tarsus radius just short of the sunk tip.
    let exposed = TIP_MM + (ANKLE_MM - TIP_MM) * 0.12;
    (2.0 * TIP_MM, 2.0 * exposed)
}

fn investment(d: &mut RingDesign) {
    let mut setup = mf::Setup::default();
    setup.recipe = mf::Recipe {
        name: "Lost wax / Gold 18k".into(),
        process: CastProcess::LostWax,
        sand: None,
        alloy: "Gold 18k".into(),
        shrink_pct: 1.3,
        min_draft_deg: 0.0,
        min_section_mm: d.draft.min_section_mm,
        min_detail_mm: d.draft.min_detail_mm,
        calibration_note: "Investment cast in 18k; legs sprued from the carapace, the abdomen's belly vented. Collets burnished over the onyx and the garnet at the bench.".into(),
        ..mf::Recipe::default()
    };
    d.manufacturing = Some(setup);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/bestiarium/arachne")
    });
    std::fs::create_dir_all(&out)?;
    println!("Arachne");
    let (mut d, lib) = author()?;
    investment(&mut d);
    let params = if draft { draft_params() } else { export_params() };
    d.build = params;
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    println!("  {} triangles in {build_s:.1} s; watertight {}; degenerate {}", built.mesh.faces.len(), v.watertight, built.report.quality.degenerate_faces);
    let (made_parts, seat_solids) = crossings(&d, &built);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = stones(&d, &lib).len();
    let (leg_min, leg_exposed) = leg_sections();
    let grams = built.report.metals.iter().find(|m| m.metal == "Gold 18k").map_or(0.0, |m| m.grams);
    let mut warnings: Vec<String> = stones_report.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let cold = if verify {
        let saved = library::load_design(out.join("design.ring.json"))?;
        let cold_lib = mf::source_library(&saved, &AlphaLibrary::default()).into_owned();
        let rebuilt = mesh::try_build(&saved, &cold_lib, params)?;
        let same = rebuilt.mesh.vertices == built.mesh.vertices && rebuilt.mesh.faces == built.mesh.faces;
        println!("  cold reload with an empty library: {}", if same { "identical" } else { "DIFFERENT" });
        Some(same)
    } else {
        None
    };
    let mut report = Report {
        name: d.name.clone(),
        process: d.draft.process.label().into(),
        size: d.size.display(),
        bore_mm: built.report.inner_diameter_mm,
        build: [params.theta_steps, params.profile_steps],
        triangles: built.mesh.faces.len(),
        build_s,
        watertight: v.watertight,
        boundary_edges: v.boundary_edges,
        non_manifold_edges: v.non_manifold_edges,
        degenerate_faces: built.report.quality.degenerate_faces,
        made_parts,
        seat_solids,
        solids_notes: built.solids.notes.clone(),
        parts_notes: built.parts.notes.clone(),
        parts_joined: built.parts.joined,
        field_verdict: field.verdict.label().into(),
        field_notes: field.notes.clone(),
        parts_undercut_mm2: field.parts.iter().map(|p| p.undercut_area_mm2).sum(),
        parts_area_mm2: field.parts.iter().map(|p| p.total_area_mm2).sum(),
        undercut_percent: field.undercut_fraction() * 100.0,
        worst_draft_deg: field.worst_draft_deg,
        thinnest_wall_mm: field.thinnest_wall_mm,
        thinnest_wall_theta_deg: field.thinnest_wall_theta_deg,
        investment_min_section_mm: d.draft.min_section_mm,
        leg_min_diameter_mm: leg_min,
        leg_min_exposed_diameter_mm: leg_exposed,
        clamp_bite_mm: None,
        dfm_findings: findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect(),
        stones_reported: stones_report.as_ref().map_or(0, |s| s.stone_count),
        stones_previewed: previewed,
        stone_carats: stones_report.as_ref().map_or(0.0, |s| s.total_carats),
        stone_warnings: warnings,
        closest_stones: stones_report.as_ref().and_then(|s| s.closest.as_ref()).map(|p| format!("{} to {}: {:.2} mm at the girdle, {:.2} mm deep", p.a, p.b, p.gap_mm, p.gap_deep_mm)),
        layers: d.layers.layers.iter().map(|e| e.name.clone()).collect(),
        cad_features: d.cad.as_ref().map_or(0, |c| c.features.len()),
        grams_18k: grams,
        cold_reload_identical: cold,
        gates_passed: false,
    };
    report.gates_passed = report.watertight
        && report.degenerate_faces == 0
        && report.made_parts.iter().chain(&report.seat_solids).all(|(_, n)| *n == 0)
        && report.solids_notes.is_empty()
        && report.field_verdict == castability::Verdict::Castable.label()
        && report.thinnest_wall_mm >= 0.8
        && report.dfm_findings.is_empty()
        && report.stones_reported as usize == report.stones_previewed
        && report.cold_reload_identical != Some(false);
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    std::fs::write(out.join("mesh.json"), serde_json::to_vec_pretty(&built.report)?)?;
    let art = out.join("artwork");
    std::fs::create_dir_all(&art)?;
    for svg in &d.svgs {
        std::fs::write(art.join(format!("{}.svg", svg.name.to_lowercase().replace(' ', "-"))), &svg.svg)?;
    }
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
    }
    renders(&out, &d, &lib, &built, if draft { 1000 } else { 1600 })?;
    println!(
        "  field {} ({:.4}% undercut), thinnest wall {:.2} mm at {:.0} deg; dfm {}; stones {} reported, {} previewed; made parts {} with crossings; solids notes {}; parts notes {}",
        report.field_verdict,
        report.undercut_percent,
        report.thinnest_wall_mm,
        report.thinnest_wall_theta_deg,
        report.dfm_findings.len(),
        report.stones_reported,
        report.stones_previewed,
        report.made_parts.iter().chain(&report.seat_solids).filter(|(_, n)| *n > 0).count(),
        report.solids_notes.len(),
        report.parts_notes.len()
    );
    for f in &report.dfm_findings {
        println!("    dfm: {f}");
    }
    for n in &report.parts_notes {
        println!("    part: {n}");
    }
    println!("  gates {}", if report.gates_passed { "passed" } else { "FAILED" });
    ensure!(report.gates_passed, "Arachne failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
