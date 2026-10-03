//! Tenebrae — Lanterna, the octagon lantern: Ely's octagon and its lantern raised on the factory 015 octagon, the
//! lantern's eight windows glazed with citrine, its star vault gathered into an amethyst boss, pinnacles on every
//! corner of both stages. Lost wax, Gold 18k.
//! cargo build --release -p ringdesign-core --example tenebrae_lanterna
//! target/release/examples/tenebrae_lanterna [OUT_DIR] [--draft] [--verify]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, RingDesign,
    cad::{
        self, Attach, Component, ComponentRole, Document, EvaluatedComponent, FaceSeat, Feature, Operation, PatternKind,
        FaceRef, MirrorPlane, Placement, PlaneBase, Profile, builders, pattern::Sources,
    },
    castability::CastProcess,
    csg, dfm,
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS},
    library, manufacturing as mf, mesh, render,
    sketch::{FaceAnchor, Geometry, Id, Sketch},
    stl,
};
use serde_json::json;
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];
type P3 = [f64; 3];

/// Bore diameter, mm.
const BORE_MM: f64 = 18.6;
/// The factory stock: 015 Octagon at its native 16 × 16 face.
const STOCK: &str = "015";
const FACE_MM: f64 = 16.0;

// Every lantern part is drawn in the table's frame: x along the finger, y round the ring, z out of the table, origin at its
// centre. The shoulders' buttresses are drawn in the world: x and y in the plane of the ring, z along the finger.

/// How far every part standing on the table, or on another part, sinks into what it stands on, mm.
const SINK_MM: f64 = 0.3;
/// The seam bead where the lantern meets the table.
const BLEND_MM: f64 = 0.0;
/// The lantern: a regular octagon standing on the factory octagon, its walls facing the table's own sides and corners. Apothem,
/// height, and the wall and roof left when it is hollowed; the hollow runs down through the head to the finger.
const DRUM_APOTHEM_MM: f64 = 6.4;
const DRUM_TOP_MM: f64 = 6.8;
const WALL_MM: f64 = 1.0;
const HOLLOW_FROM_MM: f64 = -6.5;
/// The rim moulding round the roof: its width and how proud it stands, drafted so both its faces lean.
const RIM_W_MM: f64 = 0.9;
const RIM_PROUD_MM: f64 = 0.5;
const RIM_DRAFT_DEG: f64 = 22.0;
/// The star vault on the roof: a rib from the boss to every corner, its foot and how proud it stands, the draft that gables it.
const RIB_BAR_MM: f64 = 0.8;
const RIB_PROUD_MM: f64 = 0.45;
const RIB_DRAFT_DEG: f64 = 35.0;
/// The boss at the vault's crown that carries the amethyst: radius and height over the roof, mm.
const BOSS_R_MM: f64 = 2.8;
const BOSS_H_MM: f64 = 0.5;
/// The amethyst's collet: its wall, and its lip as a share of the crown, low so the rim stands square.
const COLLET_WALL_MM: f64 = 0.6;
const COLLET_LIP: f64 = 0.1;
/// The vault's lights: a lancet pierced through the roof between every two ribs, pointing out to its wall: width, sill and apex
/// from the axis.
const LIGHT_W_MM: f64 = 1.3;
const LIGHT_FROM_MM: f64 = 3.15;
const LIGHT_TO_MM: f64 = 5.5;
/// The lantern's windows, traced from a net: the outline's width, sill and spring over the table (an equilateral head), the
/// mullion's fork over the spring and where its branches meet the head, and the tracery bar. They pierce the wall.
const WINDOW_W_MM: f64 = 3.4;
const WINDOW_SILL_MM: f64 = 0.6;
const WINDOW_SPRING_MM: f64 = 3.0;
const WINDOW_FORK_MM: f64 = 0.4;
const WINDOW_BRANCH_MM: f64 = 0.8;
const WINDOW_BAR_MM: f64 = 0.8;
/// How far outside a wall a cut's sketch is drawn, so its land is read on the metal it enters.
const CUT_OVER_MM: f64 = 0.3;
/// The lantern's corner piers and how high they rise past the roof.
const PIER_W_MM: f64 = 1.4;
const PIER_TOP_MM: f64 = 7.3;
/// The spirelets: a twisted square sweep off each pier's top, never thinner than the section.
const SPIRE_W_MM: f64 = 1.2;
const SPIRE_H_MM: f64 = 1.5;
const SPIRE_TWIST_DEG: f64 = 90.0;
/// How far a spirelet starts down inside its pier, so no face of one lies in the other's.
const SPIRE_SINK_MM: f64 = 0.1;
/// The spire cap: a pyramid off the spirelet's top to a point, its height and its point.
const CAP_H_MM: f64 = 1.9;
const CAP_POINT_MM: f64 = 0.04;
/// The crockets: a leaf knob on each arris of the cap, their radius and how far up the cap they stand.
const CROCKET_R_MM: f64 = 0.45;
const CROCKET_AT: f64 = 0.35;
/// The corbel on each pier's outward face that carries its lamp: its section as (out from the pier's axis, height) corners, and
/// its width.
const CORBEL: [P2; 4] = [[0.4, 2.3], [2.2, 3.5], [2.2, 3.8], [0.4, 5.6]];
const CORBEL_HALF_MM: f64 = 1.1;
/// The lamps: a citrine on every corbel's sloping face, and its collet's wall.
const LAMP_MM: f64 = 1.5;
const LAMP_WALL_MM: f64 = 0.3;
/// The clasping buttress on each shoulder: where it leans on the head and where it lands, its stages as (to θ, proud of the
/// shoulder), the set-offs between them, and its width along the finger.
const BUTTRESS_FROM_DEG: f64 = 122.5;
const BUTTRESS_STAGES: [(f64, f64); 3] = [(131.0, 1.5), (140.0, 1.2), (150.0, 0.9)];
const SET_OFF_DEG: f64 = 2.2;
const BUTTRESS_HALF_MM: f64 = 0.9;
/// The blind lancets on the head's walls, drawn in the table's frame on a plane outside each wall: the long walls' plane and
/// pairs, the chamfers' plane and pair, each light's width and the bar of a pair, the sill and the spring under the table, and the
/// floor they are cut back to, inside the plane.
const SIDE_PLANE_MM: f64 = 8.35;
const SIDE_PAIRS: [f64; 2] = [-3.6, 3.6];
const SIDE_W_MM: f64 = 1.0;
const SIDE_BAR_MM: f64 = 0.9;
const CHAMFER_PLANE_MM: f64 = 9.95;
const CHAMFER_W_MM: f64 = 0.8;
const CHAMFER_BAR_MM: f64 = 0.85;
const SIDE_SILL_MM: f64 = -2.8;
const SIDE_SPRING_MM: f64 = -1.75;
const SIDE_CUT_MM: f64 = 0.9;

/// Investment section floor, mm.
const MIN_SECTION_MM: f64 = 0.8;
const ALLOY: &str = "Gold 18k";

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}

fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

// --- Stones ----------------------------------------------------------------------------------------

const AMETHYST: [f32; 3] = [0.22, 0.05, 0.38];
const CITRINE: [f32; 3] = [0.75, 0.42, 0.03];

fn amethyst() -> Gem {
    Gem::calibrated(GemCut::Asscher, 3.0)
}

fn citrine() -> Gem {
    Gem::calibrated(GemCut::Round, LAMP_MM)
}

/// The light's lancet in the roof, its sill toward the boss and its apex toward the wall, at bearing 0.
fn light_outline() -> Vec<P2> {
    let spring = LIGHT_TO_MM - LIGHT_W_MM * 3f64.sqrt() / 2.0;
    lancet(LIGHT_W_MM, LIGHT_FROM_MM, spring, 2.0).into_iter().map(|[across, up]| [up, across]).collect()
}

// --- Plan geometry ---------------------------------------------------------------------------------

/// Corner `k` of a regular octagon of apothem `a` whose sides face 0°, 45°, …: the corners stand at 22.5° + 45° k.
fn corner(a: f64, k: usize) -> P2 {
    let r = a / (PI / 8.0).cos();
    let t = PI / 8.0 + k as f64 * PI / 4.0;
    [r * t.cos(), r * t.sin()]
}

fn octagon(s: &mut Sketch, a: f64) -> Id {
    let pts: Vec<Id> = (0..8).map(|k| s.point(corner(a, k))).collect();
    s.entity(Geometry::Polyline { points: pts, closed: true })
}

fn at_z(name: &str, z: f64) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    s.plane.origin = [0.0, 0.0, z];
    s
}

/// A pointed lancet: sill `w` wide at `from`, straight jambs to `spring`, a head of two arcs of radius `arc × w/2`, in (across, up).
fn lancet(w: f64, from: f64, spring: f64, arc: f64) -> Vec<P2> {
    lancet_in(w, from, spring, arc, 12)
}

/// [`lancet`] with each arc of its head walked in `steps` chords.
fn lancet_in(w: f64, from: f64, spring: f64, arc: f64, steps: usize) -> Vec<P2> {
    let h = w / 2.0;
    let r = arc * h;
    // The right arc turns about (h − r, spring), the left about (r − h, spring).
    let c = h - r;
    let rise = (r * r - c * c).sqrt();
    let end = (rise / r).asin();
    let mut pts = vec![[-h, from], [h, from], [h, spring]];
    for i in 1..steps {
        let a = end * i as f64 / steps as f64;
        pts.push([c + r * a.cos(), spring + r * a.sin()]);
    }
    pts.push([0.0, spring + rise]);
    for i in (1..steps).rev() {
        let a = end * i as f64 / steps as f64;
        pts.push([-c - r * a.cos(), spring + r * a.sin()]);
    }
    pts.push([-h, spring]);
    pts
}

fn window_apex() -> f64 {
    WINDOW_SPRING_MM + WINDOW_W_MM * 3f64.sqrt() / 2.0
}

/// A square of side `w` centred at `c`, turned `turn` radians.
fn square(s: &mut Sketch, c: P2, w: f64, turn: f64) -> Id {
    let (st, ct) = turn.sin_cos();
    let pts: Vec<Id> = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
        .iter()
        .map(|[x, y]| {
            let (x, y) = (x * w / 2.0, y * w / 2.0);
            s.point([c[0] + x * ct - y * st, c[1] + x * st + y * ct])
        })
        .collect();
    s.entity(Geometry::Polyline { points: pts, closed: true })
}

// --- The feature tree ------------------------------------------------------------------------------

struct Tree {
    doc: Document,
}

impl Tree {
    fn add(&mut self, name: &str, operation: Operation, component: Component) -> Id {
        let id = self.doc.features.len() as Id + 1;
        self.doc.append(Feature { id, name: name.into(), enabled: true, operation, component }).expect("feature appends");
        id
    }
    fn sketch(&mut self, name: &str, sketch: Sketch) -> Id {
        self.add(name, Operation::Sketch { sketch }, Component::default())
    }
    fn push(&mut self, f: Feature) -> Id {
        let id = f.id;
        self.doc.append(f).expect("feature appends");
        id
    }
    fn next(&self) -> Id {
        self.doc.features.len() as Id + 1
    }
}

fn on_table(attach: Attach, blend_mm: f64, role: ComponentRole) -> Component {
    Component { role, attach, blend_mm, material: ALLOY.into(), placement: Placement::ring(90.0, 0.0), ..Component::default() }
}

fn metal(attach: Attach) -> Component {
    Component { attach, material: ALLOY.into(), ..Component::default() }
}

fn glass() -> Component {
    Component { reference: true, role: ComponentRole::Stone, ..Component::default() }
}

/// Eight of part `source` round the axis of part `around`, as `component` says they meet the band.
fn array(t: &mut Tree, name: &str, sources: Vec<Id>, around: Id, component: Component) -> Id {
    t.add(name, Operation::Pattern { sources: Sources(sources), kind: PatternKind::About { part: around, count: 8, span_deg: 360.0 } }, component)
}

fn set_tint(f: &mut Feature, tint: [f32; 3]) {
    if let Operation::Builder { params, .. } = &mut f.operation {
        params[builders::TINT] = json!(tint);
    }
}

/// The faces the stones stand on, read off a first build: the lantern's wall facing along the finger, and the boss's top.
#[derive(Clone, Debug)]
struct Seats {
    lamp: FaceSeat,
    boss: FaceSeat,
}

/// Ids the measures and the stone pass need.
#[derive(Clone, Copy, Debug, Default)]
struct Ids {
    drum: Id,
    corbel: Id,
    boss: Id,
}

fn base() -> Result<RingDesign> {
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == STOCK).context("stock 015")?.load()?;
    ImportedBase::attach(&mut d, source)?;
    d.name = "Lanterna — the octagon lantern".into();
    d.profile.width_mm = FACE_MM;
    d.shank.head.length_mm = FACE_MM;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("size")?;
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    Ok(d)
}

fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).map_or(1.25, |m| m.shrink_pct);
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.recipe.name = "Lanterna / investment / Gold 18k".into();
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.sample_pitch_mm = 0.1;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.bench_notes = "Sacrificial investment pattern, sprued at the palm. The octagon, the lantern, its vault, the piers, spirelets and knops cast in \
        place with the collets; cut the bearings to the measured stones, set the citrine pears point up in the lantern's eight windows and the \
        amethyst on the boss, and open the amethyst's pilot. Polish the ribs, the pier faces, the spirelets and the knops."
        .into();
    s
}

/// A pinnacle on the lantern's first corner: a pier, a twisted spirelet and a knop; then one on every corner.
fn pinnacles(t: &mut Tree, around: Id) -> Id {
    let c = corner(DRUM_APOTHEM_MM, 0);
    let turn = PI / 8.0;
    let mut s = at_z("A corner pier", -SINK_MM);
    square(&mut s, c, PIER_W_MM, turn);
    let pier_s = t.sketch("Draw a pier on the lantern's first corner", s);
    let pier = t.add(
        "Raise the pier past the roof",
        Operation::Extrude { sketch: Profile::Feature { feature: pier_s }, height_mm: PIER_TOP_MM + SINK_MM, draft_deg: 0.0 },
        on_table(Attach::Join, 0.0, ComponentRole::Head),
    );
    let base = PIER_TOP_MM - SPIRE_SINK_MM;
    let mut sec = at_z("The spirelet's section", base);
    square(&mut sec, c, SPIRE_W_MM, turn);
    let mut path = Sketch::default();
    path.name = "The spirelet's axis".into();
    path.plane.origin = [c[0], c[1], base];
    path.plane.x = [0.0, 0.0, 1.0];
    path.plane.y = [turn.cos(), turn.sin(), 0.0];
    let p0 = path.point([0.0, 0.0]);
    let p1 = path.point([SPIRE_H_MM + SPIRE_SINK_MM, 0.0]);
    path.entity(Geometry::Line { a: p0, b: p1 });
    let spire = t.add(
        "Twist the spirelet up off the pier, a quarter turn",
        Operation::Twist { sketch: Profile::Inline(sec), path, degrees: SPIRE_TWIST_DEG, end_scale: (MIN_SECTION_MM + 0.02) / SPIRE_W_MM },
        on_table(Attach::Join, 0.0, ComponentRole::Head),
    );
    // The cap: a pyramid lofted from the spirelet's turned top to a point.
    let cap_from = PIER_TOP_MM + SPIRE_H_MM;
    let end = turn + SPIRE_TWIST_DEG.to_radians();
    let foot_w = MIN_SECTION_MM + 0.02;
    let mut foot = at_z("The cap's foot", cap_from - SPIRE_SINK_MM);
    square(&mut foot, c, foot_w, end);
    let mut tip = at_z("The cap's point", cap_from + CAP_H_MM);
    square(&mut tip, c, CAP_POINT_MM, end);
    let cap = t.add(
        "Loft the spire cap to a point",
        Operation::Loft { sections: vec![Profile::Inline(foot), Profile::Inline(tip)] },
        on_table(Attach::Join, 0.0, ComponentRole::Head),
    );
    // The crockets: a ball on each of the cap's arrises, part sunk into it.
    let z = cap_from - SPIRE_SINK_MM + (CAP_H_MM + SPIRE_SINK_MM) * CROCKET_AT;
    let half = (foot_w / 2.0) * (1.0 - CROCKET_AT) * 2f64.sqrt();
    let mut k = Sketch::default();
    k.name = "A crocket's half section".into();
    let mut crockets = Vec::new();
    for q in 0..4 {
        let a = end + PI / 4.0 + q as f64 * PI / 2.0;
        let at = [c[0] + half * a.cos(), c[1] + half * a.sin()];
        let mut k = k.clone();
        k.plane.origin = [at[0], at[1], z];
        k.plane.x = [a.cos(), a.sin(), 0.0];
        k.plane.y = [0.0, 0.0, 1.0];
        let middle = k.point([0.0, 0.0]);
        let south = k.point([0.0, -CROCKET_R_MM]);
        let north = k.point([0.0, CROCKET_R_MM]);
        k.entity(Geometry::Arc { center: middle, start: south, end: north });
        k.entity(Geometry::Line { a: north, b: south });
        crockets.push(t.add(
            "Set a crocket on the cap's arris",
            Operation::Revolve { sketch: Profile::Inline(k), pivot: [0.0, 0.0, 0.0], axis: [0.0, 1.0, 0.0], degrees: 360.0, in_plane: true },
            on_table(Attach::Join, 0.0, ComponentRole::Other),
        ));
    }
    // The corbel on the pier's outward face, in the plane through the corner and the axis.
    let (sb, cb) = turn.sin_cos();
    let normal = [sb, -cb, 0.0];
    let mut k = Sketch::default();
    k.name = "The corbel's section".into();
    k.plane.origin = [c[0] - normal[0] * CORBEL_HALF_MM, c[1] - normal[1] * CORBEL_HALF_MM, 0.0];
    k.plane.x = [cb, sb, 0.0];
    k.plane.y = [0.0, 0.0, 1.0];
    let pts: Vec<Id> = CORBEL.iter().map(|p| k.point(*p)).collect();
    k.entity(Geometry::Polyline { points: pts, closed: true });
    let corbel_s = t.sketch("Draw the corbel under the first pier's lamp", k);
    let corbel = t.add(
        "Bracket the corbel out from the pier",
        Operation::Extrude { sketch: Profile::Feature { feature: corbel_s }, height_mm: 2.0 * CORBEL_HALF_MM, draft_deg: 0.0 },
        on_table(Attach::Join, 0.0, ComponentRole::Other),
    );
    let mut parts = vec![pier, spire, cap];
    parts.extend(crockets);
    parts.push(corbel);
    array(t, "Stand a pinnacle on every corner", parts, around, metal(Attach::Join));
    corbel
}

/// The shoulder's outer surface at the band's mid-plane, as radii at whole degrees from 118° to 160°, read off the bare stock.
fn shoulder() -> Result<Vec<(f64, f64)>> {
    let mut bare = base()?;
    bare.cad = None;
    let m = mesh::try_build(&bare, &AlphaLibrary::builtin(), draft_params())?.mesh;
    let bvh = ringdesign_core::interaction::bvh::Bvh::build(&m);
    (118..=160)
        .map(|th| {
            let t = (th as f64).to_radians();
            let hit = bvh.ray(&m, [30.0 * t.cos(), 30.0 * t.sin(), 0.0], [-t.cos(), -t.sin(), 0.0]).context("the shoulder ray missed")?;
            Ok((th as f64, ((30.0 - hit.1) * 100.0).round() / 100.0))
        })
        .collect()
}

fn surface_at(shoulder: &[(f64, f64)], th: f64) -> f64 {
    let i = shoulder.iter().position(|(t, _)| *t >= th).unwrap_or(shoulder.len() - 1).max(1);
    let ((t0, r0), (t1, r1)) = (shoulder[i - 1], shoulder[i]);
    r0 + (r1 - r0) * ((th - t0) / (t1 - t0)).clamp(0.0, 1.0)
}

/// The clasping buttress's elevation on the plane of the ring: its foot sunk along the shoulder, its face stepping down at two
/// weathered set-offs, from where it leans on the head to where it lands.
fn buttress_outline(shoulder: &[(f64, f64)]) -> Vec<P2> {
    let at = |th: f64, r: f64| -> P2 {
        let t = th.to_radians();
        [r * t.cos(), r * t.sin()]
    };
    let mut face = Vec::new();
    let mut from = BUTTRESS_FROM_DEG;
    for (k, (to, proud)) in BUTTRESS_STAGES.iter().enumerate() {
        let start = if k == 0 { from } else { from + SET_OFF_DEG };
        let n = ((to - start) / 1.0).ceil().max(1.0) as usize;
        for i in 0..=n {
            let th = start + (to - start) * i as f64 / n as f64;
            face.push(at(th, surface_at(shoulder, th) + proud));
        }
        from = *to;
    }
    // Its foot lands on the shoulder past the last stage.
    let end = from + SET_OFF_DEG;
    face.push(at(end, surface_at(shoulder, end) + 0.05));
    let n = ((end - BUTTRESS_FROM_DEG) / 1.0).ceil() as usize;
    let foot: Vec<P2> = (0..=n).rev().map(|i| {
        let th = BUTTRESS_FROM_DEG + (end - BUTTRESS_FROM_DEG) * i as f64 / n as f64;
        at(th, surface_at(shoulder, th) - SINK_MM - 0.3)
    }).collect();
    face.extend(foot);
    face
}

/// One window's net on the wall plane: the equilateral outline, the mullion up its middle and the fork's two branches up to
/// the head, each run a little past where it meets the outline so the split finds every junction.
fn window_net(s: &mut Sketch) -> Vec<Id> {
    let (w, h, sill, spring) = (WINDOW_W_MM, WINDOW_W_MM / 2.0, WINDOW_SILL_MM, WINDOW_SPRING_MM);
    // The outline: a sill, two jambs and an equilateral head, each arc turning about the far springer.
    let (bl, br) = (s.point([-h, sill]), s.point([h, sill]));
    let (sl, sr) = (s.point([-h, spring]), s.point([h, spring]));
    let apex = s.point([0.0, spring + w * 3f64.sqrt() / 2.0]);
    let (cl, cr) = (s.point([-h, spring]), s.point([h, spring]));
    let foot = s.point([0.0, sill]);
    let mut net = vec![
        s.entity(Geometry::Line { a: bl, b: foot }),
        s.entity(Geometry::Line { a: foot, b: br }),
        s.entity(Geometry::Line { a: br, b: sr }),
        s.entity(Geometry::Arc { center: cl, start: sr, end: apex }),
        s.entity(Geometry::Arc { center: cr, start: apex, end: sl }),
        s.entity(Geometry::Line { a: sl, b: bl }),
    ];
    // The mullion up the middle to the fork, and the fork's branches out to the head.
    let fork = s.point([0.0, spring + WINDOW_FORK_MM]);
    net.push(s.entity(Geometry::Line { a: foot, b: fork }));
    for side in [-1.0, 1.0] {
        let x = side * WINDOW_BRANCH_MM;
        let y = spring + (w * w - (x + side * h) * (x + side * h)).sqrt();
        let e = s.point([x, y]);
        net.push(s.entity(Geometry::Line { a: fork, b: e }));
    }
    net
}

fn author(seats: Option<&Seats>, shoulder: &[(f64, f64)]) -> Result<(RingDesign, Ids)> {
    let mut d = base()?;
    let mut t = Tree { doc: Document::default() };
    let mut ids = Ids::default();
    t.add("Take the factory octagon as the lantern's plinth", Operation::Band, Component::default());

    // The lantern, hollowed, its hollow run down through the head so its lights open onto the finger.
    let mut s = at_z("The lantern", -SINK_MM);
    octagon(&mut s, DRUM_APOTHEM_MM);
    let drum_s = t.sketch("Draw the lantern on the table, square to its sides", s);
    ids.drum = t.add(
        "Raise the lantern off the table",
        Operation::Extrude { sketch: Profile::Feature { feature: drum_s }, height_mm: DRUM_TOP_MM + SINK_MM, draft_deg: 0.0 },
        on_table(Attach::Join, BLEND_MM, ComponentRole::Head),
    );
    let mut s = at_z("The lantern's hollow", HOLLOW_FROM_MM);
    octagon(&mut s, DRUM_APOTHEM_MM - WALL_MM);
    let hollow_s = t.sketch("Draw the lantern's hollow inside its walls, under the head", s);
    t.add(
        "Hollow the lantern and open it down to the finger",
        Operation::Extrude { sketch: Profile::Feature { feature: hollow_s }, height_mm: DRUM_TOP_MM - WALL_MM - HOLLOW_FROM_MM, draft_deg: 0.0 },
        on_table(Attach::Cut, 0.0, ComponentRole::Other),
    );

    // The rim moulding round the roof.
    let mut s = at_z("The rim", DRUM_TOP_MM - 0.1);
    octagon(&mut s, DRUM_APOTHEM_MM);
    octagon(&mut s, DRUM_APOTHEM_MM - RIM_W_MM);
    let rim_s = t.sketch("Draw the rim moulding round the roof", s);
    t.add(
        "Raise the rim moulding",
        Operation::Extrude { sketch: Profile::Feature { feature: rim_s }, height_mm: RIM_PROUD_MM + 0.1, draft_deg: RIM_DRAFT_DEG },
        on_table(Attach::Join, 0.0, ComponentRole::Other),
    );

    // The boss.
    let mut s = at_z("The boss", DRUM_TOP_MM - SINK_MM);
    let c0 = s.point([0.0, 0.0]);
    let c1 = s.point([BOSS_R_MM, 0.0]);
    s.entity(Geometry::Circle { center: c0, rim: c1 });
    let boss_s = t.sketch("Draw the boss at the vault's crown", s);
    ids.boss = t.add(
        "Raise the boss",
        Operation::Extrude { sketch: Profile::Feature { feature: boss_s }, height_mm: BOSS_H_MM + SINK_MM, draft_deg: 0.0 },
        on_table(Attach::Join, 0.0, ComponentRole::Head),
    );

    // The vault: a gabled rib from the boss to the first corner, then all eight.
    let rc = DRUM_APOTHEM_MM / (PI / 8.0).cos();
    let mut s = at_z("A vault rib", DRUM_TOP_MM - 0.1);
    let (st, ct) = (PI / 8.0).sin_cos();
    let (from, to, h) = (BOSS_R_MM - 0.3, rc - RIM_W_MM, RIB_BAR_MM / 2.0);
    let pts: Vec<Id> = [[from, -h], [to, -h], [to, h], [from, h]].iter().map(|[x, y]| s.point([x * ct - y * st, x * st + y * ct])).collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    let rib_s = t.sketch("Draw a rib from the boss to the lantern's first corner", s);
    let rib = t.add(
        "Raise the rib, gabled",
        Operation::Extrude { sketch: Profile::Feature { feature: rib_s }, height_mm: RIB_PROUD_MM + 0.1, draft_deg: RIB_DRAFT_DEG },
        on_table(Attach::Join, 0.0, ComponentRole::Other),
    );
    array(&mut t, "Spring a rib to every corner", vec![rib], ids.drum, metal(Attach::Join));

    // The vault's first light, a lancet pierced through the roof between two ribs; then all eight.
    let mut s = at_z("A light of the vault", DRUM_TOP_MM + CUT_OVER_MM);
    let pts: Vec<Id> = light_outline().into_iter().map(|p| s.point(p)).collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    let light_s = t.sketch("Draw a lancet light between the first two ribs, pointing out to its wall", s);
    let light = t.add(
        "Pierce the light through the roof",
        Operation::Extrude { sketch: Profile::Feature { feature: light_s }, height_mm: -(WALL_MM + 2.0 * CUT_OVER_MM), draft_deg: 0.0 },
        on_table(Attach::Cut, 0.0, ComponentRole::Other),
    );
    array(&mut t, "Pierce a light in every bay of the vault", vec![light], ids.drum, metal(Attach::Cut));

    // The first window, traced from its net on the wall that faces along the finger; then one in every wall.
    let mut s = Sketch::default();
    s.name = "A two-light window".into();
    s.plane.origin = [DRUM_APOTHEM_MM + CUT_OVER_MM, 0.0, 0.0];
    s.plane.x = [0.0, 1.0, 0.0];
    s.plane.y = [0.0, 0.0, 1.0];
    let net = window_net(&mut s);
    let traced = s.tracery(&net, WINDOW_BAR_MM)?;
    ensure!(traced.skipped.is_empty() && traced.lights.len() == 3, "the window traced {} lights, skipped {:?}", traced.lights.len(), traced.skipped);
    let window_s = t.sketch("Draw a window's outline, mullion and fork on the wall that faces along the finger, and trace its lights a bar apart", s);
    let window = t.add(
        "Pierce the window's lights through the wall",
        Operation::Extrude { sketch: Profile::Feature { feature: window_s }, height_mm: -(WALL_MM + 2.0 * CUT_OVER_MM), draft_deg: 0.0 },
        on_table(Attach::Cut, 0.0, ComponentRole::Other),
    );
    array(&mut t, "Open a window in every wall", vec![window], ids.drum, metal(Attach::Cut));

    let corbel = pinnacles(&mut t, ids.drum);

    // The clasping buttress on the right shoulder, drawn on a plane just under the band's mid-plane; then mirrored.
    let parting = t.add("Lay a plane under the band's mid-plane", Operation::Plane { base: PlaneBase::Parting, offset_mm: -BUTTRESS_HALF_MM }, Component::default());
    let mut s = on_plane("The clasping buttress", parting);
    let pts: Vec<Id> = buttress_outline(shoulder).into_iter().map(|p| s.point(p)).collect();
    s.entity(Geometry::Polyline { points: pts, closed: true });
    let buttress_s = t.sketch("Draw the clasping buttress's elevation on the shoulder, with its two set-offs", s);
    let buttress = t.add(
        "Raise the buttress across the band",
        Operation::Extrude { sketch: Profile::Feature { feature: buttress_s }, height_mm: 2.0 * BUTTRESS_HALF_MM, draft_deg: 0.0 },
        Component { attach: Attach::Join, material: ALLOY.into(), ..Component::default() },
    );
    t.add(
        "Mirror the buttress onto the other shoulder",
        Operation::Pattern { sources: Sources(vec![buttress]), kind: PatternKind::Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } },
        metal(Attach::Join),
    );

    // The paired blind lancets on the head's long walls and on its chamfers.
    for (what, plane, pairs, w, bar, bearing, count) in [
        ("long wall", SIDE_PLANE_MM, &SIDE_PAIRS[..], SIDE_W_MM, SIDE_BAR_MM, 0.0f64, 2u32),
        ("chamfer", CHAMFER_PLANE_MM, &[0.0][..], CHAMFER_W_MM, CHAMFER_BAR_MM, PI / 4.0, 4u32),
    ] {
        let (sn, cs) = bearing.sin_cos();
        let mut s = Sketch::default();
        s.name = format!("Paired lancets on the head's {what}");
        s.plane.origin = [plane * cs, plane * sn, 0.0];
        s.plane.x = [-sn, cs, 0.0];
        s.plane.y = [0.0, 0.0, 1.0];
        for centre in pairs {
            for side in [-1.0, 1.0] {
                let x = centre + side * (w + bar) / 2.0;
                let pts: Vec<Id> = lancet(w, SIDE_SILL_MM, SIDE_SPRING_MM, 2.0).into_iter().map(|[u, v]| s.point([x + u, v])).collect();
                s.entity(Geometry::Polyline { points: pts, closed: true });
            }
        }
        let side_s = t.sketch(&format!("Draw paired lancets on the head's first {what}"), s);
        let side = t.add(
            &format!("Cut the blind lancets into the {what}"),
            Operation::Extrude { sketch: Profile::Feature { feature: side_s }, height_mm: -SIDE_CUT_MM, draft_deg: 0.0 },
            on_table(Attach::Cut, 0.0, ComponentRole::Other),
        );
        t.add(
            &format!("Cut them into every {what}"),
            Operation::Pattern { sources: Sources(vec![side]), kind: PatternKind::About { part: ids.drum, count, span_deg: 360.0 } },
            metal(Attach::Cut),
        );
    }

    if let Some(seats) = seats {
        // The first lamp, a citrine on the first corbel's sloping face; then one on every corbel.
        let lamp = t.next();
        let mut f = cad::stone_on_face(lamp, citrine(), corbel, &seats.lamp);
        f.name = "Set a citrine lamp on the first corbel".into();
        set_tint(&mut f, CITRINE);
        t.push(f);
        let collet = t.next();
        t.push(builders::feature_on(collet, "Its collet", builders::BEZEL, lamp, json!({ "wall_mm": LAMP_WALL_MM })));
        let bur = t.next();
        t.push(builders::feature_on(bur, "Its seat", builders::BUR, lamp, json!({ "through": false })));
        array(&mut t, "Light a lamp on every corbel", vec![lamp], ids.drum, glass());
        array(&mut t, "Ring every lamp in its collet", vec![collet], ids.drum, metal(Attach::Join));
        array(&mut t, "Cut every lamp's seat", vec![bur], ids.drum, metal(Attach::Cut));
        // The amethyst on the boss.
        let stone = t.next();
        let mut f = cad::stone_on_face(stone, amethyst(), ids.boss, &seats.boss);
        f.name = "Set the amethyst on the boss".into();
        set_tint(&mut f, AMETHYST);
        t.push(f);
        let collet = t.next();
        t.push(builders::feature_on(collet, "Its collet", builders::BEZEL, stone, json!({ "wall_mm": COLLET_WALL_MM, "lip": COLLET_LIP })));
        let bur = t.next();
        t.push(builders::feature_on(bur, "Its seat, open to the lantern below", builders::BUR, stone, json!({ "through": true })));
    }
    ids.corbel = corbel;

    // A debugging switch: LANTERNA_OFF names features to leave out, by any word of their names.
    if let Ok(off) = std::env::var("LANTERNA_OFF") {
        for f in &mut t.doc.features {
            if off.split(',').any(|w| !w.is_empty() && f.name.contains(w)) {
                f.enabled = false;
            }
        }
    }
    let mut s = setup();
    s.component = None;
    d.cad = Some(t.doc);
    d.manufacturing = Some(s);
    Ok((d, ids))
}

fn on_plane(name: &str, plane: Id) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    s.plane.on_face = Some(FaceAnchor { feature: plane, face: FaceRef::bare(0) });
    s
}

/// The faces the stones stand on, read off a build without them.
fn seats(lib: &AlphaLibrary, shoulder: &[(f64, f64)]) -> Result<Seats> {
    let (d, ids) = author(None, shoulder)?;
    let built = mesh::try_build(&d, lib, draft_params())?;
    let e = built.parts.evaluated.as_ref().context("no parts evaluated")?;
    let comp = |id: Id| e.components.iter().find(|c| c.id == id).with_context(|| format!("part #{id} did not build"));
    let world = |c: &EvaluatedComponent, p: P3| -> P3 { std::array::from_fn(|k| c.frame.origin[k] + c.frame.x_axis[k] * p[0] + c.frame.y_axis[k] * p[1] + c.frame.z_axis[k] * p[2]) };
    let facing = |c: &EvaluatedComponent, dir: P3, at: P3, height: f64| -> Result<FaceSeat> {
        for face in 0..64u32 {
            let Ok(seat) = FaceSeat::on(c, face, Some(at), height) else { continue };
            let Ok(frame) = seat.face_of(c) else { continue };
            let n = frame.normal;
            if n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2] > 0.999 {
                return Ok(seat);
            }
        }
        anyhow::bail!("no face of #{} {} faces {dir:?}", c.id, c.name)
    };
    let corbel = comp(ids.corbel)?;
    let (sb, cb) = (PI / 8.0).sin_cos();
    let f = corbel.frame;
    let out: P3 = std::array::from_fn(|k| (f.x_axis[k] * cb + f.y_axis[k] * sb + f.z_axis[k]) / 2f64.sqrt());
    let c = corner(DRUM_APOTHEM_MM, 0);
    let (s0, z0) = (0.5 * (PIER_W_MM / 2.0 + CORBEL[2][0]), 0.5 * (CORBEL[3][1] - (CORBEL[3][0] - PIER_W_MM / 2.0) + CORBEL[2][1]));
    let lamp = facing(corbel, out, world(corbel, [c[0] + cb * s0, c[1] + sb * s0, z0]), builders::stand_off_mm(builders::BEZEL, citrine()))?;
    let boss = comp(ids.boss)?;
    let boss_seat = facing(boss, boss.frame.z_axis, world(boss, [0.0, 0.0, DRUM_TOP_MM + BOSS_H_MM]), builders::stand_off_mm(builders::BEZEL, amethyst()))?;
    Ok(Seats { lamp, boss: boss_seat })
}


// --- Measures --------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: m.faces.clone() }
}

fn geometry(m: &mesh::Mesh) -> (bool, usize, usize) {
    (m.validate().watertight, m.quality().degenerate_faces, csg::self_crossings(&solid_of(m)))
}

/// Every CAD part's self-crossings as built.
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
            (format!("#{} {}", c.id, c.name), n)
        })
        .collect()
}

/// The nearest vertex to the finger's axis, and how many stand inside the bore.
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

/// The thickness screen's own samples, every one under the floor named by the part it lies on: what `thickness` counts, attributed.
fn thin_samples(built: &mesh::BuildResult, d: &RingDesign, limit: f64) -> Vec<(String, f64, P3)> {
    let m = &built.mesh;
    if m.faces.len() > 250_000 {
        return Vec::new();
    }
    let tri = |f: &[u32; 3]| f.map(|i| {
        let v = m.vertices[i as usize];
        [v.0 as f64, v.1 as f64, v.2 as f64]
    });
    let tris: Vec<[P3; 3]> = m.faces.iter().map(tri).collect();
    let stride = m.faces.len().div_ceil(384).max(1);
    let name = |f: &[u32; 3]| -> String {
        let o = m.origin.get(f[0] as usize).copied().unwrap_or(0);
        built.parts.feature_of(o).and_then(|id| d.cad.as_ref()?.feature(id)).map_or("the stock".to_string(), |f| format!("#{} {}", f.id, f.name))
    };
    let sub = |a: P3, b: P3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: P3, b: P3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut out = Vec::new();
    for (k, t) in tris.iter().enumerate().step_by(stride) {
        let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let l = dot(n, n).sqrt();
        if l < 1e-12 {
            continue;
        }
        let dir = n.map(|v| -v / l);
        let c: P3 = std::array::from_fn(|i| (t[0][i] + t[1][i] + t[2][i]) / 3.0);
        let mut best = f64::MAX;
        for (j, u) in tris.iter().enumerate() {
            if j == k {
                continue;
            }
            let (e1, e2) = (sub(u[1], u[0]), sub(u[2], u[0]));
            let h = cross(dir, e2);
            let det = dot(e1, h);
            if det.abs() < 1e-12 {
                continue;
            }
            let s0 = sub(c, u[0]);
            let a = dot(s0, h) / det;
            if !(-1e-8..=1.0 + 1e-8).contains(&a) {
                continue;
            }
            let q = cross(s0, e1);
            let b = dot(dir, q) / det;
            if b < -1e-8 || a + b > 1.0 + 1e-8 {
                continue;
            }
            let tt = dot(e2, q) / det;
            if tt > 1e-5 && tt < best {
                best = tt;
            }
        }
        if best < limit {
            out.push((name(&m.faces[k]), best, c));
        }
    }
    let _ = d;
    out
}

/// The lands the plan's numbers leave, mm, against the 0.8 mm section.
fn lands() -> serde_json::Value {
    let side = 2.0 * DRUM_APOTHEM_MM * (PI / 8.0).tan();
    json!({
        "lantern_wall_mm": WALL_MM,
        "lantern_roof_mm": WALL_MM,
        "lantern_face_mm": side,
        "window_outline_to_wall_edge_mm": side / 2.0 - WINDOW_W_MM / 2.0,
        "window_apex_to_roof_mm": DRUM_TOP_MM - window_apex(),
        "window_tracery_bar_mm": WINDOW_BAR_MM,
        "light_width_mm": LIGHT_W_MM,
        "lantern_pier_mm": PIER_W_MM,
        "spirelet_least_mm": MIN_SECTION_MM + 0.02,
        "crocket_mm": 2.0 * CROCKET_R_MM,
        "rib_foot_mm": RIB_BAR_MM,
        "rim_mm": RIM_W_MM,
        "amethyst_collet_wall_mm": COLLET_WALL_MM,
        "lamp_collet_wall_mm": LAMP_WALL_MM,
        "buttress_mm": 2.0 * BUTTRESS_HALF_MM,
        "side_lancet_bar_mm": SIDE_BAR_MM,
        "chamfer_lancet_bar_mm": CHAMFER_BAR_MM,
        "side_lancet_head_to_table_mm": -(SIDE_SPRING_MM + SIDE_W_MM * 3f64.sqrt() / 2.0),
    })
}

// --- Renders ---------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.5, 0.42),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", PI * 0.5, 0.12),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn side_by_side(path: &Path, images: &[Vec<u8>], edge: usize) -> Result<()> {
    let n = images.len();
    let mut out = vec![0u8; edge * n * edge * 3];
    for (k, img) in images.iter().enumerate() {
        for y in 0..edge {
            let dst = y * edge * n * 3 + k * edge * 3;
            out[dst..dst + edge * 3].copy_from_slice(&img[y * edge * 3..(y + 1) * edge * 3]);
        }
    }
    image::save_buffer(path, &out, (edge * n) as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

fn png(path: &Path, img: &[u8], edge: usize) -> Result<()> {
    image::save_buffer(path, img, edge as u32, edge as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The faces of `m` within `radius` of `centre`.
fn crop(m: &mesh::Mesh, centre: P3, radius: f64) -> mesh::Mesh {
    let mut index = std::collections::HashMap::new();
    let mut out = mesh::Mesh::default();
    let near = |i: u32| {
        let v = m.vertices[i as usize];
        let d = [v.0 as f64 - centre[0], v.1 as f64 - centre[1], v.2 as f64 - centre[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() < radius
    };
    for f in &m.faces {
        if !f.iter().all(|&i| near(i)) {
            continue;
        }
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

fn renders(out: &Path, fin: &render::Finished, bare: &mesh::Mesh, edge: usize) -> Result<()> {
    let parts = fin.parts(render::GOLD);
    let shot = |parts: &[render::Part], yaw: f64, pitch: f64, edge: usize| render::render_parts_ss(parts, yaw, pitch, edge, edge, 3);
    if let Ok(list) = std::env::var("LANTERNA_TRY") {
        for (k, v) in list.split(';').enumerate() {
            let n: Vec<f64> = v.split(',').filter_map(|x| x.parse().ok()).collect();
            png(&out.join(format!("try-{k}.png")), &shot(&parts, n[0], n[1], 600), 600)?;
        }
        return Ok(());
    }
    for (name, yaw, pitch) in VIEWS {
        png(&out.join(format!("{name}.png")), &shot(&parts, yaw, pitch, edge), edge)?;
    }
    // The close-up frames on the lantern alone.
    let top = crop(&fin.metal, [0.0, 16.0, 0.0], 9.5);
    let mut close = vec![render::Part::metal(&top, render::GOLD)];
    close.extend(fin.stones.iter().map(|(m, t)| render::Part::tinted_stone(m, *t)));
    png(&out.join("stones.png"), &shot(&close, 0.35, 0.7, edge), edge)?;
    let bare_parts = [render::Part::metal(bare, render::GOLD)];
    side_by_side(&out.join("bare-vs-finished.png"), &[shot(&bare_parts, VIEWS[0].1, VIEWS[0].2, edge), shot(&parts, VIEWS[0].1, VIEWS[0].2, edge)], edge)?;
    png(&out.join("hero-300.png"), &shot(&parts, VIEWS[0].1, VIEWS[0].2, 300), 300)?;
    png(&out.join("face-300.png"), &shot(&parts, VIEWS[1].1, VIEWS[1].2, 300), 300)?;
    let sheet: Vec<Vec<u8>> = [0usize, 1, 3, 4, 2].iter().map(|&k| shot(&parts, VIEWS[k].1, VIEWS[k].2, 300)).collect();
    side_by_side(&out.join("contact-300.png"), &sheet, 300)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/lanterna"));
    std::fs::create_dir_all(&out)?;
    println!("Lanterna");
    if std::env::var("LANTERNA_PROBE").is_ok() {
        let mut bare = base()?;
        bare.cad = None;
        let m = mesh::try_build(&bare, &AlphaLibrary::builtin(), draft_params())?.mesh;
        let bvh = ringdesign_core::interaction::bvh::Bvh::build(&m);
        for th in (100..=170).step_by(3) {
            let t = (th as f64).to_radians();
            for z in [0.0, 2.0, 4.0] {
                let o = [30.0 * t.cos(), 30.0 * t.sin(), z];
                let hit = bvh.ray(&m, o, [-t.cos(), -t.sin(), 0.0]).map(|(_, d)| 30.0 - d);
                print!("  theta {th} z {z}: r {:.2};", hit.unwrap_or(-1.0));
            }
            println!();
        }
        for yy in [9.0, 9.5, 10.0, 10.5, 11.0, 11.5, 12.0, 12.5, 13.0, 13.2] {
            for xx in [-6.0, 0.0, 6.0] {
                let hit = bvh.ray(&m, [xx, yy, 30.0], [0.0, 0.0, -1.0]).map(|(_, d)| 30.0 - d);
                print!("  y {yy} x {xx}: side wall z {:.2};", hit.unwrap_or(-1.0));
            }
            println!();
        }
        return Ok(());
    }
    let started = std::time::Instant::now();
    let lib = AlphaLibrary::builtin();
    let shoulder = shoulder()?;
    let seats = seats(&lib, &shoulder)?;
    let (d, _ids) = author(Some(&seats), &shoulder)?;
    let params = if draft { draft_params() } else { export_params() };
    let built = mesh::try_build(&d, &lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let (watertight, degenerate, crossings) = geometry(&built.mesh);
    println!(
        "  {} triangles in {build_s:.1} s; watertight {watertight}; degenerate {degenerate}; self-crossings {crossings}; notes {:?} {:?}",
        built.mesh.faces.len(),
        built.solids.notes,
        built.parts.notes
    );
    let made = made_parts(&built);
    for (name, n) in &made {
        if *n > 0 {
            println!("  part {name}: {n} self-crossings");
        }
    }
    let (least_r, inside) = bore_intrusion(&d, &built.mesh);
    let statuses: Vec<(String, String)> = built
        .parts
        .evaluated
        .iter()
        .flat_map(|e| e.features.iter())
        .map(|r| (format!("#{} {}", r.id, r.name), format!("{:?}", r.status)))
        .collect();
    let features_ok = statuses.iter().all(|(_, s)| s == "Ok");
    for (f, s) in &statuses {
        if s != "Ok" {
            println!("  feature {f}: {s}");
        }
    }
    let thick = ringdesign_core::cad::measure::thickness(&built.mesh, MIN_SECTION_MM);
    let thin = thin_samples(&built, &d, MIN_SECTION_MM);
    let mut by_part: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
    for (name, v, _) in &thin {
        let e = by_part.entry(name.clone()).or_insert((0, f64::MAX));
        e.0 += 1;
        e.1 = e.1.min(*v);
    }
    for (name, (n, v)) in &by_part {
        println!("  thin: {n} samples on {name}, least {v:.3} mm");
    }
    if std::env::var("LANTERNA_THIN").is_ok() {
        for (name, v, p) in &thin {
            println!("    {name}: {v:.3} at [{:.2}, {:.2}, {:.2}]", p[0], p[1], p[2]);
        }
    }
    let cut_lands = dfm::cut_lands(&d, &built, MIN_SECTION_MM);
    let findings = dfm::findings_in(&d, &lib);
    let fin = render::Finished { metal: built.mesh.clone(), stones: ringdesign_core::gems::built_meshes(&d, &lib, &built) };
    let stones = ringdesign_core::stones::report_built(&d, 0.0, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let previewed = ringdesign_core::stones::all_stone_frames(&d).len();
    let prepared = mesh::try_build_pattern(&d, &lib, params)?;
    let (pw, pd, px) = geometry(&prepared.mesh);
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
    let thick_ok = thick.below_limit == 0 && thick.rays > 0;
    let gates = [
        ("finished mesh watertight, 0 degenerate faces, 0 self-crossings", watertight && degenerate == 0 && crossings == 0),
        ("every CAD part closed without crossings, every feature Ok", made.iter().all(|(_, n)| *n == 0) && features_ok),
        ("solids and parts notes empty", built.solids.notes.is_empty() && built.parts.notes.is_empty()),
        ("nothing enters the finger hole", inside == 0),
        ("thickness at the 0.8 mm section clean", thick_ok),
        ("CAD cut lands at 0.8 mm clean", cut_lands.is_empty()),
        ("zero DFM findings", findings.is_empty()),
        ("stones reported equal the preview", reported == previewed && reported == 9),
        ("casting pattern watertight, 0 degenerates, 0 crossings", pw && pd == 0 && px == 0),
        ("export build within the 2 million triangle budget", built.mesh.faces.len() <= 2_000_000),
        ("cold reload identical", cold != Some(false)),
    ];
    let features: Vec<serde_json::Value> = d.cad.as_ref().unwrap().features.iter().map(|f| json!({"id": f.id, "name": f.name, "operation": f.operation.label()})).collect();
    let grams = built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams);
    let mut report = json!({
        "name": d.name,
        "process": d.draft.process.label(),
        "alloy": ALLOY,
        "stock": STOCK,
        "size": d.size.display(),
        "bore_mm": built.report.inner_diameter_mm,
        "build": {"theta_steps": params.theta_steps, "profile_steps": params.profile_steps, "triangles": built.mesh.faces.len(), "build_s": build_s},
        "geometry": {"watertight": watertight, "boundary_edges": built.report.validation.boundary_edges, "non_manifold_edges": built.report.validation.non_manifold_edges, "degenerate_faces": degenerate, "self_crossings": crossings, "volume_mm3": built.report.volume_mm3, "bounds_mm": built.report.bounds_mm},
        "made_parts": made,
        "feature_status": statuses,
        "solids": {"notes": built.solids.notes, "parts_notes": built.parts.notes},
        "bore": {"radius_mm": d.inner_radius_mm(), "nearest_vertex_mm": least_r, "vertices_inside": inside},
        "thin_samples_by_part": by_part.iter().map(|(k, (n, v))| json!({"part": k, "samples": n, "least_mm": v})).collect::<Vec<_>>(),
        "thickness": {"limit_mm": thick.limit_mm, "sampled_min_mm": thick.sampled_min_mm, "rays": thick.rays, "unresolved": thick.unresolved, "below_limit": thick.below_limit, "note": thick.note},
        "cut_lands": cut_lands.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "land_widths": lands(),
        "dfm_findings": findings.iter().map(|f| format!("{}: {}", f.label, f.message)).collect::<Vec<_>>(),
        "stones": {"reported": reported, "previewed": previewed, "carats": stones.as_ref().map(|s| s.total_carats), "tight_pairs": stones.as_ref().map(|s| s.tight_pairs)},
        "grams_gold_18k": grams,
        "pattern": {"watertight": pw, "degenerate_faces": pd, "self_crossings": px, "triangles": prepared.mesh.faces.len()},
        "design": {"bytes": text.len(), "cad_features": features.len()},
        "features": features,
        "cold_reload_identical": cold,
        "gates": gates.iter().map(|(g, pass)| json!({"gate": g, "pass": pass})).collect::<Vec<_>>(),
        "gates_passed": gates.iter().all(|(_, p)| *p),
    });
    let report_path = out.join("report.json");
    if draft {
        report["draft"] = json!({"process": d.draft.process.label(), "min_draft_deg": d.draft.min_draft_deg, "min_section_mm": d.draft.min_section_mm, "min_detail_mm": d.draft.min_detail_mm, "build": report.clone()});
    } else if let Ok(old) = std::fs::read(&report_path).map_err(anyhow::Error::from).and_then(|b| Ok(serde_json::from_slice::<serde_json::Value>(&b)?)) {
        report["draft"] = old.get("draft").cloned().unwrap_or(serde_json::Value::Null);
    }
    std::fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        stl::write_stl(out.join("casting-pattern.stl"), &prepared.mesh, "Lanterna / wax pattern")?;
    }
    let mut bare = base()?;
    bare.cad = None;
    let bare_mesh = mesh::try_build(&bare, &lib, draft_params())?.mesh;
    renders(&out, &fin, &bare_mesh, if draft { 900 } else { 1600 })?;
    println!(
        "  thickness min {:?} below {} ; cut lands {} ; dfm {} ; stones {reported}/{previewed} ; {:.2} g gold",
        thick.sampled_min_mm, thick.below_limit, cut_lands.len(), findings.len(), grams
    );
    for f in findings.iter().chain(cut_lands.iter()) {
        println!("    dfm: {}: {}", f.label, f.message);
    }
    for (g, pass) in &gates {
        println!("  {} {g}", if *pass { "pass" } else { "FAIL" });
    }
    ensure!(gates.iter().all(|(_, p)| *p), "Lanterna failed a gate; see {}", report_path.display());
    Ok(())
}
