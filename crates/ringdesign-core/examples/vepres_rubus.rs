//! Vepres — Rubus, the bramble cane: one knuckled cane closed into a ring, its hooked prickles lying in the
//! parting plane along the crest, a fruiting runner with leaflets and blackberries down both side faces.
//! Delft sand. cargo build --release -p ringdesign-core --example vepres_rubus
//! target/release/examples/vepres_rubus [OUT_DIR] [--draft] [--verify] [--blockout]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, FieldContext, ProfileStyle, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, Stage},
    castability,
    csg, dfm,
    curve::{CurveLayer, WireProfile},
    field::{Blend, Decal, DecalLayer, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SideFacePick, SurfaceProfile, VGate},
    library, mesh,
    outline::{self, Margin},
    profile::{REFERENCE_PROFILE_STEPS, ShankKey, ShankKind},
    render,
    setting::{Stamp, StampTop},
    sketch::{Geometry, Sketch, Workplane},
    svg::SvgAlpha,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

#[path = "common/probe.rs"]
mod probe;

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// Seven leaf nodes, the first on the crest.
const NODES: usize = 7;
const NODE0_DEG: f64 = 90.0;
const STEP_DEG: f64 = 360.0 / NODES as f64;
/// The brief's knuckle contrast, with P5's station-aware gates: (width, thickness, crown). The upper nodes swell
/// further, so their faces carry the berry clusters; the palm nodes stay nearer the brief's 1.15.
const NODE_KEY: (f64, f64, f64) = (1.05, 1.35, 1.10);
const PALM_NODE_KEY: (f64, f64, f64) = (1.03, 1.15, 1.05);
const INTERNODE_KEY: (f64, f64, f64) = (0.97, 0.93, 0.95);
/// The first large prickle, on the node at 347.14 deg; five over 205.71 deg land on the upper nodes.
const LARGE_FROM_DEG: f64 = 347.142_857;
const LARGE_SPAN_DEG: f64 = 205.714_286;
/// The two small prickles of each upper internode pair, both in its upper half, so the crown leaf springing from
/// the lower node has the first and no hook hangs over it; four of each over 154.29 deg.
const SMALL_A_DEG: f64 = 20.071_429;
const SMALL_B_DEG: f64 = 28.571_429;
const SMALL_SPAN_DEG: f64 = 154.285_714;
/// How deep each prickle's foot is sunk into the cane, and its seam bead.
const PRICKLE_SINK_MM: f64 = 0.35;
const PRICKLE_BLEAD_MM: f64 = 0.35;
/// The blackberry: one drupelet cluster per node per face.
const BERRY_MM: f64 = 2.15;
/// The calyx and stalk over the berry, mm.
const CALYX_MM: f64 = 0.45;
const DRUPELET_MM: f64 = 0.52;
const BERRY_HIGH_MM: f64 = 1.0;
/// Each upper node's cluster: the berry on the node and a smaller one either side, this far round, scaled so.
const CLUSTER_DEG: f64 = 9.5;
const CLUSTER_SCALE: f64 = 0.92;
/// The art's turn on the low face; the high face's is its mirror.
const BERRY_TURN_DEG: f64 = 0.0;
const BERRY_ALPHA: &str = "Drupelet cluster";
/// The runner: a round wire on both side faces, filling this share of the face.
const RUNNER_W_MM: f64 = 1.1;
const RUNNER_H_MM: f64 = 0.55;
const RUNNER_FILL: f64 = 0.7;
/// The leaflet: a serrated blade along the ring, its tip turned from the tangent by `LEAF_POINT_DEG`.
const LEAF_LEN_MM: f64 = 4.2;
const LEAF_W_MM: f64 = 1.9;
/// Low eaves and a high fold: the blade reads by its midrib, not as a slab.
const LEAF_HIGH_MM: f64 = 0.3;
const LEAF_RISE_MM: f64 = 0.4;
const LEAF_POINT_DEG: f64 = 180.0;
const LEAF_TURN_DEG: f64 = 8.0;
const LEAF_CROWNWARD_MM: f64 = 0.0;
/// The crown leaf: its stalk's foot this far from the node, its leaflets (length, width, centre's reach from the
/// foot), the laterals spread this far off the terminal, eaves height and the terminal's fold.
const CROWN_STALK_MM: f64 = 1.6;
const CROWN_TERMINAL: (f64, f64, f64) = (3.6, 2.2, 2.6);
const CROWN_LATERAL: (f64, f64, f64) = (2.9, 1.5, 2.2);
const CROWN_SPREAD_DEG: f64 = 46.0;
/// The stalk the leaflets spring from, mm across.
const STALK_MM: f64 = 0.45;
const CROWN_HIGH_MM: f64 = 0.35;
const CROWN_RISE_MM: f64 = 0.3;
/// Bench grooves down each leaflet's midrib, mm across.
const MIDRIB_MM: f64 = 0.18;
/// A tier-1 cut is sunk from the leaf's own top: the midrib 0.12 mm into it, the sinuses through it to the cane.
const MIDRIB_SINK_MM: f64 = 0.12;
const SINUS_SINK_MM: f64 = CROWN_HIGH_MM + 0.1;

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, ..BuildParams::default() }
}

fn node_deg(k: usize) -> f64 {
    (NODE0_DEG + k as f64 * STEP_DEG).rem_euclid(360.0)
}

/// Whether node `k` is one of the five on the upper crest, 347 through 90 to 193 deg; 244 and 296 are the palm's.
fn upper(k: usize) -> bool {
    !matches!(k, 3 | 4)
}

/// The cane: a LowDome 7.0 x 3.4 with flat sides, knuckled at seven nodes.
fn cane() -> RingDesign {
    let mut d = RingDesign::default();
    d.name = "Rubus".into();
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.width_mm = 7.0;
    d.profile.thickness_mm = 3.4;
    d.profile.apply_style(ProfileStyle::LowDome);
    d.profile.crown_mm = 0.6;
    d.profile.flatten_sides();
    d.profile.edge_round_mm = 0.3;
    d.profile.comfort_fit_mm = 0.1;
    d.shank.kind = ShankKind::Keyframes;
    d.shank.amount = 1.0;
    d.shank.keys = (0..NODES)
        .flat_map(|k| {
            let node = node_deg(k);
            let key = |theta: f64, (w, t, c): (f64, f64, f64)| ShankKey { theta_deg: theta.rem_euclid(360.0), width_scale: w, thickness_scale: t, crown_scale: c };
            [key(node, if upper(k) { NODE_KEY } else { PALM_NODE_KEY }), key(node + STEP_DEG * 0.5, INTERNODE_KEY)]
        })
        .collect();
    probe::cast_in(&mut d, &probe::sand_setup(0.1));
    // The pour parts on the mid-plane, where every prickle lies; judge the field there too.
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d
}

/// A prickle's centreline in the part's tangent-radial plane: `radial` mm straight out, then an arc of `bend_r`
/// turning `bend_deg` round the ring (the prickle probe's path).
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

/// A prickle: an elliptic foot `along` x `across` mm (round the ring, along the finger), swept up `radial` mm then
/// round a `bend_r` hook of `bend_deg`, tapering to `end_scale` of the foot.
#[derive(Clone, Copy)]
struct Hook {
    along: f64,
    across: f64,
    radial: f64,
    bend_r: f64,
    bend_deg: f64,
    end_scale: f64,
}

const LARGE: Hook = Hook { along: 2.0, across: 1.2, radial: 0.7, bend_r: 2.0, bend_deg: 45.0, end_scale: 0.26 };
const SMALL: Hook = Hook { along: 1.5, across: 0.9, radial: 0.5, bend_r: 1.4, bend_deg: 42.0, end_scale: 0.36 };

/// A closed ellipse as a polyline, centred on the sketch origin: `a` mm full along the path's turn (round the
/// ring), `b` mm across it (along the finger).
fn ellipse(a: f64, b: f64) -> Sketch {
    let mut s = Sketch::default();
    s.name = "Prickle foot".into();
    let points: Vec<_> = (0..48)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / 48.0;
            s.point([0.5 * b * t.cos(), 0.5 * a * t.sin()])
        })
        .collect();
    s.entity(Geometry::Polyline { points, closed: true });
    s
}

fn prickle(h: Hook) -> Operation {
    Operation::Twist { sketch: ellipse(h.along, h.across).into(), path: hook_path(h.radial, h.bend_r, h.bend_deg), degrees: 0.0, end_scale: h.end_scale }
}

fn seated(theta: f64) -> Component {
    let mut c = Component::default();
    c.placement = Placement::Ring { theta_deg: theta, across_mm: 0.0, height_mm: -PRICKLE_SINK_MM, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 };
    c.attach = Attach::Join;
    c.stage = Stage::Cast;
    c.blend_mm = PRICKLE_BLEAD_MM;
    c
}

/// The CAD document: the band, the large prickle and its ring array, and the two small ones and theirs.
fn prickles(d: &mut RingDesign) -> Result<()> {
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Cane".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Default::default() } })?;
    doc.append(Feature { id: 2, name: "Large prickle".into(), enabled: true, operation: prickle(LARGE), component: seated(LARGE_FROM_DEG) })?;
    let mut array = seated(LARGE_FROM_DEG);
    array.placement = Placement::Free;
    doc.append(Feature { id: 3, name: "Large prickles on the upper nodes".into(), enabled: true, operation: Operation::Pattern { sources: 2.into(), kind: PatternKind::Ring { count: 5, span_deg: LARGE_SPAN_DEG } }, component: array.clone() })?;
    doc.append(Feature { id: 4, name: "Small prickle A".into(), enabled: true, operation: prickle(SMALL), component: seated(SMALL_A_DEG) })?;
    doc.append(Feature { id: 5, name: "Small prickle B".into(), enabled: true, operation: prickle(SMALL), component: seated(SMALL_B_DEG) })?;
    doc.append(Feature { id: 6, name: "Small prickles between the upper nodes".into(), enabled: true, operation: Operation::Pattern { sources: ringdesign_core::cad::pattern::Sources(vec![4, 5]), kind: PatternKind::Ring { count: 4, span_deg: SMALL_SPAN_DEG } }, component: array })?;
    d.cad = Some(doc);
    Ok(())
}

/// A domed blackberry hanging from its calyx: drupelets hex-packed over a receptacle, each a radial gradient
/// black at its crown and darker toward the berry's middle so the cluster domes, under five sepals and a short
/// stalk at the top of the art. The receptacle stays over half ink, so the gaps between drupelets are shallow
/// valleys in one mass, never gaps the sand must fill.
fn drupelet_svg() -> String {
    let w = 100.0;
    let per_mm = w / BERRY_MM;
    let top = CALYX_MM * per_mm;
    let h = w + top;
    let pitch = DRUPELET_MM * per_mm;
    let (cx, cy) = (w * 0.5, top + w * 0.5);
    let mut art = String::new();
    // The stalk, over the berry's top.
    art.push_str(&format!(r##"<rect x="{:.2}" y="0" width="{:.2}" height="{:.2}" fill="#6a6a6a"/>"##, cx - 0.22 * per_mm, 0.44 * per_mm, top + 6.0));
    art.push_str(&format!(r##"<ellipse cx="{cx}" cy="{cy:.2}" rx="{:.2}" ry="{:.2}" fill="url(#base)"/>"##, w * 0.49, w * 0.49));
    for row in -3i32..=3 {
        for col in -3i32..=3 {
            let x = (col as f64 + 0.5 * row as f64) * pitch;
            let y = row as f64 * pitch * 0.866;
            let e = x.hypot(y) / (w * 0.5);
            if e <= 0.74 {
                let g = if e < 0.3 { "d0" } else if e < 0.6 { "d1" } else { "d2" };
                art.push_str(&format!(r##"<circle cx="{:.2}" cy="{:.2}" r="{:.2}" fill="url(#{g})"/>"##, cx + x, cy + y, 0.5 * pitch));
            }
        }
    }
    let star: Vec<String> = (0..10)
        .map(|i| {
            let a = (-90.0 + 36.0 * i as f64).to_radians();
            let rr = if i % 2 == 0 { 0.8 } else { 0.42 } * per_mm;
            format!("{:.2},{:.2}", cx + rr * a.cos(), top + 0.95 * per_mm + rr * a.sin())
        })
        .collect();
    art.push_str(&format!(r##"<polygon points="{}" fill="#3c3c3c"/>"##, star.join(" ")));
    let grad = |id: &str, crown: &str, rim: &str| format!(r##"<radialGradient id="{id}" cx="50%" cy="50%" r="50%"><stop offset="0%" stop-color="{crown}"/><stop offset="100%" stop-color="{rim}"/></radialGradient>"##);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h:.2}"><defs>{}{}{}<radialGradient id="base"><stop offset="0%" stop-color="#383838"/><stop offset="55%" stop-color="#555"/><stop offset="92%" stop-color="#787878"/><stop offset="100%" stop-color="#fff"/></radialGradient></defs>{art}</svg>"##,
        grad("d0", "#000", "#383838"),
        grad("d1", "#141414", "#555"),
        grad("d2", "#383838", "#787878"),
    )
}

/// The side faces at `theta` in the chart's v, low and high, from P5's station-aware gates.
fn faces_at(ctx: &FieldContext, theta: f64) -> Result<((f64, f64), (f64, f64))> {
    let f = ctx.side_faces_at(theta).ok_or_else(|| anyhow::anyhow!("no side faces at {theta:.1} deg"))?;
    match (f.low, f.high) {
        (Some(l), Some(h)) => Ok((l, h)),
        _ => anyhow::bail!("one side face missing at {theta:.1} deg"),
    }
}

/// A cluster of three blackberries at each upper node and one at each palm node, on both faces, gated to them.
fn blackberries(d: &mut RingDesign) -> Result<()> {
    d.svgs.push(SvgAlpha { name: BERRY_ALPHA.into(), svg: drupelet_svg(), invert: false });
    let ctx = d.field_context();
    let mut decals = Vec::new();
    for k in 0..NODES {
        let node = node_deg(k);
        let cluster: &[(f64, f64)] = if upper(k) { &[(0.0, 1.0), (-CLUSTER_DEG, CLUSTER_SCALE), (CLUSTER_DEG, CLUSTER_SCALE)] } else { &[(0.0, CLUSTER_SCALE)] };
        for &(off, scale) in cluster {
            let theta = (node + off).rem_euclid(360.0);
            let (lo, hi) = faces_at(&ctx, theta)?;
            for (v, flip, rot) in [(0.5 * (lo.0 + lo.1), false, BERRY_TURN_DEG), (0.5 * (hi.0 + hi.1), true, 180.0 - BERRY_TURN_DEG)] {
                decals.push(Decal { theta_deg: theta, v_mm: v, size_mm: BERRY_MM * scale, rotation_deg: rot, height_mm: BERRY_HIGH_MM * scale.sqrt(), flip });
            }
        }
    }
    let mut e = LayerEntry::new("Blackberries", Layer::Decals(DecalLayer { alpha: BERRY_ALPHA.into(), decals, feather_mm: 0.05, invert: false }));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    Ok(())
}

/// The runner: one arch per internode cell, cresting toward the crown at each node, on both side faces.
fn runner(d: &mut RingDesign) -> Result<CurveLayer> {
    let ctx = d.field_context();
    // Cell x wraps from theta 0, and the low face's v rises toward the bore: the arch's trough (x 0.75 drawn) is
    // carried onto the nodes' fraction of a cell, so the runner swings to the crown edge at each node.
    let shift = (NODE0_DEG / STEP_DEG).fract() + 0.25;
    let mut c = CurveLayer {
        points: [[0.0, 0.0], [0.25, 1.0], [0.5, 0.0], [0.75, -1.0], [1.0, 0.0]].iter().map(|p| [p[0] + shift, p[1]]).collect(),
        repeats_around: NODES as u32,
        closed: false,
        width_mm: RUNNER_W_MM,
        height_mm: RUNNER_H_MM,
        profile: WireProfile::Round,
        taper: 0.0,
        mirror_v: true,
    };
    ensure!(c.land_on_side_face(&ctx, RUNNER_FILL), "the cane has no side face to carry the runner");
    let mut e = LayerEntry::new("Runner", Layer::Curve(c.clone()));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    Ok(c)
}

/// The leaflet blade: four saw teeth a side leaning to the tip.
fn leaf_outline() -> Vec<[f64; 2]> {
    outline::leaf(Margin::Serrate { teeth: 4, depth_mm: 0.22, lean_deg: 35.0 }, LEAF_LEN_MM, LEAF_W_MM)
}

/// One serrated leaflet per internode on each face, folded on its midrib, laid along the runner where it has
/// swung to the bore edge, alternate ones turned so neighbours read as a spray.
fn leaflets(d: &mut RingDesign, run: &CurveLayer) -> Result<Vec<serde_json::Value>> {
    let ctx = d.field_context();
    let path = run.sample_path(32);
    let cell_deg = STEP_DEG;
    let mut out = Vec::new();
    let outline = leaf_outline();
    for k in 0..NODES {
        let theta = (node_deg(k) + 0.5 * STEP_DEG).rem_euclid(360.0);
        let (lo, hi) = faces_at(&ctx, theta)?;
        // The runner's tangent at this theta, cell x to degrees and v to mm.
        let x = (theta / cell_deg).fract();
        let near = |x: f64| {
            path.iter().min_by(|a, b| {
                let da = (a[0] - x).rem_euclid(1.0).min((x - a[0]).rem_euclid(1.0));
                let db = (b[0] - x).rem_euclid(1.0).min((x - b[0]).rem_euclid(1.0));
                da.total_cmp(&db)
            }).copied().unwrap_or([x, 0.0])
        };
        let (a, b) = (near(x - 0.02), near(x + 0.02));
        let slope_deg = ((b[1] - a[1]) / (0.04 * ctx.circumference_mm / NODES as f64)).atan().to_degrees();
        let turn = if k % 2 == 0 { LEAF_TURN_DEG } else { -LEAF_TURN_DEG };
        for (face, (a0, a1), crownward) in [("low", lo, 1.0), ("high", hi, -1.0)] {
            let v = 0.5 * (a0 + a1) + crownward * LEAF_CROWNWARD_MM;
            // The chart's v runs the other way on the high face, so the tangent's slope turns with it.
            let rot = LEAF_POINT_DEG + crownward * slope_deg + turn * crownward;
            d.stamps.push(Stamp {
                name: format!("Leaflet, {}", 2 * k + usize::from(face == "high") + 1),
                theta_deg: theta,
                v_mm: v,
                rot_deg: rot,
                outline: outline.clone(),
                height_mm: LEAF_HIGH_MM,
                sink_mm: 0.25,
                draft_deg: 3.0,
                cut: false,
                bench: false,
                along_pull: true,
                tier: 0,
                top: StampTop::Ridge { rise_mm: LEAF_RISE_MM, from: [-0.5 * LEAF_LEN_MM, 0.0], to: [0.5 * LEAF_LEN_MM, 0.0], end_mm: 0.05 },
                fine_cap: true,
            });
            out.push(json!({"theta_deg": theta, "face": face, "v_mm": v, "rot_deg": rot, "face_mm": [a0, a1]}));
        }
    }
    Ok(out)
}

fn stamp(name: String, theta: f64, v: f64, rot: f64, outline: Vec<[f64; 2]>, height: f64, top: StampTop) -> Stamp {
    Stamp { name, theta_deg: theta, v_mm: v, rot_deg: rot, outline, height_mm: height, sink_mm: 0.25, draft_deg: 3.0, cut: false, bench: false, along_pull: false, tier: 0, top, fine_cap: true }
}

/// The signs that carry a stamp's own x (along) and y (across) onto the chart's theta and v at `(theta, v)`.
fn chart_signs(d: &RingDesign, ctx: &FieldContext, theta: f64, v: f64) -> (f64, f64) {
    let at = |t: f64, w: f64| stamp(String::new(), t, w, 0.0, outline::circle(1.0), 0.1, StampTop::Flat).frame(d, ctx);
    let f = at(theta, v);
    let (ft, fv) = (at(theta + 1.0, v), at(theta, v + 0.3));
    let dot = |a: [f64; 3], b: [f64; 3], o: [f64; 3]| (0..3).map(|i| (a[i] - o[i]) * b[i]).sum::<f64>();
    (dot(ft.origin, f.x, f.origin).signum(), dot(fv.origin, f.y, f.origin).signum())
}

/// `o` turned `rot_deg` about the origin, then moved `reach` mm out along that heading.
fn placed(o: &[[f64; 2]], rot_deg: f64, reach: f64) -> Vec<[f64; 2]> {
    let (sn, cs) = rot_deg.to_radians().sin_cos();
    o.iter().map(|p| [p[0] * cs - p[1] * sn + reach * cs, p[0] * sn + p[1] * cs + reach * sn]).collect()
}

/// Where the line `x` crosses polygon `o`: its lowest and highest `y`.
fn span_at(o: &[[f64; 2]], x: f64) -> Option<(f64, f64)> {
    let mut hit: Option<(f64, f64)> = None;
    for i in 0..o.len() {
        let (a, b) = (o[i], o[(i + 1) % o.len()]);
        if (a[0] - x) * (b[0] - x) <= 0.0 && (a[0] - b[0]).abs() > 1e-12 {
            let y = a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0]);
            hit = Some(hit.map_or((y, y), |(lo, hi)| (lo.min(y), hi.max(y))));
        }
    }
    hit
}

/// `o` with every edge longer than the outline step split evenly.
fn densified(o: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for i in 0..o.len() {
        let (a, b) = (o[i], o[(i + 1) % o.len()]);
        let k = ((a[0] - b[0]).hypot(a[1] - b[1]) / (outline::STEP * 0.95)).ceil().max(1.0) as usize;
        out.extend((0..k).map(|j| {
            let t = j as f64 / k as f64;
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        }));
    }
    out
}

fn x_range(o: &[[f64; 2]]) -> (f64, f64) {
    o.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[0]), b.max(p[0])))
}

/// The plan between `lower` and `upper` (functions of x giving y, `None` where absent) over their largest shared
/// run, as a counter-clockwise polygon.
fn band_between(xs: &[f64], lower: impl Fn(f64) -> Option<f64>, upper: impl Fn(f64) -> Option<f64>) -> Option<Vec<[f64; 2]>> {
    let mut runs: Vec<Vec<(f64, f64, f64)>> = vec![Vec::new()];
    for &x in xs {
        match (lower(x), upper(x)) {
            (Some(lo), Some(hi)) if hi - lo > 0.02 => runs.last_mut().unwrap().push((x, lo, hi)),
            _ => {
                if !runs.last().unwrap().is_empty() {
                    runs.push(Vec::new());
                }
            }
        }
    }
    let area = |r: &Vec<(f64, f64, f64)>| r.iter().map(|&(_, lo, hi)| hi - lo).sum::<f64>();
    let run = runs.into_iter().filter(|r| r.len() >= 4).max_by(|a, b| area(a).total_cmp(&area(b)))?;
    let mut poly: Vec<[f64; 2]> = run.iter().map(|&(x, lo, _)| [x, lo]).collect();
    poly.extend(run.iter().rev().map(|&(x, _, hi)| [x, hi]));
    Some(densified(&poly))
}

/// One crown leaf in its own plan, the stalk's foot at the origin and the terminal along +x: the filled outline
/// the pattern carries (every line along the pull crosses it once, through the parting line), the sinuses
/// between the leaflets that the bench cuts back out of it, and each leaflet's midrib as (heading, reach, length).
struct CrownLeaf {
    filled: Vec<[f64; 2]>,
    sinuses: Vec<Vec<[f64; 2]>>,
    midribs: Vec<(f64, f64, f64)>,
}

fn crown_leaf() -> Result<CrownLeaf> {
    let (tl, tw, tr) = CROWN_TERMINAL;
    let (ll, lw, lr) = CROWN_LATERAL;
    let terminal = placed(&outline::leaf(Margin::Serrate { teeth: 6, depth_mm: 0.3, lean_deg: 35.0 }, tl, tw), 0.0, tr);
    let lateral = outline::leaf(Margin::Serrate { teeth: 5, depth_mm: 0.3, lean_deg: 35.0 }, ll, lw);
    let up = placed(&lateral, CROWN_SPREAD_DEG, lr);
    let down = placed(&lateral, -CROWN_SPREAD_DEG, lr);
    let leaflets = [&terminal, &up, &down];
    let (x0, x1) = leaflets.iter().map(|o| x_range(o)).fold((f64::MAX, f64::MIN), |(a, b), (c, d)| (a.min(c), b.max(d)));
    let n = ((x1 - x0) / 0.05).ceil() as usize;
    let xs: Vec<f64> = (0..=n).map(|i| x0 + 1e-4 + (x1 - x0 - 2e-4) * i as f64 / n as f64).collect();
    let mut low = Vec::new();
    let mut high = Vec::new();
    for &x in &xs {
        let spans: Vec<(f64, f64)> = leaflets.iter().filter_map(|o| span_at(o, x)).collect();
        if spans.is_empty() {
            continue;
        }
        // Symmetric about the parting line, so the fold's ridge runs in it.
        let h = spans.iter().map(|s| s.1.max(-s.0)).fold(0.0, f64::max);
        if h < 0.04 {
            continue;
        }
        low.push([x, -h]);
        high.push([x, h]);
    }
    let mut filled = low;
    filled.extend(high.into_iter().rev());
    let filled = densified(&filled);
    outline::check(&filled).map_err(|e| anyhow::anyhow!("crown leaf outline: {e}"))?;
    let sinuses = [
        // Back past the terminal's foot the sinus runs down to the stalk.
        band_between(&xs, |x| Some(span_at(&terminal, x).map_or(STALK_MM * 0.5, |s| s.1)), |x| span_at(&up, x).map(|s| s.0)),
        band_between(&xs, |x| span_at(&down, x).map(|s| s.1), |x| Some(span_at(&terminal, x).map_or(-STALK_MM * 0.5, |s| s.0))),
    ]
    .into_iter()
    .flatten()
    .collect();
    Ok(CrownLeaf { filled, sinuses, midribs: vec![(0.0, tr, tl), (CROWN_SPREAD_DEG, lr, ll), (-CROWN_SPREAD_DEG, lr, ll)] })
}

/// A trifoliate bramble leaf lying on the crown of each upper internode, springing from the lower node away from
/// its hook. The pattern carries the leaf filled and folded on a ridge in the parting plane, so it pulls both
/// ways; the bench cuts the sinuses between its leaflets back out and grooves each leaflet's midrib.
fn crown_leaves(d: &mut RingDesign) -> Result<Vec<serde_json::Value>> {
    let ctx = d.field_context();
    let r = d.reference_loop().crest_radius_mm;
    let v0 = ctx.crest_v_mm;
    let leaf = crown_leaf()?;
    println!("  crown leaf: filled {:.2} mm2, sinuses {:?} mm2", outline::area(&leaf.filled), leaf.sinuses.iter().map(|o| (outline::area(o) * 100.0).round() / 100.0).collect::<Vec<_>>());
    let mut out = Vec::new();
    for k in (0..NODES).filter(|&k| upper(k) && upper((k + 1) % NODES)) {
        let node = node_deg(k);
        let (sx, _) = chart_signs(d, &ctx, node, v0);
        let theta = (node + (CROWN_STALK_MM / r).to_degrees()).rem_euclid(360.0);
        let rot = if sx > 0.0 { 0.0 } else { 180.0 };
        let n = out.len() + 1;
        d.stamps.push(stamp(format!("Crown leaf, {n}"), theta, v0, rot, leaf.filled.clone(), CROWN_HIGH_MM, StampTop::Gable { rise_mm: CROWN_RISE_MM, axis_deg: 0.0 }));
        for (i, sinus) in leaf.sinuses.iter().enumerate() {
            d.stamps.push(Stamp { cut: true, bench: true, tier: 1, sink_mm: SINUS_SINK_MM, height_mm: 1.0, draft_deg: 0.0, ..stamp(format!("Crown leaf sinus, {}", 2 * (n - 1) + i + 1), theta, v0, rot, sinus.clone(), 1.0, StampTop::Flat) });
        }
        for (i, &(heading, reach, len)) in leaf.midribs.iter().enumerate() {
            let rib = placed(&outline::lanceolate(len * 0.8, MIDRIB_MM, 0.04), heading, reach);
            d.stamps.push(Stamp { cut: true, bench: true, tier: 1, sink_mm: MIDRIB_SINK_MM, height_mm: 1.0, draft_deg: 0.0, ..stamp(format!("Crown midrib, {}", 3 * (n - 1) + i + 1), theta, v0, rot, rib, 1.0, StampTop::Flat) });
        }
        out.push(json!({"node_deg": node, "theta_deg": theta, "v_mm": v0, "rot_deg": rot}));
    }
    Ok(out)
}

/// The whole ring as authored at this stage.
fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = cane();
    prickles(&mut d)?;
    let run = runner(&mut d)?;
    blackberries(&mut d)?;
    leaflets(&mut d, &run)?;
    crown_leaves(&mut d)?;
    let _ = blockout;
    let mut lib = AlphaLibrary::builtin();
    d.bake_svgs(&mut lib);
    Ok((d, lib))
}

// --- The side-gate probe row: P5's gate against each station's own faces ------------------------------

/// Per degree round the ring, how far the station-aware side-face gate reaches past the station's own faces
/// (shares of the section's surface arc turned to mm), low and high: the side_gate_probe row for these keys.
fn side_gate_row(d: &RingDesign) -> serde_json::Value {
    let ctx = d.field_context();
    let inner = d.inner_radius_mm();
    let crest = d.reference_loop().crest_radius_mm;
    let (mut worst, mut worst_at, mut stations) = (0.0f64, 0.0, 0usize);
    let mut least_face = f64::MAX;
    for i in 0..360 {
        let theta = i as f64 + 0.5;
        let m = d.modulation_at(theta, inner, crest);
        let l = d.profile.sample_mod(inner, REFERENCE_PROFILE_STEPS, &m);
        let mut c = ctx.clone();
        c.surface = SurfaceProfile::from_loop(&l, 257);
        c.band_v_len_mm = l.surface_len_mm;
        c.side_faces_cache = Default::default();
        c.station_gates = None;
        let own = c.side_faces(SIDE_FACE_MIN_DRAFT_DEG);
        let gate = ctx.side_faces_at(theta);
        stations += 1;
        let (Some(own), Some(gate)) = (own, gate) else {
            worst = f64::MAX;
            continue;
        };
        let share = |v: f64| v / ctx.band_v_len_mm;
        let own_share = |v: f64| v / l.surface_len_mm;
        let mut spill = 0.0f64;
        if let (Some(g), Some(o)) = (gate.low, own.low) {
            spill = spill.max((share(g.1) - own_share(o.1)).max(0.0) * l.surface_len_mm);
            spill = spill.max((own_share(o.0) - share(g.0)).max(0.0) * l.surface_len_mm);
            least_face = least_face.min((g.1 - g.0) / ctx.band_v_len_mm * l.surface_len_mm);
        }
        if let (Some(g), Some(o)) = (gate.high, own.high) {
            spill = spill.max((own_share(o.0) - share(g.0)).max(0.0) * l.surface_len_mm);
            spill = spill.max((share(g.1) - own_share(o.1)).max(0.0) * l.surface_len_mm);
            least_face = least_face.min((g.1 - g.0) / ctx.band_v_len_mm * l.surface_len_mm);
        }
        if spill > worst {
            worst = spill;
            worst_at = theta;
        }
    }
    json!({"keys": {"node": NODE_KEY, "internode": INTERNODE_KEY}, "stations": stations, "worst_spill_mm": worst, "worst_at_deg": worst_at, "narrowest_gated_face_mm": least_face, "zero_spill": worst <= 1e-6})
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

/// The faces of `m` within `radius` of `centre`, as a mesh of their own, to frame a close-up on.
fn crop(m: &mesh::Mesh, centre: [f64; 3], radius: f64) -> mesh::Mesh {
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
                out.normals.push(m.normals[i as usize]);
                (out.vertices.len() - 1) as u32
            })
        });
        out.faces.push(g);
    }
    out
}

/// A close-up of the side face round `theta`: the runner, a leaflet and the berries either side.
fn closeup(path: &Path, m: &mesh::Mesh, theta: f64, edge: usize) -> Result<()> {
    let (s, c) = theta.to_radians().sin_cos();
    let r = 11.2;
    let near = crop(m, [r * c, r * s, 0.0], 7.5);
    // Looking down the finger's axis at the low face, turned so the crop sits square.
    render::write_png_parts(path, &[render::Part::metal(&near, render::GOLD)], 0.0, 0.0, edge)?;
    Ok(())
}

/// The crown round `theta` seen square from above: the crop turned so `theta` stands at the top.
fn crown_closeup(path: &Path, m: &mesh::Mesh, theta: f64, edge: usize) -> Result<()> {
    let (s, c) = theta.to_radians().sin_cos();
    let mut near = crop(m, [12.8 * c, 12.8 * s, 0.0], 6.5);
    let (ts, tc) = (90.0 - theta).to_radians().sin_cos();
    for v in near.vertices.iter_mut().chain(near.normals.iter_mut()) {
        let (x, y) = (v.0 as f64, v.1 as f64);
        (v.0, v.1) = ((x * tc - y * ts) as f32, (x * ts + y * tc) as f32);
    }
    render::write_png_parts(path, &[render::Part::metal(&near, render::GOLD)], 0.0, PI * 0.5, edge)?;
    Ok(())
}

fn renders(out: &Path, lib: &AlphaLibrary, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    crown_closeup(&out.join("crown-close.png"), &finished.metal, NODE0_DEG - 0.5 * STEP_DEG, edge)?;
    closeup(&out.join("cane-close.png"), &finished.metal, NODE0_DEG + 0.5 * STEP_DEG, edge)?;
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
    let bare = mesh::try_build(&cane(), lib, draft_params())?;
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
    let out = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/vepres/rubus"));
    std::fs::create_dir_all(&out)?;
    println!("Rubus{}", if blockout { " (block-out)" } else { "" });
    let started = std::time::Instant::now();
    let (d, lib) = author(blockout)?;
    let author_s = started.elapsed().as_secs_f64();
    let gate_row = side_gate_row(&d);
    println!("  side-gate probe row: {gate_row}");
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
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let (inspection, fine) = probe::pull(&d, &lib, params, 0.075)?;
    println!("  field {:?} {:.4}% worst {:+.1} deg; pull 0.100: {}; 0.075: {}", field.verdict, field.undercut_fraction() * 100.0, field.worst_draft_deg, probe::release_line(&inspection.release), probe::release_line(&fine));
    for o in inspection.release.obstructions.iter().take(24) {
        let w = o.world;
        println!("    obstruction {:?} at {:.1} deg, r {:.2}, z {:+.2}: {:.3} mm deep, {:.3} mm2", o.half, w[1].atan2(w[0]).to_degrees().rem_euclid(360.0), w[0].hypot(w[1]), w[2], o.depth_mm, o.projected_area_mm2);
    }
    let mut not_monotone = Vec::new();
    for st in &d.stamps {
        if let Err(bad) = st.parting_monotone(&d) {
            println!("    not parting-monotone: {} ({} points)", st.name, bad.len());
            not_monotone.push(st.name.clone());
        }
    }
    let findings = dfm::findings_in(&d, &lib);
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for (n, s) in &status {
        if s != "Ok" {
            println!("    feature {n}: {s}");
        }
    }
    let finished = render::finished_from(&d, &lib, built);
    renders(&out, &lib, &finished, if draft { 1000 } else { 1600 })?;
    library::save_design_embedded(out.join("design.ring.json"), &d, &lib)?;
    let report = json!({
        "name": d.name,
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": finished.metal.faces.len(), "build_s": build_s, "author_s": author_s},
        "geometry": {"watertight": watertight, "degenerate_faces": degenerate, "self_crossings": crossings},
        "made_parts": made,
        "features": status,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "undercut_percent": field.undercut_fraction() * 100.0, "worst_draft_deg": field.worst_draft_deg, "notes": field.notes},
        "release": {"at_0_100": probe::release_line(&inspection.release), "at_0_075": probe::release_line(&fine)},
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "side_gate_probe_row": gate_row,
        "stamps": {"count": d.stamps.len(), "resolved": stamped, "bench": d.stamps.iter().filter(|s| s.bench).count(), "not_parting_monotone": not_monotone},
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "clamp": "no skin::draft_clamp is used: the side-face relief is a curve, decals and stamps, none painted through a Hide",
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    let _ = verify;
    Ok(())
}
