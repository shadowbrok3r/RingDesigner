//! Vepres — Rubus, the bramble cane: one knuckled cane closed into a ring under broken, wavering bark, hooked
//! prickles along the crest and splayed off its shoulders on flared feet, a crescent leaf scar behind each upper
//! node, a blackberry hanging on a bent stalk off each shoulder of the upper nodes, and a fruiting runner down both
//! side faces with a pillowed serrate leaflet at every internode. Lost wax: two-part sand could not carry the fruit
//! or the leaf at a size that reads (see cloud-report.md). cargo build --release -p ringdesign-core --example vepres_rubus
//! target/release/examples/vepres_rubus [OUT_DIR] [--draft] [--verify] [--blockout]
use anyhow::{Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    alpha::{ProcRecipe, Procedural},
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, MirrorPlane, Operation, PatternKind, Placement, Stage, SurfaceKind, stored},
    castability,
    csg, dfm,
    curve::{CurveLayer, WireProfile},
    field::{Blend, Layer, LayerEntry, SIDE_FACE_MIN_DRAFT_DEG, SideFacePick, SurfaceProfile, VGate, Window},
    library, mesh,
    outline::{self, Margin},
    profile::{MAX_PROFILE_STEPS, REFERENCE_PROFILE_STEPS, ShankKey, ShankKind},
    render,
    setting::{self, Stamp, StampTop},
    stl,
    tiling::TilingLayer,
    sketch::{Geometry, Sketch, Workplane},
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
/// The investment's detail floor.
const MIN_DETAIL_MM: f64 = 0.15;
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
/// The high shoulder's prickle stands this far back of the internode and the low one's this far ahead, each away
/// from its own shoulder's berry.
const SMALL_STAGGER_DEG: f64 = 0.0;
const SMALL_ACROSS_MM: f64 = 2.3;
const SMALL_CANT_DEG: f64 = 50.0;
const SMALL_SPAN_DEG: f64 = 154.285_714;
/// How deep each prickle's foot is sunk into the cane. Every prickle grows out of the bark on its own flared foot
/// (`Hook::flare`), not a seam bead: on the bark's furrows a 0.3 to 0.35 mm bead folds at some stations.
const PRICKLE_SINK_MM: f64 = 0.35;
/// Sides of each prickle's foot polygon.
const PRICKLE_SIDES: usize = 24;
const PRICKLE_BLEND_MM: f64 = 0.0;
const SMALL_BLEND_MM: f64 = 0.0;
/// The runner: a round wire on both side faces, filling this share of the face.
const RUNNER_W_MM: f64 = 1.1;
const RUNNER_H_MM: f64 = 0.55;
const RUNNER_FILL: f64 = 0.7;
/// The side-face leaflet: its blade's length and width, its petiole's length and width, the teeth's depth and count.
/// It fills the crown-side half of the face at each internode, above the runner's trough, so it lies on the bare
/// face with its petiole on the runner.
const LEAF_LEN_MM: f64 = 3.5;
const LEAF_W_MM: f64 = 1.15;
const PETIOLE_MM: (f64, f64) = (0.6, 0.36);
const SIDE_TEETH_MM: f64 = 0.16;
const SIDE_TEETH: u32 = 6;
/// The radius the margin is smoothed over: teeth and notches both rounded.
const ROUND_MM: f64 = 0.07;
/// Each leaf stamp: how far it is sunk into its face, its margin's height over the face (the edge stands this
/// thick), the pillow crown the membrane presses up over it, the clearance kept to the face's edges, and the turn
/// that alternates leaf to leaf.
const LEAF_SINK_MM: f64 = 0.25;
const LEAF_EAVES_MM: f64 = 0.18;
const LEAF_CROWN_MM: f64 = 0.34;
const LEAF_CLEAR_MM: f64 = 0.22;
const LEAF_TURN_DEG: f64 = 6.0;
/// The leaf's walls stand square to the face: even 6 deg of lean mitres the bevelled base across the serrations'
/// notches and the stamp crosses itself.
const LEAF_DRAFT_DEG: f64 = 0.0;
/// The leaf scar at each upper node: a crescent across the crown behind the node's prickle, its span across the
/// band, its width round the ring, its bow, height and crown, and how far behind the node it stands.
const SCAR_SPAN_MM: f64 = 3.0;
const SCAR_W_MM: f64 = 0.55;
const SCAR_BOW_MM: f64 = 0.35;
const SCAR_HIGH_MM: f64 = 0.06;
const SCAR_CROWN_MM: f64 = 0.1;
const SCAR_BACK_DEG: f64 = 7.5;
/// The shoulder blackberries: length out of the cane, width, how far the foot is sunk, where they stand across
/// the crown and how far they lean out over the side face, their seam bead.
const BERRY3_LEN_MM: f64 = 3.8;
const BERRY3_W_MM: f64 = 3.5;
/// The stalk: its arc's length out of the cane, its further bend down the side face; the berry's foot sinks
/// this far over the stalk's end.
const STALK_LEN_MM: f64 = 1.5;
const STALK_BEND_DEG: f64 = 60.0;
const BERRY3_SINK_MM: f64 = 0.3;
/// Drupelets over the whole berry, and how deep the valleys between them run as a share of its radius; the
/// stored mesh's rings and segments, held coarse so the template stays light.
const DRUPELETS: usize = 46;
const DRUPELET_DEPTH: f64 = 0.15;
const BERRY_RINGS: usize = 36;
const BERRY_AROUND: usize = 72;
/// The stalk each berry hangs on: radius at the cane and at the calyx, and how far it reaches into the cane.
const STALK_R_MM: f64 = 0.5;
const STALK_END_R_MM: f64 = 0.46;
const STALK_ROOT_MM: f64 = 1.2;
/// The bark: broken, wavering striae from the procedural bark recipe, quarter-turned to run round the cane; tiles
/// round the ring, their height, the crown band they fill and the arc of crest.
const BARK_TILES: u32 = 13;
const BARK_HIGH_MM: f64 = 0.08;
const BARK_BAND_MM: f64 = 4.6;
const BARK_TILE_SPAN_MM: f64 = 5.4;
/// The bark fades out over this much at each shoulder, ending short of the berries' stalk roots: a stalk crossing
/// the fade joins the band degenerately.
const BARK_FADE_MM: f64 = 0.4;
const BARK_SPAN_DEG: f64 = 200.0;
const BERRY3_ACROSS_MM: f64 = 3.1;
const BERRY3_CANT_DEG: f64 = 55.0;
/// Both shoulders' berries stand this far back of the node, clear of its prickle and leaf scar: the low one is the
/// high one mirrored across the band, so they hang as a pair.
const BERRY3_STAGGER_DEG: f64 = 9.37;
/// The calyx cupping each berry's foot: five sepals lying on the fruit up to `CALYX_REACH_DEG` from its foot,
/// thicker down each sepal's middle than at its edges, tapering from root to tip, the tips peeling `CALYX3_CURL_MM`
/// off the fruit; the web between sepals stays at `CALYX_WEB` of their reach.
const CALYX_REACH_DEG: f64 = 62.0;
/// How far the cup's inner skin lies under the drupelets' crowns: below their valleys (`DRUPELET_DEPTH` of the
/// berry's half-width), deep enough that the stalk's end cap (0.3 mm over the berry's foot) lies wholly inside
/// the cup: a 0.3 mm bury laid the inner skin on that cap exactly.
const CALYX_BURY_MM: f64 = 0.5;
const CALYX_WEB: f64 = 0.3;
const CALYX3_ROOT_MM: f64 = 0.26;
const CALYX3_TIP_MM: f64 = 0.16;
const CALYX3_CURL_MM: f64 = 0.3;
const CALYX_AROUND: usize = 180;
const CALYX_RINGS: usize = 4;

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
    // `crisp_relief` stays off: the template gate lifts this design into a graph, and the lift cannot carry it yet
    // (its `design.set` at /crisp_relief fails, "a design has nothing at /crisp_relief"). Every figurative outline
    // here is true geometry (parts and stamps); only the runner and the bark are height field.
    d.crisp_relief = false;
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
    /// The foot's flare: (share of the path, scale) knots from the sunk root, swelling the prickle where it
    /// leaves the bark so it grows out of the cane on its own curve; empty runs straight to `end_scale`.
    flare: &'static [[f64; 2]],
}

/// The large prickle's path is 3.89 mm, its root sunk 0.35 mm (share 0.09): the same flared foot, narrowing to the
/// shank 0.43 mm over the bark, then tapering to a 0.21 mm point.
const LARGE: Hook = Hook { along: 2.8, across: 1.5, radial: 1.3, bend_r: 2.7, bend_deg: 55.0, end_scale: 0.14, flare: &[[0.0, 1.3], [0.09, 1.25], [0.2, 1.0], [1.0, 0.14]] };
/// The small prickle's path is 3.0 mm, its root sunk 0.35 mm (share 0.12): it leaves the bark 1.2 times its
/// shank's width and narrows to it 0.4 mm up, a flared foot in place of a seam bead the bark would fold, then
/// hooks 62 deg and tapers to a 0.15 mm point.
const SMALL: Hook = Hook { along: 2.0, across: 1.1, radial: 0.85, bend_r: 2.0, bend_deg: 62.0, end_scale: 0.14, flare: &[[0.0, 1.25], [0.12, 1.2], [0.26, 1.0], [1.0, 0.14]] };

/// A closed ellipse as a polyline, centred on the sketch origin: `a` mm full along the path's turn (round the
/// ring), `b` mm across it (along the finger).
fn ellipse(a: f64, b: f64) -> Sketch {
    let mut s = Sketch::default();
    s.name = "Prickle foot".into();
    let points: Vec<_> = (0..PRICKLE_SIDES)
        .map(|k| {
            let t = std::f64::consts::TAU * k as f64 / PRICKLE_SIDES as f64;
            s.point([0.5 * b * t.cos(), 0.5 * a * t.sin()])
        })
        .collect();
    s.entity(Geometry::Polyline { points, closed: true });
    s
}

fn prickle(h: Hook) -> Operation {
    Operation::Twist { sketch: ellipse(h.along, h.across).into(), path: hook_path(h.radial, h.bend_r, h.bend_deg).into(), degrees: 0.0, end_scale: h.end_scale, scale: h.flare.to_vec(), closed: false }
}

fn seated(theta: f64) -> Component {
    seated_at(theta, 0.0, 0.0, -PRICKLE_SINK_MM)
}

fn seated_at(theta: f64, across: f64, cant: f64, height: f64) -> Component {
    let mut c = Component::default();
    c.placement = Placement::Ring { theta_deg: theta, across_mm: across, height_mm: height, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: cant, level: false };
    c.attach = Attach::Join;
    c.stage = Stage::Cast;
    c.blend_mm = PRICKLE_BLEND_MM;
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
    doc.append(Feature { id: 4, name: "Small prickle, high shoulder".into(), enabled: true, operation: prickle(SMALL), component: Component { blend_mm: SMALL_BLEND_MM, ..seated_at(SMALL_DEG - SMALL_STAGGER_DEG, SMALL_ACROSS_MM, -SMALL_CANT_DEG, -PRICKLE_SINK_MM) } })?;
    doc.append(Feature { id: 5, name: "Small prickle, low shoulder".into(), enabled: true, operation: prickle(SMALL), component: Component { blend_mm: SMALL_BLEND_MM, ..seated_at(SMALL_DEG + SMALL_STAGGER_DEG, -SMALL_ACROSS_MM, SMALL_CANT_DEG, -PRICKLE_SINK_MM) } })?;
    doc.append(Feature { id: 6, name: "Small prickles between the upper nodes".into(), enabled: true, operation: Operation::Pattern { sources: ringdesign_core::cad::pattern::Sources(vec![4, 5]), kind: PatternKind::Ring { count: 4, span_deg: SMALL_SPAN_DEG } }, component: Component { blend_mm: SMALL_BLEND_MM, ..array } })?;
    d.cad = Some(doc);
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
        widths: Vec::new(),
        heights: Vec::new(),
        beads: None,
    };
    ensure!(c.land_on_side_face(&ctx, RUNNER_FILL), "the cane has no side face to carry the runner");
    let mut e = LayerEntry::new("Runner", Layer::Curve(c.clone()));
    e.blend = Blend::Max;
    e.window.v_gate = VGate::SideFaces(SideFacePick::Both);
    d.layers.layers.push(e);
    Ok(c)
}

/// `o` turned `rot_deg` about the origin, then moved `reach` mm out along that heading.
fn placed(o: &[[f64; 2]], rot_deg: f64, reach: f64) -> Vec<[f64; 2]> {
    let (sn, cs) = rot_deg.to_radians().sin_cos();
    o.iter().map(|p| [p[0] * cs - p[1] * sn + reach * cs, p[0] * sn + p[1] * cs + reach * sn]).collect()
}

fn inside(o: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut c = false;
    for i in 0..o.len() {
        let (a, b) = (o[i], o[(i + 1) % o.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) {
            c = !c;
        }
    }
    c
}

/// The outer outline of the union of `polys`, traced by marching squares on a `h` mm grid and walked again at
/// `spacing` mm, counter-clockwise: one stamp where three overlapping ones would be.
fn union_outline(polys: &[Vec<[f64; 2]>], h: f64, spacing: f64) -> Result<Vec<[f64; 2]>> {
    let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in polys.iter().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k] - 2.0 * h);
            hi[k] = hi[k].max(p[k] + 2.0 * h);
        }
    }
    let (nx, ny) = (((hi[0] - lo[0]) / h).ceil() as i64 + 1, ((hi[1] - lo[1]) / h).ceil() as i64 + 1);
    let at = |i: i64, j: i64| [lo[0] + i as f64 * h, lo[1] + j as f64 * h];
    let mask: Vec<f64> = (0..ny).flat_map(|j| (0..nx).map(move |i| (i, j))).map(|(i, j)| f64::from(u8::from(polys.iter().any(|o| inside(o, at(i, j)))))).collect();
    // A gaussian of `ROUND_MM` over the mask, cut again at a half: every tooth's point and every notch's root is
    // rounded, so the margin is serrate but smooth and no tooth tapers below the detail floor.
    let reach = (3.0 * ROUND_MM / h).ceil() as i64;
    let kernel: Vec<f64> = (-reach..=reach).map(|k| (-0.5 * (k as f64 * h / ROUND_MM).powi(2)).exp()).collect();
    let total: f64 = kernel.iter().sum();
    let blur = |src: &[f64], step: (i64, i64)| -> Vec<f64> {
        (0..ny)
            .flat_map(|j| (0..nx).map(move |i| (i, j)))
            .map(|(i, j)| {
                (-reach..=reach)
                    .map(|k| {
                        let (a, b) = (i + k * step.0, j + k * step.1);
                        if a < 0 || b < 0 || a >= nx || b >= ny { 0.0 } else { src[(b * nx + a) as usize] * kernel[(k + reach) as usize] }
                    })
                    .sum::<f64>()
                    / total
            })
            .collect()
    };
    let soft = blur(&blur(&mask, (1, 0)), (0, 1));
    let grid: Vec<bool> = soft.iter().map(|&v| v >= 0.5).collect();
    let g = |i: i64, j: i64| i >= 0 && j >= 0 && i < nx && j < ny && grid[(j * nx + i) as usize];
    // Edge crossings keyed on doubled grid coordinates; each cell links the crossings on its sides.
    let mut links: std::collections::HashMap<(i64, i64), Vec<(i64, i64)>> = Default::default();
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let mut e = Vec::new();
            if g(i, j) != g(i + 1, j) { e.push((2 * i + 1, 2 * j)); }
            if g(i + 1, j) != g(i + 1, j + 1) { e.push((2 * i + 2, 2 * j + 1)); }
            if g(i, j + 1) != g(i + 1, j + 1) { e.push((2 * i + 1, 2 * j + 2)); }
            if g(i, j) != g(i, j + 1) { e.push((2 * i, 2 * j + 1)); }
            for pair in e.chunks(2).filter(|c| c.len() == 2) {
                links.entry(pair[0]).or_default().push(pair[1]);
                links.entry(pair[1]).or_default().push(pair[0]);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut best: Vec<(i64, i64)> = Vec::new();
    for &start in links.keys() {
        if seen.contains(&start) { continue; }
        let mut chain = vec![start];
        seen.insert(start);
        let mut cur = start;
        while let Some(&n) = links[&cur].iter().find(|n| !seen.contains(*n)) {
            seen.insert(n);
            chain.push(n);
            cur = n;
        }
        if chain.len() > best.len() { best = chain; }
    }
    let mut pts: Vec<[f64; 2]> = best.iter().map(|&(a, b)| [lo[0] + a as f64 * 0.5 * h, lo[1] + b as f64 * 0.5 * h]).collect();
    // The grid's stair-steps smoothed away: a few passes of neighbour averaging, gentler than a tooth.
    for _ in 0..7 {
        let n = pts.len();
        pts = (0..n).map(|k| {
            let (a, b, c) = (pts[(k + n - 1) % n], pts[k], pts[(k + 1) % n]);
            [0.25 * a[0] + 0.5 * b[0] + 0.25 * c[0], 0.25 * a[1] + 0.5 * b[1] + 0.25 * c[1]]
        }).collect();
    }
    // Walk the loop again at an even spacing no longer than `spacing`.
    let n = pts.len();
    let mut cum = vec![0.0];
    for k in 0..n {
        let (a, b) = (pts[k], pts[(k + 1) % n]);
        cum.push(cum[k] + (b[0] - a[0]).hypot(b[1] - a[1]));
    }
    let total = cum[n];
    let m = (total / spacing).ceil() as usize;
    let mut out = Vec::with_capacity(m);
    let mut k = 0;
    for i in 0..m {
        let t = total * i as f64 / m as f64;
        while cum[k + 1] < t {
            k += 1;
        }
        let f = (t - cum[k]) / (cum[k + 1] - cum[k]).max(1e-12);
        let (a, b) = (pts[k], pts[(k + 1) % n]);
        out.push([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]);
    }
    if outline::area(&out) < 0.0 {
        out.reverse();
    }
    outline::check(&out).map_err(|e| anyhow::anyhow!("trifoliate outline: {e}"))?;
    Ok(out)
}

/// The side-face leaflet in its own plan, pointed along +x and centred on its blade: an ovate serrate blade with a
/// short petiole off its base, as one smoothed outline.
fn leaflet() -> Result<Vec<[f64; 2]>> {
    let blade = outline::leaf(Margin::Serrate { teeth: SIDE_TEETH, depth_mm: SIDE_TEETH_MM, lean_deg: 35.0 }, LEAF_LEN_MM, LEAF_W_MM);
    let (pl, pw) = PETIOLE_MM;
    let x0 = -0.5 * LEAF_LEN_MM;
    let petiole = vec![[x0 - pl, -0.5 * pw], [x0 + 0.5, -0.5 * pw], [x0 + 0.5, 0.5 * pw], [x0 - pl, 0.5 * pw]];
    union_outline(&[blade, petiole], 0.02, 0.0995)
}

/// The leaves as stamps on the side faces, one per face per internode: the leaflet's outline struck off the face's
/// own surface, sunk into it, its margin standing `LEAF_EAVES_MM` proud and a pillow membrane pressed up over it, so
/// it domes highest along its middle and falls to a serrate margin with no crease, following the knuckled face
/// rather than standing off it as a plate. Each lies in the crown-side half of its face, clear of the runner's
/// trough, points the other way round the ring from its neighbour and turns `LEAF_TURN_DEG` so its petiole dips
/// to the runner it grows from.
fn leaves(d: &mut RingDesign) -> Result<serde_json::Value> {
    let ctx = d.field_context();
    let leaf = leaflet()?;
    let mut record = Vec::new();
    for k in 0..NODES {
        let theta = (node_deg(k) + 0.5 * STEP_DEG).rem_euclid(360.0);
        let faces = ctx.side_faces_at(theta).ok_or_else(|| anyhow::anyhow!("no side faces at {theta:.1} deg"))?;
        for (high, face) in [(false, faces.low), (true, faces.high)] {
            let (a, b) = face.ok_or_else(|| anyhow::anyhow!("no {} face at {theta:.1} deg", if high { "high" } else { "low" }))?;
            // The face's crown edge is the end nearer the crest; `up` points to it in chart v.
            let (crown, up) = if (b - ctx.crest_v_mm).abs() < (a - ctx.crest_v_mm).abs() { (b, 1.0) } else { (a, -1.0) };
            // Alternate round the ring: point back or forward; the petiole always dips toward the bore.
            let back = k % 2 == 1;
            let rot = if back { 180.0 + up * LEAF_TURN_DEG } else { -up * LEAF_TURN_DEG };
            let outline = placed(&leaf, rot, 0.0);
            let (lo, hi) = outline.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
            let reach_up = if up > 0.0 { hi } else { -lo };
            let v = crown - up * (LEAF_CLEAR_MM + reach_up);
            let finest = dfm::stamp_finest_mm(&outline, MIN_DETAIL_MM).unwrap_or(f64::NAN);
            d.stamps.push(Stamp {
                name: format!("Bramble leaf {}, {} face", k + 1, if high { "high" } else { "low" }),
                theta_deg: theta,
                v_mm: v,
                rot_deg: 0.0,
                outline,
                height_mm: LEAF_EAVES_MM,
                sink_mm: LEAF_SINK_MM,
                draft_deg: LEAF_DRAFT_DEG,
                cut: false,
                bench: false,
                along_pull: false,
                tier: 0,
                top: StampTop::Pillow { crown_mm: LEAF_CROWN_MM },
                fine_cap: true,
            });
            record.push(json!({"theta_deg": theta, "face": if high { "high" } else { "low" }, "face_v_mm": [a, b], "v_mm": v, "turn_deg": rot, "span_v_mm": hi - lo, "finest_mm": finest}));
        }
    }
    let finest = record.iter().filter_map(|r| r["finest_mm"].as_f64()).fold(f64::MAX, f64::min);
    Ok(json!({"stamps": record.len(), "outline_points": leaf.len(), "leaf_finest_mm": finest, "margin_mm": LEAF_EAVES_MM, "crown_mm": LEAF_EAVES_MM + LEAF_CROWN_MM, "sunk_mm": LEAF_SINK_MM, "leaves": record}))
}

/// A crescent leaf scar across the crown just behind each upper node's prickle: the node ring a cane shows where a
/// leaf fell, bowed toward the prickle and tapering to points at either shoulder, low and pillowed.
fn scars(d: &mut RingDesign) {
    let ctx = d.field_context();
    let n = 24;
    let half = 0.5 * SCAR_SPAN_MM;
    let line: Vec<(f64, f64, f64)> = (0..=n)
        .map(|i| {
            let y = -half + SCAR_SPAN_MM * i as f64 / n as f64;
            let t = y / half;
            (SCAR_BOW_MM * (1.0 - t * t), y, 0.5 * SCAR_W_MM * (1.0 - t * t).max(0.0).powf(0.6).max(0.5))
        })
        .collect();
    let mut outline: Vec<[f64; 2]> = line.iter().map(|&(x, y, w)| [x + w, y]).collect();
    outline.extend(line.iter().rev().skip(1).take(n - 1).map(|&(x, y, w)| [x - w, y]));
    if outline::area(&outline) < 0.0 {
        outline.reverse();
    }
    for k in (0..NODES).filter(|&k| upper(k)) {
        d.stamps.push(Stamp {
            name: format!("Leaf scar {}", k + 1),
            theta_deg: (node_deg(k) - SCAR_BACK_DEG).rem_euclid(360.0),
            v_mm: ctx.crest_v_mm,
            rot_deg: 0.0,
            outline: outline.clone(),
            height_mm: SCAR_HIGH_MM,
            sink_mm: 0.2,
            draft_deg: 0.0,
            cut: false,
            bench: false,
            along_pull: false,
            tier: 0,
            top: StampTop::Pillow { crown_mm: SCAR_CROWN_MM },
            fine_cap: false,
        });
    }
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
fn berry_solid(half_len: f64, half_w: f64, sink: f64, (rings, around): (usize, usize)) -> csg::Solid {
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

/// The calyx: five lanceolate sepals cupping the berry's foot, one closed shell laid on the fruit's own surface.
/// Polar angle from the foot runs out to each sepal's tip (to `CALYX_WEB` of it between sepals); the shell's mid
/// surface rides the berry's drupelet crowns along the berry's normal, its proud thickness falls from `root` to `tip`
/// and from each sepal's middle to its edges, and the tips lift `curl` mm off the fruit.
fn calyx_solid(half_len: f64, half_w: f64, sink: f64, root: f64, tip: f64, curl: f64) -> csg::Solid {
    let (n, k) = (CALYX_AROUND, CALYX_RINGS);
    let reach = CALYX_REACH_DEG.to_radians();
    let lobe = |a: f64| (0.5 + 0.5 * (5.0 * a).cos()).powf(2.0);
    let rim = |a: f64| CALYX_WEB + (1.0 - CALYX_WEB) * lobe(a);
    let cz = half_len - sink;
    // The berry's surface (at its drupelets' crowns) and outward normal at polar angle `t` from the foot.
    let on = |t: f64, a: f64| {
        let (st, ct) = t.sin_cos();
        let p = [half_w * st * a.cos(), half_w * st * a.sin(), cz - half_len * ct];
        let g = [p[0] / (half_w * half_w), p[1] / (half_w * half_w), (p[2] - cz) / (half_len * half_len)];
        let l = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt().max(1e-9);
        (p, [g[0] / l, g[1] / l, g[2] / l])
    };
    let point = |q: f64, a: f64, side: f64| {
        let share = q * rim(a);
        let (p, nrm) = on(reach * share, a);
        // Thinner toward each sepal's edges, but only away from the root, where every sepal is one cup.
        let ramp = (share / 0.5).min(1.0);
        let edge = 0.45 * (1.0 - lobe(a)) * ramp * ramp * (3.0 - 2.0 * ramp);
        let thick = (root + (tip - root) * share) * (1.0 - edge);
        // The inner skin lies buried under the drupelets' valleys, so the cup crosses the fruit cleanly rather than
        // grazing its crowns; `thick` is what stands proud of the crowns.
        let lift = 0.5 * (thick - CALYX_BURY_MM) + curl * share.powi(3);
        let off = lift + side * 0.5 * (thick + CALYX_BURY_MM);
        [p[0] + nrm[0] * off, p[1] + nrm[1] * off, p[2] + nrm[2] * off]
    };
    // The angles round the cup spaced evenly along the sepals' outline, not evenly in angle: down a sepal's flank
    // the outline runs nearly radial, and even angles there make sliver cells whose normals lean off the shell.
    let fine = 4000;
    let plan: Vec<[f64; 2]> = (0..=fine).map(|i| { let a = 2.0 * PI * i as f64 / fine as f64; [rim(a) * a.cos(), rim(a) * a.sin()] }).collect();
    let mut cum = vec![0.0];
    for i in 0..fine {
        cum.push(cum[i] + (plan[i + 1][0] - plan[i][0]).hypot(plan[i + 1][1] - plan[i][1]));
    }
    let angles: Vec<f64> = (0..n)
        .map(|j| {
            let t = cum[fine] * j as f64 / n as f64;
            let i = cum.partition_point(|&c| c <= t).clamp(1, fine) - 1;
            let f = (t - cum[i]) / (cum[i + 1] - cum[i]).max(1e-12);
            2.0 * PI * (i as f64 + f) / fine as f64
        })
        .collect();
    let mut s = csg::Solid::default();
    // Face 0 is the outer skin, wound like the old disc's underside; face 1 lies on the fruit.
    s.v.push(point(0.0, 0.0, 1.0));
    s.v.push(point(0.0, 0.0, -1.0));
    let idx = |face: usize, ring: usize, j: usize| (2 + face * k * n + (ring - 1) * n + j % n) as u32;
    for face in 0..2 {
        for ring in 1..=k {
            for &a in &angles {
                s.v.push(point(ring as f64 / k as f64, a, if face == 0 { 1.0 } else { -1.0 }));
            }
        }
    }
    for j in 0..n {
        s.f.push([0, idx(0, 1, j + 1), idx(0, 1, j)]);
        s.f.push([1, idx(1, 1, j), idx(1, 1, j + 1)]);
    }
    for ring in 1..k {
        for j in 0..n {
            s.f.push([idx(0, ring, j), idx(0, ring, j + 1), idx(0, ring + 1, j + 1)]);
            s.f.push([idx(0, ring, j), idx(0, ring + 1, j + 1), idx(0, ring + 1, j)]);
            s.f.push([idx(1, ring, j), idx(1, ring + 1, j + 1), idx(1, ring, j + 1)]);
            s.f.push([idx(1, ring, j), idx(1, ring + 1, j), idx(1, ring + 1, j + 1)]);
        }
    }
    for j in 0..n {
        s.f.push([idx(0, k, j), idx(0, k, j + 1), idx(1, k, j + 1)]);
        s.f.push([idx(0, k, j), idx(1, k, j + 1), idx(1, k, j)]);
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

/// `s` turned `phi` about the part frame's y (along the ring), then moved to `at`.
fn turned(mut s: csg::Solid, phi: f64, at: P3) -> csg::Solid {
    let (sn, cs) = phi.sin_cos();
    for v in &mut s.v {
        let (x, z) = (v[0], v[2]);
        *v = [x * cs + z * sn + at[0], v[1] + at[1], -x * sn + z * cs + at[2]];
    }
    s
}

/// A round stalk rising `root` mm out of the cane along z, then bending `phi` about y over an arc `len` long;
/// returns it with its end point, where the calyx and berry seat at the same turn.
fn bent_stalk(r: f64, r_end: f64, root: f64, len: f64, phi: f64) -> (csg::Solid, P3) {
    let rr = len / phi.abs().max(1e-6);
    let centre = |a: f64| [phi.signum() * rr * (1.0 - a.abs().cos()), 0.0, rr * a.abs().sin()];
    let mut line: Vec<(P3, f64)> = vec![([0.0, 0.0, -root], 0.0)];
    let steps = 16;
    line.extend((0..=steps).map(|k| {
        let a = phi * k as f64 / steps as f64;
        (centre(a), a)
    }));
    let n = 16;
    let mut s = csg::Solid::default();
    s.v.push(line[0].0);
    s.v.push(line[line.len() - 1].0);
    let m = line.len();
    for (i, &(c, a)) in line.iter().enumerate() {
        let r = r + (r_end - r) * (i.saturating_sub(1) as f64 / (m - 2) as f64);
        let (sa, ca) = a.sin_cos();
        let x = [ca, 0.0, -sa];
        for j in 0..n {
            let u = 2.0 * PI * j as f64 / n as f64;
            s.v.push([c[0] + r * (u.cos() * x[0]), c[1] + r * u.sin(), c[2] + r * (u.cos() * x[2])]);
        }
    }
    let ring = |i: usize, j: usize| (2 + i * n + j % n) as u32;
    for j in 0..n {
        s.f.push([0, ring(0, j + 1), ring(0, j)]);
        s.f.push([1, ring(m - 1, j), ring(m - 1, j + 1)]);
    }
    for i in 0..m - 1 {
        for j in 0..n {
            s.f.push([ring(i, j), ring(i, j + 1), ring(i + 1, j + 1)]);
            s.f.push([ring(i, j), ring(i + 1, j + 1), ring(i + 1, j)]);
        }
    }
    (s, line[m - 1].0)
}

/// A blackberry hanging on a bent stalk from its calyx on each shoulder of the first upper node, staggered either
/// side of it, the stalk turning it a further `STALK_BEND_DEG` down the side face, then the six parts again on the
/// other four upper nodes: ten berries. The palm stays bare for wear.
fn berries(d: &mut RingDesign) -> Result<serde_json::Value> {
    let (hl, hw) = (0.5 * BERRY3_LEN_MM, 0.5 * BERRY3_W_MM);
    let seated = |theta: f64, across: f64, cant: f64| Component { blend_mm: 0.0, ..seated_at(theta, across, cant, 0.0) };
    let doc = d.cad.as_mut().ok_or_else(|| anyhow::anyhow!("no CAD document"))?;
    let params = json!({"length_mm": BERRY3_LEN_MM, "width_mm": BERRY3_W_MM, "drupelets": DRUPELETS, "depth": DRUPELET_DEPTH,
        "stalk": {"radius_mm": STALK_R_MM, "end_radius_mm": STALK_END_R_MM, "length_mm": STALK_LEN_MM, "bend_deg": STALK_BEND_DEG},
        "calyx": {"reach_deg": CALYX_REACH_DEG, "web": CALYX_WEB, "root_mm": CALYX3_ROOT_MM, "tip_mm": CALYX3_TIP_MM, "curl_mm": CALYX3_CURL_MM, "bury_mm": CALYX_BURY_MM}});
    let mut sources = Vec::new();
    let mut walls = Vec::new();
    // The high shoulder's fruit is built and stored; the low one is its mirror across the band's mid-plane, so one
    // stored mesh serves both and the template stays in budget.
    for (id, side, name, theta) in [(7u64, 1.0, "high", LARGE_FROM_DEG - BERRY3_STAGGER_DEG)] {
        // A positive cant leans a ring seat toward -z (`cad::ring_seat`): out over the high face takes a negative
        // one. Rounds 1 to 3 had the sign the other way, so the berries leaned in over the crown and the stalk's
        // bend stood them back up into radial pins.
        let (across, cant) = (side * BERRY3_ACROSS_MM, -side * BERRY3_CANT_DEG);
        // The bend carries on the cant's lean, so the berry droops past the shoulder and hangs down the side face.
        let phi = -side * STALK_BEND_DEG.to_radians();
        let (stalk, end) = bent_stalk(STALK_R_MM, STALK_END_R_MM, STALK_ROOT_MM, STALK_LEN_MM, phi);
        let berry = turned(berry_solid(hl, hw, BERRY3_SINK_MM, (BERRY_RINGS, BERRY_AROUND)), phi, end);
        let calyx = turned(calyx_solid(hl, hw, BERRY3_SINK_MM, CALYX3_ROOT_MM, CALYX3_TIP_MM, CALYX3_CURL_MM), phi, end);
        // Each piece's own wall, on rays, before they are made one: the stalk and berry at the section, the
        // sepals at the detail floor.
        for (piece, solid, floor) in [("stalk", &stalk, MIN_SECTION_MM), ("berry", &berry, MIN_SECTION_MM), ("calyx", &calyx, MIN_DETAIL_MM)] {
            let t = cad::measure::thickness(&mesh_of(solid), floor);
            walls.push(json!({"part": format!("{piece}, {name} shoulder"), "sampled_min_mm": t.sampled_min_mm, "floor_mm": floor, "rays": t.rays, "unresolved": t.unresolved}));
        }
        // One body, unioned once here: the ring array then copies a single closed solid. Left as three parts, the
        // three-way union redone at every copy met degenerately at some placements and build pitches.
        let mut fruit = csg::union_all(&[stalk, berry, calyx]).map_err(|e| anyhow::anyhow!("{name} blackberry: {e:?}"))?;
        csg::clean(&mut fruit, 1e-4);
        ensure!(csg::self_crossings(&fruit) == 0 && fruit.open_edges() == (0, 0), "the {name} blackberry is not one closed body");
        doc.append(Feature { id, name: format!("Blackberry with its calyx and stalk, {name} shoulder"), enabled: true, operation: stored_op(&fruit, "blackberry", params.clone())?, component: seated(theta, across, cant) })?;
        sources.push(id);
    }
    doc.append(Feature {
        id: 8,
        name: "Blackberry with its calyx and stalk, low shoulder".into(),
        enabled: true,
        operation: Operation::Pattern { sources: 7.into(), kind: PatternKind::Mirror { plane: MirrorPlane::Band } },
        component: Component { attach: Attach::Join, stage: Stage::Cast, ..Component::default() },
    })?;
    sources.push(8);
    doc.append(Feature {
        id: 9,
        name: "Blackberries on the upper nodes".into(),
        enabled: true,
        operation: Operation::Pattern { sources: ringdesign_core::cad::pattern::Sources(sources), kind: PatternKind::Ring { count: 5, span_deg: LARGE_SPAN_DEG } },
        component: Component { attach: Attach::Join, stage: Stage::Cast, ..Component::default() },
    })?;
    Ok(json!(walls))
}

fn mesh_of(s: &csg::Solid) -> mesh::Mesh {
    mesh::Mesh { vertices: s.v.iter().map(|p| mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(), normals: vec![mesh::Vec3(0.0, 0.0, 1.0); s.v.len()], faces: s.f.clone(), ..mesh::Mesh::default() }
}

/// Cane bark over the upper crown: the procedural bark's broken, wavering furrows, quarter-turned so they run along
/// the cane, sampled continuously across tile joins, faded out before the palm, which stays smooth for wear.
fn bark(d: &mut RingDesign, lib: &mut AlphaLibrary) {
    let ctx = d.field_context();
    d.recipes.push(ProcRecipe { name: "Cane bark".into(), kind: Procedural::Bark, repeats: 1, quarter_turns: 1, gamma: 1.0, invert: false });
    d.bake_all(lib);
    let mut tile = TilingLayer::default_for("Cane bark", &ctx);
    tile.repeats_around = BARK_TILES;
    tile.rows = 1;
    tile.v_center_mm = ctx.crest_v_mm;
    // The tile spans the crown wider than the window shows it, so each furrow keeps its 0.15 mm detail.
    tile.v_span_mm = BARK_TILE_SPAN_MM;
    tile.height_mm = BARK_HIGH_MM;
    tile.feather_mm = 0.5;
    tile.continuous = true;
    let mut e = LayerEntry::new("Cane bark", Layer::Tiling(tile));
    e.window = Window::around(NODE0_DEG, BARK_SPAN_DEG);
    e.window.v_gate = VGate::Band { center_mm: ctx.crest_v_mm, span_mm: BARK_BAND_MM, fade_mm: BARK_FADE_MM };
    d.layers.layers.push(e);
}

/// The whole ring as authored at this stage.
fn author(blockout: bool) -> Result<(RingDesign, AlphaLibrary, serde_json::Value)> {
    let mut d = cane();
    let mut lib = AlphaLibrary::builtin();
    prickles(&mut d)?;
    runner(&mut d)?;
    bark(&mut d, &mut lib);
    let fruit = berries(&mut d)?;
    let mut leaf = leaves(&mut d)?;
    scars(&mut d);
    let _ = blockout;
    leaf["fruit_piece_walls"] = fruit;
    Ok((d, lib, leaf))
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

/// The crest's radius and the side faces' mid-radius and height at `theta`, from the built section.
fn section_marks(d: &RingDesign, theta: f64) -> (f64, f64, f64) {
    let reference = d.reference_loop();
    let l = d.section_at(theta, MAX_PROFILE_STEPS, None, Some(&reference));
    let crest = l.pts.iter().filter(|p| p.surface).map(|p| p.r).fold(f64::MIN, f64::max);
    let z = l.pts.iter().filter(|p| p.surface).map(|p| p.z).fold(f64::MIN, f64::max);
    let inner = d.inner_radius_mm();
    (crest, 0.5 * (crest + inner), z)
}

/// Close-ups framed on whole parts, never a cropped mesh: the crown square from above round `theta`, the fruit at
/// the crest's node from three-quarters, and the side face round an internode looking down the finger.
fn closeups(out: &Path, d: &RingDesign, parts: &[render::Part<'_>], edge: usize) -> Result<()> {
    let at = |theta: f64, r: f64, z: f64| {
        let (s, c) = theta.to_radians().sin_cos();
        [r * c, r * s, z]
    };
    let crown = NODE0_DEG - 0.5 * STEP_DEG;
    let (crest, _, _) = section_marks(d, crown);
    render::write_png_framed(out.join("crown-close.png"), parts, render::yaw_facing(crown), PI * 0.5, render::Framing::new(at(crown, crest, 0.0), 5.5), edge)?;
    let (crest, _, _) = section_marks(d, NODE0_DEG);
    render::write_png_framed(out.join("stones.png"), parts, render::yaw_facing(NODE0_DEG) - 0.45, 0.75, render::Framing::new(at(NODE0_DEG, crest, 0.0), 6.0), edge)?;
    let side = NODE0_DEG + 0.5 * STEP_DEG;
    let (_, mid, z) = section_marks(d, side);
    render::write_png_framed(out.join("cane-close.png"), parts, render::yaw_facing(side) - render::yaw_facing(90.0), 0.0, render::Framing::new(at(side, mid, z), 5.0), edge)?;
    Ok(())
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, finished: &render::Finished, edge: usize) -> Result<()> {
    let parts = finished.parts(render::GOLD);
    // No stones: the close-up a stoned ring gives its setting goes to the fruit at the crest's node.
    closeups(out, d, &parts, edge)?;
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
            let solid = st.solid(&st.frame(d, &ctx), &unstruck).map_err(anyhow::Error::msg)?;
            let c = solid.check(true);
            // The stamp's own prism on surface-normal rays: a diagnostic only. It reads 0.02 to 0.05 mm on every stamp,
            // the leaf scar's 0.36 mm-deep middle included, so it does not measure a stamp's section.
            let t = cad::measure::thickness(&mesh_of(&solid), MIN_DETAIL_MM);
            Ok(json!({"name": st.name, "self_crossings": c.self_crossings, "zero_area_faces": c.zero_area_faces, "open_edges": c.open_edges, "repeated_edges": c.repeated_edges, "ray_diagnostic_mm": t.sampled_min_mm, "ray_diagnostic_at": t.point}))
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
        // A ring array's copies are its sources moved rigidly: measure each source once.
        .filter(|c| !matches!(c.made.as_ref().map(|m| m.key.as_str()), Some(cad::pattern::PATTERN)))
        .map(|c| {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            json!({"part": c.name, "point": t.point, "sampled_min_mm": t.sampled_min_mm, "rays": t.rays, "below_floor": t.below_limit, "unresolved": t.unresolved})
        })
        .collect()
}

/// Whether a part's sampled wall clears its floor: the section for bodies, the detail floor for the pointed
/// parts (prickles, calyces, and the arrays that carry them).
fn walls_ok(walls: &[serde_json::Value], detail: f64) -> bool {
    walls.iter().all(|w| {
        let name = w["part"].as_str().unwrap_or("");
        // Each blackberry is one body with its sepals, so it is judged where they are, at the detail floor; its stalk
        // and fruit are held to the section piece by piece (leaf_lands.fruit_piece_walls).
        let pointed = name.contains("rickle") || name.contains("calyx");
        w["sampled_min_mm"].as_f64().is_some_and(|m| m >= if pointed { detail } else { MIN_SECTION_MM })
    })
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
    let (d, lib, leaf_lands) = author(blockout)?;
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
    let coarse_made: Vec<_> = made_parts(&coarse).into_iter().filter(|(_, n)| *n > 0).collect();
    let coarse_ok = coarse_made.is_empty() && cw && cd == 0 && cx == 0 && coarse.solids.stamped == d.stamps.len() && coarse.solids.notes.is_empty() && coarse.parts.notes.is_empty();
    println!("  384 x 192: watertight {cw}, degenerate {cd}, crossings {cx}, parts crossing {coarse_made:?}, stamps {}/{}; notes {:?} {:?}", coarse.solids.stamped, d.stamps.len(), coarse.solids.notes, coarse.parts.notes);
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
        ("every part's ray-sampled wall at the 0.8 mm section, its pointed details (prickles, sepals) at the 0.15 mm detail floor", walls_ok(&wall, d.draft.min_detail_mm) && leaf_lands["fruit_piece_walls"].as_array().is_some_and(|w| w.iter().all(|p| p["sampled_min_mm"].as_f64().is_some_and(|m| m >= p["floor_mm"].as_f64().unwrap_or(f64::MAX))))),
        ("every leaf outline's finest stroke, and every leaf's margin over its face, at the 0.15 mm detail floor", leaf_lands["leaf_finest_mm"].as_f64().is_some_and(|f| f >= MIN_DETAIL_MM) && LEAF_EAVES_MM >= MIN_DETAIL_MM),
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
        "leaf_lands": leaf_lands,
        "wall_exceptions": format!(
            "Pointed details are judged at the 0.15 mm detail floor, not the 0.8 mm section, as Manticora's aculeus point was. Sampled minimums (ray_walls): {}. The leaves and leaf scars are stamps, relief struck off the band rather than parts, so they are not walls: each leaf's margin stands 0.18 mm proud of its face and 0.25 mm into it, rising to 0.52 mm along its middle, and the outlines' finest strokes are in leaf_lands. made_solids carries a ray sample of each stamp's prism as a diagnostic only; it reads 0.02 to 0.05 mm even at the leaf scar's 0.36 mm-deep middle, so it does not measure a stamp's section. Ring arrays are rigid copies of their sources and are measured on the sources; each blackberry is one body with its calyx and stalk, judged at the detail floor for its sepals, and its stalk and fruit sample at or above 0.8 mm piece by piece (leaf_lands.fruit_piece_walls).",
            wall.iter().filter(|w| { let n = w["part"].as_str().unwrap_or(""); n.contains("rickle") || n.contains("calyx") }).map(|w| format!("{} {:.3} mm", w["part"].as_str().unwrap_or(""), w["sampled_min_mm"].as_f64().unwrap_or(f64::NAN))).collect::<Vec<_>>().join(", ")
        ),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "side_gate_probe_row": gate_row,
        "stamps": {"count": d.stamps.len(), "resolved": stamped},
        "notes": {"solids": solids_notes, "parts": parts_notes},
        "stones": {"reported": reported, "previewed": previewed},
        "coarse": {"watertight": cw, "degenerate_faces": cd, "self_crossings": cx, "parts_crossing": coarse_made, "stamped": coarse.solids.stamped, "notes": [&coarse.solids.notes, &coarse.parts.notes]},
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
    renders(&out, &d, &lib, &finished, if draft { 1000 } else { 1600 })?;
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Rubus failed a gate; see {}", out.join("report.json").display());
    Ok(())
}
