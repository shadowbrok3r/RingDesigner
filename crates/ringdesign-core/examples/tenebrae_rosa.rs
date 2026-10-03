//! Tenebrae — Rosa, the west rose: the rose window of a west front on a cushion signet. A ruby oculus in a roll moulding,
//! eight sapphire lights in collets radiating from it in pointed petals of tracery, and eight pierced spandrels between
//! the petals' heads, all inside the table's own rim. Lost wax.
//! cargo build --release -p ringdesign-core --example tenebrae_rosa
//! target/release/examples/tenebrae_rosa [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{self, Attach, Component, ComponentRole, Document, Feature, FeatureStatus, Operation, PatternKind, Placement, PlaneBase, Profile, Stage, builders},
    castability::{self, CastProcess, SandProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library,
    manufacturing::{self as mf, Setup},
    mesh, render,
    sketch::{FaceAnchor, Geometry, Id, RegionRef, Sketch},
    stl,
};
use serde_json::json;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::path::{Path, PathBuf};

const SLUG: &str = "rosa";
const STOCK: &str = "001";
const FACE_MM: f64 = 19.0;
const BORE_MM: f64 = 18.6;
/// The stock's palm thickened its full half millimetre, so the shank's edges hold the investment's section.
const PALM_MM: f64 = 2.0;
const ALLOY: &str = "Gold 18k";
/// Least metal the investment fills, and the least a tracery bar may be, mm.
const MIN_SECTION_MM: f64 = 0.8;

/// Lights round the rose.
const LIGHTS: usize = 8;
/// The rose net: the hub circle, the circle the petals' heads touch, and the radius the mullions stop and the heads spring at, mm.
const R_HUB: f64 = 3.0;
const R_OUT: f64 = 8.5;
/// Each head's arcs are struck from a centre this share of the springing span across: 1 is an equilateral arch.
const ARCH: f64 = 1.0;
/// Tracery bar between neighbouring lights, mm.
const BAR_MM: f64 = 0.85;
/// The table plane stands this far over the table, so every cut starts clear of the metal, mm.
const LIFT_MM: f64 = 0.75;
/// How proud of the table the tracery stands, mm.
const TRACERY_MM: f64 = 0.45;
/// The ruby's feature: the part the lights are arrayed round.
const RUBY_ID: Id = 8;
/// The raised tracery runs this far into the table under it, mm.
const TRACERY_FOOT_MM: f64 = 0.1;
/// The outer order's width outside the net's outer circle, mm.
const RING_MM: f64 = 0.55;
/// Spandrels are picked this far inside the outer circle, mm.
const SPANDREL_PICK_MM: f64 = 0.75;
/// How deep the petals sink below the table, mm.
const PETAL_SINK_MM: f64 = 0.5;
/// How far below the table the spandrels are sunk, mm.
const SPANDREL_MM: f64 = 1.5;
/// How far below the table the pilots are opened: past the bore under the head, mm.
const PIERCE_MM: f64 = 7.5;

/// The oculus collet's foot stands this far proud of its lip all round: the oculus moulding, mm.
const MOULD_STEP_MM: f64 = 0.0;
/// Oculus moulding: a half-round of this radius revolved at this radius about the ruby's axis, mm.
const MOULD_R_MM: f64 = 0.4;
const MOULD_AT_MM: f64 = 2.85;
/// Ruby oculus and its collet.
const RUBY_MM: f64 = 3.5;
/// Girdles over the table: the ruby's on the hub, the sapphires' over their sunk petals, mm.
const RUBY_GIRDLE_MM: f64 = 0.75;
const LIGHT_GIRDLE_MM: f64 = 0.5;
/// Sapphire lights, centred this far from the ruby, and their collets.
const LIGHT_W_MM: f64 = 1.6;
const LIGHT_L_MM: f64 = 2.67;
/// Drawn collets: the wall, the clearance round the girdle (past the bur's own bevel, so the bur cuts only the bearing), the straight lip left over the girdle for the setter to
/// burnish, and how far the foot reaches into the metal under it, mm.
const COLLET_WALL_MM: f64 = 0.85;
const SEAT_CLEAR_MM: f64 = 0.2;
const LIP_MM: f64 = 0.35;
const COLLET_FOOT_MM: f64 = 0.1;
/// How far two joined pieces of one part run into each other, so no faces coincide, mm.
const OVERLAP_MM: f64 = 0.05;
/// The drawn bearing stands this far under the girdle, so the bur's bearing cone cuts into it, mm.
const BEARING_DROP_MM: f64 = 0.05;
/// Seam bead every joined part carries, mm.
const SEAM_MM: f64 = 0.15;

/// How far each opened pilot stands inside its collet's bearing ring, mm.
const PILOT_INSET_MM: f64 = 0.1;
/// Each drilled pilot starts this far under the table, below the collet's foot, mm.
const PILOT_START_MM: f64 = COLLET_FOOT_MM + 0.05;
/// How far a drilled pilot stands outside the bur's own pilot, mm.
const PILOT_GROW_MM: f64 = 0.1;

/// The head walls' blind arcades: each lancet's height, width, how deep it is sunk, its centre's height over the
/// finger's axis, and the lancets' centres along the wall, mm.
const ARCADE_H_MM: f64 = 2.8;
const ARCADE_W_MM: f64 = 1.5;
const ARCADE_SINK_MM: f64 = 0.35;
const ARCADE_Y_MM: f64 = 11.6;
const ARCADE_AT: [f64; 5] = [-4.6, -2.3, 0.0, 2.3, 4.6];
/// The nave's oculi on each shoulder: degrees off the crown and widths, graded toward the palm.
const OCULI: [(f64, f64); 4] = [(44.0, 1.6), (54.0, 1.4), (64.0, 1.2), (74.0, 0.9)];

const RUBY: [f32; 3] = [0.45, 0.01, 0.04];
const SAPPHIRE: [f32; 3] = [0.02, 0.06, 0.45];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 160, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

fn setup() -> Setup {
    let mut s = Setup::default();
    s.recipe = mf::Recipe::sand(SandProcess::DelftClay);
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.name = format!("Rosa / investment / {ALLOY}");
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).unwrap().shrink_pct;
    s.recipe.calibration_note = "Starting shrink allowance; confirm with the caster's alloy, pattern material and measured trials.".into();
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.5, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -33.0, 0.0], diameter_mm: 6.0 },
    ];
    s.bench_notes = "Investment cast in one piece, sprued at the palm; the stones are references. Flush investment from the pierced spandrels and every seat pilot. Cut each bearing to the measured stone, set the ruby and the eight sapphires a jour, and break every bore-side edge 0.2. Polish the table, the tracery bars and the oculus moulding; leave the sunk petal floors satin and darkened.".into();
    s
}

/// Factory 001 Cushion at an 18 mm face, investment cast, bore 18.6.
fn base() -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == STOCK).context("no stock 001")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, preset.load()?)?;
    d.imported_base.as_mut().unwrap().sand_envelope = false;
    d.name = "Rosa — the west rose".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = FACE_MM;
    d.shank.head.length_mm = FACE_MM;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).unwrap();
    d.profile.edge_round_mm = knob("ROSA_ER", 0.3);
    d.profile.comfort_fit_mm = knob("ROSA_CF", 0.1);
    d.profile.thickness_mm = knob("ROSA_T", PALM_MM);
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.min_detail_mm = 0.15;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.build = draft_params();
    d.manufacturing = Some(setup());
    Ok(d)
}

fn polar(r: f64, deg: f64) -> [f64; 2] {
    let a = deg.to_radians();
    [r * a.cos(), r * a.sin()]
}
fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn len2(a: [f64; 2]) -> f64 {
    a[0].hypot(a[1])
}

/// Radius the mullions stop and the heads spring at, so an arch of [`ARCH`] rises from there to the outer circle, mm.
fn r_spring() -> f64 {
    let hb = half_bay_deg().to_radians();
    let arch = knob("ROSA_ARCH", ARCH);
    R_OUT / (hb.cos() + hb.sin() * (4.0 * arch - 1.0).sqrt())
}

/// How far each sapphire stands in from midway along its petal, so its collet's foot runs into the oculus moulding, mm.
const HUB_BITE_MM: f64 = 0.0;

/// Where each sapphire is centred: midway between its petal light's sill and its apex (drawn in a full bar from the
/// outer circle), less the bite into the moulding, mm.
fn light_at_mm() -> f64 {
    knob("ROSA_AT", 0.5 * (R_HUB + 0.5 * BAR_MM + R_OUT - BAR_MM) - knob("ROSA_BITE", HUB_BITE_MM))
}

/// Sketch angle of light `k`'s axis, degrees: the first points along +y (along the finger).
fn light_deg(k: usize) -> f64 {
    90.0 + 360.0 * k as f64 / LIGHTS as f64
}
fn half_bay_deg() -> f64 {
    180.0 / LIGHTS as f64
}

/// Centre of the arc from a petal's springer `s` up to its apex `a`, on the springing chord through `s` along `along`.
fn head_centre(s: [f64; 2], a: [f64; 2], along: [f64; 2]) -> [f64; 2] {
    // |s + t·along − a| = |t|, so t = |s − a|² / (2 (s − a)·along).
    let d = sub2(s, a);
    let t = (d[0] * d[0] + d[1] * d[1]) / (2.0 * (d[0] * along[0] + d[1] * along[1]));
    [s[0] - t * along[0], s[1] - t * along[1]]
}

/// The rose net: hub and outer circles, a mullion between each pair of lights from the hub to the outer circle, and
/// each light's pointed head, two arcs from its springers up to an apex on the outer circle.
struct Net {
    sketch: Sketch,
    curves: Vec<Id>,
}
fn rose_net(plane: Id) -> Net {
    let mut s = Sketch::default();
    s.name = "Rose net".into();
    let o = s.point([0.0, 0.0]);
    let mut curves = Vec::new();
    for r in [R_HUB, R_OUT] {
        let rim = s.point([r, 0.0]);
        curves.push(s.entity(Geometry::Circle { center: o, rim }));
    }
    let hb = half_bay_deg();
    for k in 0..LIGHTS {
        let m = light_deg(k) - hb;
        let (a, b) = (s.point(polar(R_HUB, m)), s.point(polar(R_OUT, m)));
        curves.push(s.entity(Geometry::Line { a, b }));
    }
    for k in 0..LIGHTS {
        let phi = light_deg(k);
        let (sr, sl, apex) = (polar(r_spring(), phi - hb), polar(r_spring(), phi + hb), polar(R_OUT, phi));
        let chord = { let d = sub2(sl, sr); let l = len2(d); [d[0] / l, d[1] / l] };
        // Right arc: counter-clockwise from the right springer up to the apex; the left mirrors it.
        let cr = head_centre(sr, apex, chord);
        let cl = head_centre(sl, apex, [-chord[0], -chord[1]]);
        let (c1, s1, e1) = (s.point(cr), s.point(sr), s.point(apex));
        curves.push(s.entity(Geometry::Arc { center: c1, start: s1, end: e1 }));
        let (c2, s2, e2) = (s.point(cl), s.point(apex), s.point(sl));
        curves.push(s.entity(Geometry::Arc { center: c2, start: s2, end: e2 }));
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    Net { sketch: s, curves }
}

/// Where each kind of light is picked: inside each petal at the stone's centre, and inside each spandrel between heads.
fn petal_at(k: usize) -> [f64; 2] {
    polar(light_at_mm(), light_deg(k))
}
/// Spandrel `j`: the half on light `j / 2`'s clockwise side for even `j`, its anticlockwise side for odd.
fn spandrel_at(j: usize) -> [f64; 2] {
    let side = if j % 2 == 0 { -1.0 } else { 1.0 };
    polar(R_OUT - SPANDREL_PICK_MM, light_deg(j / 2) + side * 0.72 * half_bay_deg())
}
/// A point on the tracery's bars: halfway along the first mullion.
fn bar_at() -> [f64; 2] {
    polar(0.5 * (R_HUB + r_spring()), light_deg(0) - half_bay_deg())
}

/// The traced rose: the sketch with its lights drawn and the picks for the petals and the spandrels.
struct Rose {
    sketch: Sketch,
    petals: Vec<RegionRef>,
    spandrels: Vec<RegionRef>,
    lights: usize,
    lands: Lands,
    bars: RegionRef,
    bar_sketch: Sketch,
}

/// The metal the rose leaves, measured on the traced sketch, mm.
#[derive(Clone, Copy, Debug, serde::Serialize)]
struct Lands {
    /// Tracery bar between neighbouring lights, as traced.
    bar_mm: f64,
    /// Least metal from a sapphire collet's outer wall to the pierced spandrels beside it.
    collet_to_spandrel_mm: f64,
    /// How far a sapphire collet's wall stands past its petal light onto the bar (its foot is on the bar there).
    collet_over_bar_mm: f64,
    /// Least room between a sapphire collet's wall and its petal light's wall, negative where it stands over the bar.
    collet_in_petal_mm: f64,
    /// Ruby collet's outer wall to the nearest petal light.
    ruby_collet_to_petal_mm: f64,
    /// Ruby pilot to a sapphire pilot, edge to edge.
    pilot_to_pilot_mm: f64,
    /// Spandrel light to the table's edge along the axes, where the table is narrowest.
    spandrel_to_table_edge_mm: f64,
}

/// Points round a region's outer loop.
fn outline(r: &ringdesign_core::sketch::Region) -> Vec<[f64; 2]> {
    r.outer.iter().flat_map(|c| (0..48).map(move |k| c.point_at(k as f64 / 48.0))).collect()
}
/// Points round a collet of half-axes `a` across and `b` along its light's axis, centred on that axis at `at`.
fn collet_outline(a: f64, b: f64, at: f64, deg: f64) -> Vec<[f64; 2]> {
    let (s, c) = deg.to_radians().sin_cos();
    (0..180)
        .map(|k| {
            let t = TAU * k as f64 / 180.0;
            let (u, v) = (a * t.cos(), at + b * t.sin());
            [v * c - u * s, v * s + u * c]
        })
        .collect()
}
fn nearest(a: &[[f64; 2]], b: &[[f64; 2]]) -> f64 {
    a.iter().map(|p| b.iter().map(|q| len2(sub2(*p, *q))).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min)
}

fn measure(sketch: &Sketch, petal: &ringdesign_core::sketch::Region, spandrels: &[&ringdesign_core::sketch::Region]) -> Lands {
    let s = sapphire();
    let wall = |_: Gem, w: f64| SEAT_CLEAR_MM + w;
    let (a, b) = (0.5 * s.w_mm + wall(s, COLLET_WALL_MM), 0.5 * s.l_mm + wall(s, COLLET_WALL_MM));
    let collet = collet_outline(a, b, light_at_mm(), light_deg(0));
    let petal_line = outline(petal);
    let inside = collet.iter().map(|p| { let d = petal_line.iter().map(|q| len2(sub2(*p, *q))).fold(f64::MAX, f64::min); if petal.contains(*p) { d } else { -d } }).fold(f64::MAX, f64::min);
    let over = collet.iter().filter(|p| !petal.contains(**p)).map(|p| petal_line.iter().map(|q| len2(sub2(*p, *q))).fold(f64::MAX, f64::min)).fold(0.0, f64::max);
    let to_spandrel = spandrels.iter().map(|r| nearest(&collet, &outline(r))).fold(f64::MAX, f64::min);
    let ruby_r = 0.5 * RUBY_MM + wall(ruby(), COLLET_WALL_MM);
    let petal_min_r = petal_line.iter().map(|p| len2(*p)).fold(f64::MAX, f64::min);
    let pilot = |g: Gem| 0.52 * 0.5 * g.w_mm;
    let pilot_l = 0.52 * 0.5 * s.l_mm;
    let span_max = spandrels.iter().flat_map(|r| outline(r)).map(|p| p[0].abs().max(p[1].abs())).fold(0.0, f64::max);
    let _ = sketch;
    Lands {
        bar_mm: BAR_MM,
        collet_to_spandrel_mm: to_spandrel,
        collet_over_bar_mm: over,
        collet_in_petal_mm: inside,
        ruby_collet_to_petal_mm: petal_min_r - ruby_r,
        pilot_to_pilot_mm: light_at_mm() - pilot_l - pilot(ruby()),
        spandrel_to_table_edge_mm: 0.5 * FACE_MM - span_max,
    }
}
fn traced_rose(plane: Id) -> Result<Rose> {
    let Net { mut sketch, curves } = rose_net(plane);
    let traced = sketch.tracery(&curves, BAR_MM)?;
    ensure!(traced.skipped.is_empty(), "tracery skipped cells: {:?}", traced.skipped);
    let regions = sketch.sweep_regions()?;
    let pick = |at: [f64; 2]| -> Result<RegionRef> {
        regions.iter().find_map(|r| RegionRef::at(r, at)).with_context(|| format!("no light at {at:?}"))
    };
    let petals = (0..LIGHTS).map(|k| pick(petal_at(k))).collect::<Result<Vec<_>>>()?;
    let region_at = |at: [f64; 2]| regions.iter().find(|r| r.contains(at)).context("no region");
    let lands = measure(&sketch, region_at(petal_at(0))?, &[region_at(spandrel_at(0))?, region_at(spandrel_at(1))?]);
    let spandrels = (0..2 * LIGHTS).map(|j| pick(spandrel_at(j))).collect::<Result<Vec<_>>>()?;
    // The outer order: a circle a ring's width outside the net, so the bars between the lights are one region.
    let mut bar_sketch = sketch.clone();
    bar_sketch.name = "Tracery bars".into();
    let o = bar_sketch.point([0.0, 0.0]);
    let rim = bar_sketch.point([R_OUT + RING_MM, 0.0]);
    bar_sketch.entity(Geometry::Circle { center: o, rim });
    let regions = bar_sketch.sweep_regions()?;
    let bars = regions.iter().find_map(|r| RegionRef::at(r, bar_at())).context("no tracery region")?;
    if let Ok(path) = std::env::var("ROSA_SVG") {
        let mut svg = String::from("<svg xmlns='http://www.w3.org/2000/svg' viewBox='-10 -10 20 20'><rect x='-10' y='-10' width='20' height='20' fill='white'/>");
        for e in &sketch.entities {
            let colour = if e.construction { "#bbb" } else { "#c00" };
            for c in sketch.curves_of(e)? {
                let pts: Vec<String> = (0..=24).map(|k| { let p = c.point_at(k as f64 / 24.0); format!("{:.3},{:.3}", p[0], -p[1]) }).collect();
                svg += &format!("<polyline fill='none' stroke='{colour}' stroke-width='0.05' points='{}'/>", pts.join(" "));
            }
        }
        let sp = sapphire();
        for k in 0..LIGHTS {
            let c = collet_outline(0.5 * sp.w_mm + SEAT_CLEAR_MM + COLLET_WALL_MM, 0.5 * sp.l_mm + SEAT_CLEAR_MM + COLLET_WALL_MM, light_at_mm(), light_deg(k));
            let pts: Vec<String> = c.iter().map(|p| format!("{:.3},{:.3}", p[0], -p[1])).collect();
            svg += &format!("<polyline fill='none' stroke='#0a0' stroke-width='0.05' points='{}'/>", pts.join(" "));
        }
        for at in (0..LIGHTS).map(petal_at).chain((0..2 * LIGHTS).map(spandrel_at)).chain([bar_at()]) {
            svg += &format!("<circle cx='{:.3}' cy='{:.3}' r='0.12' fill='blue'/>", at[0], -at[1]);
        }
        svg += "</svg>";
        std::fs::write(path, svg)?;
    }
    Ok(Rose { sketch, petals, spandrels, lights: traced.lights.len(), lands, bars, bar_sketch })
}

/// The oculus moulding as an inline section in the part's x–z plane: a half-round on the table, revolved about z.
fn moulding_section() -> Sketch {
    let mut s = Sketch::default();
    s.name = "Oculus moulding section".into();
    let steps = 16;
    let ids: Vec<Id> = (0..=steps)
        .map(|k| {
            let a = PI * k as f64 / steps as f64;
            s.point([MOULD_AT_MM + MOULD_R_MM * a.cos(), MOULD_R_MM * a.sin()])
        })
        .collect();
    let mut pts = ids;
    let low = s.point([MOULD_AT_MM - MOULD_R_MM, -0.3]);
    let low2 = s.point([MOULD_AT_MM + MOULD_R_MM, -0.3]);
    pts.push(low);
    pts.push(low2);
    s.entity(Geometry::Polyline { points: pts, closed: true });
    s.plane = ringdesign_core::sketch::Workplane::section();
    s
}

fn knob(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Each seat's pilot opened just inside its bearing, past the bur's own, so the light runs through to the finger with no
/// skin left at the bore.
fn pilots_sketch(plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = "Pilots".into();
    let r = ruby();
    let o = s.point([0.0, 0.0]);
    let rim = s.point([0.5 * r.w_mm - ledge_mm(r) - PILOT_INSET_MM, 0.0]);
    s.entity(Geometry::Circle { center: o, rim });
    let g = sapphire();
    let grow = -ledge_mm(g) - PILOT_INSET_MM;
    for k in 0..LIGHTS {
        let ids: Vec<Id> = collet_outline(0.5 * g.w_mm + grow, 0.5 * g.l_mm + grow, light_at_mm(), light_deg(k)).iter().step_by(4).map(|p| s.point(*p)).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

/// The bearing ledge a collet stands inside the girdle, mm.
fn ledge_mm(g: Gem) -> f64 {
    (0.09 * g.w_mm).clamp(0.18, 0.45)
}

/// Height of the bare stock's table over the finger's axis, mm.
fn table_y() -> Result<f64> {
    let b = mesh::try_build(&base()?, &AlphaLibrary::default(), coarse_params())?;
    Ok(b.mesh.vertices.iter().map(|p| p.1 as f64).fold(f64::MIN, f64::max))
}

/// A blind arcade of lancets struck into a head wall: each lancet's centre seen along the wall's own axis, the wall's
/// side (+1 or −1), and whether the wall faces along the finger (a cheek) or round the ring (an end).
fn arcade(d: &RingDesign, a: &ringdesign_core::skin::Atlas, name: &str, cheek: bool, side: f64) -> Vec<ringdesign_core::setting::Stamp> {
    use ringdesign_core::cad::builders::cutters::{Shape, outline};
    let lancet: Vec<[f64; 2]> = outline(Shape::Lancet, ARCADE_H_MM, ARCADE_W_MM, 0.0);
    let mut out = Vec::new();
    for (k, off) in ARCADE_AT.iter().enumerate() {
        // The sample on the wall nearest the lancet's centre.
        let want = |s: &ringdesign_core::skin::Sample| {
            let on = if cheek { s.p[2] * side > 0.0 && a.cheek(s) > 0.9 } else { s.p[0] * side > 0.0 && s.n[0] * side > 0.85 };
            on.then(|| if cheek { (s.p[0] - off).hypot(s.p[1] - ARCADE_Y_MM) } else { (s.p[2] - off).hypot(s.p[1] - ARCADE_Y_MM) })
        };
        let Some(best) = a.samples.iter().filter_map(|s| want(s).map(|dist| (dist, s))).min_by(|p, q| p.0.total_cmp(&q.0)) else { continue };
        let _ = d;
        out.push(ringdesign_core::setting::Stamp {
            name: format!("{name}, {}", k + 1),
            theta_deg: best.1.theta,
            v_mm: best.1.v,
            rot_deg: knob(if cheek { "ROSA_ROT_C" } else { "ROSA_ROT_E" }, if cheek { 90.0 } else { 0.0 }),
            outline: lancet.clone(),
            height_mm: 0.3,
            sink_mm: ARCADE_SINK_MM,
            draft_deg: 0.0,
            cut: true,
            bench: false,
            along_pull: cheek,
            tier: 0,
            top: ringdesign_core::setting::StampTop::Flat,
            fine_cap: false,
        });
    }
    out
}

fn ruby() -> Gem {
    Gem { preview_tint: Some(RUBY), ..Gem::calibrated(GemCut::Round, RUBY_MM) }
}
fn sapphire() -> Gem {
    let w = knob("ROSA_W", LIGHT_W_MM);
    Gem { preview_tint: Some(SAPPHIRE), w_mm: w, l_mm: w * LIGHT_L_MM / LIGHT_W_MM, ..Gem::calibrated(GemCut::Oval, LIGHT_W_MM) }
}

fn feature(id: Id, name: &str, operation: Operation, component: Component) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation, component }
}
fn cut() -> Component {
    Component { role: ComponentRole::Other, attach: Attach::Cut, stage: Stage::Cast, ..Default::default() }
}

/// The design: the stock; the rose net traced; the petals sunk and the spandrels pierced; the oculus moulding; the ruby in
/// its collet; one sapphire light in its collet, arrayed round the ruby.
/// Plane over the table at `offset_mm`.
fn table_plane(offset_mm: f64) -> Operation {
    Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm }
}

/// One ring of a collet as a sketch on `plane`: between a loop round the stone's plan grown by `inner_mm` and one grown
/// by `outer_mm`, for the stone centred `at` mm out along `deg` (a round stone at the centre when `at` is 0).
fn collet_ring(name: &str, plane: Id, g: Gem, at: f64, deg: f64, inner_mm: f64, outer_mm: f64) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    for grow in [outer_mm, inner_mm] {
        let pts = collet_outline(0.5 * g.w_mm + grow, 0.5 * g.l_mm + grow, at, deg);
        let ids: Vec<Id> = pts.iter().step_by(2).map(|p| s.point(*p)).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: cad::FaceRef::bare(0) });
    s
}

/// A drawn collet: a straight lip ring from over the girdle down to the bearing, and under it a ring standing a ledge
/// inside the girdle, down into the metal; `floor_mm` is the metal under it below the table. Appended from id `next`.
fn collet(doc: &mut Document, next: &mut Id, who: &str, g: Gem, at: f64, deg: f64, girdle_mm: f64, floor_mm: f64, step_mm: f64) -> Result<(Id, Id)> {
    let none = Component::default;
    let join = || Component { attach: Attach::Join, stage: Stage::Cast, role: ComponentRole::Setting, blend_mm: knob("ROSA_CB", 0.0), ..none() };
    let half = ringdesign_core::setting::girdle_half_mm(g);
    let bearing = girdle_mm - half - BEARING_DROP_MM;
    let ledge = ledge_mm(g);
    let mut id = || {
        *next += 1;
        *next - 1
    };
    let (top, sk1, lip) = (id(), id(), id());
    doc.append(feature(top, &format!("{who}: lip height"), table_plane(girdle_mm + LIP_MM), none()))?;
    doc.append(feature(sk1, &format!("{who}: lip ring"), Operation::Sketch { sketch: collet_ring("Lip ring", top, g, at, deg, SEAT_CLEAR_MM, SEAT_CLEAR_MM + COLLET_WALL_MM) }, none()))?;
    doc.append(feature(lip, &format!("{who}: raise the lip to the bearing"), Operation::Extrude { sketch: Profile::Feature { feature: sk1 }, height_mm: -(LIP_MM + half + BEARING_DROP_MM + OVERLAP_MM), draft_deg: 0.0 }, Component { blend_mm: 0.0, ..join() }))?;
    let (seat, sk2, foot) = (id(), id(), id());
    doc.append(feature(seat, &format!("{who}: bearing height"), table_plane(bearing), none()))?;
    doc.append(feature(sk2, &format!("{who}: bearing ring"), Operation::Sketch { sketch: collet_ring("Bearing ring", seat, g, at, deg, -ledge, SEAT_CLEAR_MM + COLLET_WALL_MM + step_mm) }, none()))?;
    doc.append(feature(foot, &format!("{who}: stand the bearing on the metal"), Operation::Extrude { sketch: Profile::Feature { feature: sk2 }, height_mm: -(bearing + floor_mm + COLLET_FOOT_MM), draft_deg: 0.0 }, join()))?;
    Ok((lip, foot))
}

/// The design: the stock; the rose net traced; the petals and spandrels sunk; the oculus moulding; the ruby in its drawn
/// collet; one sapphire light in its collet, arrayed round the ruby; every pilot opened to the finger.
fn author() -> Result<(RingDesign, usize, Lands)> {
    let mut d = base()?;
    let mut doc = Document::default();
    let none = Component::default;
    doc.append(feature(1, "Cushion signet, factory 001 at a 19 mm face", Operation::Band, Component { role: ComponentRole::Shank, ..none() }))?;
    doc.append(feature(2, "Table, lifted clear of the metal", table_plane(LIFT_MM), none()))?;
    let rose = traced_rose(2)?;
    let lights = rose.lights;
    let rose_lands = rose.lands;
    doc.append(feature(3, "Rose net traced into lights a bar apart", Operation::Sketch { sketch: rose.sketch }, none()))?;
    doc.append(feature(4, "Tracery top", table_plane(TRACERY_MM), none()))?;
    let mut bar_sketch = rose.bar_sketch;
    bar_sketch.plane.on_face = Some(FaceAnchor { feature: 4, face: cad::FaceRef::bare(0) });
    doc.append(feature(5, "The bars between the lights, inside the outer order", Operation::Sketch { sketch: bar_sketch }, none()))?;
    let _ = &rose.petals;
    doc.append(feature(
        6,
        "Raise the tracery: the oculus order, eight mullions, the heads and the outer order",
        Operation::Extrude { sketch: Profile::Region { feature: 5, region: rose.bars }, height_mm: -(TRACERY_MM + TRACERY_FOOT_MM), draft_deg: 0.0 },
        Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: knob("ROSA_TB", 0.0), ..none() },
    ))?;
    doc.append(feature(
        7,
        "Sink the sixteen spandrels deep",
        Operation::Extrude { sketch: Profile::Regions { feature: 3, regions: rose.spandrels }, height_mm: -(LIFT_MM + SPANDREL_MM), draft_deg: 0.0 },
        cut(),
    ))?;
    let r = ruby();
    doc.append(builders::stone_feature(RUBY_ID, r, Placement::ring(90.0, RUBY_GIRDLE_MM)))?;
    doc.append(builders::feature_on(RUBY_ID + 1, "Oculus seat", builders::BUR, RUBY_ID, json!({"through": false})))?;
    let mut next: Id = RUBY_ID + 2;
    collet(&mut doc, &mut next, "Oculus collet", r, 0.0, 90.0, RUBY_GIRDLE_MM, 0.0, MOULD_STEP_MM)?;
    let s = sapphire();
    let first = next;
    doc.append(builders::stone_feature(
        first,
        s,
        Placement::Ring { theta_deg: 90.0, across_mm: light_at_mm(), height_mm: LIGHT_GIRDLE_MM, spin_deg: 90.0, tilt_deg: 0.0, cant_deg: 0.0 },
    ))?;
    doc.append(builders::feature_on(first + 1, "First light's seat", builders::BUR, first, json!({"through": false})))?;
    next = first + 2;
    let (lip, foot) = collet(&mut doc, &mut next, "First light's collet", s, light_at_mm(), light_deg(0), LIGHT_GIRDLE_MM, 0.0, 0.0)?;
    let about = PatternKind::About { part: RUBY_ID, count: LIGHTS as u32, span_deg: 360.0 };
    let array = |sources: Vec<Id>| Operation::Pattern { sources: cad::pattern::Sources(sources), kind: about.clone() };
    doc.append(feature(next, "Eight sapphire lights round the oculus", array(vec![first]), builders::component(builders::STONE)))?;
    doc.append(feature(next + 1, "Their eight seats", array(vec![first + 1]), builders::component(builders::BUR)))?;
    doc.append(feature(
        next + 2,
        "Their eight collets",
        array(vec![lip, foot]),
        Component { attach: Attach::Join, stage: Stage::Cast, role: ComponentRole::Setting, blend_mm: knob("ROSA_CB", 0.0), ..none() },
    ))?;
    // Each pilot is drilled toward the finger's axis, so it meets the bore square and leaves no knife edge there.
    let table = table_y()?;
    let mut id = next + 3;
    let at = [[0.0, 0.0]].into_iter().chain((0..LIGHTS).map(|k| polar(light_at_mm(), light_deg(k))));
    for (k, [x, y]) in at.enumerate() {
        let r = if k == 0 { 0.52 * 0.5 * r.l_mm + PILOT_GROW_MM } else { 0.52 * 0.5 * s.l_mm + PILOT_GROW_MM };
        let lean = (x / table).atan().to_degrees();
        let name = if k == 0 { "Drill the oculus pilot to the finger".to_string() } else { format!("Drill light {k}'s pilot to the finger") };
        doc.append(feature(
            id,
            &name,
            Operation::Extrude { sketch: Profile::Inline(Sketch::circle(r)), height_mm: -PIERCE_MM, draft_deg: 0.0 },
            Component {
                placement: Placement::Ring { theta_deg: 90.0 - lean, across_mm: y, height_mm: -PILOT_START_MM, spin_deg: 0.0, tilt_deg: knob("ROSA_TILT", 1.0) * lean, cant_deg: 0.0 },
                ..cut()
            },
        ))?;
        id += 1;
    }
    // The nave's oculi down each shoulder, pierced along the surface and mirrored through the crown.
    let first_oculus = id;
    for (k, (off, w)) in OCULI.iter().enumerate() {
        doc.append(Feature {
            id,
            name: format!("Oculus of the nave, {}", k + 1),
            enabled: true,
            operation: Operation::Builder { key: builders::PIERCE.into(), on: None, params: json!({"shape": if *w >= 1.2 { "Quatrefoil" } else { "Round" }, "width_mm": w, "length_mm": w, "through": true}) },
            component: Component { placement: Placement::ring(90.0 - off, 0.0), ..builders::component(builders::PIERCE) },
        })?;
        id += 1;
    }
    doc.append(feature(
        id,
        "Mirror the oculi through the crown",
        Operation::Pattern { sources: cad::pattern::Sources((first_oculus..id).collect()), kind: PatternKind::Mirror { plane: cad::MirrorPlane::Section { theta_deg: 90.0 } } },
        builders::component(builders::PIERCE),
    ))?;
    if let Ok(off) = std::env::var("ROSA_OFF") {
        for id in off.split(',').filter_map(|t| t.parse::<Id>().ok()) {
            if let Some(f) = doc.features.iter_mut().find(|f| f.id == id) {
                f.enabled = false;
            }
        }
    }
    d.cad = Some(doc);
    let a = ringdesign_core::skin::Atlas::of(&d, 1024, 384)?;
    for (name, cheek, side) in [("Wall arcade, near cheek", true, 1.0), ("Wall arcade, far cheek", true, -1.0), ("Wall arcade, right end", false, 1.0), ("Wall arcade, left end", false, -1.0)] {
        let row = arcade(&d, &a, name, cheek, side);
        d.stamps.extend(row);
    }
    Ok((d, lights, rose_lands))
}

/// Stones the preview draws: its loose triangles welded into connected pieces.
fn preview_count(stones: &[(mesh::Mesh, [f32; 3])]) -> usize {
    fn find(p: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let n = p[j];
            p[j] = r;
            j = n;
        }
        r
    }
    let mut total = 0;
    for (m, _) in stones {
        let mut index = std::collections::HashMap::new();
        let mut parent: Vec<usize> = Vec::new();
        for f in &m.faces {
            let ids: Vec<usize> = f
                .iter()
                .map(|&i| {
                    let v = m.vertices[i as usize];
                    let key = [v.0, v.1, v.2].map(|c| (c as f64 * 1e3).round() as i64);
                    *index.entry(key).or_insert_with(|| {
                        parent.push(parent.len());
                        parent.len() - 1
                    })
                })
                .collect();
            for k in 1..3 {
                let (a, b) = (find(&mut parent, ids[0]), find(&mut parent, ids[k]));
                parent[a] = b;
            }
        }
        total += (0..parent.len()).filter(|&i| find(&mut parent, i) == i).count();
    }
    total
}

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
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

/// Views: yaw about the finger axis, pitch from looking along the finger (0) to down onto the table (pi/2).
const VIEWS: &[(&str, f64, f64)] = &[
    ("hero", -0.6, 1.0),
    ("face", 0.0, FRAC_PI_2),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.0),
    ("shoulder", 0.75, 0.6),
    ("reverse", PI - 0.5, 0.35),
];

/// The metal split for the antique finish: faces sunk under the table or walling a cut, darkened at the bench.
fn antiqued(m: &mesh::Mesh, table_y: f64) -> (mesh::Mesh, mesh::Mesh) {
    let dark_face = |f: &[u32; 3]| {
        let Some((a, b, c)) = m.triangle(f) else { return false };
        let y = (a[1] + b[1] + c[1]) / 3.0;
        let r = ((a[0] + b[0] + c[0]) / 3.0).hypot((a[2] + b[2] + c[2]) / 3.0);
        // Everything sunk under the table inside the rose: the spandrels' floors and walls, the pilots.
        r < R_OUT + RING_MM - 0.05 && y < table_y + TRACERY_MM - 0.05 && y > table_y - 3.0
    };
    let mut bright = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..mesh::Mesh::default() };
    let mut dark = bright.clone();
    let corners: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    for (i, f) in m.faces.iter().enumerate() {
        let target = if dark_face(f) { &mut dark } else { &mut bright };
        if let Some(c) = corners.get(&(i as u32)) {
            target.corner_normals.push((target.faces.len() as u32, *c));
        }
        target.faces.push(*f);
    }
    (bright, dark)
}
const ANTIQUE: [f32; 3] = [0.16, 0.12, 0.06];

fn renders(out: &Path, lib: &AlphaLibrary, d: &RingDesign, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let bare = base()?;
    let b = mesh::try_build(&bare, lib, draft_params())?;
    let table_y = b.mesh.vertices.iter().map(|p| p.1 as f64).fold(f64::MIN, f64::max);
    let finished = render::finished_from(d, lib, built);
    let (bright, dark) = antiqued(&finished.metal, table_y);
    let mut parts = vec![render::Part::metal(&bright, render::GOLD), render::Part::metal(&dark, ANTIQUE)];
    parts.extend(finished.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, *yaw, *pitch, edge)?;
    }
    for name in ["hero", "face"] {
        let (_, yaw, pitch) = VIEWS.iter().find(|v| v.0 == name).unwrap();
        render::write_png_parts(out.join(format!("{name}-300.png")), &parts, *yaw, *pitch, 300)?;
    }
    // Stones: the rose close and square, framed on the table's own metal.
    let close = crop(&bright, table_y);
    let mut cparts = vec![render::Part::metal(&close, render::GOLD), render::Part::metal(&dark, ANTIQUE)];
    cparts.extend(finished.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    render::write_png_parts(out.join("stones.png"), &cparts, -0.25, 1.25, edge)?;
    let (_, yaw, pitch) = VIEWS[0];
    let bare_img = render::render_parts_ss(&[render::Part::metal(&b.mesh, render::GOLD)], yaw, pitch, edge, edge, 3);
    let finished_img = render::render_parts_ss(&parts, yaw, pitch, edge, edge, 3);
    side_by_side(&out.join("bare-vs-finished.png"), &bare_img, &finished_img, edge)?;
    let tiles: Vec<Vec<u8>> = ["hero", "face", "side", "palm"]
        .iter()
        .map(|n| {
            let (_, yaw, pitch) = VIEWS.iter().find(|v| v.0 == *n).unwrap();
            render::render_parts_ss(&parts, *yaw, *pitch, 300, 300, 3)
        })
        .collect();
    let mut sheet = vec![0u8; 1200 * 300 * 3];
    for (k, t) in tiles.iter().enumerate() {
        for y in 0..300 {
            sheet[y * 3600 + k * 900..y * 3600 + (k + 1) * 900].copy_from_slice(&t[y * 900..(y + 1) * 900]);
        }
    }
    image::save_buffer(out.join("contact-300.png"), &sheet, 1200, 300, image::ColorType::Rgb8)?;
    Ok(())
}

/// The metal near the table, so a close-up frames on the rose.
fn crop(m: &mesh::Mesh, table_y: f64) -> mesh::Mesh {
    let keep = |i: u32| m.vertices[i as usize].1 as f64 > table_y - 4.0;
    let mut out = mesh::Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..mesh::Mesh::default() };
    let corners: std::collections::HashMap<u32, [mesh::Vec3; 3]> = m.corner_normals.iter().copied().collect();
    for (i, f) in m.faces.iter().enumerate() {
        if f.iter().all(|&v| keep(v)) {
            if let Some(c) = corners.get(&(i as u32)) {
                out.corner_normals.push((out.faces.len() as u32, *c));
            }
            out.faces.push(*f);
        }
    }
    out
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae").join(SLUG));
    std::fs::create_dir_all(&out)?;
    println!("Rosa");
    let (mut d, lights, rose_lands) = author()?;
    println!("  lands {rose_lands:?}");
    if std::env::var("ROSA_ATLAS").is_ok() {
        let a = ringdesign_core::skin::Atlas::of(&base()?, 1024, 384)?;
        let span = |f: &dyn Fn(&ringdesign_core::skin::Sample) -> bool| {
            let v: Vec<&ringdesign_core::skin::Sample> = a.samples.iter().filter(|s| f(s)).collect();
            let r = |k: usize| v.iter().map(|s| s.p[k]).fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            (v.len(), r(0), r(1), r(2))
        };
        println!("  top {:.2}", a.top);
        println!("  cheek +z: {:?}", span(&|s| s.p[2] > 0.0 && a.cheek(s) > 0.9));
        println!("  end +x: {:?}", span(&|s| s.n[0] > 0.85 && s.p[1] > a.top - 4.0));
        for y in [9.5, 10.5, 11.5, 12.3] {
            println!("  cheek +z at y {y}: x {:?}", span(&|s| s.p[2] > 0.0 && a.cheek(s) > 0.9 && (s.p[1] - y).abs() < 0.2).1);
            println!("  end +x at y {y}: z {:?}", span(&|s| s.n[0] > 0.85 && (s.p[1] - y).abs() < 0.2).3);
        }
        return Ok(());
    }
    if std::env::var("ROSA_LANDS_ONLY").is_ok() {
        return Ok(());
    }
    let lib = AlphaLibrary::builtin();
    let params = if draft { draft_params() } else { export_params() };
    d.build = params;
    let started = std::time::Instant::now();
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    println!("  {} triangles in {build_s:.1} s; watertight {}; degenerate {}; traced lights {lights}", built.mesh.faces.len(), v.watertight, q.degenerate_faces);
    let features: Vec<(Id, String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| {
            let s = match &r.status {
                FeatureStatus::Ok => "Ok".to_string(),
                FeatureStatus::Suppressed => "Suppressed".into(),
                FeatureStatus::Failed(m) => format!("Failed: {m}"),
                FeatureStatus::Skipped(m) => format!("Skipped: {m}"),
            };
            (r.id, r.name.clone(), s)
        })
        .collect();
    for (id, name, s) in &features {
        println!("    #{id} {name}: {s}");
    }
    let made: Vec<(String, usize)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.components.iter())
        .map(|c| (c.name.clone(), csg::self_crossings(&solid_of(&c.mesh))))
        .collect();
    if std::env::var("ROSA_PARTS").is_ok() {
        for c in built.parts.evaluated.iter().flat_map(|e| e.components.iter()) {
            let t = cad::measure::thickness(&c.mesh, MIN_SECTION_MM);
            if c.id == 11 || c.id == 8 {
                let b = c.mesh.bounds();
                println!("      bounds {b:?}");
                render::write_png_parts(format!("/tmp/claude-0/part-{}.png", c.id), &[render::Part::metal(&c.mesh, render::GOLD)], 0.3, 0.5, 800)?;
            }
            println!("    part #{} {}: {} faces, thickness min {:?} below {} of {} at {:?}", c.id, c.name, c.mesh.faces.len(), t.sampled_min_mm, t.below_limit, t.rays, t.point);
        }
    }
    if let Ok(spec) = std::env::var("ROSA_RAY") {
        let v: Vec<f64> = spec.split(',').filter_map(|t| t.parse().ok()).collect();
        let (o, dir) = ([v[0], v[1], v[2]], [v[3], v[4], v[5]]);
        let m = &built.mesh;
        let mut hits = Vec::new();
        for f in &m.faces {
            let p: Vec<[f64; 3]> = f.iter().map(|&i| { let q = m.vertices[i as usize]; [q.0 as f64, q.1 as f64, q.2 as f64] }).collect();
            let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let cr = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
            let dt = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let h = cr(dir, e2);
            let det = dt(e1, h);
            if det.abs() < 1e-12 { continue; }
            let sv = [o[0] - p[0][0], o[1] - p[0][1], o[2] - p[0][2]];
            let u = dt(sv, h) / det;
            if !(0.0..=1.0).contains(&u) { continue; }
            let q = cr(sv, e1);
            let w = dt(dir, q) / det;
            if w < 0.0 || u + w > 1.0 { continue; }
            let t = dt(e2, q) / det;
            if t > -1.0 { hits.push((t, det > 0.0)); }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!("  ray hits: {:?}", hits);
    }
    let ring_crossings = csg::self_crossings(&solid_of(&built.mesh));
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|p| (p.0 as f64).hypot(p.1 as f64) < bore - 0.01).count();
    let min_r = built.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    let mut field = castability::attributed_field_report(&d, &lib, &d.draft, 256, 128);
    castability::judge_parts(&mut field, &d, &built);
    let findings = dfm::findings_in(&d, &lib);
    let stones_report = ringdesign_core::stones::report(&d, field.parting_z_mm);
    let previewed = preview_count(&ringdesign_core::gems::built_meshes(&d, &lib, &built));
    let bare_min_r = mesh::try_build(&base()?, &lib, coarse_params())?.mesh.vertices.iter().map(|p| (p.0 as f64).hypot(p.1 as f64)).fold(f64::MAX, f64::min);
    println!("  bare stock min vertex radius {bare_min_r:.3}");
    if std::env::var("ROSA_BARE").is_ok() {
        let bm = mesh::try_build(&base()?, &lib, coarse_params())?.mesh;
        let t = cad::measure::thickness(&bm, MIN_SECTION_MM);
        println!("  bare thickness: min {:?}, {} below of {} at {:?}", t.sampled_min_mm, t.below_limit, t.rays, t.point);
        let b = base()?;
        for yl in [7.0, 8.0, 8.5, 9.0] {
            let zmax = bm.vertices.iter().filter(|p| (p.0 as f64).hypot(p.1 as f64) < 9.8 && (p.1 as f64) > yl).map(|p| (p.2 as f64).abs()).fold(0.0, f64::max);
            println!("  bore edge above y {yl}: |z| {zmax:.2}");
        }
        println!("  profile thickness {:.2}, edge round {:.2}, comfort {:.2}", b.profile.thickness_mm, b.profile.edge_round_mm, b.profile.comfort_fit_mm);
    }
    if std::env::var("ROSA_TABLE").is_ok() {
        let bare = mesh::try_build(&base()?, &lib, params)?.mesh;
        for zr in [-8.5, -7.0, -5.0, -3.0, 0.0, 3.0, 5.0, 7.0, 8.5] {
            let row: Vec<String> = [-8.5, -7.0, -5.0, -3.0, 0.0, 3.0, 5.0, 7.0, 8.5]
                .iter()
                .map(|&xr: &f64| {
                    let y = bare.vertices.iter().filter(|p| ((p.0 as f64) - xr).hypot((p.2 as f64) - zr) < 0.35 && p.1 > 8.0).map(|p| p.1 as f64).fold(f64::MIN, f64::max);
                    format!("{y:7.3}")
                })
                .collect();
            println!("    z {zr:5.1}: {}", row.join(" "));
        }
    }
    let coarse = BuildParams { theta_steps: 384, profile_steps: 160, ..params };
    let thin_mesh = mesh::try_build(&d, &lib, coarse)?.mesh;
    let thickness = cad::measure::thickness(&thin_mesh, MIN_SECTION_MM);
    println!(
        "  thickness at {}x{} ({} faces): {} rays, min {:?}, {} below, {} unresolved at {:?}",
        coarse.theta_steps,
        coarse.profile_steps,
        thin_mesh.faces.len(),
        thickness.rays,
        thickness.sampled_min_mm,
        thickness.below_limit,
        thickness.unresolved,
        thickness.point
    );
    let lands = dfm::cut_lands(&d, &built, MIN_SECTION_MM);
    let grams = built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams);
    library::save_design(out.join("design.ring.json"), &d)?;
    let design_bytes = std::fs::metadata(out.join("design.ring.json"))?.len();
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
    let pattern = mesh::try_build_pattern(&d, &lib, params)?;
    let pattern_crossings = csg::self_crossings(&solid_of(&pattern.mesh));
    let pattern_ok = pattern.report.validation.watertight && pattern.report.quality.degenerate_faces == 0 && pattern_crossings == 0;
    let stone_count = stones_report.as_ref().map_or(0, |s| s.stone_count) as usize;
    let lands_out: Vec<String> = lands.iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let gates = json!({
        "watertight": v.watertight,
        "degenerate_faces": q.degenerate_faces,
        "ring_self_crossings": ring_crossings,
        "made_part_self_crossings": made,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "features": features.iter().map(|(id, n, s)| json!({"id": id, "name": n, "status": s})).collect::<Vec<_>>(),
        "stamps_unresolved": built.solids.notes.len(),
        "bore_radius_mm": bore,
        "min_vertex_radius_mm": min_r,
        "vertices_inside_bore": inside,
        "field_verdict": field.verdict.label(),
        "field_notes": field.notes,
        "thinnest_wall_mm": field.thinnest_wall_mm,
        "investment_min_section_mm": d.draft.min_section_mm,
        "thickness": {"build": [coarse.theta_steps, coarse.profile_steps], "faces": thin_mesh.faces.len(), "limit_mm": thickness.limit_mm, "rays": thickness.rays, "sampled_min_mm": thickness.sampled_min_mm, "below_limit": thickness.below_limit, "unresolved": thickness.unresolved, "note": thickness.note},
        "cut_lands_0_8": lands_out,
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones_reported": stone_count,
        "stones_previewed": previewed,
        "stones_crowding": stones_report.as_ref().map(|s| json!({"tight_pairs": s.tight_pairs, "closest_gap_mm": s.closest.as_ref().map(|c| c.gap_mm), "closest": s.closest.as_ref().map(|c| format!("{} / {}", c.a, c.b)), "note": s.crowding_note(), "fill_floor_mm": s.fill_floor_mm, "carats": s.total_carats})),
        "cold_reload_identical": cold,
        "triangles": built.mesh.faces.len(),
        "pattern": {"watertight": pattern.report.validation.watertight, "degenerate_faces": pattern.report.quality.degenerate_faces, "self_crossings": pattern_crossings},
    });
    let passed = v.watertight
        && q.degenerate_faces == 0
        && ring_crossings == 0
        && made.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && features.iter().all(|(_, _, s)| s == "Ok")
        && inside == 0
        && thickness.rays > 0
        && thickness.below_limit == 0
        && thickness.unresolved == 0
        && lands.is_empty()
        && findings.is_empty()
        && stone_count == previewed
        && cold != Some(false)
        && built.mesh.faces.len() <= 2_000_000
        && pattern_ok
        && rose_lands.collet_to_spandrel_mm >= MIN_SECTION_MM
        && rose_lands.ruby_collet_to_petal_mm >= 0.0
        && rose_lands.pilot_to_pilot_mm >= MIN_SECTION_MM
        && rose_lands.spandrel_to_table_edge_mm >= MIN_SECTION_MM;
    let block = json!({"build": [params.theta_steps, params.profile_steps], "build_s": build_s, "traced_lights": lights, "lands": rose_lands, "gates": gates, "gates_passed": passed});
    let report_path = out.join("report.json");
    let mut report: serde_json::Value = std::fs::read(&report_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    report["name"] = json!(d.name);
    report["process"] = json!(d.draft.process.label());
    report["alloy"] = json!(ALLOY);
    report["base"] = json!({"stock": STOCK, "face_mm": [FACE_MM, FACE_MM], "bore_mm": BORE_MM});
    report["size"] = json!(d.size.display());
    report["grams_gold_18k"] = json!(grams);
    report["design_bytes"] = json!(design_bytes);
    report["cad_features"] = json!(d.cad.as_ref().map_or(0, |c| c.features.len()));
    report["draft_settings"] = json!(d.draft);
    report[if draft { "draft" } else { "export" }] = block;
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        write_reference_stones(&out, &d, &lib, &built)?;
    }
    println!("  field {}, thinnest {:.2}; dfm {}; lands {}; stones {stone_count}/{previewed}; min r {min_r:.3} vs bore {bore:.3}; {grams:.1} g 18k", field.verdict.label(), field.thinnest_wall_mm, findings.len(), lands.len());
    for f in findings.iter().chain(&lands) {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for n in built.parts.notes.iter().chain(&built.solids.notes) {
        println!("    note: {n}");
    }
    renders(&out, &lib, &d, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if passed { "passed" } else { "FAILED" });
    ensure!(passed || std::env::var("ROSA_ALLOW_FAIL").is_ok(), "Rosa failed its gates; see {}", report_path.display());
    Ok(())
}

/// Each stone's reference mesh, one STL per kind.
fn write_reference_stones(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> Result<()> {
    let mut entries = Vec::new();
    for (m, tint) in ringdesign_core::gems::built_meshes(d, lib, built) {
        let (file, name, ior, dispersion) = if tint[0] > tint[2] { ("reference-ruby.stl", "Ruby oculus", 1.77, 0.018) } else { ("reference-sapphire.stl", "Sapphire lights", 1.77, 0.018) };
        stl::write_stl(out.join(file), &m, &format!("Rosa reference {name}"))?;
        entries.push(json!({"mesh": file, "name": name, "tint": tint, "ior": ior, "dispersion": dispersion, "roughness": 0.065, "transmission": 0.72}));
    }
    std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({"stones": entries}))?)?;
    Ok(())
}
