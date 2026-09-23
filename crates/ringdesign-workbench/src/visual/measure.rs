//! Measure: distances and angles between picks on the ring, and the corner a chained third pick makes.
use ringdesign_core::{
    BuildResult, RingDesign,
    interaction::pick::{Entity, Pick},
};

/// A face this far off its own plane at any vertex is not flat, mm.
const FLAT_MM: f64 = 1e-3;
/// Planes this close to parallel are measured plane to plane, degrees.
const PARALLEL_DEG: f64 = 0.5;

/// One thing a measurement reads.
#[derive(Clone, Debug, PartialEq)]
pub enum Picked {
    /// A vertex, a band point, a stone's centre or a pin.
    Point { at: [f64; 3], label: String },
    /// An edge as its polyline, and where on it the pick fell.
    Edge { line: Vec<[f64; 3]>, at: [f64; 3], label: String },
    /// A face: its area centroid and unit normal, whether it is flat, and where it was picked.
    Face { centroid: [f64; 3], normal: [f64; 3], flat: bool, at: [f64; 3], label: String },
}

impl Picked {
    /// Where the pick fell.
    pub fn at(&self) -> [f64; 3] {
        match self {
            Self::Point { at, .. } | Self::Edge { at, .. } | Self::Face { at, .. } => *at,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Point { label, .. } | Self::Edge { label, .. } | Self::Face { label, .. } => label,
        }
    }
}

/// What one pair of picks measures.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// The dimension line's ends.
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub distance_mm: f64,
    /// Between two faces or two edges, or at the middle pick of a chain, degrees.
    pub angle_deg: Option<f64>,
    /// "Distance", "To the edge", "To the plane", "Plane to plane", "Faces", "Edges", "Corner".
    pub what: &'static str,
}

impl Reading {
    /// The value as the viewport writes it by its dimension line.
    pub fn text(&self) -> String {
        match (self.what, self.angle_deg) {
            ("Corner", Some(a)) => format!("{a:.1}°"),
            (_, Some(a)) => format!("{:.3} mm · {a:.1}°", self.distance_mm),
            _ => format!("{:.3} mm", self.distance_mm),
        }
    }

    /// What the reading is and its value: "Distance 3.142 mm", "Corner 90.0°".
    pub fn line(&self) -> String {
        format!("{} {}", self.what, self.text())
    }
}

/// The picks of the measurement in hand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measure {
    pub picks: Vec<Picked>,
}

impl Measure {
    /// A new pick: chained on it extends a run of two to three, else a pick past a pair starts again.
    pub fn add(&mut self, p: Picked, chain: bool) {
        let limit = if chain { 3 } else { 2 };
        if self.picks.len() >= limit {
            self.picks.clear();
        }
        self.picks.push(p);
    }

    pub fn clear(&mut self) {
        self.picks.clear();
    }

    /// Every pair in order, then the corner a chain of three makes at its middle pick.
    pub fn readings(&self) -> Vec<Reading> {
        let mut out: Vec<Reading> = self.picks.windows(2).map(|w| measure(&w[0], &w[1])).collect();
        if let [a, b, c] = self.picks.as_slice() {
            out.extend(corner(a.at(), b.at(), c.at()));
        }
        out
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn along(a: [f64; 3], d: [f64; 3], t: f64) -> [f64; 3] {
    std::array::from_fn(|k| a[k] + d[k] * t)
}
/// The angle between two directions, 0 to 180 degrees.
fn between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let l = len(a) * len(b);
    if l < 1e-18 { 0.0 } else { (dot(a, b) / l).clamp(-1.0, 1.0).acos().to_degrees() }
}
/// The acute angle between two lines, 0 to 90 degrees.
fn acute(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = between(a, b);
    d.min(180.0 - d)
}

/// The point of segment `a..b` nearest `p`.
fn nearest_on(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    if l2 < 1e-18 {
        return a;
    }
    along(a, ab, (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0))
}

/// The closest points of segments `p0..p1` and `q0..q1`.
fn segments(p0: [f64; 3], p1: [f64; 3], q0: [f64; 3], q1: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (d1, d2, r) = (sub(p1, p0), sub(q1, q0), sub(p0, q0));
    let (a, e, f) = (dot(d1, d1), dot(d2, d2), dot(d2, r));
    if a < 1e-18 && e < 1e-18 {
        return (p0, q0);
    }
    let (s, t) = if a < 1e-18 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = dot(d1, r);
        if e < 1e-18 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let s = if denom > 1e-18 { ((b * f - c * e) / denom).clamp(0.0, 1.0) } else { 0.0 };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    (along(p0, d1, s), along(q0, d2, t))
}

/// The point of a polyline nearest `p`, and the direction of the segment it lies on.
fn on_line(p: [f64; 3], line: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    match line {
        [] => (p, [0.0; 3]),
        [only] => (*only, [0.0; 3]),
        _ => line
            .windows(2)
            .map(|w| (nearest_on(p, w[0], w[1]), sub(w[1], w[0])))
            .min_by(|a, b| len(sub(a.0, p)).total_cmp(&len(sub(b.0, p))))
            .unwrap_or((p, [0.0; 3])),
    }
}

/// The closest points of two polylines and the directions of the segments they lie on.
fn between_lines(a: &[[f64; 3]], b: &[[f64; 3]]) -> ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) {
    let mut best = (f64::INFINITY, [0.0; 3], [0.0; 3], [0.0; 3], [0.0; 3]);
    for u in a.windows(2) {
        for v in b.windows(2) {
            let (p, q) = segments(u[0], u[1], v[0], v[1]);
            let d = len(sub(q, p));
            if d < best.0 {
                best = (d, p, q, sub(u[1], u[0]), sub(v[1], v[0]));
            }
        }
    }
    (best.1, best.2, best.3, best.4)
}

/// Where `p` meets the plane through `c` with unit normal `n`, dropped square to it.
fn foot(p: [f64; 3], c: [f64; 3], n: [f64; 3]) -> [f64; 3] {
    along(p, n, -dot(sub(p, c), n))
}

/// What two picks measure: points directly, an edge at its closest point, a flat face by its plane.
pub fn measure(a: &Picked, b: &Picked) -> Reading {
    use Picked::*;
    let reading = |from: [f64; 3], to: [f64; 3], angle_deg: Option<f64>, what| Reading { from, to, distance_mm: len(sub(to, from)), angle_deg, what };
    match (a, b) {
        (Face { centroid: c1, normal: n1, flat: true, .. }, Face { centroid: c2, normal: n2, flat: true, at, .. }) => {
            let angle = between(*n1, *n2);
            if angle < PARALLEL_DEG || angle > 180.0 - PARALLEL_DEG {
                reading(foot(*c2, *c1, *n1), *c2, Some(angle), "Plane to plane")
            } else {
                reading(a.at(), *at, Some(angle), "Faces")
            }
        }
        (Edge { line: l1, .. }, Edge { line: l2, .. }) => {
            let (p, q, d1, d2) = between_lines(l1, l2);
            reading(p, q, Some(acute(d1, d2)), "Edges")
        }
        (Edge { line, .. }, Face { centroid, normal, flat: true, .. }) | (Face { centroid, normal, flat: true, .. }, Edge { line, .. }) => {
            // The edge's point nearest the plane, and the edge's lean off it.
            let near = line.iter().copied().min_by(|p, q| dot(sub(*p, *centroid), *normal).abs().total_cmp(&dot(sub(*q, *centroid), *normal).abs())).unwrap_or(a.at());
            let dir = line.last().zip(line.first()).map_or([0.0; 3], |(l, f)| sub(*l, *f));
            let lean = (90.0 - acute(dir, *normal)).abs();
            let (from, to) = if matches!(a, Edge { .. }) { (near, foot(near, *centroid, *normal)) } else { (foot(near, *centroid, *normal), near) };
            reading(from, to, Some(lean), "To the plane")
        }
        (Edge { line, .. }, other) | (other, Edge { line, .. }) => {
            let (q, _) = on_line(other.at(), line);
            let (from, to) = if matches!(a, Edge { .. }) { (q, other.at()) } else { (other.at(), q) };
            reading(from, to, None, "To the edge")
        }
        (Face { centroid, normal, flat: true, .. }, other) | (other, Face { centroid, normal, flat: true, .. }) => {
            let f = foot(other.at(), *centroid, *normal);
            let (from, to) = if matches!(a, Face { .. }) { (f, other.at()) } else { (other.at(), f) };
            reading(from, to, None, "To the plane")
        }
        _ => reading(a.at(), b.at(), None, "Distance"),
    }
}

/// The angle the chain `a, b, c` makes at `b`; `None` where two of them meet.
pub fn corner(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Option<Reading> {
    let (u, v) = (sub(a, b), sub(c, b));
    (len(u) > 1e-9 && len(v) > 1e-9).then(|| Reading { from: a, to: c, distance_mm: len(sub(c, a)), angle_deg: Some(between(u, v)), what: "Corner" })
}

/// A face of a part by its triangles: area centroid, unit normal, and whether it lies in its plane.
pub fn plane_of(triangles: &[[[f64; 3]; 3]]) -> Option<([f64; 3], [f64; 3], bool)> {
    let (mut area, mut centroid, mut normal) = (0.0, [0.0; 3], [0.0; 3]);
    for t in triangles {
        let n = cross(sub(t[1], t[0]), sub(t[2], t[0]));
        let a = len(n) * 0.5;
        area += a;
        normal = along(normal, n, 0.5);
        centroid = along(centroid, [(t[0][0] + t[1][0] + t[2][0]) / 3.0, (t[0][1] + t[1][1] + t[2][1]) / 3.0, (t[0][2] + t[1][2] + t[2][2]) / 3.0], a);
    }
    let l = len(normal);
    if !(area > 1e-12) || !(l > 1e-12) {
        return None;
    }
    let (n, c) = (normal.map(|v| v / l), centroid.map(|v| v / area));
    let flat = triangles.iter().flatten().all(|p| dot(sub(*p, c), n).abs() <= FLAT_MM);
    Some((c, n, flat))
}

/// What a pick measures from: a vertex, an edge's line, a face's plane, a stone's centre or a band point; `None` for a part gone.
pub fn picked(pick: &Pick, design: &RingDesign, built: &BuildResult) -> Option<Picked> {
    let label = crate::viewport::label(&pick.entity, design, Some(built));
    let part = |id| built.parts.evaluated.as_ref()?.components.iter().find(|c| c.id == id);
    Some(match &pick.entity {
        Entity::Vertex { feature, vertex } => Picked::Point { at: *part(*feature)?.trace.vertices.get(*vertex as usize)?, label },
        Entity::Edge { feature, edge } => Picked::Edge { line: part(*feature)?.edges.get(*edge as usize)?.clone(), at: pick.world, label },
        Entity::Face { feature, face } => {
            let c = part(*feature)?;
            let at = |i: u32| c.mesh.vertices.get(i as usize).map(|v| [f64::from(v.0), f64::from(v.1), f64::from(v.2)]);
            let triangles: Vec<[[f64; 3]; 3]> = c
                .mesh
                .faces
                .iter()
                .zip(&c.trace.tri_face)
                .filter(|(_, f)| *f == face)
                .filter_map(|(t, _)| Some([at(t[0])?, at(t[1])?, at(t[2])?]))
                .collect();
            let (centroid, normal, flat) = plane_of(&triangles)?;
            Picked::Face { centroid, normal, flat, at: pick.world, label }
        }
        Entity::Part { feature } => {
            let c = part(*feature)?;
            let at = if c.settings.reference { c.frame.origin } else { pick.world };
            Picked::Point { at, label }
        }
        Entity::Stone { path } => {
            let (st, frame) = ringdesign_core::stones::stone_frames(design).into_iter().find(|(s, _)| s.path == *path)?;
            Picked::Point { at: frame.girdle, label: format!("stone {}", st.label) }
        }
        Entity::Band => Picked::Point { at: pick.world, label },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(at: [f64; 3]) -> Picked {
        Picked::Point { at, label: String::new() }
    }
    fn edge(line: Vec<[f64; 3]>) -> Picked {
        let at = line[0];
        Picked::Edge { line, at, label: String::new() }
    }
    fn face(centroid: [f64; 3], normal: [f64; 3]) -> Picked {
        Picked::Face { centroid, normal, flat: true, at: centroid, label: String::new() }
    }
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn points_edges_and_planes_measure_at_their_closest_and_faces_read_their_angle() {
        let r = measure(&point([0.0, 0.0, 0.0]), &point([3.0, 4.0, 0.0]));
        assert_eq!((r.distance_mm, r.what, r.text()), (5.0, "Distance", "5.000 mm".to_string()));
        // A point off the middle of an edge meets it square.
        let r = measure(&point([1.0, 2.0, 0.0]), &edge(vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0]]));
        assert_eq!((r.to, r.distance_mm), ([1.0, 0.0, 0.0], 2.0));
        // Two skew edges meet at their common perpendicular, a quarter turn apart.
        let r = measure(&edge(vec![[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0]]), &edge(vec![[0.5, -2.0, 1.5], [0.5, 2.0, 1.5]]));
        assert_eq!((r.from, r.to, r.distance_mm), ([0.5, 0.0, 0.0], [0.5, 0.0, 1.5], 1.5));
        assert!(close(r.angle_deg.unwrap(), 90.0));
        assert_eq!(r.text(), "1.500 mm · 90.0°");
        // Parallel planes read plane to plane, whatever the centroids' offset along them.
        let r = measure(&face([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &face([3.0, -1.0, 2.25], [0.0, 0.0, -1.0]));
        assert_eq!((r.what, r.from, r.distance_mm), ("Plane to plane", [3.0, -1.0, 0.0], 2.25));
        assert!(close(r.angle_deg.unwrap(), 180.0));
        // Faces at a slant read their angle.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let r = measure(&face([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &face([0.0, 5.0, 0.0], [0.0, s, s]));
        assert_eq!(r.what, "Faces");
        assert!(close(r.angle_deg.unwrap(), 45.0), "{r:?}");
        // A point to a plane drops square onto it.
        let r = measure(&point([1.0, 1.0, 3.5]), &face([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]));
        assert_eq!((r.to, r.distance_mm, r.what), ([1.0, 1.0, 1.0], 2.5, "To the plane"));
        // An edge standing on a plane leans off it by its own angle.
        let r = measure(&edge(vec![[0.0, 0.0, 1.0], [0.0, 1.0, 2.0]]), &face([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
        assert!(close(r.distance_mm, 1.0) && close(r.angle_deg.unwrap(), 45.0), "{r:?}");
    }

    #[test]
    fn a_chained_third_pick_reads_the_corner_and_a_fourth_starts_again() {
        let mut m = Measure::default();
        m.add(point([2.0, 0.0, 0.0]), false);
        m.add(point([0.0, 0.0, 0.0]), false);
        assert_eq!(m.readings().len(), 1);
        m.add(point([0.0, 3.0, 0.0]), true);
        let r = m.readings();
        assert_eq!(r.iter().map(|r| r.line()).collect::<Vec<_>>(), ["Distance 2.000 mm", "Distance 3.000 mm", "Corner 90.0°"]);
        // Without the chain a third pick is the first of a new pair.
        m.add(point([1.0, 1.0, 1.0]), false);
        assert_eq!(m.picks.len(), 1);
        assert!(m.readings().is_empty());
        m.clear();
        assert!(m.picks.is_empty());
        assert!(corner([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn a_face_is_flat_by_its_own_triangles() {
        let square = [[[0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [2.0, 2.0, 1.0]], [[0.0, 0.0, 1.0], [2.0, 2.0, 1.0], [0.0, 2.0, 1.0]]];
        let (c, n, flat) = plane_of(&square).unwrap();
        assert!(flat && n == [0.0, 0.0, 1.0] && c.iter().zip([1.0, 1.0, 1.0]).all(|(a, b)| close(*a, b)), "{c:?} {n:?}");
        let bent = [square[0], [[0.0, 0.0, 1.0], [2.0, 2.0, 1.0], [0.0, 2.0, 1.5]]];
        assert!(!plane_of(&bent).unwrap().2);
        assert!(plane_of(&[]).is_none());
    }
}
