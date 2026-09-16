//! Measurements on evaluated triangles. Surface-normal rays screen local wall
//! thickness; sparse samples and tessellation do not establish an exact minimum.
use crate::{
    Mesh,
    mesh::{cross, norm, sub},
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Thickness {
    pub sampled_min_mm: Option<f64>,
    pub point: Option<[f64; 3]>,
    pub rays: usize,
    pub unresolved: usize,
    pub below_limit: usize,
    pub limit_mm: f64,
    pub note: &'static str,
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(a, b)| a * b).sum()
}

fn hit(
    origin: [f64; 3],
    direction: [f64; 3],
    a: [f64; 3],
    b: [f64; 3],
    c: [f64; 3],
) -> Option<f64> {
    let e1 = sub(b, a);
    let e2 = sub(c, a);
    let h = cross(direction, e2);
    let det = dot(e1, h);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = sub(origin, a);
    let u = dot(s, h) / det;
    if !(-1e-8..=1.0 + 1e-8).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(direction, q) / det;
    if v < -1e-8 || u + v > 1.0 + 1e-8 {
        return None;
    }
    let t = dot(e2, q) / det;
    (t > 1e-5).then_some(t)
}
pub fn thickness(mesh: &Mesh, limit_mm: f64) -> Thickness {
    let mut r = Thickness {
        sampled_min_mm: None,
        point: None,
        rays: 0,
        unresolved: 0,
        below_limit: 0,
        limit_mm,
        note: "Up to 384 surface-normal samples on the display mesh; small unsampled features and oblique walls require section inspection",
    };
    if !mesh.validate().watertight || mesh.faces.len() > 250_000 {
        r.note = "Thickness not assessed: invalid mesh or sampling work limit exceeded";
        return r;
    }
    let stride = mesh.faces.len().div_ceil(384).max(1);
    let triangles: Vec<_> = mesh.faces.iter().filter_map(|f| mesh.triangle(f)).collect();
    for (index, (a, b, c)) in triangles.iter().enumerate().step_by(stride) {
        let normal = cross(sub(*b, *a), sub(*c, *a));
        let length = norm(normal);
        if length < 1e-12 {
            continue;
        }
        let center = std::array::from_fn(|i| (a[i] + b[i] + c[i]) / 3.0);
        let inward = normal.map(|v| -v / length);
        r.rays += 1;
        let nearest = triangles
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != index)
            .filter_map(|(_, (a, b, c))| hit(center, inward, *a, *b, *c))
            .min_by(f64::total_cmp);
        if let Some(value) = nearest {
            if value < limit_mm {
                r.below_limit += 1;
            }
            if r.sampled_min_mm.is_none_or(|old| value < old) {
                r.sampled_min_mm = Some(value);
                r.point = Some(center);
            }
        } else {
            r.unresolved += 1;
        }
    }
    r
}

/// Intersect actual triangles with an axis-aligned plane. Returned segment
/// coordinates use the other two cyclic axes, in millimeters.
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
