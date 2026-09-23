//! The tiered snap engine: pins and stations, part vertices, midpoints and edges, side faces, then named angles and lines round the ring, the grid last.
use super::session::Axis;
use ringdesign_core::{
    RingDesign, ShankKind,
    cad::{Evaluated, Placement},
    interaction::pick::ViewScale,
    profile::{ProfileLoop, TOP_DEG},
    sketch::Id,
};
use std::sync::{LazyLock, OnceLock};

/// Grid pitches in the ring frame; a zero pitch leaves that coordinate alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub theta_deg: f64,
    pub across_mm: f64,
    pub height_mm: f64,
}

/// A point in the ring frame: round the ring, along the finger, out from the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingPoint {
    pub theta_deg: f64,
    pub across_mm: f64,
    pub height_mm: f64,
}
impl RingPoint {
    /// A world point's angle about the finger axis and its offset along it, as `Placement::Ring` reads them.
    pub fn of_world(world: [f64; 3], height_mm: f64) -> Self {
        Self { theta_deg: wrap360(world[1].atan2(world[0]).to_degrees()), across_mm: world[2], height_mm }
    }
}

/// Placed part geometry in world mm: vertices, and edges as polylines.
#[derive(Clone, Copy, Debug, Default)]
pub struct SnapGeometry<'a> {
    pub vertices: &'a [[f64; 3]],
    pub edges: &'a [Vec<[f64; 3]>],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// A pin dropped on the ring.
    Pin,
    /// Where a stone sits: a layer's stone at its station, or a reference stone part at its seat.
    Station,
    Vertex,
    Midpoint,
    Edge,
    /// A side face's edge or centre line.
    SideFace,
    /// A named angle round the ring.
    Angle,
    PartingPlane,
    Crest,
    Grid,
}

/// The tiers in the order they answer: every feature of the ring before the grid.
pub const TIERS: [SnapKind; 10] = [
    SnapKind::Pin,
    SnapKind::Station,
    SnapKind::Vertex,
    SnapKind::Midpoint,
    SnapKind::Edge,
    SnapKind::SideFace,
    SnapKind::Angle,
    SnapKind::PartingPlane,
    SnapKind::Crest,
    SnapKind::Grid,
];

/// Where the pointer settled, in the world and in the ring frame, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapHit {
    pub kind: SnapKind,
    pub world: [f64; 3],
    pub ring: RingPoint,
    pub label: String,
}

/// A point the ring offers: a pin or a stone's station, in the world and on the ring.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub kind: SnapKind,
    pub world: [f64; 3],
    pub ring: RingPoint,
    pub label: String,
}

/// A side face's edge or centre line, as a share of the reference section's surface arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SideLine {
    pub share: f64,
    pub label: &'static str,
}

/// The ring's own snap targets, gathered once a build.
#[derive(Clone, Debug, Default)]
pub struct RingFeatures {
    /// Angles round the ring and what each is called.
    pub angles: Vec<(f64, String)>,
    /// Offsets along the finger a line runs round the ring at, and what each is called.
    pub across: Vec<(f64, String)>,
    /// Pins and stations.
    pub points: Vec<Target>,
    pub side_lines: Vec<SideLine>,
    /// The section the side lines are shares of.
    reference: Option<ProfileLoop>,
    /// Each side line's (r, z) on the section at each whole degree, read the first time it is asked for.
    rows: Vec<OnceLock<Vec<Option<[f64; 2]>>>>,
}

/// The features a snap reads without a ring: the parting plane at world z = 0.
static PLANE: LazyLock<RingFeatures> = LazyLock::new(|| RingFeatures { across: vec![(0.0, "parting plane".into())], ..RingFeatures::default() });

impl RingFeatures {
    /// The top, the sides, the palm, a signet's heads, the side faces' lines and the parting line at `parting_z_mm`.
    pub fn of(design: &RingDesign, parting_z_mm: f64) -> Self {
        let mut f = Self::default();
        if design.shank.kind == ShankKind::Signet {
            for head in std::iter::once(&design.shank.head).chain(&design.shank.extra_heads) {
                f.angle(head.theta_deg, "head");
            }
        }
        for (deg, name) in [(TOP_DEG, "top"), (0.0, "side"), (180.0, "side"), (270.0, "palm")] {
            f.angle(deg, name);
        }
        f.across.push((if parting_z_mm.is_finite() { parting_z_mm } else { 0.0 }, "parting line".into()));
        let ctx = design.field_context();
        if let Some(faces) = ctx.side_faces_std().filter(|_| ctx.band_v_len_mm > 1e-9) {
            for (a, b) in [faces.low, faces.high].into_iter().flatten() {
                for (v, label) in [(a, "side face edge"), ((a + b) * 0.5, "side face centre"), (b, "side face edge")] {
                    f.side_lines.push(SideLine { share: v / ctx.band_v_len_mm, label });
                }
            }
            f.reference = Some(design.reference_loop());
            f.rows = (0..360).map(|_| OnceLock::new()).collect();
        }
        f
    }

    /// Adds `deg` under `name` unless an angle already stands there.
    fn angle(&mut self, deg: f64, name: &str) {
        let deg = wrap360(deg);
        if deg.is_finite() && !self.angles.iter().any(|(a, _)| wrap180(a - deg).abs() < 1e-6) {
            self.angles.push((deg, name.to_owned()));
        }
    }

    /// Every part's angle round the ring but the carried one's, and each reference stone's seat as a station.
    pub fn with_parts(mut self, evaluated: Option<&Evaluated>, carried: Option<Id>) -> Self {
        for c in evaluated.into_iter().flat_map(|e| &e.components).filter(|c| Some(c.id) != carried) {
            let Placement::Ring { theta_deg, across_mm, .. } = c.settings.placement else { continue };
            if c.settings.reference {
                let ring = RingPoint { theta_deg: wrap360(theta_deg), across_mm, height_mm: 0.0 };
                self.points.push(Target { kind: SnapKind::Station, world: c.frame.origin, ring, label: format!("stone {}", c.name) });
            } else {
                self.angle(theta_deg, &c.name);
            }
        }
        self
    }

    /// Every stone the design's layers set, at its station on the bare band.
    pub fn with_stones(mut self, design: &RingDesign) -> Self {
        for (st, frame) in ringdesign_core::stones::stone_frames(design) {
            let off = st.stand_off_mm();
            let world: [f64; 3] = std::array::from_fn(|k| frame.girdle[k] - frame.normal[k] * off);
            let ring = RingPoint { theta_deg: wrap360(st.theta_deg), across_mm: world[2], height_mm: 0.0 };
            self.points.push(Target { kind: SnapKind::Station, world, ring, label: format!("stone {}", st.label) });
        }
        self
    }

    /// Adds points such as pins.
    pub fn with_points(mut self, points: impl IntoIterator<Item = Target>) -> Self {
        self.points.extend(points);
        self
    }

    /// Each side-face line's point at `theta_deg`, blended between the sections at the whole degrees either side.
    pub fn side_points(&self, design: &RingDesign, theta_deg: f64) -> Vec<([f64; 3], &'static str)> {
        if self.side_lines.is_empty() || self.rows.len() != 360 {
            return Vec::new();
        }
        let t = wrap360(theta_deg);
        let i = (t.floor() as usize).min(359);
        let (a, b) = (self.row(design, i), self.row(design, (i + 1) % 360));
        let f = t - i as f64;
        let (s, c) = t.to_radians().sin_cos();
        self.side_lines
            .iter()
            .zip(a.iter().zip(b))
            .filter_map(|(line, (p, q))| {
                let (p, q) = ((*p)?, (*q)?);
                let (r, z) = (p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f);
                Some(([r * c, r * s, z], line.label))
            })
            .collect()
    }

    /// Each side line's (r, z) on the section at whole degree `deg`.
    fn row(&self, design: &RingDesign, deg: usize) -> &[Option<[f64; 2]>] {
        self.rows[deg].get_or_init(|| {
            let Some(reference) = self.reference.as_ref() else { return Vec::new() };
            let section = design.section_at(deg as f64, SIDE_STEPS, None, Some(reference));
            let surface: Vec<_> = section.pts.iter().filter(|p| p.surface).collect();
            self.side_lines
                .iter()
                .map(|line| {
                    let v = line.share * section.surface_len_mm;
                    let w = surface.windows(2).find(|w| w[0].v_mm <= v && v <= w[1].v_mm)?;
                    let t = if w[1].v_mm - w[0].v_mm > 1e-12 { (v - w[0].v_mm) / (w[1].v_mm - w[0].v_mm) } else { 0.0 };
                    Some([w[0].r + (w[1].r - w[0].r) * t, w[0].z + (w[1].z - w[0].z) * t])
                })
                .collect()
        })
    }
}

/// Samples of the section a side face's lines are read on.
const SIDE_STEPS: usize = 128;

/// Which of a ring point's coordinates a snap may set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dofs {
    pub theta: bool,
    pub across: bool,
    pub height: bool,
}
impl Dofs {
    pub const ALL: Self = Self { theta: true, across: true, height: true };
    pub const THETA: Self = Self { theta: true, across: false, height: false };
    pub const ACROSS: Self = Self { theta: false, across: true, height: false };
    pub const HEIGHT: Self = Self { theta: false, across: false, height: true };

    /// The coordinate an axis lock leaves free, or all three with no lock or one off the ring's frame.
    pub fn of(lock: Option<Axis>) -> Self {
        match lock {
            Some(Axis::Theta) => Self::THETA,
            Some(Axis::Across) => Self::ACROSS,
            Some(Axis::Height) => Self::HEIGHT,
            _ => Self::ALL,
        }
    }
}

/// What a snap reads besides the point: view, aperture, part geometry, ring features, design, and a ring point's world place.
pub struct Scene<'a> {
    pub view: ViewScale,
    pub aperture_px: f32,
    pub geometry: SnapGeometry<'a>,
    pub features: &'a RingFeatures,
    pub design: Option<&'a RingDesign>,
    pub world_of: &'a dyn Fn(RingPoint) -> Option<[f64; 3]>,
}

/// Which snaps are on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapper {
    pub grid: Option<Grid>,
    pub vertices: bool,
    pub midpoints: bool,
    pub edges: bool,
    /// The mould's parting plane: the verdict's parting line among the ring's features, world z = 0 without them.
    pub parting_plane: bool,
    /// The band's crest line, across = 0.
    pub crest: bool,
    pub pins: bool,
    pub stations: bool,
    /// The named angles round the ring.
    pub angles: bool,
    pub side_faces: bool,
}
impl Default for Snapper {
    fn default() -> Self {
        Self { grid: None, vertices: true, midpoints: true, edges: true, parting_plane: true, crest: false, pins: true, stations: true, angles: true, side_faces: true }
    }
}
impl Snapper {
    /// Every snap off.
    pub fn off() -> Self {
        Self { grid: None, vertices: false, midpoints: false, edges: false, parting_plane: false, crest: false, pins: false, stations: false, angles: false, side_faces: false }
    }
}

/// An angle in [0, 360), with no negative zero.
pub fn wrap360(deg: f64) -> f64 {
    let d = deg.rem_euclid(360.0);
    if d >= 360.0 { 0.0 } else { d + 0.0 }
}
/// An angle in [-180, 180).
fn wrap180(deg: f64) -> f64 {
    (deg + 180.0).rem_euclid(360.0) - 180.0
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn lerp(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
}
/// Screen distance between two world points through the view's axes.
fn screen_px(a: [f64; 3], b: [f64; 3], view: &ViewScale) -> f64 {
    let d = sub(a, b);
    dot(d, view.right).hypot(dot(d, view.up)) * view.px_per_mm
}
/// The point of segment `a..b` nearest `p` on screen.
fn on_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3], view: &ViewScale) -> [f64; 3] {
    let flat = |v: [f64; 3]| [dot(v, view.right), dot(v, view.up)];
    let (ab, ap) = (flat(sub(b, a)), flat(sub(p, a)));
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    if len2 < 1e-18 {
        return a;
    }
    lerp(a, b, ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0))
}
/// The point halfway along an open polyline by length; `None` for a closed or degenerate one.
fn midpoint(poly: &[[f64; 3]]) -> Option<[f64; 3]> {
    let len = |w: &[[f64; 3]]| dot(sub(w[1], w[0]), sub(w[1], w[0])).sqrt();
    let (first, last) = (*poly.first()?, *poly.last()?);
    let total: f64 = poly.windows(2).map(len).sum();
    if total < 1e-9 || dot(sub(last, first), sub(last, first)).sqrt() < 1e-9 {
        return None;
    }
    let mut left = total / 2.0;
    for w in poly.windows(2) {
        let l = len(w);
        if l >= left && l > 0.0 {
            return Some(lerp(w[0], w[1], left / l));
        }
        left -= l;
    }
    Some(last)
}
fn round_to(v: f64, pitch: f64) -> f64 {
    if pitch > 0.0 { (v / pitch).round() * pitch + 0.0 } else { v }
}
/// No world point for any ring point.
fn nowhere(_: RingPoint) -> Option<[f64; 3]> {
    None
}
/// The world point read as `from`, turned to `to`'s angle, slid to its across and moved out by its change in height.
fn carried(world: [f64; 3], from: RingPoint, to: RingPoint) -> [f64; 3] {
    let r = world[0].hypot(world[1]) + (to.height_mm - from.height_mm);
    let a = to.theta_deg.to_radians();
    [r * a.cos(), r * a.sin(), to.across_mm]
}

impl Snapper {
    /// The best snap within `aperture_px` on part geometry, the parting plane at z = 0 and the crest, then the grid.
    pub fn snap(&self, world: [f64; 3], ring: RingPoint, view: &ViewScale, aperture_px: f32, geometry: &SnapGeometry) -> Option<SnapHit> {
        let scene = Scene { view: *view, aperture_px, geometry: *geometry, features: &PLANE, design: None, world_of: &nowhere };
        self.snap_ring(world, ring, Dofs::ALL, &scene)
    }

    /// The best snap moving only `dofs`: a point tier with all three free, else lines round and along the ring (judged in mm at the view's scale), else the grid.
    pub fn snap_ring(&self, world: [f64; 3], ring: RingPoint, dofs: Dofs, s: &Scene) -> Option<SnapHit> {
        if dofs == Dofs::ALL
            && let Some(hit) = self.point(world, ring, s)
        {
            return Some(hit);
        }
        let aperture = f64::from(s.aperture_px);
        let px = s.view.px_per_mm.max(0.0);
        let radius = world[0].hypot(world[1]).max(1e-9);
        let loose = dofs != Dofs::ALL;
        let nearest = |c: &mut dyn Iterator<Item = (f64, String, SnapKind, f64)>| {
            c.filter(|q| q.3 <= aperture).min_by(|a, b| a.3.total_cmp(&b.3)).map(|(v, name, kind, _)| (v, name, kind))
        };
        let round = |deg: f64| radius * wrap180(deg - ring.theta_deg).abs().to_radians() * px;
        let along = |mm: f64| (mm - ring.across_mm).abs() * px;
        let theta = if dofs.theta {
            let named = s.features.angles.iter().filter(|_| self.angles).map(|(a, n)| (*a, n.clone(), SnapKind::Angle, round(*a)));
            // Sliding round the ring alone, a station or a pin offers its angle too.
            let points = s.features.points.iter().filter(|t| loose && if t.kind == SnapKind::Pin { self.pins } else { self.stations });
            nearest(&mut named.chain(points.map(|t| (t.ring.theta_deg, t.label.clone(), t.kind, round(t.ring.theta_deg)))))
        } else {
            None
        };
        let across = if dofs.across {
            let lines = s.features.across.iter().filter(|_| self.parting_plane).map(|(a, n)| (*a, n.clone(), SnapKind::PartingPlane, along(*a)));
            let crest = self.crest.then(|| (0.0, "crest".to_owned(), SnapKind::Crest, along(0.0)));
            nearest(&mut lines.chain(crest))
        } else {
            None
        };
        let mut to = ring;
        let (mut named, mut gridded) = (Vec::new(), Vec::new());
        let (mut kind, mut rounded) = (None, false);
        if let Some((deg, name, k)) = theta {
            to.theta_deg = wrap360(deg);
            named.push(format!("{name} {:.1}°", to.theta_deg));
            kind = Some(k);
        } else if let Some(g) = self.grid.filter(|g| dofs.theta && g.theta_deg > 0.0) {
            to.theta_deg = wrap360(round_to(ring.theta_deg, g.theta_deg));
            gridded.push(format!("{:.1}°", to.theta_deg));
            rounded = true;
        }
        if let Some((mm, name, k)) = across {
            to.across_mm = mm;
            named.push(name);
            kind = kind.or(Some(k));
        } else if let Some(g) = self.grid.filter(|g| dofs.across && g.across_mm > 0.0) {
            to.across_mm = round_to(ring.across_mm, g.across_mm);
            gridded.push(format!("{:.2} mm", to.across_mm));
            rounded = true;
        }
        // A stand-off keeps its own value unless the height alone is free.
        if let Some(g) = self.grid.filter(|g| dofs == Dofs::HEIGHT && g.height_mm > 0.0) {
            to.height_mm = round_to(ring.height_mm, g.height_mm);
            rounded = true;
            gridded.push(format!("height {:.2} mm", to.height_mm));
        }
        if named.is_empty() && !rounded {
            return None;
        }
        let label = match (named.is_empty(), gridded.is_empty()) {
            (false, false) => format!("{} · grid {}", named.join(" · "), gridded.join(" ")),
            (false, true) => named.join(" · "),
            (true, false) => format!("grid {}", gridded.join(" ")),
            (true, true) => "grid".into(),
        };
        let unmoved = to.theta_deg == ring.theta_deg && to.height_mm == ring.height_mm;
        let world = if to == ring {
            world
        } else if unmoved {
            (s.world_of)(to).unwrap_or([world[0], world[1], world[2] + to.across_mm - ring.across_mm])
        } else {
            (s.world_of)(to).unwrap_or_else(|| carried(world, ring, to))
        };
        Some(SnapHit { kind: kind.unwrap_or(SnapKind::Grid), world, ring: to, label })
    }

    /// The point tiers: a pin, a station, a part's vertex, midpoint or edge, a side face's line.
    fn point(&self, world: [f64; 3], ring: RingPoint, s: &Scene) -> Option<SnapHit> {
        let aperture = f64::from(s.aperture_px);
        let px = s.view.px_per_mm.max(0.0);
        let radius = world[0].hypot(world[1]).max(1e-9);
        let on_ring = |t: &Target| (radius * wrap180(t.ring.theta_deg - ring.theta_deg).abs().to_radians()).hypot(t.ring.across_mm - ring.across_mm) * px;
        for (kind, on) in [(SnapKind::Pin, self.pins), (SnapKind::Station, self.stations)] {
            let best = s.features.points.iter().filter(|t| on && t.kind == kind).map(|t| (t, on_ring(t))).filter(|(_, d)| *d <= aperture).min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((t, _)) = best {
                // The carried point keeps its own stand-off on the target's place.
                let to = RingPoint { theta_deg: t.ring.theta_deg, across_mm: t.ring.across_mm, height_mm: ring.height_mm };
                let at = if (to.height_mm - t.ring.height_mm).abs() < 1e-9 { t.world } else { (s.world_of)(to).unwrap_or(t.world) };
                return Some(SnapHit { kind, world: at, ring: to, label: t.label.clone() });
            }
        }
        let view = &s.view;
        let nearest = |points: &mut dyn Iterator<Item = [f64; 3]>| -> Option<([f64; 3], f64)> {
            points.map(|p| (p, screen_px(p, world, view))).filter(|(_, d)| *d <= aperture).min_by(|a, b| a.1.total_cmp(&b.1))
        };
        // A part point takes its own θ and across; the pointer's stand-off rides along.
        let on_part = |kind, name: &str, (p, d): ([f64; 3], f64)| SnapHit { kind, world: p, ring: RingPoint::of_world(p, ring.height_mm), label: format!("{name} ({d:.0} px)") };
        let g = &s.geometry;
        if self.vertices
            && let Some(hit) = nearest(&mut g.vertices.iter().copied())
        {
            return Some(on_part(SnapKind::Vertex, "vertex", hit));
        }
        if self.midpoints
            && let Some(hit) = nearest(&mut g.edges.iter().filter_map(|e| midpoint(e)))
        {
            return Some(on_part(SnapKind::Midpoint, "midpoint", hit));
        }
        if self.edges
            && let Some(hit) = nearest(&mut g.edges.iter().flat_map(|e| e.windows(2).map(|w| on_segment(world, w[0], w[1], view))))
        {
            return Some(on_part(SnapKind::Edge, "edge", hit));
        }
        if let (true, Some(design)) = (self.side_faces, s.design) {
            let lines = s.features.side_points(design, ring.theta_deg);
            let best = lines.into_iter().map(|(p, label)| (p, label, screen_px(p, world, view))).filter(|q| q.2 <= aperture).min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((p, label, _)) = best {
                return Some(SnapHit { kind: SnapKind::SideFace, world: p, ring: RingPoint::of_world(p, ring.height_mm), label: label.into() });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Looking down −y at the top of the ring: x across the screen, the finger axis up it, 10 px/mm.
    fn view() -> ViewScale {
        ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 0.0, 1.0], px_per_mm: 10.0 }
    }
    fn ring(theta: f64, across: f64) -> RingPoint {
        RingPoint { theta_deg: theta, across_mm: across, height_mm: 0.0 }
    }
    /// A point on a 9.5 mm crest at `theta`, `across` along the finger.
    fn at(theta: f64, across: f64) -> [f64; 3] {
        let (s, c) = theta.to_radians().sin_cos();
        [9.5 * c, 9.5 * s, across]
    }
    const GRID: Grid = Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.5 };

    #[test]
    fn a_vertex_three_px_away_beats_an_edge_one_px_away_and_takes_its_own_ring_point() {
        let vertices = [[0.3, 9.5, 0.0]];
        let edges = vec![vec![[-5.0, 9.5, 0.1], [5.0, 9.5, 0.1]]];
        let geometry = SnapGeometry { vertices: &vertices, edges: &edges };
        let s = Snapper::default();
        let hit = s.snap([0.0, 9.5, 0.0], ring(90.0, 0.0), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.world, hit.label.as_str()), (SnapKind::Vertex, [0.3, 9.5, 0.0], "vertex (3 px)"));
        assert!((hit.ring.theta_deg - 9.5f64.atan2(0.3).to_degrees()).abs() < 1e-12, "{:?}", hit.ring);
        assert!((hit.ring.theta_deg - 88.1913).abs() < 1e-4);
        // Without vertices the midpoint tier answers before the nearer edge point.
        let s = Snapper { vertices: false, ..Snapper::default() };
        let hit = s.snap([0.4, 9.5, 0.0], ring(90.0, 0.0), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.world), (SnapKind::Midpoint, [0.0, 9.5, 0.1]));
        assert_eq!(hit.ring, RingPoint { theta_deg: 90.0, across_mm: 0.1, height_mm: 0.0 });
        let s = Snapper { vertices: false, midpoints: false, ..Snapper::default() };
        let hit = s.snap([2.0, 9.5, 0.0], ring(90.0, 0.0), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.world, hit.label.as_str()), (SnapKind::Edge, [2.0, 9.5, 0.1], "edge (1 px)"));
    }

    #[test]
    fn a_midpoint_is_halfway_along_an_open_edge_and_a_closed_loop_has_none() {
        // An L whose legs are 3 and 1 mm: halfway is 2 mm along the first leg.
        let edges = vec![vec![[0.0, 9.5, 0.0], [3.0, 9.5, 0.0], [3.0, 9.5, 1.0]]];
        let s = Snapper { vertices: false, edges: false, parting_plane: false, ..Snapper::default() };
        let hit = s.snap([2.2, 9.5, 0.0], ring(90.0, 0.0), &view(), 5.0, &SnapGeometry { vertices: &[], edges: &edges }).unwrap();
        assert_eq!((hit.kind, hit.world), (SnapKind::Midpoint, [2.0, 9.5, 0.0]));
        let square = vec![vec![[0.0, 9.5, 0.0], [1.0, 9.5, 0.0], [1.0, 9.5, 1.0], [0.0, 9.5, 1.0], [0.0, 9.5, 0.0]]];
        let geometry = SnapGeometry { vertices: &[], edges: &square };
        assert!(s.snap([0.5, 9.5, 0.5], ring(90.0, 0.5), &view(), 50.0, &geometry).is_none());
    }

    #[test]
    fn outside_the_aperture_only_the_grid_answers_and_rounds_exactly() {
        let vertices = [[5.0, 0.0, 0.0]];
        let edges = vec![vec![[0.0, 0.0, 3.0], [10.0, 0.0, 3.0]]];
        let geometry = SnapGeometry { vertices: &vertices, edges: &edges };
        let world = at(87.0, 0.7);
        let s = Snapper { parting_plane: false, ..Snapper::default() };
        assert!(s.snap(world, ring(87.0, 0.7), &view(), 8.0, &geometry).is_none());
        let s = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.0 }), ..s };
        let hit = s.snap(world, ring(87.0, 0.7), &view(), 8.0, &geometry).unwrap();
        assert_eq!(hit.kind, SnapKind::Grid);
        assert_eq!(hit.ring, RingPoint { theta_deg: 85.0, across_mm: 0.5, height_mm: 0.0 });
        assert!((hit.world[0] - 9.5 * 85f64.to_radians().cos()).abs() < 1e-12);
        assert!((hit.world[1] - 9.5 * 85f64.to_radians().sin()).abs() < 1e-12);
        assert_eq!((hit.world[2], hit.label.as_str()), (0.5, "grid 85.0° 0.50 mm"));
        // Past 357.5° the grid wraps to 0, and a negative half-pitch rounds to a positive zero.
        let hit = s.snap(world, RingPoint { theta_deg: 358.0, across_mm: -0.2, height_mm: 0.0 }, &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.ring.theta_deg, hit.ring.across_mm.to_bits()), (0.0, 0f64.to_bits()));
        assert!(Snapper::off().snap(world, ring(87.0, 0.7), &view(), 8.0, &geometry).is_none());
    }

    #[test]
    fn the_parting_plane_and_the_crest_answer_by_their_offset_in_mm() {
        let geometry = SnapGeometry::default();
        let s = Snapper { grid: Some(Grid { theta_deg: 5.0, across_mm: 0.5, height_mm: 0.0 }), ..Snapper::default() };
        // 0.5 mm off the plane is 5 px at 10 px/mm: inside an 8 px aperture, outside a 4 px one.
        let hit = s.snap([0.0, 9.5, 0.5], ring(90.0, 0.5), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.world, hit.ring.across_mm), (SnapKind::PartingPlane, [0.0, 9.5, 0.0], 0.0));
        let hit = s.snap([0.0, 9.5, 0.5], ring(92.0, 0.5), &view(), 4.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.ring.theta_deg), (SnapKind::Grid, 90.0));
        // On the plane the grid still rounds the angle, and the point turns to it.
        let hit = s.snap(at(87.0, 0.1), ring(87.0, 0.1), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.label.as_str()), (SnapKind::PartingPlane, ring(85.0, 0.0), "parting plane · grid 85.0°"));
        let b = 85f64.to_radians();
        assert!((hit.world[0] - 9.5 * b.cos()).abs() < 1e-12 && (hit.world[1] - 9.5 * b.sin()).abs() < 1e-12 && hit.world[2] == 0.0, "{:?}", hit.world);
        // The crest answers where the parting plane is off.
        let s = Snapper { crest: true, parting_plane: false, grid: None, ..Snapper::default() };
        let hit = s.snap([0.0, 9.5, 0.3], ring(90.0, 0.3), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.label.as_str()), (SnapKind::Crest, "crest"));
        assert_eq!(RingPoint::of_world([0.0, -9.5, 1.25], 0.4), RingPoint { theta_deg: 270.0, across_mm: 1.25, height_mm: 0.4 });
        assert_eq!(wrap360(-1e-17), 0.0);
    }

    /// The ring's features without a design: the fixed angles, a part at 47°, a station, a pin, the parting line.
    fn features() -> RingFeatures {
        let mut f = RingFeatures::default();
        for (deg, name) in [(90.0, "top"), (0.0, "side"), (180.0, "side"), (270.0, "palm"), (47.0, "Cylinder")] {
            f.angle(deg, name);
        }
        f.across.push((0.0, "parting line".into()));
        f.points.push(Target { kind: SnapKind::Station, world: at(135.0, 0.6), ring: ring(135.0, 0.6), label: "stone Round 2.5 mm".into() });
        f.points.push(Target { kind: SnapKind::Pin, world: at(200.0, -0.4), ring: ring(200.0, -0.4), label: "Pin 1".into() });
        f
    }
    /// A ring point on the 9.5 mm crest.
    fn crest(p: RingPoint) -> Option<[f64; 3]> {
        Some(at(p.theta_deg, p.across_mm))
    }
    fn scene(f: &RingFeatures) -> Scene<'_> {
        Scene { view: view(), aperture_px: 8.0, geometry: SnapGeometry::default(), features: f, design: None, world_of: &crest }
    }

    #[test]
    fn the_rings_features_answer_before_the_grid_and_say_where_they_landed() {
        let (f, s) = (features(), Snapper { grid: Some(GRID), crest: true, ..Snapper::default() });
        // Every tier of the ring stands before the grid, which answers last.
        assert_eq!(TIERS.last(), Some(&SnapKind::Grid));
        assert_eq!(TIERS.iter().position(|k| *k == SnapKind::Angle), Some(6));
        // 271° is 0.17 mm round a 9.5 mm crest from the palm: 1.7 px, where the grid would say 270 too — the palm answers.
        let hit = s.snap_ring(at(271.0, 0.2), ring(271.0, 0.2), Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.label.as_str()), (SnapKind::Angle, ring(270.0, 0.0), "palm 270.0° · parting line"));
        assert!((0..3).all(|k| (hit.world[k] - at(270.0, 0.0)[k]).abs() < 1e-12), "the hit lands where it says: {:?}", hit.world);
        // At 272.5° the grid would round to 275; the palm, 4 px off, answers instead.
        let hit = s.snap_ring(at(272.5, 0.9), ring(272.5, 0.9), Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.ring.theta_deg, hit.label.as_str()), (270.0, "palm 270.0° · grid 1.00 mm"));
        // Another part's angle is named by the part; off every line only the grid answers.
        let hit = s.snap_ring(at(47.3, 3.0), ring(47.3, 3.0), Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.label.as_str()), (SnapKind::Angle, "Cylinder 47.0° · grid 3.00 mm"));
        let hit = s.snap_ring(at(62.4, 3.2), ring(62.4, 3.2), Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.label.as_str()), (SnapKind::Grid, ring(60.0, 3.0), "grid 60.0° 3.00 mm"));
        // A station and a pin take both coordinates at once, keeping the point's own stand-off.
        let near = RingPoint { theta_deg: 135.2, across_mm: 0.55, height_mm: 0.3 };
        let hit = s.snap_ring(at(135.2, 0.55), near, Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.label.as_str()), (SnapKind::Station, RingPoint { theta_deg: 135.0, across_mm: 0.6, height_mm: 0.3 }, "stone Round 2.5 mm"));
        let hit = s.snap_ring(at(199.8, -0.35), ring(199.8, -0.35), Dofs::ALL, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.world, hit.label.as_str()), (SnapKind::Pin, ring(200.0, -0.4), at(200.0, -0.4), "Pin 1"));
        // Ctrl frees the pointer: the caller snaps nothing.
        assert!(Snapper::off().snap_ring(at(271.0, 0.2), ring(271.0, 0.2), Dofs::ALL, &scene(&f)).is_none());
    }

    #[test]
    fn one_free_coordinate_snaps_along_it_alone_and_a_station_offers_it_too() {
        let (f, s) = (features(), Snapper { grid: Some(GRID), crest: true, ..Snapper::default() });
        // Round the ring only: the across stays wherever it was.
        let hit = s.snap_ring(at(46.8, 1.3), ring(46.8, 1.3), Dofs::THETA, &scene(&f)).unwrap();
        assert_eq!((hit.ring, hit.label.as_str()), (ring(47.0, 1.3), "Cylinder 47.0°"));
        let hit = s.snap_ring(at(134.7, 1.3), ring(134.7, 1.3), Dofs::THETA, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring.theta_deg, hit.ring.across_mm), (SnapKind::Station, 135.0, 1.3));
        let hit = s.snap_ring(at(61.3, 1.3), ring(61.3, 1.3), Dofs::THETA, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring.theta_deg, hit.label.as_str()), (SnapKind::Grid, 60.0, "grid 60.0°"));
        // Along the finger only: the parting line before the crest on it, then the grid; no grid, no snap off a line.
        let hit = s.snap_ring(at(61.3, 0.4), ring(61.3, 0.4), Dofs::ACROSS, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring, hit.label.as_str()), (SnapKind::PartingPlane, ring(61.3, 0.0), "parting line"));
        let hit = s.snap_ring(at(61.3, 1.3), ring(61.3, 1.3), Dofs::ACROSS, &scene(&f)).unwrap();
        assert_eq!((hit.kind, hit.ring.across_mm, hit.label.as_str()), (SnapKind::Grid, 1.5, "grid 1.50 mm"));
        let arrow = Snapper { grid: None, ..s };
        assert!(arrow.snap_ring(at(61.3, 1.3), ring(61.3, 1.3), Dofs::ACROSS, &scene(&f)).is_none());
        assert!(arrow.snap_ring(at(61.3, 1.3), RingPoint { height_mm: 0.37, ..ring(61.3, 1.3) }, Dofs::HEIGHT, &scene(&f)).is_none());
        let hit = s.snap_ring(at(61.3, 1.3), RingPoint { height_mm: 0.37, ..ring(61.3, 1.3) }, Dofs::HEIGHT, &scene(&f)).unwrap();
        assert_eq!((hit.ring.height_mm, hit.label.as_str()), (0.5, "grid height 0.50 mm"));
        assert_eq!([Dofs::of(Some(Axis::Theta)), Dofs::of(Some(Axis::Across)), Dofs::of(None), Dofs::of(Some(Axis::Spin))], [Dofs::THETA, Dofs::ACROSS, Dofs::ALL, Dofs::ALL]);
    }

    #[test]
    fn a_signets_heads_and_its_side_faces_are_features_of_the_ring() {
        use ringdesign_core::ProfileStyle;
        let mut d = RingDesign::default();
        d.shank.kind = ShankKind::Signet;
        d.shank.head.theta_deg = 100.0;
        let f = RingFeatures::of(&d, 0.25);
        assert_eq!(f.angles.iter().map(|(a, n)| (*a, n.as_str())).collect::<Vec<_>>(), [(100.0, "head"), (90.0, "top"), (0.0, "side"), (180.0, "side"), (270.0, "palm")]);
        assert_eq!(f.across, [(0.25, "parting line".to_string())]);
        // A flat band has two square side faces: an edge, a centre and an edge each, on the side's own plane.
        let mut flat = RingDesign::default();
        flat.profile.width_mm = 7.0;
        flat.profile.thickness_mm = 3.4;
        flat.profile.apply_style(ProfileStyle::Flat);
        flat.profile.flatten_sides();
        let f = RingFeatures::of(&flat, 0.0);
        assert_eq!(f.side_lines.iter().map(|l| l.label).collect::<Vec<_>>(), ["side face edge", "side face centre", "side face edge", "side face edge", "side face centre", "side face edge"]);
        let points = f.side_points(&flat, 90.0);
        assert_eq!(points.len(), 6);
        let half = flat.profile.width_mm * 0.5;
        // The centre lies on the face's own plane; an edge where the face rolls into its fillet.
        for (p, label) in &points {
            let off = (p[2].abs() - half).abs();
            assert!(if *label == "side face centre" { off < 0.01 } else { off < 0.1 }, "{label} stands on a side face: {p:?}");
            assert!(p[0].abs() < 1e-9 && p[1] > flat.inner_radius_mm(), "{p:?}");
        }
        // Looking down the finger the two faces' centre lines coincide on screen, and a pointer near them snaps onto one.
        let centre = points[1].0;
        let down = ViewScale { right: [1.0, 0.0, 0.0], up: [0.0, 1.0, 0.0], px_per_mm: 10.0 };
        let scene = Scene { view: down, aperture_px: 8.0, geometry: SnapGeometry::default(), features: &f, design: Some(&flat), world_of: &nowhere };
        let near = [centre[0] + 0.2, centre[1] + 0.3, centre[2]];
        let hit = Snapper::default().snap_ring(near, RingPoint::of_world(near, 0.0), Dofs::ALL, &scene).unwrap();
        assert_eq!((hit.kind, hit.label.as_str()), (SnapKind::SideFace, "side face centre"));
        let there = f.side_points(&flat, hit.ring.theta_deg);
        assert!([1, 4].iter().any(|&i| (0..3).all(|k| (hit.world[k] - there[i].0[k]).abs() < 1e-9)), "{:?}", hit.world);
    }
}
