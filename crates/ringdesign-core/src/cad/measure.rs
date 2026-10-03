//! Measurements on evaluated triangles: the wall [`census`] and plane [`section`]s.
use crate::Mesh;
use serde::Serialize;
use std::sync::atomic::AtomicBool;

mod census;

/// Every surface sample's section along its inward normal, each reading under the floor classed edge or wall.
#[derive(Clone, Debug, Serialize)]
pub struct Thickness {
    /// The thinnest section read anywhere, edges included, mm.
    pub sampled_min_mm: Option<f64>,
    /// Where it was read.
    pub point: Option<[f64; 3]>,
    /// Surface samples read.
    pub rays: usize,
    /// Samples whose ray found no way out of the metal, or crossed it inconsistently.
    pub unresolved: usize,
    /// Samples under the floor classed as wall: the count a gate reads.
    pub below_limit: usize,
    pub limit_mm: f64,
    pub note: &'static str,
    /// Samples under the floor classed as edge.
    pub edge_below_limit: usize,
    /// Samples on a face inside another shell, which is no surface of the metal and is not read.
    pub internal: usize,
    /// Whether the mesh was read at all.
    pub assessed: bool,
    /// Surface area the samples stand for, mm².
    pub area_mm2: f64,
    /// Spacing of the samples, mm.
    pub pitch_mm: f64,
    /// Furthest a section may stay under the floor from the edge it closes at and still read as an edge, mm.
    pub edge_reach_mm: f64,
    /// Surface area reading under the floor as wall, mm².
    pub wall_area_mm2: f64,
    /// Surface area reading under the floor as edge, mm².
    pub edge_area_mm2: f64,
    /// The [`MAX_ZONES`] largest wall zones, largest first; `below_limit` and `wall_area_mm2` count every wall sample.
    pub walls: Vec<ThinZone>,
    /// The [`MAX_ZONES`] largest edge zones, largest first; `edge_below_limit` and `edge_area_mm2` count every edge sample.
    pub edges: Vec<ThinZone>,
}

/// Zones of each kind a [`Thickness`] lists at most; its counts and areas cover every sample.
pub const MAX_ZONES: usize = 64;

impl Thickness {
    /// Read, every sample resolved, and no wall under the floor: the lost-wax wall gate.
    pub fn clean(&self) -> bool {
        self.assessed && self.unresolved == 0 && self.below_limit == 0
    }
}

/// Why a section is under the floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinKind {
    /// The section closes at a free edge, reaches the floor within the edge reach of it, and is everywhere at least floor / reach of its distance from that edge.
    Edge,
    /// The section stays under the floor beyond the edge reach of any free edge, or is thinner on the way than an edge may be.
    Wall,
}

/// One connected run of samples under the floor, all of one kind.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ThinZone {
    pub kind: ThinKind,
    /// Surface area under the floor, mm²; both faces of a sheet count.
    pub area_mm2: f64,
    /// The zone's thinnest section, mm.
    pub thinnest_mm: f64,
    /// Where it was read, on the surface.
    pub point: [f64; 3],
    pub samples: usize,
    /// For an edge, the furthest from its free edge a sample's section reached the floor, mm.
    pub depth_mm: Option<f64>,
    /// Longest side of the box round the zone's mid-surface points, mm.
    pub span_mm: f64,
}

/// What [`census`] reads and how finely.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CensusOptions {
    /// The section floor, mm.
    pub floor_mm: f64,
    /// Sample spacing, mm; `None` is a floor's eighth held to 0.02-0.1.
    pub pitch_mm: Option<f64>,
    /// Edge reach, mm; `None` is one floor.
    pub edge_reach_mm: Option<f64>,
}

impl CensusOptions {
    pub fn floor(floor_mm: f64) -> Self {
        Self { floor_mm, pitch_mm: None, edge_reach_mm: None }
    }
}

/// [`census`] at `limit_mm` with the default pitch and edge reach; `below_limit` counts wall samples only.
pub fn thickness(mesh: &Mesh, limit_mm: f64) -> Thickness {
    census(mesh, &CensusOptions::floor(limit_mm))
}

/// [`thickness`] that stops soon after `cancel` is raised, with `None`.
pub fn thickness_until(mesh: &Mesh, limit_mm: f64, cancel: &AtomicBool) -> Option<Thickness> {
    census_until(mesh, &CensusOptions::floor(limit_mm), cancel)
}

/// Area-sampled sections of a closed mesh, each reading under the floor classed by a march along its mid-surface.
pub fn census(mesh: &Mesh, options: &CensusOptions) -> Thickness {
    census::run(mesh, options, None).expect("a census with no cancel flag runs to the end")
}

/// [`census`] that stops soon after `cancel` is raised, with `None`.
pub fn census_until(mesh: &Mesh, options: &CensusOptions, cancel: &AtomicBool) -> Option<Thickness> {
    census::run(mesh, options, Some(cancel))
}

/// Segments where the triangles cross an axis-aligned plane, in the other two cyclic axes, mm.
pub fn section(mesh: &Mesh, axis: usize, offset: f64) -> Vec<[[f64; 2]; 2]> {
    if axis > 2 || !offset.is_finite() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for f in &mesh.faces {
        let Some((a, b, c)) = mesh.triangle(f) else {
            continue;
        };
        let p = [a, b, c];
        let mut points = Vec::new();
        for i in 0..3 {
            let a = p[i];
            let b = p[(i + 1) % 3];
            let da = a[axis] - offset;
            let db = b[axis] - offset;
            if (da >= 0.0 && db < 0.0) || (da < 0.0 && db >= 0.0) {
                let t = da / (da - db);
                points.push(std::array::from_fn(|k| {
                    let j = (axis + 1 + k) % 3;
                    a[j] + t * (b[j] - a[j])
                }));
            }
        }
        if points.len() == 2 {
            lines.push([points[0], points[1]]);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hollow_box_detects_wall_and_has_inner_section() {
        use super::super::{Document, Feature, Operation, evaluate};
        let mut d = crate::RingDesign::default();
        let mut doc = Document::default();
        for (id, operation) in [
            (1, Operation::Box { size: [8.0; 3] }),
            (
                2,
                Operation::Shell {
                    source: 1,
                    open_faces: vec![],
                    thickness_mm: 1.0,
                },
            ),
        ] {
            doc.append(Feature {
                id,
                name: operation.label().into(),
                enabled: true,
                operation,
                component: Default::default(),
            })
            .unwrap();
        }
        d.cad = Some(doc);
        let e = evaluate(
            &d,
            &crate::AlphaLibrary::builtin(),
            crate::BuildParams::default(),
        )
        .unwrap();
        let m = &e.components[0].mesh;
        let t = thickness(m, 1.1);
        assert!((t.sampled_min_mm.unwrap() - 1.0).abs() < 1e-5);
        assert!(t.below_limit > 0);
        assert_eq!(t.unresolved, 0);
        let cut = section(m, 2, 0.0);
        assert!(cut.len() >= 8);
        assert!(
            cut.iter()
                .flatten()
                .any(|p| (p[0].abs() - 3.0).abs() < 1e-6)
        );
    }
}
