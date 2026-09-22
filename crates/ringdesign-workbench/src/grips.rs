//! Parameter grips: the handles a primitive's size is dragged by, in the part's own frame.
use crate::command::Unit;
use ringdesign_core::cad::{Operation, Profile};

/// The smallest size a grip drags a dimension down to.
pub const MIN_SIZE_MM: f64 = 0.001;
/// How far past a free part's translation its position grips stand.
pub const POSITION_REACH_MM: f64 = 4.0;

/// One draggable parameter of an operation, laid out in the part's own frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Grip {
    /// The parameter it sets, as [`with`] names it.
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub value: f64,
    /// Where the dimension line to the grip starts.
    pub start: [f64; 3],
    /// The grip itself.
    pub at: [f64; 3],
    /// The unit direction a drag moves the grip along.
    pub direction: [f64; 3],
    /// Change in the parameter per millimetre the grip moves along `direction`.
    pub gain: f64,
    pub minimum: f64,
    /// Moves the part rather than sizing it.
    pub position: bool,
}

impl Grip {
    fn size(key: &'static str, label: &'static str, value: f64, start: [f64; 3], at: [f64; 3], direction: [f64; 3], gain: f64) -> Self {
        Self { key, label, unit: Unit::Mm, value, start, at, direction, gain, minimum: MIN_SIZE_MM, position: false }
    }

    /// The value once the grip has moved `along_mm` along its direction from where a drag found it at `from`.
    pub fn dragged(&self, from: f64, along_mm: f64) -> f64 {
        (from + along_mm * self.gain).max(self.minimum)
    }
}

fn unit(axis: usize) -> [f64; 3] {
    std::array::from_fn(|k| if k == axis { 1.0 } else { 0.0 })
}

/// The grips of an operation in its part's own frame; empty for one with no size of its own.
pub fn grips(op: &Operation) -> Vec<Grip> {
    match op {
        Operation::Box { size } => (0..3)
            .map(|axis| {
                let (key, label) = [("x", "Width X"), ("y", "Length Y"), ("z", "Height Z")][axis];
                let half = |sign: f64| std::array::from_fn(|k| if k == axis { sign * size[axis] / 2.0 } else { 0.0 });
                Grip::size(key, label, size[axis], half(-1.0), half(1.0), unit(axis), 2.0)
            })
            .collect(),
        Operation::Cylinder { radius_mm: r, height_mm: h } => vec![
            Grip::size("radius", "Radius", *r, [0.0; 3], [*r, 0.0, 0.0], unit(0), 1.0),
            Grip::size("height", "Height", *h, [0.0, 0.0, -h / 2.0], [0.0, 0.0, h / 2.0], unit(2), 2.0),
        ],
        Operation::Sphere { radius_mm: r } => vec![Grip::size("radius", "Radius", *r, [0.0; 3], [*r, 0.0, 0.0], unit(0), 1.0)],
        Operation::Torus { major_mm: a, minor_mm: b } => vec![
            Grip::size("major", "Major radius", *a, [0.0; 3], [*a, 0.0, 0.0], unit(0), 1.0),
            Grip::size("minor", "Tube radius", *b, [*a, 0.0, 0.0], [a + b, 0.0, 0.0], unit(0), 1.0),
        ],
        Operation::TwistedRing { major_mm: r, radial_mm: t, axial_mm: w, .. } => vec![
            Grip::size("major", "Major radius", *r, [0.0; 3], [*r, 0.0, 0.0], unit(0), 1.0),
            Grip::size("radial", "Radial thickness", *t, [r - t / 2.0, 0.0, 0.0], [r + t / 2.0, 0.0, 0.0], unit(0), 2.0),
            Grip::size("axial", "Band width", *w, [*r, 0.0, -w / 2.0], [*r, 0.0, w / 2.0], unit(2), 2.0),
        ],
        Operation::Transform { translation, .. } => (0..3)
            .map(|axis| {
                let (key, label) = [("tx", "Position X"), ("ty", "Position Y"), ("tz", "Position Z")][axis];
                let at = std::array::from_fn(|k| translation[k] + if k == axis { POSITION_REACH_MM } else { 0.0 });
                Grip { key, label, unit: Unit::Mm, value: translation[axis], start: *translation, at, direction: unit(axis), gain: 1.0, minimum: f64::NEG_INFINITY, position: true }
            })
            .collect(),
        Operation::Extrude { sketch: Profile::Inline(sketch), height_mm, .. } if sketch.plane.on_face.is_none() => {
            // Only a sketch drawn in the feature on its own plane has a plane to read here.
            let Some(normal) = sketch.plane.plane().ok().and_then(|p| p.normal()) else { return Vec::new() };
            let base = sketch.plane.origin;
            let at = std::array::from_fn(|k| base[k] + normal[k] * height_mm);
            vec![Grip::size("height", "Extrusion", *height_mm, base, at, normal, 1.0)]
        }
        _ => Vec::new(),
    }
}

/// The operation with the parameter `key` set to `value`; `None` when it has no such parameter.
pub fn with(op: &Operation, key: &str, value: f64) -> Option<Operation> {
    let mut op = op.clone();
    let slot: &mut f64 = match (&mut op, key) {
        (Operation::Box { size }, "x") => &mut size[0],
        (Operation::Box { size }, "y") => &mut size[1],
        (Operation::Box { size }, "z") => &mut size[2],
        (Operation::Cylinder { radius_mm, .. }, "radius") | (Operation::Sphere { radius_mm }, "radius") => radius_mm,
        (Operation::Cylinder { height_mm, .. }, "height") | (Operation::Extrude { height_mm, .. }, "height") => height_mm,
        (Operation::Torus { major_mm, .. }, "major") | (Operation::TwistedRing { major_mm, .. }, "major") => major_mm,
        (Operation::Torus { minor_mm, .. }, "minor") => minor_mm,
        (Operation::TwistedRing { radial_mm, .. }, "radial") => radial_mm,
        (Operation::TwistedRing { axial_mm, .. }, "axial") => axial_mm,
        (Operation::Transform { translation, .. }, "tx") => &mut translation[0],
        (Operation::Transform { translation, .. }, "ty") => &mut translation[1],
        (Operation::Transform { translation, .. }, "tz") => &mut translation[2],
        _ => return None,
    };
    *slot = value;
    Some(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::sketch::{Sketch, Workplane};

    fn keys(op: &Operation) -> Vec<(&'static str, f64, f64)> {
        grips(op).into_iter().map(|g| (g.key, g.value, g.gain)).collect()
    }

    #[test]
    fn every_primitive_lists_its_grips_at_its_own_extents_with_the_gain_a_symmetric_size_needs() {
        let bx = Operation::Box { size: [2.0, 3.0, 4.0] };
        assert_eq!(keys(&bx), [("x", 2.0, 2.0), ("y", 3.0, 2.0), ("z", 4.0, 2.0)]);
        let g = grips(&bx);
        assert_eq!((g[1].start, g[1].at, g[1].direction), ([0.0, -1.5, 0.0], [0.0, 1.5, 0.0], [0.0, 1.0, 0.0]));
        assert_eq!(g.iter().map(|g| g.label).collect::<Vec<_>>(), ["Width X", "Length Y", "Height Z"]);
        let cyl = Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 };
        assert_eq!(keys(&cyl), [("radius", 1.5, 1.0), ("height", 2.5, 2.0)]);
        let g = grips(&cyl);
        assert_eq!((g[0].at, g[1].start, g[1].at), ([1.5, 0.0, 0.0], [0.0, 0.0, -1.25], [0.0, 0.0, 1.25]));
        assert_eq!(keys(&Operation::Sphere { radius_mm: 0.75 }), [("radius", 0.75, 1.0)]);
        let torus = grips(&Operation::Torus { major_mm: 9.0, minor_mm: 1.0 });
        assert_eq!((torus[0].at, torus[1].start, torus[1].at), ([9.0, 0.0, 0.0], [9.0, 0.0, 0.0], [10.0, 0.0, 0.0]));
        assert_eq!(keys(&Operation::Torus { major_mm: 9.0, minor_mm: 1.0 }), [("major", 9.0, 1.0), ("minor", 1.0, 1.0)]);
        let twisted = Operation::TwistedRing { major_mm: 9.0, radial_mm: 1.5, axial_mm: 3.0, turns: 2.0 };
        assert_eq!(keys(&twisted), [("major", 9.0, 1.0), ("radial", 1.5, 2.0), ("axial", 3.0, 2.0)]);
        assert_eq!(grips(&twisted)[2].at, [9.0, 0.0, 1.5]);
        let moved = Operation::Transform { source: 3, translation: [1.0, 2.0, 3.0], rotation_deg: [0.0; 3] };
        let g = grips(&moved);
        assert!(g.iter().all(|g| g.position && g.minimum == f64::NEG_INFINITY));
        assert_eq!((g[2].key, g[2].start, g[2].at), ("tz", [1.0, 2.0, 3.0], [1.0, 2.0, 7.0]));
        // Every size grip ends a dimension line as long as its value, and moving it the value over the gain doubles it.
        for op in [bx, cyl, Operation::Sphere { radius_mm: 0.75 }, Operation::Torus { major_mm: 9.0, minor_mm: 1.0 }, twisted] {
            for g in grips(&op) {
                let line: f64 = (0..3).map(|k| (g.at[k] - g.start[k]) * g.direction[k]).sum();
                assert!((line - g.value).abs() < 1e-12, "{op:?} {}", g.key);
                assert_eq!(g.dragged(g.value, g.value / g.gain), 2.0 * g.value);
            }
        }
    }

    #[test]
    fn an_extrude_grips_only_a_sketch_on_its_own_plane_and_the_rest_have_none() {
        let mut sketch = Sketch::default();
        sketch.plane = Workplane { origin: [0.0, 0.0, 1.0], x: [1.0, 0.0, 0.0], y: [0.0, 1.0, 0.0], on_face: None };
        let op = Operation::Extrude { sketch: sketch.into(), height_mm: 2.0, draft_deg: 0.0 };
        let g = grips(&op);
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].key, g[0].start, g[0].at, g[0].direction), ("height", [0.0, 0.0, 1.0], [0.0, 0.0, 3.0], [0.0, 0.0, 1.0]));
        let named = Operation::Extrude { sketch: Profile::Feature { feature: 4 }, height_mm: 2.0, draft_deg: 0.0 };
        assert!(grips(&named).is_empty(), "a named sketch's plane lives in another feature");
        for op in [Operation::Band, Operation::Fillet { source: 1, edges: vec![], radius_mm: 0.3 }, Operation::Sketch { sketch: Sketch::default() }] {
            assert!(grips(&op).is_empty(), "{op:?}");
        }
    }

    #[test]
    fn a_drag_stops_at_the_minimum_and_with_sets_exactly_the_named_parameter() {
        let g = &grips(&Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 })[1];
        assert_eq!(g.dragged(2.5, 0.5), 3.5, "the height grows twice what its grip moves");
        assert_eq!(g.dragged(2.5, -5.0), MIN_SIZE_MM);
        let moved = &grips(&Operation::Transform { source: 3, translation: [0.0; 3], rotation_deg: [0.0; 3] })[0];
        assert_eq!(moved.dragged(0.0, -7.5), -7.5, "a position has no floor");
        let cyl = Operation::Cylinder { radius_mm: 1.5, height_mm: 2.5 };
        assert!(matches!(with(&cyl, "radius", 2.0), Some(Operation::Cylinder { radius_mm: 2.0, height_mm: 2.5 })));
        assert!(matches!(with(&cyl, "height", 4.0), Some(Operation::Cylinder { radius_mm: 1.5, height_mm: 4.0 })));
        assert!(with(&cyl, "x", 1.0).is_none());
        assert!(matches!(with(&Operation::Box { size: [1.0; 3] }, "y", 3.0), Some(Operation::Box { size: [1.0, 3.0, 1.0] })));
        assert!(matches!(with(&Operation::Torus { major_mm: 9.0, minor_mm: 1.0 }, "minor", 0.5), Some(Operation::Torus { major_mm: 9.0, minor_mm: 0.5 })));
        let t = with(&Operation::Transform { source: 3, translation: [0.0; 3], rotation_deg: [0.0; 3] }, "ty", -2.0);
        assert!(matches!(t, Some(Operation::Transform { translation: [0.0, -2.0, 0.0], .. })));
        // Every grip a primitive lists is a parameter `with` sets.
        for op in [
            Operation::Box { size: [1.0; 3] },
            Operation::Cylinder { radius_mm: 1.0, height_mm: 1.0 },
            Operation::Sphere { radius_mm: 1.0 },
            Operation::Torus { major_mm: 9.0, minor_mm: 1.0 },
            Operation::TwistedRing { major_mm: 9.0, radial_mm: 1.0, axial_mm: 2.0, turns: 1.0 },
            Operation::Transform { source: 1, translation: [0.0; 3], rotation_deg: [0.0; 3] },
        ] {
            for g in grips(&op) {
                let set = with(&op, g.key, g.value + 1.0).unwrap_or_else(|| panic!("{} on {op:?}", g.key));
                let again = grips(&set).into_iter().find(|h| h.key == g.key).unwrap();
                assert_eq!(again.value, g.value + 1.0, "{} on {op:?}", g.key);
            }
        }
    }
}
