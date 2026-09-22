//! The tiered snap engine: vertex over midpoint over edge over parting plane and crest, then the grid.
use ringdesign_core::interaction::pick::ViewScale;

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
    Vertex,
    Midpoint,
    Edge,
    PartingPlane,
    Crest,
    Grid,
}

/// Where the pointer settled, in the world and in the ring frame, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapHit {
    pub kind: SnapKind,
    pub world: [f64; 3],
    pub ring: RingPoint,
    pub label: String,
}

/// Which snaps are on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapper {
    pub grid: Option<Grid>,
    pub vertices: bool,
    pub midpoints: bool,
    pub edges: bool,
    /// The mould's parting plane, world z = 0.
    pub parting_plane: bool,
    /// The band's crest line, across = 0.
    pub crest: bool,
}
impl Default for Snapper {
    fn default() -> Self {
        Self { grid: None, vertices: true, midpoints: true, edges: true, parting_plane: true, crest: false }
    }
}

/// An angle in [0, 360), with no negative zero.
pub fn wrap360(deg: f64) -> f64 {
    let d = deg.rem_euclid(360.0);
    if d >= 360.0 { 0.0 } else { d + 0.0 }
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

impl Snapper {
    /// The best snap within `aperture_px`: by tier, then nearest on screen; the grid needs no aperture.
    pub fn snap(
        &self,
        world: [f64; 3],
        ring: RingPoint,
        view: &ViewScale,
        aperture_px: f32,
        geometry: &SnapGeometry,
    ) -> Option<SnapHit> {
        let aperture = f64::from(aperture_px);
        let nearest = |points: &mut dyn Iterator<Item = [f64; 3]>| -> Option<([f64; 3], f64)> {
            points.map(|p| (p, screen_px(p, world, view))).filter(|(_, px)| *px <= aperture).min_by(|a, b| a.1.total_cmp(&b.1))
        };
        // A part point takes its own θ and across; the pointer's stand-off rides along.
        let on_part = |kind, name: &str, (p, px): ([f64; 3], f64)| SnapHit {
            kind,
            world: p,
            ring: RingPoint::of_world(p, ring.height_mm),
            label: format!("{name} ({px:.0} px)"),
        };
        if self.vertices
            && let Some(hit) = nearest(&mut geometry.vertices.iter().copied())
        {
            return Some(on_part(SnapKind::Vertex, "vertex", hit));
        }
        if self.midpoints
            && let Some(hit) = nearest(&mut geometry.edges.iter().filter_map(|e| midpoint(e)))
        {
            return Some(on_part(SnapKind::Midpoint, "midpoint", hit));
        }
        if self.edges
            && let Some(hit) = nearest(&mut geometry.edges.iter().flat_map(|e| e.windows(2).map(|w| on_segment(world, w[0], w[1], view))))
        {
            return Some(on_part(SnapKind::Edge, "edge", hit));
        }
        // The planes are judged by their offset in mm at the view's scale, whatever the view direction.
        let plane = [
            (self.parting_plane, SnapKind::PartingPlane, "parting plane", world[2].abs() * view.px_per_mm),
            (self.crest, SnapKind::Crest, "crest", ring.across_mm.abs() * view.px_per_mm),
        ]
        .into_iter()
        .filter(|(on, _, _, px)| *on && *px <= aperture)
        .min_by(|a, b| a.3.total_cmp(&b.3));
        if let Some((_, kind, label, _)) = plane {
            return Some(SnapHit {
                kind,
                world: [world[0], world[1], 0.0],
                ring: RingPoint { across_mm: 0.0, ..ring },
                label: label.into(),
            });
        }
        let grid = self.grid?;
        let snapped = RingPoint {
            theta_deg: wrap360(round_to(ring.theta_deg, grid.theta_deg)),
            across_mm: round_to(ring.across_mm, grid.across_mm),
            height_mm: round_to(ring.height_mm, grid.height_mm),
        };
        // Turned to the snapped angle, slid to the snapped across and out by the height change.
        let r = world[0].hypot(world[1]) + (snapped.height_mm - ring.height_mm);
        let a = snapped.theta_deg.to_radians();
        Some(SnapHit {
            kind: SnapKind::Grid,
            world: [r * a.cos(), r * a.sin(), snapped.across_mm],
            ring: snapped,
            label: format!("grid {:.1}° {:.2} mm", snapped.theta_deg, snapped.across_mm),
        })
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
        let a = 87f64.to_radians();
        let world = [9.5 * a.cos(), 9.5 * a.sin(), 0.7];
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
        let none = Snapper { vertices: false, midpoints: false, edges: false, parting_plane: false, crest: false, grid: None };
        assert!(none.snap(world, ring(87.0, 0.7), &view(), 8.0, &geometry).is_none());
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
        // Of the two planes the nearer answers: here the crest, read off the ring point.
        let s = Snapper { crest: true, grid: None, ..Snapper::default() };
        let hit = s.snap([0.0, 9.5, 0.6], ring(90.0, 0.3), &view(), 8.0, &geometry).unwrap();
        assert_eq!((hit.kind, hit.label.as_str()), (SnapKind::Crest, "crest"));
        assert_eq!(RingPoint::of_world([0.0, -9.5, 1.25], 0.4), RingPoint { theta_deg: 270.0, across_mm: 1.25, height_mm: 0.4 });
        assert_eq!(wrap360(-1e-17), 0.0);
    }
}
