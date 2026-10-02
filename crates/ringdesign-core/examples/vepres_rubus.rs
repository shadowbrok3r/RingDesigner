//! Vepres — Rubus, the bramble cane: one knuckled cane closed into a ring, hooked prickles along the crest and
//! splayed off its shoulders, a trifoliate leaf on the crown of each upper internode, blackberries on the crown's
//! shoulders at the upper nodes, and a fruiting runner with leaflets down both side faces. Lost wax: two-part
//! sand could not carry the fruit or the leaf at a size that reads (see cloud-report.md). cargo build --release -p ringdesign-core --example vepres_rubus
//! target/release/examples/vepres_rubus [OUT_DIR] [--draft] [--verify] [--blockout]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, FieldContext, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, Stage, SurfaceKind, stored},
    castability,
    csg, dfm,
    curve::{CurveLayer, WireProfile},
    field::{Blend, Decal, DecalLayer, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SideFacePick, SurfaceProfile, VGate},
    library, mesh,
    outline::{self, Margin},
    profile::{REFERENCE_PROFILE_STEPS, ShankKey, ShankKind},
    render,
    setting::{self, Stamp, StampTop},
    stl,
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
/// The lost-wax section floor the brief sets for a sand ring moved to wax.
const MIN_SECTION_MM: f64 = 0.8;
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
/// Each upper internode's pair of small prickles, one on each shoulder at its middle, between the staggered
/// berries, splayed out over the side faces so the outline bristles; four pairs over 154.29 deg.
const SMALL_DEG: f64 = 12.857_143;
const SMALL_ACROSS_MM: f64 = 2.3;
const SMALL_CANT_DEG: f64 = 50.0;
const SMALL_SPAN_DEG: f64 = 154.285_714;
/// How deep each prickle's foot is sunk into the cane, and its seam bead.
const PRICKLE_SINK_MM: f64 = 0.35;
const PRICKLE_BLEAD_MM: f64 = 0.2;
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
/// The side-face trifoliate leaf: each leaflet's (length, width, centre's reach from the foot), the laterals'
/// spread off the terminal, the teeth's depth, eaves height and fold.
const SIDE_TERMINAL: (f64, f64, f64) = (3.6, 1.7, 1.9);
const SIDE_LATERAL: (f64, f64, f64) = (2.5, 1.05, 1.25);
const SIDE_SPREAD_DEG: f64 = 24.0;
const SIDE_TEETH_MM: f64 = 0.3;
const SIDE_HIGH_MM: f64 = 0.4;
const SIDE_RISE_MM: f64 = 0.2;
/// Bench-width grooves down each leaflet's midrib, cut 0.12 mm into its own top.
const MIDRIB_MM: f64 = 0.24;
const MIDRIB_SINK_MM: f64 = 0.12;
/// The shoulder blackberries: length out of the cane, width, how far the foot is sunk, where they stand across
/// the crown and how far they lean out over the side face, their seam bead.
const BERRY3_LEN_MM: f64 = 3.8;
const BERRY3_W_MM: f64 = 3.5;
const BERRY3_SINK_MM: f64 = 0.5;
const BERRY3_ACROSS_MM: f64 = 3.0;
const BERRY3_CANT_DEG: f64 = 55.0;
/// The high shoulder's berry stands this far back of the node and the low one's this far ahead, clear of the
/// node's prickle and never read as a matched pair.
const BERRY3_STAGGER_DEG: f64 = 7.0;
const BERRY3_BLEND_MM: f64 = 0.0;
/// Drupelets over the whole berry, and how deep the valleys between them run as a share of its radius.
const DRUPELETS: usize = 46;
const DRUPELET_DEPTH: f64 = 0.13;
const BERRY_RINGS: usize = 48;
const BERRY_AROUND: usize = 96;
/// The calyx under each berry: across its sepal tips, its thickness, its height over the seat.
const CALYX3_MM: f64 = 4.3;
const CALYX3_THICK_MM: f64 = 0.8;
const CALYX3_Z_MM: f64 = 0.5;

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
    // Lost wax at the investment floors: 0.8 mm section (the brief's rule for a sand ring moved to wax), 0.15 mm
    // detail, no draft. The field is still judged on the mid-plane, where the crest prickles lie.
    probe::cast_in(&mut d, &probe::wax_setup(0.1));
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

const LARGE: Hook = Hook { along: 2.8, across: 1.5, radial: 1.3, bend_r: 2.7, bend_deg: 55.0, end_scale: 0.17 };
const SMALL: Hook = Hook { along: 2.2, across: 1.2, radial: 0.7, bend_r: 1.8, bend_deg: 50.0, end_scale: 0.22 };

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
    seated_at(theta, 0.0, 0.0, -PRICKLE_SINK_MM)
}

fn seated_at(theta: f64, across: f64, cant: f64, height: f64) -> Component {
    let mut c = Component::default();
    c.placement = Placement::Ring { theta_deg: theta, across_mm: across, height_mm: height, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: cant };
    c.attach = Attach::Join;
    c.stage = Stage::Cast;
    c.blend_mm = PRICKLE_BLEAD_MM;
    c
}

/// The CAD document: the band, the large prickle and its ring array, the two small ones and theirs.
fn prickles(d: &mut RingDesign) -> Result<()> {
    let mut doc = Document::default();
    doc.append(Feature { id: 1, name: "Cane".into(), enabled: true, operation: Operation::Band, component: Component { role: ComponentRole::Shank, ..Default::default() } })?;
    doc.append(Feature { id: 2, name: "Large prickle".into(), enabled: true, operation: prickle(LARGE), component: seated(LARGE_FROM_DEG) })?;
    let mut array = seated(LARGE_FROM_DEG);
    array.placement = Placement::Free;
    doc.append(Feature { id: 3, name: "Large prickles on the upper nodes".into(), enabled: true, operation: Operation::Pattern { sources: 2.into(), kind: PatternKind::Ring { count: 5, span_deg: LARGE_SPAN_DEG } }, component: array.clone() })?;
    doc.append(Feature { id: 4, name: "Small prickle, high shoulder".into(), enabled: true, operation: prickle(SMALL), component: seated_at(SMALL_DEG, SMALL_ACROSS_MM, SMALL_CANT_DEG, -PRICKLE_SINK_MM) })?;
    doc.append(Feature { id: 5, name: "Small prickle, low shoulder".into(), enabled: true, operation: prickle(SMALL), component: seated_at(SMALL_DEG, -SMALL_ACROSS_MM, -SMALL_CANT_DEG, -PRICKLE_SINK_MM) })?;
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

/// A cluster of three small blackberries at each palm node, on both side faces, gated to them.
fn blackberries(d: &mut RingDesign) -> Result<()> {
    d.svgs.push(SvgAlpha { name: BERRY_ALPHA.into(), svg: drupelet_svg(), invert: false });
    let ctx = d.field_context();
    let mut decals = Vec::new();
    for k in 0..NODES {
        let node = node_deg(k);
        // The upper nodes carry their fruit on the crown's shoulders as parts; the side faces keep the palm's.
        let cluster: &[(f64, f64)] = if upper(k) { &[] } else { &[(0.0, 1.0), (-CLUSTER_DEG, CLUSTER_SCALE), (CLUSTER_DEG, CLUSTER_SCALE)] };
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

/// One trifoliate leaf per internode on each side face, laid over the runner where it has swung to the bore edge:
/// a terminal leaflet pointing back down the cane and two laterals spread either side of it from one foot, each
/// serrated, folded on its own midrib and grooved down it. The laterals overlap the terminal's foot, so the three
/// tips fan out of one mass on a face only 2.2 to 2.7 mm across.
fn leaflets(d: &mut RingDesign, run: &CurveLayer) -> Result<Vec<serde_json::Value>> {
    let ctx = d.field_context();
    let path = run.sample_path(32);
    let r = d.inner_radius_mm() + 0.5 * d.profile.thickness_mm;
    let mut out = Vec::new();
    let (tl, tw, tr) = SIDE_TERMINAL;
    let (ll, lw, lr) = SIDE_LATERAL;
    let leaflets = [("terminal", 0.0, tl, tw, tr, 6), ("lateral", SIDE_SPREAD_DEG, ll, lw, lr, 5), ("lateral", -SIDE_SPREAD_DEG, ll, lw, lr, 5)];
    for k in 0..NODES {
        let mid = (node_deg(k) + 0.5 * STEP_DEG).rem_euclid(360.0);
        let (lo, hi) = faces_at(&ctx, mid)?;
        // The runner's slope in the chart at the internode: the leaf lies along it.
        let x = (mid / STEP_DEG).fract();
        let near = |x: f64| {
            path.iter()
                .min_by(|a, b| {
                    let da = (a[0] - x).rem_euclid(1.0).min((x - a[0]).rem_euclid(1.0));
                    let db = (b[0] - x).rem_euclid(1.0).min((x - b[0]).rem_euclid(1.0));
                    da.total_cmp(&db)
                })
                .copied()
                .unwrap_or([x, 0.0])
        };
        let (a, b) = (near(x - 0.02), near(x + 0.02));
        let slope = ((b[1] - a[1]) / (0.04 * ctx.circumference_mm / NODES as f64)).atan().to_degrees();
        for (face, (a0, a1)) in [("low", lo), ("high", hi)] {
            let v_mid = 0.5 * (a0 + a1);
            let (sx, sy) = chart_signs(d, &ctx, mid, v_mid);
            // The leaf's foot sits up the cane from the internode so the whole leaf centres on it, pointing back.
            let heading0 = 180.0 + slope;
            let foot = (mid + (0.5 * (tr + 0.5 * tl) / r).to_degrees(), v_mid);
            let n = out.len() + 1;
            for &(kind, spread, len, wid, reach, teeth) in &leaflets {
                let h = (heading0 + spread).to_radians();
                let (theta, v) = ((foot.0 + (reach * h.cos() / r).to_degrees()).rem_euclid(360.0), foot.1 + reach * h.sin());
                let rot = (sy * h.sin()).atan2(sx * h.cos()).to_degrees();
                let blade = outline::leaf(Margin::Serrate { teeth, depth_mm: SIDE_TEETH_MM, lean_deg: 35.0 }, len, wid);
                let mut st = stamp(String::new(), theta, v, rot, blade, SIDE_HIGH_MM, StampTop::Gable { rise_mm: SIDE_RISE_MM, axis_deg: 0.0 });
                st.name = format!("Side leaf {n}, {kind} leaflet{}", if spread > 0.0 { " (up)" } else if spread < 0.0 { " (down)" } else { "" });
                st.along_pull = true;
                d.stamps.push(st);
                // The groove stops where the blade is still wide enough to leave a ledge either side of it.
                let rib = outline::lanceolate(len * if spread == 0.0 { 0.68 } else { 0.58 }, MIDRIB_MM, 0.17);
                let mut cut = Stamp { cut: true, tier: 1, sink_mm: MIDRIB_SINK_MM, height_mm: 1.0, draft_deg: 0.0, ..stamp(String::new(), theta, v, rot, rib, 1.0, StampTop::Flat) };
                cut.name = format!("Side leaf {n}, {kind} midrib{}", if spread > 0.0 { " (up)" } else if spread < 0.0 { " (down)" } else { "" });
                cut.along_pull = true;
                d.stamps.push(cut);
            }
            out.push(json!({"internode_deg": mid, "face": face, "face_mm": [a0, a1], "slope_deg": slope}));
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

// --- Blackberries as parts ------------------------------------------------------------------------------

type P3 = [f64; 3];

/// Points spread evenly over the unit sphere.
fn fibonacci(n: usize) -> Vec<P3> {
    let golden = PI * (3.0 - 5f64.sqrt());
    (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let rr = (1.0 - z * z).sqrt();
            let a = golden * i as f64;
            [z, rr * a.cos(), rr * a.sin()]
        })
        .collect()
}

/// A blackberry in the part's frame (z out of the cane): an ellipsoid `2 * half_len` long along z and
/// `2 * half_w` across, its foot at the origin sunk `sink` mm, its surface raised into hemispherical drupelets
/// with dark valleys between them. Star-shaped about its centre, so it closes without crossings.
fn berry_solid(half_len: f64, half_w: f64, sink: f64) -> csg::Solid {
    let centres = fibonacci(DRUPELETS);
    let spacing = (4.0 * PI / DRUPELETS as f64).sqrt();
    let reach = spacing * 0.62;
    let lift = |u: P3| {
        let best = centres.iter().map(|c| (u[0] * c[0] + u[1] * c[1] + u[2] * c[2]).clamp(-1.0, 1.0).acos()).fold(f64::MAX, f64::min);
        let t = (best / reach).min(1.0);
        // A round-topped drupelet falling to the valley between it and its neighbours.
        (1.0 - t * t).max(0.0).sqrt()
    };
    let centre_z = half_len - sink;
    let at = |t: f64, a: f64| {
        // u: the direction in the unit ellipsoid's own space, t from the berry's tip.
        let u = [t.cos(), t.sin() * a.cos(), t.sin() * a.sin()];
        let k = 1.0 - DRUPELET_DEPTH + DRUPELET_DEPTH * lift(u);
        [half_w * u[1] * k, half_w * u[2] * k, centre_z + half_len * u[0] * k]
    };
    let (rings, around) = (BERRY_RINGS, BERRY_AROUND);
    let mut s = csg::Solid::default();
    s.v.push(at(0.0, 0.0));
    for i in 1..rings {
        let t = PI * i as f64 / rings as f64;
        for j in 0..around {
            s.v.push(at(t, 2.0 * PI * j as f64 / around as f64));
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
    s
}

/// The calyx: five pointed sepals as a star plate `thick` mm through, lying square to z at `z0`, rounded points.
fn calyx_solid(outer: f64, inner: f64, thick: f64, z0: f64) -> csg::Solid {
    let n = 80;
    let rim: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let a = 2.0 * PI * i as f64 / n as f64;
            // Five lobes: a raised cosine sharpened toward each tip.
            let lobe = (0.5 + 0.5 * (5.0 * a).cos()).powf(2.5);
            let rr = inner + (outer - inner) * lobe;
            [rr * a.cos(), rr * a.sin()]
        })
        .collect();
    let mut s = csg::Solid::default();
    let (bot, top) = (z0 - 0.5 * thick, z0 + 0.5 * thick);
    s.v.push([0.0, 0.0, bot]);
    s.v.push([0.0, 0.0, top]);
    for p in &rim {
        s.v.push([p[0], p[1], bot]);
    }
    for p in &rim {
        s.v.push([p[0], p[1], top]);
    }
    let (b, t) = (|i: usize| (2 + i % n) as u32, |i: usize| (2 + n + i % n) as u32);
    for i in 0..n {
        s.f.push([0, b(i + 1), b(i)]);
        s.f.push([1, t(i), t(i + 1)]);
        s.f.push([b(i), b(i + 1), t(i + 1)]);
        s.f.push([b(i), t(i + 1), t(i)]);
    }
    s
}

fn stored_op(solid: &csg::Solid, op: &str, params: serde_json::Value) -> Result<Operation> {
    Ok(Operation::Stored {
        recipe: stored::Recipe { kernel: "vepres_rubus".into(), op: op.into(), params, digest: String::new() },
        sources: Vec::new(),
        mesh: stored::Packed::encode(&solid.v, &solid.f, &vec![0; solid.f.len()], &[SurfaceKind::Freeform])?,
    })
}

/// A blackberry and its calyx on each shoulder of the first upper node, staggered either side of it, then the four parts again on the other four upper nodes: ten berries, each hanging out over its side face.
fn berries(d: &mut RingDesign) -> Result<()> {
    let (hl, hw) = (0.5 * BERRY3_LEN_MM, 0.5 * BERRY3_W_MM);
    let berry = berry_solid(hl, hw, BERRY3_SINK_MM);
    let calyx = calyx_solid(CALYX3_MM * 0.5, hw * 0.55, CALYX3_THICK_MM, CALYX3_Z_MM);
    let joined = |theta: f64, across: f64, cant: f64, blend: f64| {
        let mut c = seated_at(theta, across, cant, 0.0);
        c.blend_mm = blend;
        c
    };
    let doc = d.cad.as_mut().ok_or_else(|| anyhow::anyhow!("no CAD document"))?;
    let params = json!({"length_mm": BERRY3_LEN_MM, "width_mm": BERRY3_W_MM, "drupelets": DRUPELETS, "depth": DRUPELET_DEPTH});
    let calyx_params = json!({"across_mm": CALYX3_MM, "thick_mm": CALYX3_THICK_MM});
    for (id, side, name, theta) in [(7u64, 1.0, "high", LARGE_FROM_DEG - BERRY3_STAGGER_DEG), (9, -1.0, "low", LARGE_FROM_DEG + BERRY3_STAGGER_DEG)] {
        doc.append(Feature { id, name: format!("Blackberry, {name} shoulder"), enabled: true, operation: stored_op(&berry, "blackberry", params.clone())?, component: joined(theta, side * BERRY3_ACROSS_MM, side * BERRY3_CANT_DEG, BERRY3_BLEND_MM) })?;
        doc.append(Feature { id: id + 1, name: format!("Calyx, {name} shoulder"), enabled: true, operation: stored_op(&calyx, "calyx", calyx_params.clone())?, component: joined(theta, side * BERRY3_ACROSS_MM, side * BERRY3_CANT_DEG, 0.0) })?;
    }
    doc.append(Feature {
        id: 11,
        name: "Blackberries on the upper nodes".into(),
        enabled: true,
        operation: Operation::Pattern { sources: ringdesign_core::cad::pattern::Sources(vec![7, 8, 9, 10]), kind: PatternKind::Ring { count: 5, span_deg: LARGE_SPAN_DEG } },
        component: Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: BERRY3_BLEND_MM, ..Component::default() },
    })?;
    Ok(())
}

/// The whole ring as authored at this stage.
fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary)> {
    let mut d = cane();
    prickles(&mut d)?;
    let run = runner(&mut d)?;
    blackberries(&mut d)?;
    leaflets(&mut d, &run)?;
    berries(&mut d)?;
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
    // No stones: the close-up a stoned ring gives its setting goes to the fruit at the crest's node.
    let fruit = crop(&finished.metal, [0.0, 13.0, 0.0], 7.0);
    render::write_png_parts(out.join("stones.png"), &[render::Part::metal(&fruit, render::GOLD)], -0.45, 0.75, edge)?;
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

/// Every stamp's solid, checked closed and uncrossed.
fn made_solids(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<Vec<serde_json::Value>> {
    let unstruck = solid_of(&mesh::try_build(&setting::without_solids(d), lib, params)?.mesh);
    let ctx = d.field_context();
    d.stamps
        .iter()
        .map(|st| {
            let c = st.solid(&st.frame(d, &ctx), &unstruck).map_err(anyhow::Error::msg)?.check(true);
            Ok(json!({"name": st.name, "self_crossings": c.self_crossings, "zero_area_faces": c.zero_area_faces, "open_edges": c.open_edges, "repeated_edges": c.repeated_edges}))
        })
        .collect()
}

/// Each CAD part's thinnest metal as surface-normal rays find it (`cad::measure::thickness`, up to 384 rays a
/// part). The band's own section is the field's thinnest wall. The prickles' points taper below the 0.8 mm
/// section by design; they are details judged at the 0.15 mm floor, as Manticora's aculeus point was.
fn walls(built: &mesh::BuildResult) -> Vec<serde_json::Value> {
    built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "sampled_min_mm": t.sampled_min_mm, "rays": t.rays, "below_floor": t.below_limit, "unresolved": t.unresolved})
        })
        .collect()
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
    let solids = made_solids(&d, &lib, draft_params())?;
    let (stamped, solids_notes, parts_notes) = (built.solids.stamped, built.solids.notes.clone(), built.parts.notes.clone());
    let status = feature_status(&built);
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let wall = walls(&built);
    println!("  field {} thinnest wall {:.2} mm at {:.0} deg; part walls {}", field.verdict.label(), field.thinnest_wall_mm, field.thinnest_wall_theta_deg, serde_json::to_string(&wall)?);
    let findings = dfm::findings_in(&d, &lib);
    for f in &findings {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for (n, s) in &status {
        if s != "Ok" {
            println!("    feature {n}: {s}");
        }
    }
    let stones = ringdesign_core::stones::report_built(&d, field.parting_z_mm, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::gems::built_meshes(&d, &lib, &built).len();
    // Stamps at the coarse pitch: phantoms move with resolution, real faults converge.
    let coarse = mesh::try_build(&d, &lib, BuildParams { theta_steps: 384, profile_steps: 192, ..BuildParams::default() })?;
    let (cw, cd, cx) = geometry(&coarse.mesh);
    let coarse_ok = cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}, stamps {}/{}", coarse.solids.stamped, d.stamps.len());
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
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part and stamp solid closed without crossings", made.iter().all(|(_, n)| *n == 0) && solids.iter().all(|g| g["self_crossings"] == 0 && g["open_edges"] == 0 && g["repeated_edges"] == 0 && g["zero_area_faces"] == 0)),
        ("solids and parts notes empty, every stamp resolved, every feature Ok", solids_notes.is_empty() && parts_notes.is_empty() && stamped == d.stamps.len() && status.iter().all(|(_, s)| s == "Ok")),
        ("nothing enters the finger hole", inside == 0),
        ("lost-wax verdict Castable with the 0.8 mm section", field.process == castability::CastProcess::LostWax && field.verdict == castability::Verdict::Castable && field.thinnest_wall_mm >= MIN_SECTION_MM),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview (none)", reported == previewed),
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
        "made_solids": solids,
        "features": status,
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "field": {"verdict": field.verdict.label(), "thinnest_wall_mm": field.thinnest_wall_mm, "thinnest_wall_theta_deg": field.thinnest_wall_theta_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "undercut_percent_reported_only": field.undercut_fraction() * 100.0, "notes": field.notes},
        "ray_walls": wall,
        "wall_exceptions": "the prickles taper to their points below 0.8 mm by design: details judged at the 0.15 mm floor (points 0.26 mm and over), as Manticora's aculeus point was",
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "side_gate_probe_row": gate_row,
        "stamps": {"count": d.stamps.len(), "resolved": stamped},
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "stones": {"reported": reported, "previewed": previewed},
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "stamped": coarse.solids.stamped},
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": pattern.mesh.faces.len()},
        "design": {"bytes": text.len(), "format_version": design_format, "cad_features": d.cad.as_ref().map_or(0, |c| c.features.len())},
        "layers": d.layers.layers.iter().map(|e| e.name.clone()).collect::<Vec<_>>(),
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    // The export report keeps the draft build's gates beside its own.
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
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, "Rubus / investment pattern")?;
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": []}))?)?;
    }
    renders(&out, &lib, &finished, if draft { 1000 } else { 1600 })?;
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Rubus failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
