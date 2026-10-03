//! Tenebrae — Porta, the portal: a cathedral's west portal cut into factory 009 Drop, poured in lost wax.
//! Four archivolts step down into the head, a ruby sits in the tympanum over the twin doors, and an ogee
//! hood rises to a fleur-de-lis at the drop's point.
//! cargo build --release -p ringdesign-core --example tenebrae_porta
//! target/release/examples/tenebrae_porta [OUT_DIR] [--draft] [--verify] [--blockout]
use anyhow::{Context, Result, ensure};
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign,
    cad::{Attach, Component, ComponentRole, Document, Feature, Operation, Placement, Profile, Stage, builders},
    castability::{self, CastProcess},
    csg, dfm,
    gem::{Gem, GemCut},
    imported_base::{ImportedBase, PRESETS, SurfaceChart},
    library, manufacturing as mf, mesh, render,
    sketch::{Geometry, Id, Sketch},
    stl,
};
use serde_json::{Value, json};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

type P2 = [f64; 2];

/// The face asked for: length round the ring by width across it (the drop's long axis runs along the finger).
const FACE: (f64, f64) = (14.5, 19.0);
const BORE_MM: f64 = 18.6;
const ALLOY: &str = "Gold 18k";
/// Investment section and detail for the lost-wax pour.
const MIN_SECTION_MM: f64 = 0.8;

/// The portal's θ on the ring, and the turn that lays the part frame's x toward the drop's point (world +z).
const CROWN_DEG: f64 = 90.0;
const SPIN_DEG: f64 = 180.0;

// --- The portal, in the table's frame: u along the finger toward the drop's point, w round the ring. ---
/// Outer span of the arch, and its head's radius: a drop arch, its centres within the span on the opposite sides,
/// blunter than equilateral so the opening keeps room for the tympanum's ruby over doors of a proper height.
const ARCH_W: f64 = 10.4;
const ARCH_R: f64 = ARCH_W * 0.75;
/// The springing line and the outer threshold, u.
const SPRING_U: f64 = -0.95;
const THRESHOLD_U: f64 = -5.0;
/// Archivolt orders: each steps in by `STEP_W` and down by `STEP_DEPTH`; the sill steps up `SILL_STEP` per order.
const ORDERS: usize = 4;
const STEP_W: f64 = 0.7;
const STEP_DEPTH: f64 = 0.45;
const SILL_STEP: f64 = 0.1;
/// The roll in each order's foot: its radius, and its centre this many radii out from the riser (so its back is buried).
const ROLL_R: f64 = 0.3;
const ROLL_IN: f64 = 0.5;
/// Every round moulding's centre stands this far under the surface it rises from, so less than half of it shows and
/// every ray in from its face meets buried metal past its centre.
const BED: f64 = 0.06;
/// The doorway: the trumeau between the leaves and its column, the jambs either side, the sill under the leaves,
/// and the lintel bar over them.
const TRUMEAU_W: f64 = 0.85;
const TRUMEAU_R: f64 = TRUMEAU_W / 2.0;
const DOOR_JAMB: f64 = 0.35;
/// Each leaf's width at most: a lancet, taller than it is wide.
const DOOR_LEAF_W: f64 = 1.45;
const SILL_H: f64 = 0.12;
const LINTEL_R: f64 = 0.42;
/// The ruby: a 3.0 mm round, the clear margin kept round it in the tympanum, and how far its girdle stands over the floor.
const RUBY_MM: f64 = 3.0;
const COLLET_WALL_MM: f64 = 0.3;
const GIRDLE_OVER_FLOOR: f64 = -0.12;
const RUBY_TINT: [f32; 3] = [0.45, 0.01, 0.04];
/// The ruby's light under its culet: a pilot this radius, straight to the finger.
const PILOT_R: f64 = 1.15;
/// The ogee hood: its centreline clears the outer arch by this, its roll's width, and the ogee point.
const HOOD_GAP: f64 = 1.0;
const HOOD_W: f64 = 0.92;
const HOOD_RISE: f64 = 0.7;
/// The hood's label stops: bosses this radius, their centres this far over the table.
const LABEL_STOP_R: f64 = 0.62;
const LABEL_STOP_LIFT: f64 = 0.1;
const OGEE_U: f64 = 7.7;
/// The hood's sides meet the point this far off the axis, degrees.
const OGEE_TIP_DEG: f64 = 28.0;
/// Crockets up each side of the hood, as fractions of its length from the springer: each a tapering leaf of this
/// radius at its root, curling round a circle of `CROCKET_CURL`.
const CROCKETS: [f64; 3] = [0.5, 0.68, 0.84];
const CROCKET_R: f64 = 0.3;
const CROCKET_CURL: f64 = 0.42;
/// The fleur-de-lis finial: three tapering petals and a band, standing on the ogee's point.
const FLEUR_BASE: f64 = 0.15;
const FLEUR_PETAL_R: f64 = 0.42;
const FLEUR_SIDE_R: f64 = 0.34;
const FLEUR_BAND_R: f64 = 0.28;
const FLEUR_CURL: f64 = 0.6;
/// The crypt trefoil at the round end.
const CRYPT_U: f64 = -7.1;
const CRYPT_MM: f64 = 2.4;
/// The palm's comfort bevel: from θ and through how many degrees, and its line z = r − this.
const PALM_BEVEL_DEG: (f64, f64) = (215.0, 110.0);
const PALM_BEVEL_OFF: f64 = 7.05;
/// The shoulder lancets: θ off the crown and length, each side.
const LANCETS: [(f64, f64); 4] = [(42.0, 2.8), (52.0, 2.45), (62.0, 2.1), (72.0, 1.8)];

fn draft_params() -> BuildParams {
    BuildParams { theta_steps: 768, profile_steps: 320, refine: None, ..BuildParams::default() }
}
fn coarse_params() -> BuildParams {
    BuildParams { theta_steps: 384, profile_steps: 192, refine: None, ..BuildParams::default() }
}
fn export_params() -> BuildParams {
    BuildParams { theta_steps: 1536, profile_steps: 448, refine: None, ..BuildParams::default() }
}

/// Lost wax in 18k: investment, no draft, 0.8 mm section.
fn setup() -> mf::Setup {
    let mut s = mf::Setup::default();
    s.recipe = mf::Recipe::sand(castability::SandProcess::DelftClay);
    s.recipe.name = "Porta / investment / Gold 18k".into();
    s.recipe.alloy = ALLOY.into();
    s.recipe.shrink_pct = ringdesign_core::metal::find(ALLOY).map_or(s.recipe.shrink_pct, |m| m.shrink_pct);
    s.recipe.process = CastProcess::LostWax;
    s.recipe.sand = None;
    s.recipe.min_draft_deg = 0.0;
    s.recipe.min_detail_mm = 0.15;
    s.recipe.min_section_mm = MIN_SECTION_MM;
    s.sample_pitch_mm = 0.1;
    s.auto_parting = false;
    s.parting_mm = 0.0;
    s.flask.width_mm = 80.0;
    s.flask.length_mm = 80.0;
    s.channels = vec![
        mf::Channel { kind: mf::ChannelKind::Gate, start: [0.0, -10.5, 0.0], end: [0.0, -21.0, 0.0], diameter_mm: 4.0 },
        mf::Channel { kind: mf::ChannelKind::Sprue, start: [0.0, -21.0, 0.0], end: [0.0, -30.0, 0.0], diameter_mm: 5.0 },
    ];
    s.bench_notes = "Lost wax, sprued at the palm. After casting: open the ruby's seat with the bur, set the stone in its collet \
        and burnish the lip; polish the table, the hood and the fleur bright; leave the archivolt floors and the doors satin."
        .into();
    s
}

/// Factory 009 Drop, unmirrored, resized to 14.5 x 19 on an 18.6 mm bore.
fn stock() -> Result<RingDesign> {
    let preset = PRESETS.iter().find(|p| p.id == "009").context("no stock 009")?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, preset.load()?)?;
    d.name = "Porta \u{2014} the portal".into();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = FACE.1;
    d.shank.head.length_mm = FACE.0;
    d.size = ringdesign_core::resize::size_from_bore(BORE_MM).context("bore")?;
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    d.build = export_params();
    CastProcess::LostWax.apply(&mut d.draft);
    d.draft.min_section_mm = MIN_SECTION_MM;
    d.draft.auto_parting = false;
    d.draft.parting_z_mm = 0.0;
    d.manufacturing = Some(setup());
    Ok(d)
}

// --- Portal geometry ------------------------------------------------------------------------------------------------

/// The springing line of the outer arch.
fn springing_u() -> f64 {
    SPRING_U
}

/// How far each head arc's centre stands past the axis, on the far side from its own jamb.
fn centre_off() -> f64 {
    ARCH_R - ARCH_W / 2.0
}

/// The point at `phi` up the right head arc of radius `r`, from its springer (`phi` 0) toward the apex.
fn arc_pt(r: f64, phi: f64) -> P2 {
    [springing_u() + r * phi.sin(), -centre_off() + r * phi.cos()]
}

/// The turn up a right head arc of radius `r` at which it meets the axis: the apex.
fn apex_phi(r: f64) -> f64 {
    (centre_off() / r).acos()
}

/// The arch loop set in by `inset` on every side: threshold, jambs, and the two head arcs about the outer springers.
fn arch_loop(inset: f64) -> Vec<P2> {
    let r = ARCH_R - inset;
    let ut = sill_u(inset);
    let half = ARCH_W / 2.0 - inset;
    let phi_top = apex_phi(r);
    let n = ((r * phi_top) / 0.08).ceil() as usize;
    let mut pts = vec![[ut, -half], [ut, half]];
    // Right jamb up to the springing, then the right head arc (its centre past the axis) to the apex.
    for i in 0..=n {
        pts.push(arc_pt(r, phi_top * i as f64 / n as f64));
    }
    // The left head arc back down, skipping the shared apex.
    for i in (0..n).rev() {
        let p = arc_pt(r, phi_top * i as f64 / n as f64);
        pts.push([p[0], -p[1]]);
    }
    pts
}

/// The sill of the order set in by `inset`: each order steps up `SILL_STEP` from the threshold.
fn sill_u(inset: f64) -> f64 {
    THRESHOLD_U + inset / STEP_W * SILL_STEP
}

/// Half the opening of the arch set in by `inset`, at height `u`.
fn half_width(inset: f64, u: f64) -> f64 {
    let us = springing_u();
    if u <= us {
        return ARCH_W / 2.0 - inset;
    }
    let r = ARCH_R - inset;
    let dy = u - us;
    if dy >= r {
        return 0.0;
    }
    ((r * r - dy * dy).sqrt() - centre_off()).max(0.0)
}

/// The tympanum's floor depth (the innermost opening), the doors' depth, and the lintel's top and bottom.
struct Inner {
    floor: f64,
    doors: f64,
    lintel_lo: f64,
    lintel_hi: f64,
    ruby_u: f64,
}

fn inner() -> Inner {
    let inset = ORDERS as f64 * STEP_W;
    let floor = (ORDERS + 1) as f64 * STEP_DEPTH;
    let ut = sill_u(inset);
    let collet_r = RUBY_MM / 2.0 + COLLET_WALL_MM;
    // The ruby stands as high in the tympanum as its collet clears the opening's walls by 0.1 mm; the lintel bar sits under it.
    let clearance = |u: f64| {
        (0..=72)
            .map(|j| {
                let a = 2.0 * PI * j as f64 / 72.0;
                half_width(inset, u + collet_r * a.sin()) - (collet_r * a.cos()).abs()
            })
            .fold(f64::MAX, f64::min)
    };
    let apex = arc_pt(ARCH_R - inset, apex_phi(ARCH_R - inset))[0];
    let mut ruby_u = apex - collet_r;
    while clearance(ruby_u) < 0.1 && ruby_u > ut {
        ruby_u -= 0.01;
    }
    let lintel_u = ruby_u - collet_r - 0.05 - LINTEL_R;
    Inner { floor, doors: floor, lintel_lo: lintel_u - LINTEL_R, lintel_hi: lintel_u, ruby_u }
}

/// A cubic Bézier sampled at `n` + 1 points.
fn bez(p: [P2; 4], n: usize) -> Vec<P2> {
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let s = 1.0 - t;
            let (a, b, c, d) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
            [a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0], a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1]]
        })
        .collect()
}

/// The hood's right centreline for a turn at `phi_end` round the arch and Bézier arms `a`, `b` (fractions of the rise left).
fn hood_path(phi_end: f64, a: f64, b: f64) -> Vec<P2> {
    let r = ARCH_R + HOOD_GAP;
    // Convex: the arch's own curve offset out by the gap, from the springing to `phi_end` of its head.
    let n = 30;
    let mut pts: Vec<P2> = (0..=n).map(|i| arc_pt(r, phi_end * i as f64 / n as f64)).collect();
    // Then an S-curve into the ogee point, leaving tangent to the arc and arriving at `OGEE_TIP_DEG` off the axis,
    // so the two sides meet in a mitred point.
    let p0 = *pts.last().unwrap();
    let tan = [phi_end.cos(), -phi_end.sin()];
    let reach = (OGEE_U - p0[0]).max(0.5);
    let p1 = [p0[0] + tan[0] * reach * a, p0[1] + tan[1] * reach * a];
    let p3 = [OGEE_U, 0.0];
    let tip = OGEE_TIP_DEG.to_radians();
    let p2 = [OGEE_U - reach * b * tip.cos(), reach * b * tip.sin()];
    pts.extend(bez([p0, p1, p2, p3], 40).into_iter().skip(1));
    pts
}

/// How far the hood's strip stands clear of the outer arch, mm.
fn hood_clearance(c: &[P2]) -> f64 {
    let arch = arch_loop(0.0);
    c.iter().map(|p| arch.iter().map(|q| (p[0] - q[0]).hypot(p[1] - q[1])).fold(f64::MAX, f64::min)).fold(f64::MAX, f64::min) - HOOD_W / 2.0
}

/// The hood's right centreline: up from the springer, round the arch, reversing into the ogee point on the axis,
/// its turn and arms chosen to keep the strip clearest of the arch.
fn hood_centre() -> Vec<P2> {
    let mut best = (f64::MIN, (0.5, 0.45, 0.55));
    for i in 0..=12 {
        for j in 0..=6 {
            for k in 0..=6 {
                let (phi, a, b) = (0.25 + 0.05 * i as f64, 0.2 + 0.1 * j as f64, 0.2 + 0.1 * k as f64);
                let c = hood_path(phi, a, b);
                // The hood climbs all the way to its point.
                if c.windows(2).any(|w| w[1][0] < w[0][0] - 1e-9) {
                    continue;
                }
                let clear = hood_clearance(&c);
                if clear > best.0 + 1e-6 {
                    best = (clear, (phi, a, b));
                }
            }
        }
    }
    let (phi, a, b) = best.1;
    hood_path(phi, a, b)
}

fn unit(v: P2) -> P2 {
    let l = v[0].hypot(v[1]).max(1e-12);
    [v[0] / l, v[1] / l]
}

/// A foil of `lobes` overlapping round lobes about its centre, the first toward +u (up, toward the portal), as one outline.
fn foil(centre: P2, across: f64, lobes: usize) -> Vec<P2> {
    let r_lobe = across * if lobes == 4 { 0.25 } else { 0.29 };
    let r_c = across / 2.0 - r_lobe;
    let mut pts = Vec::new();
    let n = 180;
    for i in 0..n {
        let a = 2.0 * PI * i as f64 / n as f64;
        // The radius of the union of three circles along direction a, by sampling.
        let mut best: f64 = 0.0;
        for k in 0..lobes {
            let b = 2.0 * PI * k as f64 / lobes as f64;
            let (cx, cy) = (r_c * b.cos(), r_c * b.sin());
            // Ray from the centre: |t·d − c|² = r²; take the far root.
            let (dx, dy) = (a.cos(), a.sin());
            let dot = dx * cx + dy * cy;
            let disc = dot * dot - (cx * cx + cy * cy - r_lobe * r_lobe);
            if disc >= 0.0 {
                best = best.max(dot + disc.sqrt());
            }
        }
        pts.push([centre[0] + best * a.cos(), centre[1] + best * a.sin()]);
    }
    pts
}

/// A lancet `w` wide from `from` to `to` (x across, y up): straight jambs, a flat sill and an equilateral pointed head.
fn lancet(w: f64, from: f64, to: f64) -> Vec<P2> {
    let h = w / 2.0;
    let spring = to - w * 3f64.sqrt() / 2.0;
    let steps = 12;
    let mut pts = vec![[-h, from], [h, from], [h, spring]];
    // The right half of the head turns about the left springer, the left half about the right one.
    for i in 1..steps {
        let a = PI / 3.0 * i as f64 / steps as f64;
        pts.push([-h + w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([0.0, to]);
    for i in 1..steps {
        let a = PI / 3.0 * (steps - i) as f64 / steps as f64;
        pts.push([h - w * a.cos(), spring + w * a.sin()]);
    }
    pts.push([-h, spring]);
    pts
}

fn sketch_of(name: &str, loops: &[Vec<P2>]) -> Sketch {
    let mut s = Sketch::default();
    s.name = name.into();
    for l in loops {
        let ids: Vec<Id> = l.iter().map(|p| s.point(*p)).collect();
        s.entity(Geometry::Polyline { points: ids, closed: true });
    }
    s
}

// --- The feature tree -----------------------------------------------------------------------------------------------

struct Tree {
    doc: Document,
}

impl Tree {
    fn add(&mut self, name: &str, operation: Operation, component: Component) -> Result<Id> {
        let id = self.doc.features.len() as Id + 1;
        self.doc.append(Feature { id, name: name.into(), enabled: true, operation, component })?;
        Ok(id)
    }
    fn push(&mut self, mut f: Feature) -> Result<Id> {
        f.id = self.doc.features.len() as Id + 1;
        let id = f.id;
        f.component.material = ALLOY.into();
        self.doc.append(f)?;
        Ok(id)
    }
}

fn seat(height: f64) -> Placement {
    Placement::Ring { theta_deg: CROWN_DEG, across_mm: 0.0, height_mm: height, spin_deg: SPIN_DEG, tilt_deg: 0.0, cant_deg: 0.0 }
}

fn part(attach: Attach, placement: Placement, blend: f64) -> Component {
    Component { role: ComponentRole::Other, material: ALLOY.into(), placement, attach, stage: Stage::Cast, blend_mm: blend, ..Component::default() }
}

/// Sink `loops` from `lift` over the table down to `to` under it: a Cut part on the table's frame. Nested cuts take
/// different lifts, so no two entry faces lie in one plane.
fn cut(t: &mut Tree, name: &str, loops: &[Vec<P2>], lift: f64, to: f64) -> Result<Id> {
    t.add(
        name,
        Operation::Extrude { sketch: Profile::Inline(sketch_of(name, loops)), height_mm: -(to + lift), draft_deg: 0.0 },
        part(Attach::Cut, seat(lift), 0.0),
    )
}

/// A round moulding of radius `r` swept along `path` (the table's frame, u and w) at height `z`: a twisted sweep
/// with no twist, so it is a mesh, mitred at every joint. Its section stands square to the path where it starts.
fn tube(t: &mut Tree, name: &str, path: &[P2], z: f64, r: f64, attach: Attach) -> Result<Id> {
    oval_tube(t, name, path, z, r, r, attach)
}

/// [`tube`] with a section `rx` across the path and `rz` up out of the table: a half-ellipse where they differ.
fn oval_tube(t: &mut Tree, name: &str, path: &[P2], z: f64, r: f64, rz: f64, attach: Attach) -> Result<Id> {
    ensure!(path.len() >= 2, "{name}: a path needs two points");
    let path = &eased(path, r, 0.12);
    let (p0, p1) = (path[0], path[1]);
    let d = unit([p1[0] - p0[0], p1[1] - p0[1]]);
    // Across the path in the table's plane, and up out of it: their cross is along the path.
    let across = [-d[1], d[0], 0.0];
    let mut section = Sketch::default();
    section.name = format!("{name}: section");
    section.plane = ringdesign_core::sketch::Workplane { origin: [p0[0], p0[1], z], x: across, y: [0.0, 0.0, 1.0], on_face: None };
    let ring: Vec<Id> = (0..24).map(|i| 2.0 * PI * i as f64 / 24.0).map(|a| section.point([r * a.cos(), rz * a.sin()])).collect();
    section.entity(Geometry::Polyline { points: ring, closed: true });
    let mut line = Sketch::default();
    line.name = format!("{name}: path");
    line.plane = ringdesign_core::sketch::Workplane { origin: [0.0, 0.0, z], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], on_face: None };
    let ids: Vec<Id> = path.iter().map(|p| line.point(*p)).collect();
    line.entity(Geometry::Polyline { points: ids, closed: false });
    t.add(name, Operation::Twist { sketch: Profile::Inline(section), path: line, degrees: 0.0, end_scale: 1.0 }, part(attach, seat(0.0), 0.0))
}

/// `path` walked again at about `pitch`, every corner (a turn past 40°) kept and given a straight run either side
/// long enough for a section of radius `r` to mitre round it.
fn eased(path: &[P2], r: f64, pitch: f64) -> Vec<P2> {
    let turn = |a: P2, b: P2, c: P2| {
        let (u, v) = (unit([b[0] - a[0], b[1] - a[1]]), unit([c[0] - b[0], c[1] - b[1]]));
        (u[0] * v[0] + u[1] * v[1]).clamp(-1.0, 1.0).acos()
    };
    let mut corners = vec![0];
    for i in 1..path.len() - 1 {
        if turn(path[i - 1], path[i], path[i + 1]) > 40f64.to_radians() {
            corners.push(i);
        }
    }
    corners.push(path.len() - 1);
    let need = |i: usize| -> f64 {
        if i == 0 || i == path.len() - 1 {
            return 0.0;
        }
        r * (0.5 * turn(path[i - 1], path[i], path[i + 1])).tan() * 1.3 + 0.05
    };
    let mut out: Vec<P2> = vec![path[0]];
    for w in corners.windows(2) {
        let run = &path[w[0]..=w[1]];
        let mut acc = vec![0.0];
        for s in run.windows(2) {
            acc.push(acc.last().unwrap() + (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]));
        }
        let total = *acc.last().unwrap();
        let (from, to) = (need(w[0]), total - need(w[1]));
        let at = |d: f64| -> P2 {
            let k = acc.partition_point(|a| *a < d).clamp(1, run.len() - 1);
            let t = ((d - acc[k - 1]) / (acc[k] - acc[k - 1]).max(1e-12)).clamp(0.0, 1.0);
            [run[k - 1][0] + (run[k][0] - run[k - 1][0]) * t, run[k - 1][1] + (run[k][1] - run[k - 1][1]) * t]
        };
        if to > from {
            let n = ((to - from) / pitch).ceil().max(1.0) as usize;
            for i in 0..=n {
                let d = from + (to - from) * i as f64 / n as f64;
                if d > 1e-9 && d < total - 1e-9 {
                    out.push(at(d));
                }
            }
        }
        out.push(run[run.len() - 1]);
    }
    out.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6);
    out
}

/// A tapering half-round petal along `path`, `r` at its foot and `r · end` at its tip: a Join part.
fn taper(t: &mut Tree, name: &str, path: &[P2], r: f64, end: f64) -> Result<Id> {
    let id = tube(t, name, path, -BED, r, Attach::Join)?;
    if let Some(f) = t.doc.features.iter_mut().find(|f| f.id == id) {
        if let Operation::Twist { end_scale, .. } = &mut f.operation {
            *end_scale = end;
        }
    }
    Ok(id)
}

/// The table's height over the finger's axis, read off the bare stock.
fn table_height(d: &RingDesign) -> Result<f64> {
    let mut bare = d.clone();
    bare.cad = None;
    let b = mesh::try_build(&bare, &AlphaLibrary::builtin(), coarse_params())?;
    Ok(b.mesh.vertices.iter().map(|v| v.1 as f64).fold(f64::MIN, f64::max))
}

/// Along the hood's right side: the unit tangent and the outward normal (away from the arch) at each point.
fn hood_frame(c: &[P2]) -> (Vec<P2>, Vec<P2>) {
    let n = c.len();
    let tg: Vec<P2> = (0..n).map(|i| unit([c[(i + 1).min(n - 1)][0] - c[i.saturating_sub(1)][0], c[(i + 1).min(n - 1)][1] - c[i.saturating_sub(1)][1]])).collect();
    // Up the right side, the arch is to the left of travel: outward is the right-hand normal.
    let nm: Vec<P2> = tg.iter().map(|t| [-t[1], t[0]]).map(|m| if m[1] < 0.0 && m[0] < 0.5 { [-m[0], -m[1]] } else { m }).collect();
    (tg, nm)
}

/// The cut `pocket` with `keep` taken out of it, so the metal `keep` covers is left standing: a Cut part of its own.
fn spare(t: &mut Tree, name: &str, pocket: Id, keep: Id) -> Result<Id> {
    t.add(name, Operation::Boolean { a: pocket, b: keep, kind: ringdesign_core::cad::Boolean::Subtract }, part(Attach::Cut, Placement::Free, 0.0))
}

/// The centreline `n` in from the outer arch: up the right jamb from `from_u`, over the apex, down the left jamb.
fn arch_path(n: f64, from_u: f64) -> Vec<P2> {
    let r = ARCH_R - n;
    let phi_top = apex_phi(r);
    let steps = ((r * phi_top) / 0.1).ceil() as usize;
    let mut right: Vec<P2> = vec![[from_u, ARCH_W / 2.0 - n]];
    for i in 0..=steps {
        right.push(arc_pt(r, phi_top * i as f64 / steps as f64));
    }
    let mut path = right.clone();
    path.extend(right.iter().rev().skip(1).map(|p| [p[0], -p[1]]));
    path
}

#[derive(serde::Serialize)]
struct Placed {
    springing_u: f64,
    inner_threshold_u: f64,
    inner_apex_u: f64,
    tympanum_floor_mm: f64,
    doors_floor_mm: f64,
    lintel_u: [f64; 2],
    ruby_u: f64,
    hood_clear_of_arch_mm: f64,
    fleur_u: [f64; 2],
}

fn author(blockout: bool) -> Result<(RingDesign, Placed)> {
    let mut d = stock()?;
    let mut t = Tree { doc: Document::default() };
    t.add("The drop's band", Operation::Band, Component { role: ComponentRole::Shank, material: ALLOY.into(), ..Component::default() })?;
    let inn = inner();
    let inset = ORDERS as f64 * STEP_W;
    let ut = sill_u(inset);
    // A half-round roll in the foot of each order's riser, from sill to sill over the apex, and a hollow run beside it.
    // The roll is no part of its own: its order's cut is taken round it, so it stands out of the stock's own metal.
    let roll_path = |m: usize| -> (Vec<P2>, f64) {
        let n = m as f64 * STEP_W + ROLL_R * ROLL_IN;
        (arch_path(n, sill_u(m as f64 * STEP_W) - 0.05), -((m + 1) as f64) * STEP_DEPTH - BED)
    };
    for k in 0..ORDERS {
        let depth = (k + 1) as f64 * STEP_DEPTH;
        let what = format!("archivolt {}", k + 1);
        let pocket = cut(&mut t, &format!("Draw {what}'s order, {depth:.2} mm deep"), &[arch_loop(k as f64 * STEP_W)], 0.03 * (k + 1) as f64, depth)?;
        let (path, z) = roll_path(k);
        let roll = tube(&mut t, &format!("Run the roll up the foot of {what}"), &path, z, ROLL_R, Attach::Cut)?;
        spare(&mut t, &format!("Sink {what}, sparing its roll"), pocket, roll)?;
    }
    // The tympanum and the door opening, sparing the lintel bar and the trumeau's column.
    let tympanum = cut(&mut t, "Draw the tympanum and the door opening", &[arch_loop(inset)], 0.03 * (ORDERS + 1) as f64, inn.floor)?;
    let lintel_w = half_width(inset, inn.lintel_hi) + 0.4;
    let lintel = tube(&mut t, "Lay the lintel bar across the doorway", &[[inn.lintel_hi, lintel_w], [inn.lintel_hi, -lintel_w]], -inn.floor - BED, LINTEL_R, Attach::Cut)?;
    let column = tube(&mut t, "Stand the trumeau's column between the doors", &[[ut - 0.05, 0.0], [inn.lintel_hi, 0.0]], -inn.floor - BED, TRUMEAU_R, Attach::Cut)?;
    let spared = spare(&mut t, "Sink the tympanum, sparing the lintel bar", tympanum, lintel)?;
    spare(&mut t, "Sink the door opening, sparing the trumeau", spared, column)?;
    // The twin doors under the lintel, either side of the trumeau: lancet-headed leaves pierced through to the finger, so they read dark.
    let (lo, hi) = (ut + SILL_H, inn.lintel_lo);
    let leaf_w = (half_width(inset, hi) - DOOR_JAMB - TRUMEAU_W / 2.0).min(DOOR_LEAF_W);
    for side in [1.0, -1.0] {
        let which = if side > 0.0 { "right" } else { "left" };
        let centre = side * (TRUMEAU_W / 2.0 + leaf_w / 2.0);
        let leaf: Vec<P2> = lancet(leaf_w, lo, hi).into_iter().map(|[x, y]| [y, centre + x]).collect();
        cut(&mut t, &format!("Open the {which} door leaf through to the finger"), &[leaf], 0.03 * (ORDERS + 2) as f64, 7.0)?;
    }
    // The ruby in the tympanum, its collet and its seat bur through to the bore.
    let mut gem = Gem::calibrated(GemCut::Round, RUBY_MM);
    gem.preview_tint = Some(RUBY_TINT);
    let stone = t.push(builders::stone_feature(
        0,
        gem,
        Placement::Ring { theta_deg: CROWN_DEG, across_mm: inn.ruby_u, height_mm: -inn.floor + GIRDLE_OVER_FLOOR, spin_deg: SPIN_DEG, tilt_deg: 0.0, cant_deg: 0.0 },
    ))?;
    // Set flush (gypsy): its girdle just under the tympanum's floor, the seat burred into the floor's own metal and its rim
    // burnished over the girdle at the bench, so no thin collet wall stands in the cut.
    let mut collet = builders::feature_on(0, "Collet the ruby in the tympanum floor", builders::BEZEL, stone, json!({ "wall_mm": COLLET_WALL_MM }));
    collet.component.stage = Stage::Cast;
    t.push(collet)?;
    let mut bur = builders::feature_on(0, "Bur the ruby's flush seat into the tympanum floor", builders::BUR, stone, json!({ "through": false }));
    bur.component.stage = Stage::Cast;
    t.push(bur)?;
    // Its light: a straight pilot under the culet to the finger, so the ruby is set à jour.
    let light: Vec<P2> = (0..48).map(|i| 2.0 * PI * i as f64 / 48.0).map(|a| [inn.ruby_u + PILOT_R * a.cos(), PILOT_R * a.sin()]).collect();
    cut(&mut t, "Open the ruby's light through to the finger, à jour", &[light], 0.03 * (ORDERS + 3) as f64, 7.0)?;
    // The ogee hood-mould: one half-round roll from springer to springer, mitred into the ogee's point.
    let right = hood_centre();
    let mut hood: Vec<P2> = right.iter().map(|p| [p[0], p[1]]).collect();
    hood.extend(right.iter().rev().skip(1).map(|p| [p[0], -p[1]]));
    oval_tube(&mut t, "Roll the ogee hood-mould over the arch", &hood, -BED, HOOD_W / 2.0, HOOD_RISE + BED, Attach::Join)?;
    // Its label stops: a boss over each end, where the hood comes down beside the springing.
    let table = table_height(&d)?;
    for (end, which) in [(hood[0], "right"), (hood[hood.len() - 1], "left")] {
        t.add(
            &format!("Stop the hood's {which} end on a boss"),
            Operation::Sphere { radius_mm: LABEL_STOP_R },
            part(Attach::Join, Placement::Ring { theta_deg: table.atan2(end[1]).to_degrees(), across_mm: end[0], height_mm: LABEL_STOP_LIFT, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 }, 0.0),
        )?;
    }
    // Crockets: tapering leaves up each side of the hood, each springing out of its back and curling over toward the point.
    let (along, normals) = hood_frame(&right);
    for (i, f) in CROCKETS.iter().enumerate() {
        let k = ((right.len() - 1) as f64 * f).round() as usize;
        let (root, n, tg) = (right[k], normals[k], along[k]);
        let root = [root[0] + n[0] * (HOOD_W / 2.0 - 0.15), root[1] + n[1] * (HOOD_W / 2.0 - 0.15)];
        let c = [root[0] + tg[0] * CROCKET_CURL, root[1] + tg[1] * CROCKET_CURL];
        let curl: Vec<P2> = (0..=36)
            .map(|j| (165.0 * j as f64 / 36.0f64).to_radians())
            .map(|phi| [c[0] + CROCKET_CURL * (-tg[0] * phi.cos() + n[0] * phi.sin()), c[1] + CROCKET_CURL * (-tg[1] * phi.cos() + n[1] * phi.sin())])
            .collect();
        for side in [1.0, -1.0] {
            let which = if side > 0.0 { "right" } else { "left" };
            let path: Vec<P2> = curl.iter().map(|p| [p[0], side * p[1]]).collect();
            taper(&mut t, &format!("Curl crocket {} up the hood's {which}", i + 1), &path, CROCKET_R, 0.3)?;
        }
    }
    // The fleur-de-lis finial on the ogee's point: a tapering centre petal, two curling side petals, and the band.
    let base = OGEE_U + FLEUR_BASE;
    taper(&mut t, "Rise the fleur's centre petal", &[[base, 0.0], [base + 0.8, 0.0], [base + 1.95, 0.0]], FLEUR_PETAL_R, 0.15)?;
    for side in [1.0, -1.0] {
        let which = if side > 0.0 { "right" } else { "left" };
        // Up and out from the band, over, and down the outside: an arc of `FLEUR_CURL` round its centre.
        let c = [base + 0.42, 0.82];
        let curl: Vec<P2> = (0..=40).map(|i| (270.0 + 150.0 * i as f64 / 40.0f64).to_radians()).map(|a| [c[0] + FLEUR_CURL * a.cos(), side * (c[1] + FLEUR_CURL * a.sin())]).collect();
        taper(&mut t, &format!("Curl the fleur's {which} petal"), &curl, FLEUR_SIDE_R, 0.2)?;
    }
    tube(&mut t, "Bind the fleur's petals with its band", &[[base + 0.42, 0.85], [base + 0.42, -0.85]], -BED, FLEUR_BAND_R, Attach::Join)?;
    // The crypt trefoil pierced through the round end.
    let head = d.inner_radius_mm();
    let _ = head;
    cut(&mut t, "Pierce the crypt quatrefoil through the round end", &[foil([CRYPT_U, 0.0], CRYPT_MM, 4)], 0.03, 6.0)?;
    // The palm's inner edges eased with a comfort bevel, which also takes away the stock's own folded facets there.
    for side in [1.0, -1.0] {
        let a = PALM_BEVEL_DEG.0.to_radians();
        let mut sk = Sketch::default();
        sk.name = "Palm bevel".into();
        sk.plane = ringdesign_core::sketch::Workplane { origin: [0.0; 3], x: [a.cos(), a.sin(), 0.0], y: [0.0, 0.0, 1.0], on_face: None };
        let (r0, r1) = (d.inner_radius_mm() - 0.4, d.inner_radius_mm() + 1.6);
        let tri: Vec<Id> = [[r0, side * (r0 - PALM_BEVEL_OFF)], [r1, side * (r1 - PALM_BEVEL_OFF)], [r0, side * (r1 - PALM_BEVEL_OFF)]].iter().map(|p| sk.point(*p)).collect();
        sk.entity(Geometry::Polyline { points: tri, closed: true });
        let which = if side > 0.0 { "front" } else { "back" };
        t.add(
            &format!("Bevel the palm's {which} inner edge for comfort"),
            Operation::Revolve { sketch: Profile::Inline(sk), pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: PALM_BEVEL_DEG.1, in_plane: false },
            part(Attach::Cut, Placement::Free, 0.0),
        )?;
    }
    // Four lancets pierced through each shoulder, diminishing away from the head, each pointing to it.
    for (off, len) in LANCETS {
        for side in [1.0, -1.0] {
            let theta = CROWN_DEG + side * off;
            let width = len * 0.42;
            let params = json!({ "shape": "Lancet", "width_mm": width, "length_mm": len, "turn_deg": side * 90.0, "through": true, "depth_mm": 4.0, "chamfer_mm": 0.0 });
            let which = if side > 0.0 { "left" } else { "right" };
            t.add(
                &format!("Pierce the {which} shoulder's lancet at {off:.0} degrees"),
                Operation::Builder { key: builders::PIERCE.into(), on: None, params },
                Component { placement: Placement::ring(theta, 0.0), stage: Stage::Cast, attach: Attach::Cut, material: ALLOY.into(), ..Component::default() },
            )?;
        }
    }
    let _ = blockout;
    let mut s = setup();
    s.component = None;
    d.cad = Some(t.doc);
    d.manufacturing = Some(s);
    // Where everything fell.
    let clear = hood_clearance(&hood_centre());
    let (fl, fh) = (OGEE_U + FLEUR_BASE, OGEE_U + FLEUR_BASE + 1.95);
    let placed = Placed {
        springing_u: springing_u(),
        inner_threshold_u: ut,
        inner_apex_u: arc_pt(ARCH_R - inset, apex_phi(ARCH_R - inset))[0],
        tympanum_floor_mm: inn.floor,
        doors_floor_mm: inn.doors,
        lintel_u: [inn.lintel_lo, inn.lintel_hi],
        ruby_u: inn.ruby_u,
        hood_clear_of_arch_mm: clear,
        fleur_u: [fl, fh],
    };
    Ok((d, placed))
}

// --- Gates ----------------------------------------------------------------------------------------------------------

fn solid_of(m: &mesh::Mesh) -> csg::Solid {
    csg::Solid { v: m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect(), f: m.faces.clone() }
}

fn self_crossings(m: &mesh::Mesh) -> usize {
    csg::self_crossings(&solid_of(m))
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
            (c.name.clone(), n)
        })
        .collect()
}

/// Every feature's status.
fn feature_status(built: &mesh::BuildResult) -> Vec<(String, String)> {
    built.parts.evaluated.iter().flat_map(|e| e.features.iter()).map(|r| (r.name.clone(), format!("{:?}", r.status))).collect()
}

/// The closest any vertex comes to the finger axis, less the bore radius, mm.
fn bore_margin(d: &RingDesign, m: &mesh::Mesh) -> f64 {
    let r = d.inner_radius_mm();
    m.vertices.iter().map(|v| (v.0 as f64).hypot(v.1 as f64) - r).fold(f64::MAX, f64::min)
}

/// Stones the gem preview draws: its triangles welded into connected pieces.
fn preview_count(d: &RingDesign, lib: &AlphaLibrary, built: &mesh::BuildResult) -> usize {
    let v = ringdesign_core::gems::built_vertices(d, lib, built);
    let mut index = std::collections::HashMap::new();
    let mut parent: Vec<usize> = Vec::new();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
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
    for tri in v.chunks_exact(36) {
        let ids: Vec<usize> = (0..3)
            .map(|k| {
                let key = [tri[k * 12], tri[k * 12 + 1], tri[k * 12 + 2]].map(|c| (c * 1e3).round() as i64);
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
    let n = parent.len();
    (0..n).filter(|&i| find(&mut parent, i) == i).count()
}

/// The thickness screen's own samples (every `stride`-th face, a ray in along its normal) that fall under `limit`,
/// each with the feature its first vertex came from: where `cad::measure::thickness` finds thin metal.
fn thin_samples(d: &RingDesign, built: &mesh::BuildResult, limit: f64) -> Vec<(String, [f64; 3], f64)> {
    let m = &built.mesh;
    let stride = m.faces.len().div_ceil(384).max(1);
    let tris: Vec<_> = m.faces.iter().filter_map(|f| m.triangle(f)).collect();
    let name = |f: &[u32; 3]| -> String {
        let o = m.origin.get(f[0] as usize).copied().unwrap_or(0);
        match built.parts.feature_of(o).and_then(|id| d.cad.as_ref()?.feature(id)) {
            Some(feat) => feat.name.clone(),
            None if o >= mesh::SOLID_VERTEX => "stone or stamp solid".into(),
            None => "band".into(),
        }
    };
    let mut out = Vec::new();
    for (i, (a, b, c)) in tris.iter().enumerate().step_by(stride) {
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l < 1e-12 {
            continue;
        }
        let dir = n.map(|v| -v / l);
        let o = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let t = tris
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .filter_map(|(_, (p, q, r))| ray_tri(o, dir, *p, *q, *r))
            .fold(f64::MAX, f64::min);
        if t < limit {
            let e = [o[0] + dir[0] * t, o[1] + dir[1] * t, o[2] + dir[2] * t];
            out.push((format!("{} (exits at u {:.2} w {:.2} h {:.2}, dir {:.2} {:.2} {:.2})", name(&m.faces[i]), e[2], e[0], e[1] - 13.322, dir[2], dir[0], dir[1]), o, t));
        }
    }
    out
}

fn ray_tri(o: [f64; 3], d: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<f64> {
    let sub = |x: [f64; 3], y: [f64; 3]| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
    let cross = |x: [f64; 3], y: [f64; 3]| [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let (e1, e2) = (sub(b, a), sub(c, a));
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = sub(o, a);
    let u = dot(s, h) / det;
    if !(-1e-8..=1.0 + 1e-8).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) / det;
    if v < -1e-8 || u + v > 1.0 + 1e-8 {
        return None;
    }
    let t = dot(e2, q) / det;
    (t > 1e-5).then_some(t)
}

/// Every gate at one build size, and whether all are green.
fn gates(d: &RingDesign, lib: &AlphaLibrary, params: BuildParams) -> Result<(Value, bool, mesh::BuildResult)> {
    let started = std::time::Instant::now();
    let built = mesh::try_build(d, lib, params)?;
    let build_s = started.elapsed().as_secs_f64();
    let v = &built.report.validation;
    let q = built.report.quality;
    let crossings = self_crossings(&built.mesh);
    if crossings > 0 && std::env::var("PORTA_DEBUG").is_ok() {
        let mut bare = d.clone();
        bare.cad = None;
        let bb = mesh::try_build(&bare, lib, params)?;
        eprintln!("  bare stock: {} crossings, watertight {}", self_crossings(&bb.mesh), bb.report.validation.watertight);
        let mut bare2 = bare.clone();
        bare2.imported_base.as_mut().unwrap().bare = true;
        let bb2 = mesh::try_build(&bare2, lib, params)?;
        eprintln!("  bare (bare=true): {} crossings", self_crossings(&bb2.mesh));
        for (fl, fw) in [(0.0, 0.0), (0.3, 0.1), (0.3, 0.0), (0.0, 0.1), (0.5, 0.2)] {
            let mut b3 = bare.clone();
            b3.profile.edge_round_mm = fl;
            b3.profile.comfort_fit_mm = fw;
            eprintln!("  was {} {}", bare.profile.edge_round_mm, bare.profile.comfort_fit_mm);
            b3.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: b3.profile.clone(), bore_radius_mm: b3.inner_radius_mm() });
            for p in [draft_params(), export_params(), coarse_params()] {
                let m = mesh::try_build(&b3, lib, p)?;
                eprintln!("  face {fl} x {fw} at {}: {} crossings", p.theta_steps, self_crossings(&m.mesh));
            }
        }
        if std::env::var("PORTA_SECTION").is_ok() {
            // The head's outline in the plane through the crown (w = 0): (u, y) of every vertex within 0.08 of it, past |u| 6.
            let mut pts: Vec<(f32, f32)> = bb.mesh.vertices.iter().filter(|v| v.0.abs() < 0.08 && v.2.abs() > 5.5 && v.1 > 0.0).map(|v| (v.2, v.1)).collect();
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            eprintln!("  crown section: {:?}", pts.iter().map(|(u, y)| format!("({u:.2},{y:.2})")).collect::<Vec<_>>());
            let su: f32 = std::env::var("PORTA_SECTION").unwrap().parse().unwrap_or(5.0);
            let mut pts: Vec<(f32, f32)> = bb.mesh.vertices.iter().filter(|v| (v.2 - su).abs() < 0.08 && v.1 > 5.0 && v.0 < 0.0).map(|v| (v.0, v.1)).collect();
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            eprintln!("  section u=5: {:?}", pts.iter().map(|(u, y)| format!("({u:.2},{y:.2})")).collect::<Vec<_>>());
        }
        for th in [-90.0f64, -50.9, -45.0, -30.0, -129.1] {
            let sec = ringdesign_core::cad::measure::section(&bb.mesh, 0, 0.0);
            let _ = sec;
            let (c, sn) = (th.to_radians().cos(), th.to_radians().sin());
            let mut pts: Vec<(f64, f64)> = bb.mesh.vertices.iter().filter(|v| {
                let t = (v.1 as f64).atan2(v.0 as f64).to_degrees();
                (t - th).abs() < 1.2 && (v.0 as f64 * c + v.1 as f64 * sn) > 0.0
            }).map(|v| ((v.0 as f64).hypot(v.1 as f64), v.2 as f64)).filter(|(_, z)| *z > 1.5).collect();
            pts.sort_by(|a, b| a.1.total_cmp(&b.1));
            eprintln!("  section {th}: {:?}", pts.iter().map(|(r, z)| format!("({r:.2},{z:.2})")).collect::<Vec<_>>());
        }
        // Bisect the bare stock's crossings down to small boxes.
        let mut stack = vec![([-20.0f32, -20.0, -20.0], [20.0f32, 20.0, 20.0])];
        while let Some((lo, hi)) = stack.pop() {
            let mut m = built.mesh.clone();
            m.faces.retain(|f| f.iter().any(|&i| { let v = built.mesh.vertices[i as usize]; let p = [v.0, v.1, v.2]; (0..3).all(|k| p[k] >= lo[k] && p[k] < hi[k]) }));
            let n = self_crossings(&m);
            if n == 0 { continue; }
            let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f32::max);
            if size < 0.15 {
                let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
                let mut names: Vec<String> = m.faces.iter().map(|f| {
                    let o = built.mesh.origin.get(f[0] as usize).copied().unwrap_or(0);
                    built.parts.named_of(o).and_then(|id| d.cad.as_ref()?.feature(id)).map_or("band".to_string(), |f| f.name.clone())
                }).collect();
                names.sort();
                names.dedup();
                eprintln!("  crossing at u {:.2} w {:.2} h {:.2}: {n} ({} faces) {:?}", c[2], c[0], c[1] - 13.322, m.faces.len(), names);
                continue;
            }
            let k = (0..3).max_by(|a, b| (hi[*a] - lo[*a]).total_cmp(&(hi[*b] - lo[*b]))).unwrap();
            let mid = 0.5 * (lo[k] + hi[k]);
            let (mut h1, mut l2) = (hi, lo);
            h1[k] = mid;
            l2[k] = mid;
            stack.push((lo, h1));
            stack.push((l2, hi));
        }
        let boxes: Vec<(f32, f32, f32, f32)> = vec![(-20.0, 20.0, 9.0, 99.0), (-20.0, 20.0, -99.0, 9.0), (-20.0, 0.0, 9.0, 99.0), (0.0, 20.0, 9.0, 99.0), (-20.0, 20.0, 12.0, 99.0), (-20.0, 20.0, 9.0, 12.0)];
        for (zl, zh, yl, yh) in boxes {
            let mut m = built.mesh.clone();
            m.faces.retain(|f| f.iter().all(|&i| { let v = built.mesh.vertices[i as usize]; v.2 >= zl && v.2 < zh && v.1 >= yl && v.1 < yh }));
            eprintln!("  box z {zl}..{zh} y {yl}..{yh}: {} crossings of {} faces", self_crossings(&m), m.faces.len());
        }
        for ub in -10..11 {
            let (lo, hi) = (ub as f32 - 0.5, ub as f32 + 0.5);
            let mut m = built.mesh.clone();
            m.faces.retain(|f| f.iter().all(|&i| {
                let v = built.mesh.vertices[i as usize];
                v.2 >= lo && v.2 < hi && v.1 > 9.0
            }));
            let n = self_crossings(&m);
            if n > 0 {
                let ys: Vec<f32> = m.faces.iter().map(|f| built.mesh.vertices[f[0] as usize].1).collect();
                eprintln!("  crossings near u {ub}: {n} (faces {}, y {:.2}..{:.2})", m.faces.len(), ys.iter().cloned().fold(f32::MAX, f32::min), ys.iter().cloned().fold(f32::MIN, f32::max));
            }
        }
    }
    let parts = made_parts(&built);
    let status = feature_status(&built);
    let margin = bore_margin(d, &built.mesh);
    // Lost wax: the investment section by surface-normal rays on a mesh the sampler takes (a coarse build of the same design).
    let thick_mesh = if built.mesh.faces.len() <= 250_000 { built.mesh.clone() } else { mesh::try_build(d, lib, coarse_params())?.mesh };
    let thick = ringdesign_core::cad::measure::thickness(&thick_mesh, MIN_SECTION_MM);
    if std::env::var("PORTA_DEBUG").is_ok() {
        for f in &built.mesh.faces {
            let Some((a, b, c)) = built.mesh.triangle(f) else { continue };
            let e = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let g = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]];
            if 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() < 1e-10 {
                let o = built.mesh.origin.get(f[0] as usize).copied().unwrap_or(0);
                let who = built.parts.named_of(o).and_then(|id| d.cad.as_ref()?.feature(id)).map_or("band".to_string(), |f| f.name.clone());
                eprintln!("  degenerate at u {:.3} w {:.3} h {:.3}: {who}", a[2], a[0], a[1] - 13.322);
            }
        }
        let thin_built = if built.mesh.faces.len() <= 250_000 { None } else { Some(mesh::try_build(d, lib, coarse_params())?) };
        let mut bare = d.clone();
        bare.cad = None;
        let bb = mesh::try_build(&bare, lib, params)?;
        let bt = ringdesign_core::cad::measure::thickness(&bb.mesh, MIN_SECTION_MM);
        eprintln!("  bare stock thickness: {} of {} below, min {:?} at {:?}", bt.below_limit, bt.rays, bt.sampled_min_mm, bt.point);
        for (who, p, t) in thin_samples(&bare, &bb, MIN_SECTION_MM) {
            eprintln!("  bare thin {t:.3} mm at u {:.2} w {:.2} h {:.2}: {who}", p[2], p[0], p[1] - 13.322);
        }
        for (who, p, t) in thin_samples(d, thin_built.as_ref().unwrap_or(&built), MIN_SECTION_MM) {
            eprintln!("  thin {t:.3} mm at u {:.2} w {:.2} h {:.2}: {who}", p[2], p[0], p[1] - 13.322);
        }
    }
    // Every sample the screen finds thin, with the feature its face came from, and the bare stock's own screen beside it.
    let thin_on = if built.mesh.faces.len() <= 250_000 { None } else { Some(mesh::try_build(d, lib, coarse_params())?) };
    let thin: Vec<String> = thin_samples(d, thin_on.as_ref().unwrap_or(&built), MIN_SECTION_MM)
        .into_iter()
        .map(|(who, p, t)| format!("{t:.3} mm at u {:.2} w {:.2}, {:.2} under the table: {who}", p[2], p[0], 13.322 - p[1]))
        .collect();
    let mut bare = d.clone();
    bare.cad = None;
    let bare_thick = ringdesign_core::cad::measure::thickness(&mesh::try_build(&bare, lib, coarse_params())?.mesh, MIN_SECTION_MM);
    let lands: Vec<String> = dfm::cut_lands(d, &built, MIN_SECTION_MM).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let field = castability::attributed_field_report(d, lib, &d.draft, 256, 128);
    let findings: Vec<String> = dfm::findings_in(d, lib).iter().map(|f| format!("{}: {}", f.label, f.message)).collect();
    let stones = ringdesign_core::stones::report(d, field.parting_z_mm);
    let previewed = preview_count(d, lib, &built);
    let reported = stones.as_ref().map_or(0, |s| s.stone_count as usize);
    let mut warnings: Vec<String> = stones.iter().flat_map(|s| s.seats.iter().flat_map(|seat| seat.warnings.iter().cloned())).collect();
    warnings.dedup();
    let all_ok = status.iter().all(|(_, s)| s == "Ok");
    let thick_ok = thick.below_limit == 0 && thick.rays > 0;
    let pass = v.watertight
        && q.degenerate_faces == 0
        && crossings == 0
        && parts.iter().all(|(_, n)| *n == 0)
        && built.solids.notes.is_empty()
        && built.parts.notes.is_empty()
        && all_ok
        && margin >= -0.01
        && thick_ok
        && lands.is_empty()
        && findings.is_empty()
        && reported == previewed
        && built.mesh.faces.len() <= 2_000_000;
    let g = json!({
        "build": [params.theta_steps, params.profile_steps],
        "build_s": build_s,
        "triangles": built.mesh.faces.len(),
        "watertight": v.watertight,
        "boundary_edges": v.boundary_edges,
        "non_manifold_edges": v.non_manifold_edges,
        "degenerate_faces": q.degenerate_faces,
        "min_angle_deg": q.min_angle_deg,
        "self_crossings": crossings,
        "made_parts": parts,
        "features": status,
        "every_feature_ok": all_ok,
        "solids_notes": built.solids.notes,
        "parts_notes": built.parts.notes,
        "bore_margin_mm": margin,
        "thickness_0_8": {
            "mesh_triangles": thick_mesh.faces.len(),
            "rays": thick.rays,
            "unresolved": thick.unresolved,
            "below_limit": thick.below_limit,
            "sampled_min_mm": thick.sampled_min_mm,
            "at": thick.point,
            "note": thick.note,
            "samples_below": thin,
            "bare_stock_same_screen": { "rays": bare_thick.rays, "below_limit": bare_thick.below_limit, "sampled_min_mm": bare_thick.sampled_min_mm },
            "pass": thick_ok,
        },
        "cut_lands_0_8": lands,
        "field_verdict_informational": field.verdict.label(),
        "field_notes": field.notes,
        "dfm_findings": findings,
        "stones_reported": reported,
        "stones_previewed": previewed,
        "stone_carats": stones.as_ref().map_or(0.0, |s| s.total_carats),
        "stone_warnings": warnings,
        "grams_18k": built.report.metals.iter().find(|m| m.metal == ALLOY).map_or(0.0, |m| m.grams),
        "pass": pass,
    });
    Ok((g, pass, built))
}

// --- Renders --------------------------------------------------------------------------------------------------------

const VIEWS: [(&str, f64, f64); 6] = [
    ("hero", 0.48, 1.0),
    ("face", 0.0, PI * 0.5),
    ("palm", PI, 1.05),
    ("side", 0.0, 0.05),
    ("shoulder", -0.9, 0.62),
    ("reverse", 1.6, 0.8),
];

fn save_rgb(path: &Path, rgb: &[u8], w: usize, h: usize) -> Result<()> {
    image::save_buffer(path, rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
    Ok(())
}

/// The mesh turned half round the crown's axis (world y), so the drop's point, and the portal's apex, stand up in every view.
fn turned(m: &mesh::Mesh) -> mesh::Mesh {
    let t = |v: mesh::Vec3| mesh::Vec3(-v.0, v.1, -v.2);
    let mut out = m.clone();
    out.vertices.iter_mut().for_each(|v| *v = t(*v));
    out.normals.iter_mut().for_each(|v| *v = t(*v));
    out.corner_normals.iter_mut().for_each(|(_, c)| c.iter_mut().for_each(|v| *v = t(*v)));
    out
}

fn renders(out: &Path, d: &RingDesign, lib: &AlphaLibrary, built: mesh::BuildResult, edge: usize) -> Result<()> {
    let fin = render::finished_from(d, lib, built);
    let fin = render::Finished { metal: turned(&fin.metal), stones: fin.stones.iter().map(|(m, t)| (turned(m), *t)).collect() };
    let parts = fin.parts(render::GOLD);
    for (name, yaw, pitch) in VIEWS {
        render::write_png_parts(out.join(format!("{name}.png")), &parts, yaw, pitch, edge)?;
    }
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
    // Close-ups framed on the whole ring, never a cropped mesh: the ruby in its tympanum, and the portal face-on at 2x.
    let top = fin.metal.vertices.iter().map(|v| v.1 as f64).fold(0.0, f64::max);
    let ruby = fin.stones.first().map(|(m, _)| {
        let n = m.vertices.len().max(1) as f64;
        let s = m.vertices.iter().fold([0.0; 3], |a, v| [a[0] + v.0 as f64, a[1] + v.1 as f64, a[2] + v.2 as f64]);
        [s[0] / n, s[1] / n, s[2] / n]
    });
    let centre = ruby.unwrap_or([0.0, top - 1.0, 0.0]);
    render::write_png_framed(out.join("stones.png"), &parts, 0.3, 1.15, render::Framing::new(centre, 4.5), edge)?;
    render::write_png_framed(out.join("portal-2x.png"), &parts, 0.0, PI * 0.5, render::Framing::new([0.0, top, -2.0], 5.5), edge)?;
    // Bare stock against the finished ring, at the hero's angle.
    let mut bare = d.clone();
    bare.cad = None;
    bare.stamps.clear();
    bare.layers.layers.clear();
    let b = turned(&mesh::try_build(&bare, lib, draft_params())?.mesh);
    let left = render::render_parts_ss(&[render::Part::metal(&b, render::GOLD)], 0.48, 1.0, edge, edge, 3);
    let right = render::render_parts_ss(&parts, 0.48, 1.0, edge, edge, 3);
    let mut both = vec![0u8; edge * 2 * edge * 3];
    for y in 0..edge {
        both[y * edge * 6..y * edge * 6 + edge * 3].copy_from_slice(&left[y * edge * 3..(y + 1) * edge * 3]);
        both[y * edge * 6 + edge * 3..(y + 1) * edge * 6].copy_from_slice(&right[y * edge * 3..(y + 1) * edge * 3]);
    }
    save_rgb(&out.join("bare-vs-finished.png"), &both, edge * 2, edge)?;
    Ok(())
}

// --- Main -----------------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let draft = args.iter().any(|a| a == "--draft");
    let verify = args.iter().any(|a| a == "--verify");
    let blockout = args.iter().any(|a| a == "--blockout");
    let out = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../showcase/tenebrae/porta"));
    std::fs::create_dir_all(&out)?;
    println!("Porta");
    if let Ok(su) = std::env::var("PORTA_PROBE_U") {
        // The bare stock's section across the finger's axis at u: every vertex within 0.06 of the plane, on the w < 0 side.
        let su: f32 = su.parse()?;
        let mut bare = stock()?;
        bare.cad = None;
        let bb = mesh::try_build(&bare, &AlphaLibrary::builtin(), draft_params())?;
        let mut pts: Vec<(f32, f32)> = bb.mesh.vertices.iter().filter(|v| (v.2 - su).abs() < 0.06 && v.1 > 4.0 && v.0 < 0.0).map(|v| (v.0, v.1)).collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        println!("{:?}", pts.iter().map(|(w, y)| format!("({w:.2},{y:.2})")).collect::<Vec<_>>());
        return Ok(());
    }
    let (d, placed) = author(blockout)?;
    println!("  placed {}", serde_json::to_string(&placed)?);
    let lib = AlphaLibrary::builtin();
    if std::env::var("PORTA_FRAMES").is_ok() {
        let mut bare = d.clone();
        bare.cad = None;
        let bb = mesh::try_build(&bare, &lib, draft_params())?;
        for f in d.cad.as_ref().unwrap().features.iter().filter(|f| f.name.contains("niche")) {
            let fr = f.component.placement.frame_on(&d, Some(&bb.mesh))?;
            println!("  {}: origin {:?} z {:?}", f.name, fr.origin.map(|v| (v * 100.0).round() / 100.0), fr.z_axis.map(|v| (v * 100.0).round() / 100.0));
        }
    }
    let params = if draft { draft_params() } else { export_params() };
    let (draft_gates, draft_pass, draft_built) = gates(&d, &lib, draft_params())?;
    println!("  draft: {draft_gates}");
    let (export_gates, export_pass, built) = if draft {
        (Value::Null, true, draft_built)
    } else {
        let (g, p, b) = gates(&d, &lib, params)?;
        println!("  export: {g}");
        (g, p, b)
    };
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
    let mut pattern_gates = Value::Null;
    let mut pattern_pass = true;
    if !draft {
        stl::write_stl(out.join("finished-metal.stl"), &built.mesh, &d.name)?;
        let pattern = mesh::try_build_pattern(&d, &lib, params)?;
        let pq = pattern.mesh.quality();
        let pc = self_crossings(&pattern.mesh);
        let pv = pattern.mesh.validate();
        pattern_pass = pv.watertight && pq.degenerate_faces == 0 && pc == 0;
        pattern_gates = json!({ "triangles": pattern.mesh.faces.len(), "watertight": pv.watertight, "degenerate_faces": pq.degenerate_faces, "self_crossings": pc, "pass": pattern_pass });
        stl::write_stl(out.join("casting-pattern.stl"), &pattern.mesh, &d.name)?;
        let fin = render::finished_from(&d, &lib, mesh::try_build(&d, &lib, params)?);
        let mut materials = Vec::new();
        for (k, (m, tint)) in fin.stones.iter().enumerate() {
            let file = if k == 0 { "reference-ruby.stl".to_string() } else { format!("reference-ruby-{k}.stl") };
            stl::write_stl(out.join(&file), m, "Porta reference stone")?;
            materials.push(json!({ "mesh": file, "name": "Ruby", "tint": tint, "ior": 1.77, "dispersion": 0.018, "roughness": 0.05, "transmission": 0.6 }));
        }
        std::fs::write(out.join("stones.json"), serde_json::to_vec_pretty(&json!({ "stones": materials }))?)?;
    }
    let all = draft_pass && export_pass && pattern_pass && (draft || cold == Some(true));
    let report = json!({
        "name": d.name,
        "slug": "porta",
        "stage": if blockout { "block-out" } else { "full" },
        "process": d.draft.process.label(),
        "investment": "Lost wax, Gold 18k, 0.8 mm section, 0.15 mm detail",
        "base": "Factory 009 Drop, unmirrored, resized to 14.5 x 19",
        "size": d.size.display(),
        "bore_mm": BORE_MM,
        "placed": placed,
        "features": d.cad.as_ref().map(|c| c.features.iter().map(|f| f.name.clone()).collect::<Vec<_>>()),
        "stamps": d.stamps.iter().map(|s| json!({ "name": s.name, "theta_deg": s.theta_deg, "v_mm": s.v_mm, "cut": s.cut, "along_pull": s.along_pull, "height_mm": s.height_mm })).collect::<Vec<_>>(),
        "design_bytes": text.len(),
        "design_format": serde_json::from_str::<Value>(&text)?.get("format_version").cloned(),
        "draft": draft_gates,
        "export": export_gates,
        "casting_pattern": pattern_gates,
        "cold_reload_identical": cold,
        "gates_passed": all,
    });
    std::fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    renders(&out, &d, &lib, built, if draft { 1000 } else { 1600 })?;
    println!("  gates {}", if all { "passed" } else { "FAILED" });
    ensure!(all || draft, "Porta failed its gates; see {}", out.join("report.json").display());
    Ok(())
}
