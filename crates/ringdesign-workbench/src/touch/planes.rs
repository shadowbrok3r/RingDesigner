//! Work planes on a view: each plane's rectangle in the world, where its name is written, and which plane a finger takes.
use egui::{Pos2, Rect, Vec2};
use ringdesign_core::{
    BuildResult, RingDesign,
    cad::{Operation, PlaneBase},
    sketch::Id,
};

/// How far past the ring a plane through it reaches, mm.
pub const MARGIN_MM: f64 = 1.5;
/// Half the side of a plane laid square to the band or on a face, mm.
pub const PATCH_MM: f64 = 3.0;
/// Thinner than this on screen a plane is seen edge on, and only its name takes a finger, points.
pub const EDGE_ON_PT: f32 = 18.0;
/// How near a plane's outline a finger must land to take it, points.
pub const REACH_PT: f32 = 14.0;
/// How far round its name a finger still takes a plane, points.
pub const NAME_PAD_PT: f32 = 8.0;

/// A work plane as a view draws it: its feature, its name and its rectangle's corners in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub id: Id,
    pub name: String,
    pub corners: [[f64; 3]; 4],
}

/// Every enabled work plane `built` carries, in the document's order: a plane through the ring spans it, one on the band or a face is a patch round its origin.
pub fn shapes(design: &RingDesign, built: &BuildResult) -> Vec<Shape> {
    let (Some(doc), Some(e)) = (design.cad.as_ref(), built.parts.evaluated.as_ref()) else { return Vec::new() };
    let (lo, hi) = built.mesh.bounds().unwrap_or_default();
    let reach = f64::from(lo.0.abs().max(hi.0.abs()).max(lo.1.abs()).max(hi.1.abs())) + MARGIN_MM;
    e.planes
        .iter()
        .filter_map(|p| {
            let f = doc.feature(p.id).filter(|f| f.enabled)?;
            let Operation::Plane { base, .. } = &f.operation else { return None };
            let (x, y) = match base {
                PlaneBase::Section { .. } => ([-reach, reach], [f64::from(lo.2) - MARGIN_MM - p.origin[2], f64::from(hi.2) + MARGIN_MM - p.origin[2]]),
                PlaneBase::Parting => ([-reach, reach], [-reach, reach]),
                PlaneBase::Tangent { .. } | PlaneBase::Face { .. } => ([-PATCH_MM, PATCH_MM], [-PATCH_MM, PATCH_MM]),
            };
            let at = |u: f64, v: f64| std::array::from_fn(|k| p.origin[k] + p.x[k] * u + p.y[k] * v);
            Some(Shape { id: p.id, name: f.name.clone(), corners: [at(x[0], y[0]), at(x[1], y[0]), at(x[1], y[1]), at(x[0], y[1])] })
        })
        .collect()
}

/// A plane on screen: its corners and the rectangle its name is written in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub id: Id,
    pub corners: [Pos2; 4],
    pub name: Rect,
}

impl Drawn {
    /// `shape` projected by `project`, its name `text` points in size written over its top corner, the left of two level ones.
    pub fn new(shape: &Shape, project: impl Fn([f64; 3]) -> Pos2, text: Vec2) -> Self {
        let corners = shape.corners.map(project);
        let top = corners.iter().copied().fold(corners[0], |a, b| if b.y < a.y - 0.5 || ((b.y - a.y).abs() <= 0.5 && b.x < a.x) { b } else { a });
        let at = top + egui::vec2(4.0, -4.0 - text.y);
        Self { id: shape.id, corners, name: Rect::from_min_size(at, text) }
    }

    /// Whether the rectangle is thinner than [`EDGE_ON_PT`] across on screen: its area over its longer side.
    pub fn edge_on(&self) -> bool {
        let p = &self.corners;
        let (u, v) = (p[1] - p[0], p[3] - p[0]);
        let longest = u.length().max(v.length());
        longest <= f32::EPSILON || (u.x * v.y - u.y * v.x).abs() / longest < EDGE_ON_PT
    }

    /// How far `p` lies from the plane's outline, zero on its name; seen edge on only the name answers.
    pub fn distance(&self, p: Pos2) -> f32 {
        if self.name.expand(NAME_PAD_PT).contains(p) {
            return 0.0;
        }
        if self.edge_on() {
            return f32::INFINITY;
        }
        (0..4).map(|i| segment(p, self.corners[i], self.corners[(i + 1) % 4])).fold(f32::INFINITY, f32::min)
    }
}

fn segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = if ab.length_sq() > 1e-9 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
    p.distance(a + ab * t)
}

/// The plane a finger at `p` takes: the nearest whose name or outline lies within `reach` points.
pub fn at(drawn: &[Drawn], p: Pos2, reach: f32) -> Option<Id> {
    drawn.iter().map(|d| (d.id, d.distance(p))).filter(|(_, d)| *d <= reach).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id)
}

/// The plane whose name lies under a finger at `p`: the one a plane answers by when a part under the finger outranks its outline.
pub fn name_at(drawn: &[Drawn], p: Pos2) -> Option<Id> {
    drawn.iter().find(|d| d.name.expand(NAME_PAD_PT).contains(p)).map(|d| d.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2};
    use ringdesign_core::{
        AlphaLibrary, BuildParams,
        cad::{Attach, Component, Document, Feature, Placement},
        mesh, templates,
    };

    fn plane(id: Id, name: &str, base: PlaneBase) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation: Operation::Plane { base, offset_mm: 0.0 }, component: Component::default() }
    }

    /// The Court band with a post at its top and three work planes: through 0°, the parting plane, and square to the band at 90°.
    fn design() -> RingDesign {
        let mut d = templates::all().iter().find(|t| t.name == "Court band").unwrap().design();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        let post = Component { attach: Attach::Join, placement: Placement::ring(90.0, 0.0), ..Component::default() };
        doc.append(Feature { id: 2, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 2.0 }, component: post }).unwrap();
        doc.append(plane(3, "Section at 0°", PlaneBase::Section { theta_deg: 0.0 })).unwrap();
        doc.append(plane(4, "Parting", PlaneBase::Parting)).unwrap();
        doc.append(plane(5, "Tangent at 90°", PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 })).unwrap();
        let mut off = plane(6, "Hidden", PlaneBase::Section { theta_deg: 45.0 });
        off.enabled = false;
        doc.append(off).unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn a_plane_through_the_ring_spans_it_and_one_on_the_band_is_a_patch() {
        let d = design();
        let built = mesh::build(&d, &AlphaLibrary::builtin(), BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..BuildParams::default() });
        let s = shapes(&d, &built);
        assert_eq!(s.iter().map(|s| (s.id, s.name.as_str())).collect::<Vec<_>>(), [(3, "Section at 0°"), (4, "Parting"), (5, "Tangent at 90°")], "a disabled plane is not drawn");
        let (lo, hi) = built.mesh.bounds().unwrap();
        let reach = f64::from(lo.0.abs().max(hi.0.abs()).max(lo.1.abs()).max(hi.1.abs())) + MARGIN_MM;
        // The section through 0° lies in y = 0, past the ring by the margin every way.
        let section = &s[0].corners;
        assert!(section.iter().all(|c| c[1].abs() < 1e-9), "{section:?}");
        let xs: Vec<f64> = section.iter().map(|c| c[0]).collect();
        assert!((xs.iter().cloned().fold(f64::MIN, f64::max) - reach).abs() < 1e-9 && (xs.iter().cloned().fold(f64::MAX, f64::min) + reach).abs() < 1e-9, "{xs:?} against {reach}");
        let zs: Vec<f64> = section.iter().map(|c| c[2]).collect();
        assert!((zs.iter().cloned().fold(f64::MIN, f64::max) - (f64::from(hi.2) + MARGIN_MM)).abs() < 1e-9);
        // The parting plane lies in z = 0 and spans the ring both ways.
        assert!(s[1].corners.iter().all(|c| c[2].abs() < 1e-9 && (c[0].abs() - reach).abs() < 1e-9 && (c[1].abs() - reach).abs() < 1e-9), "{:?}", s[1].corners);
        // Square to the band at the top: a 6 mm patch standing on the crest, square to y.
        let top = &s[2].corners;
        let side = |a: [f64; 3], b: [f64; 3]| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt();
        assert!((side(top[0], top[1]) - 2.0 * PATCH_MM).abs() < 1e-9 && (side(top[1], top[2]) - 2.0 * PATCH_MM).abs() < 1e-9);
        let y = top[0][1];
        assert!(top.iter().all(|c| (c[1] - y).abs() < 1e-4) && y > 9.0, "{top:?}");
    }

    fn drawn(corners: [Pos2; 4]) -> Drawn {
        let shape = Shape { id: 7, name: "P".into(), corners: [[0.0; 3]; 4] };
        let mut d = Drawn::new(&shape, |_| Pos2::ZERO, vec2(30.0, 14.0));
        d.corners = corners;
        let top = corners.iter().copied().fold(corners[0], |a, b| if b.y < a.y - 0.5 || ((b.y - a.y).abs() <= 0.5 && b.x < a.x) { b } else { a });
        d.name = Rect::from_min_size(top + vec2(4.0, -18.0), vec2(30.0, 14.0));
        d
    }

    #[test]
    fn its_name_stands_over_the_top_corner_the_left_of_two_level_ones() {
        let shape = Shape { id: 3, name: "Section".into(), corners: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]] };
        // Screen y grows down, so the corners at z = 1 stand highest; of those two the left one carries the name.
        let d = Drawn::new(&shape, |c| pos2(100.0 + c[0] as f32 * 80.0, 300.0 - c[2] as f32 * 60.0), vec2(50.0, 14.0));
        assert_eq!(d.corners, [pos2(100.0, 300.0), pos2(180.0, 300.0), pos2(180.0, 240.0), pos2(100.0, 240.0)]);
        assert_eq!(d.name, Rect::from_min_size(pos2(104.0, 222.0), vec2(50.0, 14.0)));
    }

    #[test]
    fn a_finger_takes_a_plane_by_its_outline_or_its_name_and_one_seen_edge_on_by_its_name_alone() {
        let square = drawn([pos2(100.0, 100.0), pos2(300.0, 100.0), pos2(300.0, 300.0), pos2(100.0, 300.0)]);
        assert!(!square.edge_on());
        // 10 pt outside its right side, inside the reach; its middle is the ring's, not the plane's.
        assert_eq!(at(&[square], pos2(310.0, 200.0), REACH_PT), Some(7));
        assert_eq!(at(&[square], pos2(200.0, 200.0), REACH_PT), None);
        assert_eq!(at(&[square], pos2(318.0, 200.0), REACH_PT), None, "18 pt off is past a finger's reach of 14");
        // Its name answers anywhere on it, and a finger's pad round it.
        assert_eq!(square.distance(pos2(120.0, 88.0)), 0.0);
        assert_eq!(square.distance(pos2(120.0, 76.0)), 0.0, "{:?}", square.name);
        // Seen edge on it is a line across the ring: only its name takes a finger.
        let line = drawn([pos2(100.0, 200.0), pos2(300.0, 204.0), pos2(300.0, 210.0), pos2(100.0, 206.0)]);
        assert!(line.edge_on());
        assert_eq!(at(&[line], pos2(200.0, 203.0), REACH_PT), None);
        assert_eq!(at(&[line], line.name.center(), REACH_PT), Some(7));
        // By name alone, as when a part under the finger outranks the outline.
        assert_eq!(name_at(&[square], pos2(310.0, 200.0)), None, "the outline does not answer");
        assert_eq!(name_at(&[square], square.name.center()), Some(7));
        // Two in reach: the nearer outline wins.
        let mut near = square;
        near.id = 8;
        near.corners = square.corners.map(|c| c + vec2(12.0, 0.0));
        near.name = square.name.translate(vec2(0.0, -300.0));
        assert_eq!(at(&[square, near], pos2(313.0, 200.0), REACH_PT), Some(8), "1 pt from the second's side, 13 from the first's");
        assert_eq!(at(&[square, near], pos2(301.0, 200.0), REACH_PT), Some(7));
        assert_eq!(at(&[], pos2(0.0, 0.0), REACH_PT), None);
    }
}
